//! Outlook mail (Outlook.com, Hotmail and Microsoft 365) through Microsoft Graph. Folders with a
//! well-known name are the system mailboxes and mail in any other folder is archived; Outlook's
//! categories are the labels. Each folder is listed newest first, a page per pass and the inbox
//! first, through its delta query, whose last link then says what changed. Ids are immutable, so a
//! message keeps its id when it moves, and one a folder lost is looked up to see where it went.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{SecondsFormat, Utc};
use futures_util::{StreamExt, stream};
use mail_protocol::{Address, Attachment, Draft, Identity, Op, Recipients, role};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{OnceCell, Semaphore};
use uuid::Uuid;

use super::{Batch, Reauth, Refused, Synced, one_line};
use crate::db::{self, AccountRow, RemoteMessage};
use crate::mime::{File, ListHeaders};
use crate::{AppState, mime};

pub const API_URL: &str = "https://graph.microsoft.com/v1.0";
/// Graph runs 4 requests at once for a mailbox and refuses more.
const CONCURRENT: usize = 4;
const PREFER: &str = "IdType=\"ImmutableId\", odata.maxpagesize=100";
const ARCHIVE: &str = "archive";
const SKIP: &str = "skip";
/// The well-known folders and what they are; mail in the skipped ones isn't synced.
const FOLDERS: [(&str, &str); 9] = [
    ("inbox", role::INBOX),
    ("archive", ARCHIVE),
    ("sentitems", role::SENT),
    ("drafts", role::DRAFTS),
    ("deleteditems", role::TRASH),
    ("junkemail", role::SPAM),
    ("outbox", SKIP),
    ("conversationhistory", SKIP),
    ("syncissues", SKIP),
];
const SELECT: &str = "id,conversationId,parentFolderId,from,sender,toRecipients,ccRecipients,bccRecipients,replyTo,\
    subject,bodyPreview,receivedDateTime,isRead,flag,categories,hasAttachments,internetMessageId,\
    inferenceClassification,unsubscribeData";
/// Graph takes requests up to 4 MB, and sendMail takes the message in base64.
const SEND_LIMIT: usize = 4 * 1024 * 1024 - 64 * 1024;
/// How long a push subscription lasts; it is renewed daily. Graph allows mail ones under 7 days.
const SUBSCRIPTION_DAYS: i64 = 3;

#[derive(Clone)]
pub struct Outlook {
    state: AppState,
    account_id: Uuid,
    api: Arc<String>,
    refresh_token: Arc<Mutex<String>>,
    token: Arc<tokio::sync::Mutex<Option<(String, Instant)>>>,
    permits: Arc<Semaphore>,
    /// Folder id → what it is, of the well-known folders.
    roles: Arc<OnceCell<HashMap<String, String>>>,
}

#[derive(Deserialize)]
pub struct Credentials {
    pub refresh_token: String,
}

#[derive(Deserialize, Serialize, Default)]
struct SyncState {
    /// Folder id → where its delta query stands.
    #[serde(default)]
    folders: HashMap<String, Delta>,
    /// How many messages the first listing has brought.
    #[serde(default)]
    fetched: usize,
}

#[derive(Deserialize, Serialize, Clone)]
struct Delta {
    /// A next link while the folder is first listed, then the delta link.
    link: String,
    listed: bool,
}

impl Outlook {
    pub async fn open(state: &AppState, account: &AccountRow, secret: &str) -> anyhow::Result<Self> {
        let credentials: Credentials = serde_json::from_str(secret)?;
        let outlook = Self {
            state: state.clone(),
            account_id: account.id,
            api: Arc::new(state.config.graph_api_url.clone()),
            refresh_token: Arc::new(Mutex::new(credentials.refresh_token)),
            token: Arc::new(tokio::sync::Mutex::new(None)),
            permits: Arc::new(Semaphore::new(CONCURRENT)),
            roles: Arc::new(OnceCell::new()),
        };
        outlook.access_token().await?;
        Ok(outlook)
    }

