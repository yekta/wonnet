//! Gmail through its REST API, and its IMAP for work in bulk. The first sync lists the last 90
//! days (or INITIAL_SYNC_LIMIT messages) a page at a time, the inbox before the rest and a small
//! page first so the list has mail at once; then older mail is backfilled a page at a time; after
//! that history.list says what changed since the last historyId. Those pages, and bodies fetched
//! ahead, go over IMAP, which spends none of the API's per-user quota; when IMAP fails the API
//! does them, and IMAP is tried again a while later.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, StreamExt, stream};
use mail_protocol::{Address, Attachment, Draft, Identity, Op, role};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::gmail_imap::{self, Imap};
use super::{Batch, Reauth, Refused, Synced, html_text, one_line, unescape};
use crate::db::{self, AccountRow, Relabel, RemoteMessage};
use crate::mime::File;
use crate::{AppState, mime};

pub const API_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const PAGE: usize = 500;
/// The first page of a new account: what the burst of its quota fetches at once.
const FIRST_PAGE: usize = 20;
const REFRESH_PAGE: usize = 100;
const BACKFILL_PAGE: usize = 100;
const RECENT: &str = "newer_than:90d";
const CONCURRENT_GETS: usize = 8;
/// Gmail lets each user spend 6,000 quota units a minute on our project; requests are paced so
/// that no minute, burst included, spends more.
const UNITS_PER_SECOND: f64 = 90.0;
const BURST_UNITS: f64 = 500.0;
/// What the worker's own requests (syncing, prefetching) leave for those someone waits for.
const RESERVE_UNITS: f64 = 100.0;
const IMAP_PAGE: usize = 500;
/// How much of each message a page over IMAP fetches: enough for its headers and snippet.
const RECENT_BYTES: u32 = 32 * 1024;
/// Less for older mail, as Gmail lets IMAP download 2.5 GB a day.
const OLDER_BYTES: u32 = 16 * 1024;
/// How long the API does IMAP's work after IMAP failed.
const IMAP_RETRY: Duration = Duration::from_secs(10 * 60);
const HEADERS: [&str; 15] = [
    "From",
    "To",
    "Cc",
    "Bcc",
    "Reply-To",
    "Subject",
    "Message-ID",
    "In-Reply-To",
    "References",
    "Content-Type",
    "List-Unsubscribe",
    "List-Unsubscribe-Post",
    "List-Id",
    "Precedence",
    "Auto-Submitted",
];
/// Gmail's tabs for mail sent in bulk.
const BULK_CATEGORIES: [&str; 4] = ["CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL", "CATEGORY_UPDATES", "CATEGORY_FORUMS"];

#[derive(Clone)]
pub struct Gmail {
    http: reqwest::Client,
    api: Arc<String>,
    oauth: Arc<OAuth>,
    token: Arc<Mutex<Option<(String, Instant)>>>,
    pace: Quota,
    background: bool,
    address: Arc<String>,
    imap_url: Option<Arc<String>>,
    imap: Arc<Mutex<ImapSlot>>,
}

/// The account's IMAP connection, opened when first needed and kept, or when it failed last.
#[derive(Default)]
struct ImapSlot {
    imap: Option<Imap>,
    failed_at: Option<Instant>,
}

/// An account's quota, shared by all its connections.
pub type Quota = Arc<Mutex<Pace>>;

/// The quota units that can be spent now, and when that was counted.
pub struct Pace {
    units: f64,
    at: Instant,
}

impl Default for Pace {
    fn default() -> Self {
        Self { units: BURST_UNITS, at: Instant::now() }
    }
}

struct OAuth {
    token_url: String,
    client_id: String,
    client_secret: String,
    refresh_token: String,
}