    /// Microsoft hands out a new refresh token with each access token; it is kept, as the old one
    /// stops working after 90 days.
    async fn access_token(&self) -> anyhow::Result<String> {
        let mut token = self.token.lock().await;
        if let Some((access, expires)) = token.as_ref()
            && *expires > Instant::now()
        {
            return Ok(access.clone());
        }
        let config = &self.state.config;
        let refresh = self.refresh_token.lock().unwrap().clone();
        let response = self
            .state
            .http
            .post(&config.microsoft_token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
                ("client_id", config.microsoft_client_id.as_str()),
                ("client_secret", config.microsoft_client_secret.as_str()),
                ("scope", crate::microsoft::SCOPES),
            ])
            .send()
            .await?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        // An expired client secret is ours to fix, not the user's.
        if body["error"] == "invalid_grant" {
            return Err(Reauth(body["error_description"].as_str().unwrap_or("refused").to_string()).into());
        }
        let Some(access) = body["access_token"].as_str() else {
            anyhow::bail!("Microsoft answered the token refresh with {status}: {}", body["error"]);
        };
        if let Some(renewed) = body["refresh_token"].as_str().filter(|renewed| *renewed != refresh) {
            let sealed = self.state.sealer.seal(json!({ "refresh_token": renewed }).to_string().as_bytes());
            db::set_credentials(&self.state.db, self.account_id, &sealed).await?;
            *self.refresh_token.lock().unwrap() = renewed.to_string();
        }
        let lifetime = body["expires_in"].as_u64().unwrap_or(3600).saturating_sub(120);
        *token = Some((access.to_string(), Instant::now() + Duration::from_secs(lifetime)));
        Ok(access.to_string())
    }

    /// Sends what `build` makes with the access token, again after a refresh or a busy answer.
    async fn execute(
        &self,
        what: &str,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> anyhow::Result<reqwest::Response> {
        for attempt in 0..5 {
            let permit = self.permits.acquire().await?;
            let token = self.access_token().await?;
            let response = build().bearer_auth(token).header("Prefer", PREFER).send().await?;
            drop(permit);
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                *self.token.lock().await = None;
                continue;
            }
            if [StatusCode::TOO_MANY_REQUESTS, StatusCode::SERVICE_UNAVAILABLE, StatusCode::GATEWAY_TIMEOUT]
                .contains(&status)
            {
                let wait = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok()?.parse().ok())
                    .map(Duration::from_secs)
                    .unwrap_or(Duration::from_secs(2 << attempt));
                tokio::time::sleep(wait.min(Duration::from_secs(60))).await;
                continue;
            }
            if status.is_server_error() {
                tokio::time::sleep(Duration::from_millis(500 << attempt)).await;
                continue;
            }
            // A delta link that expired answers 410; the folder is listed again either way.
            if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
                return Err(NotFound.into());
            }
            if status.is_success() {
                return Ok(response);
            }
            let text = response.text().await.unwrap_or_default();
            if status == StatusCode::BAD_REQUEST || status == StatusCode::CONFLICT {
                return Err(Refused(format!("Graph answered {what} with {status}: {text}")).into());
            }
            anyhow::bail!("Graph answered {what} with {status}: {text}");
        }
        anyhow::bail!("Graph kept refusing {what}")
    }

    /// A path under the API, or a link Graph gave.
    async fn json(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<Value> {
        let url = if path.starts_with("http") { path.to_string() } else { format!("{}{path}", self.api) };
        let what = format!("{method} {}", path.split('?').next().unwrap_or_default());
        let response = self
            .execute(&what, || {
                let request = self.state.http.request(method.clone(), &url);
                match body {
                    Some(body) => request.json(body),
                    None => request,
                }
            })
            .await?;
        let text = response.text().await?;
        Ok(if text.is_empty() { Value::Null } else { serde_json::from_str(&text)? })
    }

    async fn get(&self, path: &str) -> anyhow::Result<Value> {
        self.json(Method::GET, path, None).await
    }

    /// The well-known folders, by id. The archive is made when there is none.
    async fn roles(&self) -> anyhow::Result<&HashMap<String, String>> {
        self.roles
            .get_or_try_init(|| async {
                let mut roles = HashMap::new();
                for (name, role) in FOLDERS {
                    match self.get(&format!("/me/mailFolders/{name}?$select=id")).await {
                        Ok(folder) => roles.extend(folder["id"].as_str().map(|id| (id.to_string(), role.to_string()))),
                        Err(error) if error.is::<NotFound>() => {}
                        // Not every mailbox knows every name.
                        Err(error) if role == SKIP && error.is::<Refused>() => {}
                        Err(error) => return Err(error),
                    }
                }
                if !roles.values().any(|role| role == ARCHIVE) {
                    let archive = self.archive().await?;
                    roles.extend(archive.map(|id| (id, ARCHIVE.to_string())));
                }
                Ok(roles)
            })
            .await
    }

    /// A folder named Archive for a mailbox without the well-known one, made when there is none.
    async fn archive(&self) -> anyhow::Result<Option<String>> {
        let named = self.get("/me/mailFolders?$filter=displayName%20eq%20'Archive'&$select=id").await?;
        if let Some(id) = named["value"][0]["id"].as_str() {
            return Ok(Some(id.to_string()));
        }
        let folder = self.json(Method::POST, "/me/mailFolders", Some(&json!({ "displayName": "Archive" }))).await?;
        Ok(folder["id"].as_str().map(String::from))
    }

    /// Every folder whose mail is synced, the inbox first and the trash and spam last.
    async fn folders(&self) -> anyhow::Result<Vec<String>> {
        let roles = self.roles().await?;
        let mut found = Vec::new();
        let mut parents = vec!["/me/mailFolders".to_string()];
        while let Some(parent) = parents.pop() {
            let mut next = Some(format!("{parent}?$top=100&$select=id,childFolderCount"));
            while let Some(link) = next.take() {
                let page = self.get(&link).await?;
                for folder in page["value"].as_array().into_iter().flatten() {
                    let Some(id) = folder["id"].as_str() else { continue };
                    if roles.get(id).is_some_and(|role| role == SKIP) {
                        continue;
                    }
                    if folder["childFolderCount"].as_u64().unwrap_or(0) > 0 {
                        parents.push(format!("/me/mailFolders/{}/childFolders", encode(id)));
                    }
                    found.push(id.to_string());
                }
                next = page["@odata.nextLink"].as_str().map(String::from);
            }
        }
        let rank = |id: &String| match roles.get(id).map(String::as_str) {
            Some(role::INBOX) => 0,
            Some(ARCHIVE) => 1,
            Some(role::SENT) => 2,
            Some(role::DRAFTS) => 3,
            None => 4,
            _ => 5,
        };
        found.sort_by_key(rank);
        Ok(found)
    }

    async fn sync_labels(&self, account: &AccountRow) -> anyhow::Result<HashMap<String, Uuid>> {
        let response = self.get("/me/outlook/masterCategories").await?;
        let categories: Vec<(String, String)> = response["value"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|category| category["displayName"].as_str())
            .map(|name| (name.to_string(), name.to_string()))
            .collect();
        Ok(db::sync_labels(&self.state.db, account, &categories).await?)
    }

    /// Catches up on every folder listed, and lists a page more of the first one that isn't.
    pub async fn sync(&self, state: &AppState, account: &AccountRow) -> anyhow::Result<Synced> {
        let mut sync: SyncState = serde_json::from_value(account.sync_state.clone()).unwrap_or_default();
        let labels = self.sync_labels(account).await?;
        let folders = self.folders().await?;
        sync.folders.retain(|id, _| folders.contains(id));

        let mut listing = false;
        for folder in &folders {
            let delta = sync.folders.get(folder).cloned();
            if listing && !delta.as_ref().is_some_and(|delta| delta.listed) {
                continue;
            }
            let link = delta.as_ref().map(|delta| delta.link.clone()).unwrap_or_else(|| {
                format!(
                    "/me/mailFolders/{}/messages/delta?$select={SELECT}&$orderby=receivedDateTime%20desc",
                    encode(folder)
                )
            });
            let result = match delta {
                Some(Delta { listed: true, .. }) => self.catch_up(account, &labels, link).await,
                _ => {
                    listing = true;
                    self.page(account, &labels, &link).await.map(|(count, next)| {
                        sync.fetched += count;
                        next
                    })
                }
            };
            match result {
                Ok(next) => {
                    sync.folders.insert(folder.clone(), next);
                }
                Err(error) if error.is::<NotFound>() => {
                    sync.folders.remove(folder);
                }
                Err(error) => return Err(error),
            }
            db::save_sync_state(&state.db, account.id, &json!(sync)).await?;
        }
        let listed = folders.iter().all(|folder| sync.folders.get(folder).is_some_and(|delta| delta.listed));
        let limit = state.config.initial_sync_limit;
        Ok(Synced { backfilling: !listed, syncing: !listed && sync.fetched < limit })
    }

    /// Follows a delta link to the next one.
    async fn catch_up(
        &self,
        account: &AccountRow,
        labels: &HashMap<String, Uuid>,
        link: String,
    ) -> anyhow::Result<Delta> {
        let mut link = link;
        loop {
            let (_, next) = self.page(account, labels, &link).await?;
            if next.listed {
                return Ok(next);
            }
            link = next.link;
        }
    }

    /// Stores one page of a delta query and answers where to go on from, with how many it had.
    async fn page(
        &self,
        account: &AccountRow,
        labels: &HashMap<String, Uuid>,
        link: &str,
    ) -> anyhow::Result<(usize, Delta)> {
        let page = self.get(link).await?;
        let items = page["value"].as_array().cloned().unwrap_or_default();
        let (removed, changed): (Vec<&Value>, Vec<&Value>) =
            items.iter().partition(|item| item.get("@removed").is_some());
        let removed = removed.iter().filter_map(|item| item["id"].as_str().map(String::from)).collect();
        self.store(account, labels, changed.into_iter().cloned().collect(), removed).await?;
        let next = match (page["@odata.nextLink"].as_str(), page["@odata.deltaLink"].as_str()) {
            (Some(next), _) => Delta { link: next.to_string(), listed: false },
            (None, Some(delta)) => Delta { link: delta.to_string(), listed: true },
            _ => anyhow::bail!("Graph answered a delta query without a link"),
        };
        Ok((items.len(), next))
    }

    /// A message a folder lost is looked up: it may only have moved to another.
    async fn store(
        &self,
        account: &AccountRow,
        labels: &HashMap<String, Uuid>,
        mut found: Vec<Value>,
        removed: Vec<String>,
    ) -> anyhow::Result<()> {
        let looked: Vec<(String, anyhow::Result<Value>)> = stream::iter(removed)
            .map(|id| async move {
                let message = self.get(&format!("/me/messages/{}?$select={SELECT}", encode(&id))).await;
                (id, message)
            })
            .buffer_unordered(CONCURRENT)
            .collect()
            .await;
        let mut gone = Vec::new();
        for (id, message) in looked {
            match message {
                Ok(message) => found.push(message),
                Err(error) if error.is::<NotFound>() => gone.push(id),
                Err(error) => return Err(error),
            }
        }
        let roles = self.roles().await?;
        let mut messages = Vec::new();
        for message in &found {
            match remote(message, labels, roles) {
                Some(message) => messages.push(message),
                None => gone.extend(message["id"].as_str().map(String::from)),
            }
        }
        db::upsert_messages(&self.state.db, account, &messages).await?;
        crate::avatars::warm(&self.state, messages.iter().map(|message| message.from.email.as_str()));
        db::delete_messages(&self.state.db, account, &gone).await?;
        Ok(())
    }

    pub async fn apply(&self, batch: &Batch, labels: &HashMap<Uuid, String>) -> anyhow::Result<()> {
        let category = |id: &String| id.parse::<Uuid>().ok().and_then(|id| labels.get(&id).cloned());
        let change = match &batch.op {
            Op::Delete { .. } => Change::Delete,
            Op::SetUnread { unread, .. } => Change::Patch(json!({ "isRead": !unread })),
            Op::SetStarred { starred, .. } => {
                Change::Patch(json!({ "flag": { "flagStatus": if *starred { "flagged" } else { "notFlagged" } } }))
            }
            Op::Archive { .. } | Op::Snooze { .. } => Change::Archive,
            Op::RemoveLabel { label, .. } if label == role::INBOX => Change::Archive,
            Op::MoveToInbox { .. } => Change::ToInbox,
            Op::AddLabel { label, .. } if label == role::INBOX => Change::ToInbox,
            Op::Trash { .. } => Change::Move(role::TRASH),
            Op::AddLabel { label, .. } if label == role::TRASH => Change::Move(role::TRASH),
            Op::Spam { .. } => Change::Move(role::SPAM),
            Op::AddLabel { label, .. } if label == role::SPAM => Change::Move(role::SPAM),
            Op::RemoveLabel { label, .. } if label == role::TRASH || label == role::SPAM => Change::Move(ARCHIVE),
            Op::AddLabel { label, .. } => match category(label) {
                Some(name) => Change::Category(name, true),
                None => return Ok(()),
            },
            Op::RemoveLabel { label, .. } => match category(label) {
                Some(name) => Change::Category(name, false),
                None => return Ok(()),
            },
            _ => return Ok(()),
        };
        let folders: HashMap<&str, &str> =
            self.roles().await?.iter().map(|(id, role)| (role.as_str(), id.as_str())).collect();
        let (change, folders) = (&change, &folders);
        let results: Vec<anyhow::Result<()>> = stream::iter(batch.provider_ids.clone())
            .map(|id| async move { self.change(&id, change, folders).await })
            .buffer_unordered(CONCURRENT)
            .collect()
            .await;
        for result in results {
            match result {
                Err(error) if !error.is::<NotFound>() => return Err(error),
                _ => {}
            }
        }
        Ok(())
    }

    /// Changes one message. Archiving moves it only out of the inbox, and back to the inbox goes
    /// only what isn't sent, a draft or there already, as Outlook keeps a message in one folder.
    async fn change(&self, id: &str, change: &Change, folders: &HashMap<&str, &str>) -> anyhow::Result<()> {
        let path = format!("/me/messages/{}", encode(id));
        let current_path = format!("{path}?$select=parentFolderId,categories");
        let current = || self.get(&current_path);
        let move_to = |role: &'static str| {
            let destination = folders.get(role).copied();
            let path = format!("{path}/move");
            async move {
                let Some(destination) = destination else { anyhow::bail!("Outlook has no {role} folder") };
                self.json(Method::POST, &path, Some(&json!({ "destinationId": destination }))).await.map(drop)
            }
        };
        let in_folder = |message: &Value, roles: &[&str]| {
            roles.iter().any(|role| folders.get(role).is_some_and(|id| message["parentFolderId"] == *id))
        };
        match change {
            Change::Delete => self.json(Method::DELETE, &path, None).await.map(drop),
            Change::Patch(body) => self.json(Method::PATCH, &path, Some(body)).await.map(drop),
            Change::Move(role) => move_to(role).await,
            Change::Archive => match in_folder(&current().await?, &[role::INBOX]) {
                true => move_to(ARCHIVE).await,
                false => Ok(()),
            },
            Change::ToInbox => match in_folder(&current().await?, &[role::INBOX, role::SENT, role::DRAFTS]) {
                true => Ok(()),
                false => move_to(role::INBOX).await,
            },
            Change::Category(name, add) => {
                let message = current().await?;
                let mut categories: Vec<&str> = message["categories"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|category| category != name)
                    .collect();
                if *add {
                    categories.push(name);
                }
                self.json(Method::PATCH, &path, Some(&json!({ "categories": categories }))).await.map(drop)
            }
        }
    }

    pub async fn raw(&self, provider_id: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("{}/me/messages/{}/$value", self.api, encode(provider_id));
        let response = self.execute("GET /me/messages/$value", || self.state.http.get(&url)).await?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Outlook keeps what it sends in Sent Items on its own, but doesn't say under which id.
    pub async fn send(&self, draft: &Draft, from: &Address, files: &[File]) -> anyhow::Result<Option<String>> {
        let domain = from.email.split('@').nth(1).unwrap_or("outlook.com");
        let raw = mime::build(draft, from, &mime::new_message_id(domain), true, files);
        let body = STANDARD.encode(raw);
        if body.len() > SEND_LIMIT {
            return Err(Refused("Outlook takes messages up to 3 MB, files included.".into()).into());
        }
        let url = format!("{}/me/sendMail", self.api);
        self.execute("POST /me/sendMail", || {
            self.state.http.post(&url).header(reqwest::header::CONTENT_TYPE, "text/plain").body(body.clone())
        })
        .await?;
        Ok(None)
    }

    pub async fn identities(&self) -> anyhow::Result<Vec<Identity>> {
        let me = self.get("/me?$select=displayName,mail,userPrincipalName").await?;
        let email = [&me["mail"], &me["userPrincipalName"]]
            .into_iter()
            .filter_map(Value::as_str)
            .find(|email| email.contains('@'));
        let Some(email) = email else { return Ok(Vec::new()) };
        let address = Address::new(me["displayName"].as_str(), email);
        Ok(vec![Identity { name: address.name, email: address.email, signature: None }])
    }

    pub async fn create_label(&self, name: &str) -> anyhow::Result<String> {
        let body = json!({ "displayName": name, "color": "preset0" });
        let category = self.json(Method::POST, "/me/outlook/masterCategories", Some(&body)).await?;
        Ok(category["displayName"].as_str().unwrap_or(name).to_string())
    }

    /// Asks Graph to post to the hook when mail changes, renewing the subscription made before.
    /// A server Graph can't reach over https polls instead.
    pub async fn watch(&self, state: &AppState) -> anyhow::Result<()> {
        if !state.config.public_url.starts_with("https://") {
            return Ok(());
        }
        let url = format!("{}/hooks/outlook?account={}", state.config.public_url, self.account_id);
        let expires =
            (Utc::now() + chrono::Duration::days(SUBSCRIPTION_DAYS)).to_rfc3339_opts(SecondsFormat::Secs, true);
        let existing = self.get("/subscriptions").await.unwrap_or_default();
        let made = existing["value"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|subscription| subscription["notificationUrl"] == url);
        if let Some(id) = made.and_then(|subscription| subscription["id"].as_str()) {
            let renewal = json!({ "expirationDateTime": expires });
            match self.json(Method::PATCH, &format!("/subscriptions/{}", encode(id)), Some(&renewal)).await {
                Ok(_) => return Ok(()),
                Err(error) if error.is::<NotFound>() => {}
                Err(error) => return Err(error),
            }
        }
        let subscription = json!({
            "changeType": "created,updated,deleted",
            "notificationUrl": url,
            "resource": "me/messages",
            "expirationDateTime": expires,
            "clientState": client_state(&state.config.secret_key, self.account_id),
        });
        self.json(Method::POST, "/subscriptions", Some(&subscription)).await?;
        Ok(())
    }
}

enum Change {
    Delete,
    Patch(Value),
    Archive,
    ToInbox,
    Move(&'static str),
    Category(String, bool),
}

/// What Graph's notices for an account carry, so that only Graph can wake its worker.
pub fn client_state(secret_key: &str, account_id: Uuid) -> String {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret_key.as_bytes());
    hex::encode(ring::hmac::sign(&key, format!("outlook-hook:{account_id}").as_bytes()))
}

fn encode(id: &str) -> String {
    utf8_percent_encode(id, NON_ALPHANUMERIC).to_string()
}

#[derive(Debug)]
struct NotFound;

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Outlook has no such item")
    }
}

impl std::error::Error for NotFound {}

fn addresses(value: &Value) -> Vec<Address> {
    value.as_array().into_iter().flatten().filter_map(address).collect()
}

fn address(value: &Value) -> Option<Address> {
    let email = &value["emailAddress"];
    Some(Address::new(email["name"].as_str(), email["address"].as_str().filter(|address| !address.is_empty())?))
}

/// None for a message in a folder that isn't synced.
fn remote(message: &Value, labels: &HashMap<String, Uuid>, roles: &HashMap<String, String>) -> Option<RemoteMessage> {
    let role = message["parentFolderId"].as_str().and_then(|folder| roles.get(folder)).map(String::as_str);
    if role == Some(SKIP) {
        return None;
    }
    let mut normalized: Vec<String> = role.filter(|role| *role != ARCHIVE).map(String::from).into_iter().collect();
    let categories = message["categories"].as_array().into_iter().flatten().filter_map(Value::as_str);
    normalized.extend(categories.filter_map(|name| labels.get(name)).map(Uuid::to_string));
    let list = ListHeaders {
        unsubscribe: message["unsubscribeData"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|url| url.trim().trim_start_matches('<').trim_end_matches('>').to_string())
            .collect(),
        unsubscribe_post: None,
        list_id: None,
        precedence: None,
        auto_submitted: None,
    };
    let date = message["receivedDateTime"]
        .as_str()
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.timestamp_millis())
        .unwrap_or(0);
    let attachments = match message["hasAttachments"] == true {
        true => vec![Attachment { name: String::new(), mime: String::new(), size: 0 }],
        false => Vec::new(),
    };
    Some(RemoteMessage {
        provider_id: message["id"].as_str()?.to_string(),
        thread_id: message["conversationId"].as_str().unwrap_or_default().to_string(),
        from: address(&message["from"]).or_else(|| address(&message["sender"])).unwrap_or_default(),
        recipients: Recipients {
            to: addresses(&message["toRecipients"]),
            cc: addresses(&message["ccRecipients"]),
            bcc: addresses(&message["bccRecipients"]),
            reply_to: addresses(&message["replyTo"]),
        },
        subject: message["subject"].as_str().unwrap_or_default().to_string(),
        snippet: one_line(message["bodyPreview"].as_str().unwrap_or_default()),
        date,
        unread: message["isRead"] == false,
        starred: message["flag"]["flagStatus"] == "flagged",
        labels: normalized,
        attachments,
        message_id: message["internetMessageId"].as_str().map(String::from),
        in_reply_to: None,
        references: Vec::new(),
        bulk: list.bulk() || message["inferenceClassification"] == "other",
        unsubscribe: list.unsubscribe(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_folders_and_categories_to_labels() {
        let work = Uuid::new_v4();
        let labels = HashMap::from([("Work".to_string(), work)]);
        let roles = HashMap::from([
            ("in".to_string(), role::INBOX.to_string()),
            ("arc".to_string(), ARCHIVE.to_string()),
            ("out".to_string(), SKIP.to_string()),
        ]);
        let message = |folder: &str| {
            json!({
                "id": "m1", "conversationId": "c1", "parentFolderId": folder, "isRead": false,
                "flag": { "flagStatus": "flagged" }, "categories": ["Work", "Unknown"],
                "from": { "emailAddress": { "name": "Ann", "address": "Ann@Example.com" } },
                "toRecipients": [{ "emailAddress": { "name": "", "address": "cy@example.com" } }],
                "subject": "Hi", "bodyPreview": "Hello\r\nthere", "receivedDateTime": "2024-03-03T09:41:00Z",
                "hasAttachments": true, "inferenceClassification": "focused",
            })
        };
        let inbox = remote(&message("in"), &labels, &roles).unwrap();
        assert_eq!(inbox.labels, [role::INBOX.to_string(), work.to_string()]);
        assert!(inbox.unread && inbox.starred && !inbox.bulk);
        assert_eq!((inbox.thread_id.as_str(), inbox.snippet.as_str()), ("c1", "Hello there"));
        assert_eq!(inbox.from.email, "ann@example.com");
        assert_eq!(inbox.recipients.to[0].email, "cy@example.com");
        assert_eq!(inbox.date, 1_709_458_860_000);
        assert_eq!(inbox.attachments.len(), 1);

        let archived = remote(&message("arc"), &labels, &roles).unwrap();
        assert_eq!(archived.labels, [work.to_string()]);
        assert!(remote(&message("out"), &labels, &roles).is_none());
    }

    #[test]
    fn newsletters_are_bulk() {
        let message = json!({
            "id": "m", "inferenceClassification": "other",
            "unsubscribeData": ["<mailto:leave@list.org>", "<https://list.org/leave>"],
        });
        let newsletter = remote(&message, &HashMap::new(), &HashMap::new()).unwrap();
        assert!(newsletter.bulk);
        let unsubscribe = newsletter.unsubscribe.unwrap();
        assert_eq!(unsubscribe.url.as_deref(), Some("https://list.org/leave"));
        assert_eq!(unsubscribe.mailto.as_deref(), Some("mailto:leave@list.org"));
    }
}