#[derive(Deserialize)]
pub struct Credentials {
    pub refresh_token: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MessageRef {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ListResponse {
    #[serde(default)]
    messages: Vec<MessageRef>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GmailMessage {
    id: String,
    thread_id: String,
    #[serde(default)]
    label_ids: Vec<String>,
    #[serde(default)]
    snippet: String,
    internal_date: Option<String>,
    payload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    headers: Vec<Header>,
}

#[derive(Deserialize)]
struct Header {
    name: String,
    value: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct HistoryPage {
    #[serde(default)]
    history: Vec<History>,
    history_id: Option<String>,
    next_page_token: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct History {
    #[serde(default)]
    messages_added: Vec<Changed>,
    #[serde(default)]
    messages_deleted: Vec<Changed>,
    #[serde(default)]
    labels_added: Vec<Changed>,
    #[serde(default)]
    labels_removed: Vec<Changed>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Changed {
    message: MessageRef,
    /// The labels added or removed, not all the message has.
    #[serde(default)]
    label_ids: Vec<String>,
}

/// What the history says changed. New mail is fetched whole; a change of labels is written as
/// it is, since fetching every message marked read at once would take hours of the quota.
#[derive(Default)]
struct Changes {
    /// New messages, oldest first.
    added: Vec<String>,
    /// Each message's labels added (true) or removed (false), the last change of each kept.
    marks: HashMap<String, HashMap<String, bool>>,
    deleted: Vec<String>,
    latest: String,
}

impl Changes {
    fn add(&mut self, entry: History) {
        self.added.extend(entry.messages_added.into_iter().map(|item| item.message.id));
        for (items, on) in [(entry.labels_added, true), (entry.labels_removed, false)] {
            for item in items {
                let marks = self.marks.entry(item.message.id).or_default();
                marks.extend(item.label_ids.into_iter().map(|label| (label, on)));
            }
        }
        self.deleted.extend(entry.messages_deleted.into_iter().map(|item| item.message.id));
    }

    /// The label changes of messages that weren't added or deleted meanwhile. One moved between
    /// Gmail's tabs is fetched again instead, for whether it is bulk.
    fn relabels(&mut self, labels: &HashMap<String, Uuid>) -> Vec<Relabel> {
        let settled: HashSet<String> = self.deleted.iter().chain(&self.added).cloned().collect();
        let mut relabels = Vec::new();
        for (id, marks) in std::mem::take(&mut self.marks) {
            if settled.contains(&id) {
                continue;
            }
            if marks.keys().any(|label| BULK_CATEGORIES.contains(&label.as_str())) {
                self.added.push(id);
                continue;
            }
            let mut relabel = Relabel { provider_id: id, ..Default::default() };
            for (label, on) in marks {
                match (mark(&label, labels), on) {
                    (Mark::Label(label), true) => relabel.add.push(label),
                    (Mark::Label(label), false) => relabel.remove.push(label),
                    (Mark::Unread, _) => relabel.unread = Some(on),
                    (Mark::Starred, _) => relabel.starred = Some(on),
                    (Mark::Bulk | Mark::Other, _) => {}
                }
            }
            relabels.push(relabel);
        }
        relabels
    }

    /// The new messages still there, newest first, each once.
    fn newest_first(&self) -> Vec<String> {
        let mut seen: HashSet<&str> = self.deleted.iter().map(String::as_str).collect();
        self.added.iter().rev().filter(|id| seen.insert(id.as_str())).cloned().collect()
    }
}

/// What one of Gmail's label ids is to the server.
enum Mark {
    /// A role or a custom label's id.
    Label(String),
    Unread,
    Starred,
    Bulk,
    Other,
}

fn mark(label: &str, labels: &HashMap<String, Uuid>) -> Mark {
    match label {
        "INBOX" => Mark::Label(role::INBOX.to_string()),
        "SENT" => Mark::Label(role::SENT.to_string()),
        "DRAFT" => Mark::Label(role::DRAFTS.to_string()),
        "TRASH" => Mark::Label(role::TRASH.to_string()),
        "SPAM" => Mark::Label(role::SPAM.to_string()),
        "UNREAD" => Mark::Unread,
        "STARRED" => Mark::Starred,
        _ if BULK_CATEGORIES.contains(&label) => Mark::Bulk,
        other => labels.get(other).map_or(Mark::Other, |id| Mark::Label(id.to_string())),
    }
}

#[derive(Deserialize, Default, serde::Serialize)]
struct SyncState {
    history_id: Option<String>,
    backfill_token: Option<String>,
    #[serde(default)]
    backfilled: bool,
    /// The last 90 days were listed whole: at first, and again for what was added after they
    /// were first synced (`bulk`, `unsubscribe`), keeping the history id.
    #[serde(default)]
    refreshed: bool,
    #[serde(default)]
    refresh: Refresh,
    /// Where IMAP's backfill is: the oldest UID it fetched.
    backfill_below: Option<u32>,
    /// The UID validity the UIDs saved here belong to.
    uid_validity: Option<u32>,
}

/// Where the listing of the last 90 days stands: the inbox first, then the rest, newest first.
#[derive(Deserialize, Default, serde::Serialize)]
struct Refresh {
    #[serde(default)]
    inbox_listed: bool,
    page_token: Option<String>,
    #[serde(default)]
    fetched: usize,
    /// Where IMAP's listing is: the oldest UID it fetched.
    below_uid: Option<u32>,
}

/// A page of a search over IMAP, newest first.
struct ImapPage {
    messages: Vec<RemoteMessage>,
    /// The oldest UID fetched, where the next page starts below.
    lowest: Option<u32>,
    more: bool,
    uid_validity: u32,
}

impl Gmail {
    pub async fn open(state: &AppState, account: &AccountRow, secret: &str) -> anyhow::Result<Self> {
        let credentials: Credentials = serde_json::from_str(secret)?;
        let config = &state.config;
        let gmail = Self {
            http: state.http.clone(),
            api: Arc::new(config.gmail_api_url.clone()),
            oauth: Arc::new(OAuth {
                token_url: config.google_token_url.clone(),
                client_id: config.google_client_id.clone(),
                client_secret: config.google_client_secret.clone(),
                refresh_token: credentials.refresh_token,
            }),
            token: Arc::new(Mutex::new(None)),
            pace: state.workers.quota(account.id),
            background: false,
            address: Arc::new(account.address.clone()),
            imap_url: config.gmail_imap_url.clone().map(Arc::new),
            imap: Default::default(),
        };
        gmail.access_token().await?;
        Ok(gmail)
    }

    /// The same connection for what the worker does unasked, which leaves RESERVE_UNITS of the
    /// quota to what someone is waiting for.
    pub fn background(&self) -> Self {
        Self { background: true, ..self.clone() }
    }

    async fn access_token(&self) -> anyhow::Result<String> {
        let mut token = self.token.lock().await;
        if let Some((access, expires)) = token.as_ref()
            && *expires > Instant::now()
        {
            return Ok(access.clone());
        }
        let response = self
            .http
            .post(&self.oauth.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", self.oauth.refresh_token.as_str()),
                ("client_id", self.oauth.client_id.as_str()),
                ("client_secret", self.oauth.client_secret.as_str()),
            ])
            .send()
            .await?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        if status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED {
            return Err(Reauth(body["error"].as_str().unwrap_or("refused").to_string()).into());
        }
        let Some(access) = body["access_token"].as_str() else {
            anyhow::bail!("Google answered the token refresh with {status}");
        };
        let lifetime = body["expires_in"].as_u64().unwrap_or(3600).saturating_sub(120);
        *token = Some((access.to_string(), Instant::now() + Duration::from_secs(lifetime)));
        Ok(access.to_string())
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> anyhow::Result<T> {
        let url = format!("{}{path}", self.api);
        let what = format!("{method} {path}");
        self.execute(&what, units(&method, path, query), || {
            let request = self.http.request(method.clone(), &url).query(query);
            match body {
                Some(body) => request.json(body),
                None => request,
            }
        })
        .await
    }

    /// Sends what `build` makes with the access token, again after a refresh, a busy answer or
    /// Gmail saying the user's quota is spent, which it answers with a 403.
    async fn execute<T: DeserializeOwned>(
        &self,
        what: &str,
        units: u32,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> anyhow::Result<T> {
        for attempt in 0..4 {
            self.spend(units).await;
            let response = build().bearer_auth(self.access_token().await?).send().await?;
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                *self.token.lock().await = None;
                continue;
            }
            if status.is_server_error() {
                tokio::time::sleep(Duration::from_millis(500 << attempt)).await;
                continue;
            }
            if status == StatusCode::NOT_FOUND {
                return Err(NotFound.into());
            }
            if status.is_success() {
                let text = response.text().await?;
                return Ok(serde_json::from_str(if text.is_empty() { "null" } else { &text })?);
            }
            let text = response.text().await.unwrap_or_default();
            if status == StatusCode::TOO_MANY_REQUESTS || (status == StatusCode::FORBIDDEN && rate_limited(&text)) {
                self.hold(Duration::from_secs(2 << attempt)).await;
                continue;
            }
            // Accounts added before the app asked for all of Gmail can't delete for good.
            if status == StatusCode::FORBIDDEN && missing_scope(&text) {
                return Err(Reauth(format!("Gmail answered {what} without the access it needs")).into());
            }
            if status == StatusCode::BAD_REQUEST || status == StatusCode::CONFLICT {
                return Err(Refused(format!("Gmail answered {what} with {status}: {text}")).into());
            }
            anyhow::bail!("Gmail answered {what} with {status}: {text}");
        }
        anyhow::bail!("Gmail kept refusing {what}")
    }

    /// Waits until `units` of the quota can be spent, and spends them.
    async fn spend(&self, units: u32) {
        let units = f64::from(units);
        let keep = if self.background { RESERVE_UNITS } else { 0.0 };
        loop {
            let short = {
                let mut pace = self.pace.lock().await;
                let now = Instant::now();
                pace.units =
                    (pace.units + now.duration_since(pace.at).as_secs_f64() * UNITS_PER_SECOND).min(BURST_UNITS);
                pace.at = now;
                if pace.units >= units + keep {
                    pace.units -= units;
                    return;
                }
                units + keep - pace.units
            };
            tokio::time::sleep(Duration::from_secs_f64(short / UNITS_PER_SECOND)).await;
        }
    }

    /// Spends nothing for `wait`, from now: Gmail said the quota is spent.
    async fn hold(&self, wait: Duration) {
        let mut pace = self.pace.lock().await;
        pace.units = pace.units.min(-wait.as_secs_f64() * UNITS_PER_SECOND);
        pace.at = Instant::now();
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<T> {
        self.request(Method::GET, path, query, None).await
    }

    /// Catches up on what changed, through the API.
    pub async fn sync(&self, state: &AppState, account: &AccountRow) -> anyhow::Result<Synced> {
        let mut sync: SyncState = serde_json::from_value(account.sync_state.clone()).unwrap_or_default();
        let (labels, _) = self.sync_labels(state, account).await?;

        match sync.history_id.clone() {
            // The history id is kept before the listing starts, so what arrives meanwhile is
            // caught up on later, and saved at once, so a failed pass carries on instead of
            // starting over.
            None => {
                let profile: Value = self.get("/profile", &[]).await?;
                sync.history_id = Some(profile["historyId"].as_str().unwrap_or_default().to_string());
                sync.refreshed = false;
                sync.refresh = Refresh::default();
                db::save_sync_state(&state.db, account.id, &json!(sync)).await?;
            }
            Some(history_id) => match self.history(&history_id).await {
                Ok(mut changes) => {
                    db::relabel_messages(&state.db, account, &changes.relabels(&labels)).await?;
                    db::delete_messages(&state.db, account, &changes.deleted).await?;
                    self.fetch_and_store(state, account, &labels, &changes.newest_first()).await?;
                    sync.history_id = Some(changes.latest);
                }
                Err(error) if error.is::<NotFound>() => {
                    tracing::info!("Gmail history of {} expired, syncing again", account.address);
                    sync.history_id = None;
                    db::save_sync_state(&state.db, account.id, &json!(sync)).await?;
                    return Ok(Synced { backfilling: true, syncing: true });
                }
                Err(error) => return Err(error),
            },
        }
        db::save_sync_state(&state.db, account.id, &json!(sync)).await?;
        Ok(Synced { backfilling: !sync.refreshed || !sync.backfilled, syncing: !sync.refreshed })
    }

    /// A page more of the last 90 days until they are listed, then of older mail.
    pub async fn fill(&self, state: &AppState, account: &AccountRow) -> anyhow::Result<Synced> {
        let mut sync: SyncState = serde_json::from_value(account.sync_state.clone()).unwrap_or_default();
        if sync.history_id.is_none() {
            return Ok(Synced { backfilling: true, syncing: true });
        }
        if sync.refreshed && sync.backfilled {
            return Ok(Synced { backfilling: false, syncing: false });
        }
        let (labels, names) = self.sync_labels(state, account).await?;
        if !sync.refreshed {
            self.refresh_page(state, account, &labels, &names, &mut sync).await?;
        } else {
            let query = "older_than:90d";
            let (below, validity) = (sync.backfill_below, sync.uid_validity);
            let page = self
                .over_imap(|imap| {
                    let names = names.clone();
                    async move { imap_page(imap, query, below, validity, IMAP_PAGE, OLDER_BYTES, &names).await }.boxed()
                })
                .await;
            match page {
                Some(page) => {
                    store(state, account, &page.messages).await?;
                    sync.keep_uid_validity(page.uid_validity);
                    sync.backfill_below = page.lowest;
                    sync.backfilled = !page.more;
                }
                None => {
                    let (ids, next) = self.list(query, BACKFILL_PAGE, sync.backfill_token.clone()).await?;
                    self.fetch_and_store(state, account, &labels, &ids).await?;
                    sync.backfilled = next.is_none();
                    sync.backfill_token = next;
                }
            }
        }
        db::save_sync_state(&state.db, account.id, &json!(sync)).await?;
        Ok(Synced { backfilling: !sync.refreshed || !sync.backfilled, syncing: !sync.refreshed })
    }

    /// One page of the last 90 days: the inbox's until it is listed, then everyone else's. Up to
    /// INITIAL_SYNC_LIMIT messages in all.
    async fn refresh_page(
        &self,
        state: &AppState,
        account: &AccountRow,
        labels: &HashMap<String, Uuid>,
        names: &HashMap<String, Uuid>,
        sync: &mut SyncState,
    ) -> anyhow::Result<()> {
        let limit = state.config.initial_sync_limit;
        let query = match sync.refresh.inbox_listed {
            true => format!("{RECENT} -in:inbox"),
            false => format!("{RECENT} in:inbox"),
        };
        let left = limit.saturating_sub(sync.refresh.fetched);
        if left == 0 {
            sync.refreshed = true;
            return Ok(());
        }
        let first = sync.refresh.fetched == 0;
        let imap_wanted = if first { FIRST_PAGE } else { IMAP_PAGE }.min(left);
        let (below, validity) = (sync.refresh.below_uid, sync.uid_validity);
        let page = self
            .over_imap(|imap| {
                let (query, names) = (query.clone(), names.clone());
                async move { imap_page(imap, &query, below, validity, imap_wanted, RECENT_BYTES, &names).await }.boxed()
            })
            .await;
        let more = match page {
            Some(page) => {
                store(state, account, &page.messages).await?;
                sync.keep_uid_validity(page.uid_validity);
                sync.refresh.fetched += page.messages.len();
                sync.refresh.below_uid = page.lowest;
                page.more
            }
            None => {
                let wanted = if first { FIRST_PAGE } else { REFRESH_PAGE }.min(left);
                let (ids, next) = self.list(&query, wanted, sync.refresh.page_token.take()).await?;
                self.fetch_and_store(state, account, labels, &ids).await?;
                sync.refresh.fetched += ids.len();
                sync.refresh.page_token = next;
                sync.refresh.page_token.is_some()
            }
        };
        let refresh = &mut sync.refresh;
        if !more {
            refresh.below_uid = None;
            refresh.page_token = None;
            match refresh.inbox_listed {
                true => sync.refreshed = true,
                false => refresh.inbox_listed = true,
            }
        }
        if refresh.fetched >= limit {
            sync.refreshed = true;
        }
        Ok(())
    }

    /// Runs `work` over IMAP, again on a new connection if a kept one failed. Nothing when there
    /// is no IMAP or it failed lately: the API does the work then.
    async fn over_imap<T>(
        &self,
        mut work: impl for<'i> FnMut(&'i mut Imap) -> BoxFuture<'i, anyhow::Result<T>>,
    ) -> Option<T> {
        let url = self.imap_url.as_deref()?;
        let mut slot = self.imap.lock().await;
        if slot.failed_at.is_some_and(|at| at.elapsed() < IMAP_RETRY) {
            return None;
        }
        loop {
            // Taken out while in use, so a session left halfway by work given up on isn't kept.
            let kept = slot.imap.take();
            let fresh = kept.is_none();
            let mut imap = match kept {
                Some(imap) => imap,
                None => {
                    let connected = match self.access_token().await {
                        Ok(token) => Imap::connect(url, &self.address, &token).await,
                        Err(error) => Err(error),
                    };
                    match connected {
                        Ok(imap) => imap,
                        Err(error) => {
                            tracing::warn!("IMAP of {} is off for now: {error:#}", self.address);
                            slot.failed_at = Some(Instant::now());
                            return None;
                        }
                    }
                }
            };
            match work(&mut imap).await {
                Ok(value) => {
                    slot.imap = Some(imap);
                    return Some(value);
                }
                Err(error) => {
                    imap.logout().await;
                    if fresh {
                        tracing::warn!("IMAP of {} failed, the API takes over for now: {error:#}", self.address);
                        slot.failed_at = Some(Instant::now());
                        return None;
                    }
                }
            }
        }
    }

    /// The custom labels' ids by Gmail's ids, and by their names, as IMAP gives them.
    async fn sync_labels(
        &self,
        state: &AppState,
        account: &AccountRow,
    ) -> anyhow::Result<(HashMap<String, Uuid>, HashMap<String, Uuid>)> {
        let response: Value = self.get("/labels", &[]).await?;
        let custom: Vec<(String, String)> = response["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|label| label["type"] == "user")
            .filter_map(|label| Some((label["id"].as_str()?.to_string(), label["name"].as_str()?.to_string())))
            .collect();
        let ids = db::sync_labels(&state.db, account, &custom).await?;
        let names = custom.into_iter().filter_map(|(id, name)| Some((name, *ids.get(&id)?))).collect();
        Ok((ids, names))
    }

    /// Message ids matching `query`, newest first, up to `limit`, and where the listing stopped.
    async fn list(
        &self,
        query: &str,
        limit: usize,
        mut page: Option<String>,
    ) -> anyhow::Result<(Vec<String>, Option<String>)> {
        let mut ids = Vec::new();
        loop {
            let mut parameters =
                vec![("q", query.to_string()), ("maxResults", PAGE.min(limit - ids.len()).to_string())];
            if let Some(token) = &page {
                parameters.push(("pageToken", token.clone()));
            }
            let response: ListResponse = self.get("/messages", &parameters).await?;
            ids.extend(response.messages.into_iter().map(|message| message.id));
            page = response.next_page_token;
            if page.is_none() || ids.len() >= limit {
                return Ok((ids, page));
            }
        }
    }

    /// What changed after `history_id`, and the latest history id.
    async fn history(&self, history_id: &str) -> anyhow::Result<Changes> {
        let mut changes = Changes { latest: history_id.to_string(), ..Default::default() };
        let mut page: Option<String> = None;
        loop {
            let mut parameters = vec![("startHistoryId", history_id.to_string()), ("maxResults", "500".to_string())];
            if let Some(token) = &page {
                parameters.push(("pageToken", token.clone()));
            }
            let response: HistoryPage = self.get("/history", &parameters).await?;
            response.history.into_iter().for_each(|entry| changes.add(entry));
            if let Some(id) = response.history_id {
                changes.latest = id;
            }
            page = response.next_page_token;
            if page.is_none() {
                return Ok(changes);
            }
        }
    }

    async fn fetch_and_store(
        &self,
        state: &AppState,
        account: &AccountRow,
        labels: &HashMap<String, Uuid>,
        ids: &[String],
    ) -> anyhow::Result<()> {
        for chunk in ids.chunks(50) {
            let fetched: Vec<(String, anyhow::Result<GmailMessage>)> = stream::iter(chunk.iter().cloned())
                .map(|id| async move {
                    let mut query = vec![("format", "metadata".to_string())];
                    query.extend(HEADERS.iter().map(|header| ("metadataHeaders", header.to_string())));
                    let message = self.get(&format!("/messages/{id}"), &query).await;
                    (id, message)
                })
                .buffer_unordered(CONCURRENT_GETS)
                .collect()
                .await;
            let mut messages = Vec::new();
            let mut gone = Vec::new();
            for (id, message) in fetched {
                match message {
                    Ok(message) => messages.push(remote(message, labels)),
                    Err(error) if error.is::<NotFound>() => gone.push(id),
                    Err(error) => return Err(error),
                }
            }
            store(state, account, &messages).await?;
            db::delete_messages(&state.db, account, &gone).await?;
        }
        Ok(())
    }

    pub async fn apply(&self, batch: &Batch, labels: &HashMap<Uuid, String>) -> anyhow::Result<()> {
        if let Op::Delete { .. } = batch.op {
            for ids in batch.provider_ids.chunks(1000) {
                let body = json!({ "ids": ids });
                let _: Value = self.request(Method::POST, "/messages/batchDelete", &[], Some(&body)).await?;
            }
            return Ok(());
        }
        let label = |id: &String| match id.as_str() {
            role::INBOX => Some("INBOX".to_string()),
            role::TRASH => Some("TRASH".to_string()),
            role::SPAM => Some("SPAM".to_string()),
            _ => id.parse::<Uuid>().ok().and_then(|id| labels.get(&id).cloned()),
        };
        let (add, remove): (Vec<String>, Vec<String>) = match &batch.op {
            Op::SetUnread { unread: true, .. } => (vec!["UNREAD".into()], vec![]),
            Op::SetUnread { unread: false, .. } => (vec![], vec!["UNREAD".into()]),
            Op::SetStarred { starred: true, .. } => (vec!["STARRED".into()], vec![]),
            Op::SetStarred { starred: false, .. } => (vec![], vec!["STARRED".into()]),
            Op::Archive { .. } | Op::Snooze { .. } => (vec![], vec!["INBOX".into()]),
            Op::MoveToInbox { .. } => (vec!["INBOX".into()], vec!["TRASH".into(), "SPAM".into()]),
            Op::Trash { .. } => (vec!["TRASH".into()], vec!["INBOX".into(), "SPAM".into()]),
            Op::Spam { .. } => (vec!["SPAM".into()], vec!["INBOX".into(), "TRASH".into()]),
            Op::AddLabel { label: id, .. } => (label(id).into_iter().collect(), vec![]),
            Op::RemoveLabel { label: id, .. } => (vec![], label(id).into_iter().collect()),
            _ => return Ok(()),
        };
        if add.is_empty() && remove.is_empty() {
            return Ok(());
        }
        for ids in batch.provider_ids.chunks(1000) {
            let body = json!({ "ids": ids, "addLabelIds": add, "removeLabelIds": remove });
            let _: Value = self.request(Method::POST, "/messages/batchModify", &[], Some(&body)).await?;
        }
        Ok(())
    }

    /// Over IMAP when the worker fetches ahead; through the API when someone waits for it.
    pub async fn raw(&self, provider_id: &str) -> anyhow::Result<Vec<u8>> {
        if self.background
            && let Some(Some(raw)) = self
                .over_imap(|imap| {
                    let id = provider_id.to_string();
                    async move { imap.raw(&id).await }.boxed()
                })
                .await
        {
            return Ok(raw);
        }
        let message: Value = self.get(&format!("/messages/{provider_id}"), &[("format", "raw".into())]).await?;
        let raw = message["raw"].as_str().unwrap_or_default().trim_end_matches('=');
        Ok(URL_SAFE_NO_PAD.decode(raw)?)
    }

    /// Goes through Gmail's upload URL, which takes messages up to 35 MB; the plain one takes 5.
    pub async fn send(&self, draft: &Draft, from: &Address, files: &[File]) -> anyhow::Result<Option<String>> {
        let domain = from.email.split('@').nth(1).unwrap_or("gmail.com");
        let raw = mime::build(draft, from, &mime::new_message_id(domain), true, files);
        let mut metadata = json!({});
        if let Some(thread_id) = &draft.thread_id {
            metadata["threadId"] = json!(thread_id);
        }
        let boundary = format!("part-{}", crate::random_token());
        let mut body = format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n\
             --{boundary}\r\nContent-Type: message/rfc822\r\n\r\n"
        )
        .into_bytes();
        body.extend(raw);
        body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
        let url = format!("{}/messages/send", self.api.replacen("/gmail/v1/", "/upload/gmail/v1/", 1));
        let content_type = format!("multipart/related; boundary={boundary}");
        let sent: Value = self
            .execute("POST /messages/send", 100, || {
                self.http
                    .post(&url)
                    .query(&[("uploadType", "multipart")])
                    .header(reqwest::header::CONTENT_TYPE, &content_type)
                    .body(body.clone())
            })
            .await?;
        Ok(sent["id"].as_str().map(String::from))
    }

    /// The verified send-as addresses, the default first, then the account's own.
    pub async fn identities(&self) -> anyhow::Result<Vec<Identity>> {
        let response: Value = self.get("/settings/sendAs", &[]).await?;
        let mut list: Vec<&Value> = response["sendAs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|send_as| send_as["isPrimary"] == true || send_as["verificationStatus"] == "accepted")
            .collect();
        list.sort_by_key(|send_as| (send_as["isDefault"] != true, send_as["isPrimary"] != true));
        Ok(list
            .into_iter()
            .filter_map(|send_as| {
                let address = Address::new(send_as["displayName"].as_str(), send_as["sendAsEmail"].as_str()?);
                let signature = send_as["signature"].as_str().map(html_text).filter(|text| !text.is_empty());
                Some(Identity { name: address.name, email: address.email, signature })
            })
            .collect())
    }

    pub async fn create_label(&self, name: &str) -> anyhow::Result<String> {
        let body = json!({ "name": name, "labelListVisibility": "labelShow", "messageListVisibility": "show" });
        let label: Value = self.request(Method::POST, "/labels", &[], Some(&body)).await?;
        label["id"].as_str().map(String::from).ok_or_else(|| anyhow::anyhow!("Gmail made the label without an id"))
    }

    pub async fn watch(&self, state: &AppState) -> anyhow::Result<()> {
        let Some(topic) = &state.config.gmail_pubsub_topic else { return Ok(()) };
        let body = json!({ "topicName": topic });
        let _: Value = self.request(Method::POST, "/watch", &[], Some(&body)).await?;
        Ok(())
    }
}

impl SyncState {
    /// Saved UIDs of another validity mean nothing: the listings over IMAP start again.
    fn keep_uid_validity(&mut self, uid_validity: u32) {
        if self.uid_validity == Some(uid_validity) {
            return;
        }
        self.uid_validity = Some(uid_validity);
        self.refresh.below_uid = None;
        self.backfill_below = None;
    }
}

async fn store(state: &AppState, account: &AccountRow, messages: &[RemoteMessage]) -> anyhow::Result<()> {
    db::upsert_messages(&state.db, account, messages).await?;
    crate::avatars::warm(state, messages.iter().map(|message| message.from.email.as_str()));
    Ok(())
}

/// The next `size` messages a search finds over IMAP, newest first, below the UID `below` if it
/// is of the mailbox's `uid_validity`.
async fn imap_page(
    imap: &mut Imap,
    query: &str,
    below: Option<u32>,
    uid_validity: Option<u32>,
    size: usize,
    bytes: u32,
    names: &HashMap<String, Uuid>,
) -> anyhow::Result<ImapPage> {
    let below = below.filter(|_| uid_validity == Some(imap.uid_validity));
    let (uids, more) = imap.search(query, below, size).await?;
    let mut page =
        ImapPage { messages: Vec::new(), lowest: uids.last().copied(), more, uid_validity: imap.uid_validity };
    if uids.is_empty() {
        return Ok(page);
    }
    let bulk = imap.bulk(&uids).await?;
    let fetched = imap.fetch(&uids, Some(bytes)).await?;
    page.messages = fetched
        .into_iter()
        .map(|message| {
            let is_bulk = bulk.contains(&message.uid);
            gmail_imap::remote(message, names, is_bulk)
        })
        .collect();
    Ok(page)
}

/// What a request costs of the user's quota, from Gmail's table.
fn units(method: &Method, path: &str, query: &[(&str, String)]) -> u32 {
    let whole = query.iter().any(|(key, value)| *key == "format" && matches!(value.as_str(), "full" | "raw"));
    match path {
        "/profile" | "/settings/sendAs" => 1,
        "/labels" if method == Method::GET => 1,
        "/history" => 2,
        "/messages/batchModify" | "/messages/batchDelete" => 50,
        "/watch" => 100,
        _ if method == Method::GET && path.starts_with("/messages/") && whole => 60,
        _ if method == Method::GET && path.starts_with("/messages/") => 20,
        _ => 5,
    }
}

fn missing_scope(body: &str) -> bool {
    body.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT") || body.contains("insufficientPermissions")
}

/// Gmail answers a spent quota with a 403 that says so, unlike a 403 for missing access.
fn rate_limited(body: &str) -> bool {
    body.to_ascii_lowercase().contains("ratelimitexceeded")
}

#[derive(Debug)]
struct NotFound;

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gmail has no such item")
    }
}

impl std::error::Error for NotFound {}

fn remote(message: GmailMessage, labels: &HashMap<String, Uuid>) -> RemoteMessage {
    let lines: Vec<(String, String)> = message
        .payload
        .map(|payload| payload.headers.into_iter().map(|header| (header.name, header.value)).collect())
        .unwrap_or_default();
    let headers = mime::headers(&lines);
    let (mut unread, mut starred, mut bulk) = (false, false, headers.bulk);
    let mut normalized = Vec::new();
    for label in &message.label_ids {
        match mark(label, labels) {
            Mark::Label(label) => normalized.push(label),
            Mark::Unread => unread = true,
            Mark::Starred => starred = true,
            Mark::Bulk => bulk = true,
            Mark::Other => {}
        }
    }
    let date = message.internal_date.and_then(|date| date.parse().ok()).or(headers.date).unwrap_or(0);
    let attachments = match headers.multipart_mixed {
        true => vec![Attachment { name: String::new(), mime: String::new(), size: 0 }],
        false => Vec::new(),
    };
    RemoteMessage {
        provider_id: message.id,
        thread_id: message.thread_id,
        from: headers.from,
        recipients: headers.recipients,
        subject: headers.subject,
        snippet: one_line(&unescape(&message.snippet)),
        date,
        unread,
        starred,
        labels: normalized,
        attachments,
        message_id: headers.message_id,
        in_reply_to: headers.in_reply_to,
        references: headers.references,
        bulk,
        unsubscribe: headers.unsubscribe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_labels_and_flags() {
        let custom = Uuid::new_v4();
        let labels = HashMap::from([("Label_7".to_string(), custom)]);
        let message: GmailMessage = serde_json::from_value(json!({
            "id": "m1", "threadId": "t1", "snippet": "Hi &amp; bye",
            "labelIds": ["INBOX", "UNREAD", "STARRED", "Label_7", "CATEGORY_PERSONAL"],
            "internalDate": "1700000000000",
            "payload": {"headers": [{"name": "From", "value": "Ann <ann@example.com>"}, {"name": "Subject", "value": "Hello"}]}
        }))
        .unwrap();
        let remote = remote(message, &labels);
        assert_eq!(remote.labels, ["inbox".to_string(), custom.to_string()]);
        assert!(remote.unread && remote.starred);
        assert_eq!(remote.snippet, "Hi & bye");
        assert_eq!(remote.date, 1_700_000_000_000);
        assert_eq!(remote.from.email, "ann@example.com");
        assert!(!remote.bulk);
    }

    #[test]
    fn promotions_and_lists_are_bulk() {
        let message = |labels: Value, headers: Value| -> GmailMessage {
            serde_json::from_value(
                json!({ "id": "m", "threadId": "t", "labelIds": labels, "payload": { "headers": headers } }),
            )
            .unwrap()
        };
        let promotion = remote(message(json!(["INBOX", "CATEGORY_PROMOTIONS"]), json!([])), &HashMap::new());
        assert!(promotion.bulk && promotion.unsubscribe.is_none());
        let headers = json!([{ "name": "List-Unsubscribe", "value": "<mailto:leave@list.org>" }]);
        let list = remote(message(json!(["INBOX"]), headers), &HashMap::new());
        assert!(list.bulk);
        assert_eq!(list.unsubscribe.unwrap().mailto.as_deref(), Some("mailto:leave@list.org"));
    }
}
