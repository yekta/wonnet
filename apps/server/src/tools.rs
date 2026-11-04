//! What a connected app does over MCP (`mcp.rs`): each tool takes its arguments and answers JSON,
//! or what to tell the app when it can't. A tool reaches only the accounts the caller's access
//! does; mail elsewhere isn't there. Changes go through `ops::apply`, as the apps' do.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use mail_protocol::rules::RULE_KEY;
use mail_protocol::{MessageState, Op, Rule, role};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::Json;
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::access::{Caller, Privilege};
use crate::db::{ACCOUNT_COLUMNS, AccountRow, MESSAGE_COLUMNS, MessageRow};
use crate::ops::{self, Outcome};
use crate::{AppState, mime, search, sync, unsubscribe};

pub type Answer = Result<Value, String>;

const PAGE: u32 = 25;
const MOST: u32 = 100;
/// The most threads one `modify` changes; the answer says when more matched.
const MOST_THREADS: i64 = 5000;
const OP_SIZE: usize = 500;
const SAMPLE: usize = 10;
const THREAD_BODIES: usize = 20;
const BODY_CHARS: usize = 20_000;
const MOST_SENDERS: usize = 25;
const UNSUBSCRIBING_AT_ONCE: usize = 5;

fn failed(error: impl std::fmt::Display) -> String {
    tracing::error!("mcp tool: {error:#}");
    "The server couldn't do that. Try again in a moment.".into()
}

fn uuid(text: &str, what: &str) -> Result<Uuid, String> {
    text.trim().parse().map_err(|_| format!("{text:?} isn't a {what} id."))
}

/// The user's accounts the caller reaches, oldest first.
async fn reached(state: &AppState, caller: &Caller) -> Result<Vec<AccountRow>, String> {
    let rows: Vec<AccountRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE user_id = $1 AND NOT deleted ORDER BY created_at"
    )))
    .bind(caller.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(failed)?;
    Ok(rows.into_iter().filter(|account| caller.access.reaches(account.id)).collect())
}

/// The reached accounts a tool looks in: all of them, or the one asked for.
fn chosen(accounts: &[AccountRow], account_id: Option<&str>) -> Result<Vec<Uuid>, String> {
    let all = accounts.iter().map(|account| account.id).collect();
    let Some(account_id) = account_id.filter(|id| !id.trim().is_empty()) else { return Ok(all) };
    let id = uuid(account_id, "account")?;
    match accounts.iter().any(|account| account.id == id) {
        true => Ok(vec![id]),
        false => Err("No such account. list_accounts lists them.".into()),
    }
}

fn own_addresses(accounts: &[AccountRow]) -> Vec<String> {
    let all: HashSet<String> = accounts.iter().flat_map(AccountRow::own_addresses).collect();
    all.into_iter().collect()
}

#[derive(sqlx::FromRow)]
struct LabelName {
    id: Uuid,
    account_id: Uuid,
    name: String,
}

async fn labels_of(state: &AppState, accounts: &[Uuid]) -> Result<Vec<LabelName>, String> {
    sqlx::query_as("SELECT id, account_id, name FROM labels WHERE account_id = ANY($1) AND NOT deleted ORDER BY name")
        .bind(accounts)
        .fetch_all(&state.db)
        .await
        .map_err(failed)
}

fn names(labels: &[LabelName]) -> HashMap<String, String> {
    labels.iter().map(|label| (label.id.to_string(), label.name.clone())).collect()
}

fn unsubscribe_way(row: &MessageRow) -> Option<&'static str> {
    let way = &row.unsubscribe.as_ref()?.0;
    match (&way.url, &way.mailto) {
        (Some(_), _) if way.one_click => Some("one_click"),
        (_, Some(_)) => Some("email"),
        (Some(_), None) => Some("web"),
        (None, None) => None,
    }
}

fn message_json(row: &MessageRow, labels: &HashMap<String, String>) -> Value {
    let label_names: Vec<&str> =
        row.labels.iter().map(|label| labels.get(label).map(String::as_str).unwrap_or(label)).collect();
    let emails = |addresses: &[mail_protocol::Address]| -> Vec<String> {
        addresses.iter().map(|address| address.email.clone()).collect()
    };
    let mut message = json!({
        "id": row.id,
        "account_id": row.account_id,
        "thread_id": row.thread_id,
        "date": row.date.to_rfc3339(),
        "from": { "name": row.from_name, "email": row.from_email },
        "to": emails(&row.recipients.to),
        "subject": row.subject,
        "snippet": row.snippet,
        "unread": row.unread,
        "starred": row.starred,
        "labels": label_names,
        "bulk": row.bulk,
        "unsubscribe": unsubscribe_way(row),
        "attachments": row.attachments.len(),
    });
    if !row.recipients.cc.is_empty() {
        message["cc"] = json!(emails(&row.recipients.cc));
    }
    if let Some(until) = row.snoozed_until {
        message["snoozed_until"] = json!(until.to_rfc3339());
    }
    message
}

/// What the agent narrows mail by beyond the search's words and operators.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Filters {
    /// true: only mail sent to a list or by a machine (newsletters, notifications, receipts).
    /// false: only mail from people.
    pub bulk: Option<bool>,
    /// true: only mail that says how to unsubscribe from it.
    pub unsubscribable: Option<bool>,
    /// true: only threads Split Inbox puts in Important: someone other than the user wrote in
    /// them, and not only to a list. The user's own splits aside.
    pub important: Option<bool>,
    /// true: only threads whose newest message isn't the user's.
    pub unanswered: Option<bool>,
}

/// `SELECT {select} FROM messages` narrowed to the caller's accounts, the search and the filters.
fn matching(
    select: &str,
    caller: &Caller,
    accounts: Vec<Uuid>,
    me: Vec<String>,
    text: &str,
    filters: &Filters,
) -> QueryBuilder<Postgres> {
    let mut sql = QueryBuilder::new(format!("SELECT {select} FROM messages WHERE NOT deleted AND user_id = "));
    sql.push_bind(caller.user_id).push(" AND account_id = ANY(").push_bind(accounts).push(")");
    search::narrow(&mut sql, caller.user_id, search::parse(text, Utc::now()));
    if let Some(bulk) = filters.bulk {
        sql.push(if bulk { " AND bulk" } else { " AND NOT bulk" });
    }
    if let Some(unsubscribable) = filters.unsubscribable {
        sql.push(if unsubscribable { " AND unsubscribe IS NOT NULL" } else { " AND unsubscribe IS NULL" });
    }
    let others = " FROM messages other WHERE other.account_id = messages.account_id
        AND other.thread_id = messages.thread_id AND NOT other.deleted AND NOT (other.from_email = ANY(";
    if let Some(important) = filters.important {
        sql.push(if important { " AND (NOT EXISTS (SELECT 1" } else { " AND NOT (NOT EXISTS (SELECT 1" });
        sql.push(others).push_bind(me.clone()).push("))) OR EXISTS (SELECT 1").push(others).push_bind(me.clone());
        sql.push(")) AND NOT other.bulk))");
    }
    if let Some(unanswered) = filters.unanswered {
        sql.push(
            " AND (SELECT newest.from_email FROM messages newest WHERE newest.account_id = messages.account_id
            AND newest.thread_id = messages.thread_id AND NOT newest.deleted ORDER BY newest.date DESC LIMIT 1)",
        );
        sql.push(if unanswered { " <> ALL(" } else { " = ANY(" }).push_bind(me).push(")");
    }
    sql
}

// ---------- reading ----------

pub async fn list_accounts(state: &AppState, caller: &Caller) -> Answer {
    let accounts = reached(state, caller).await?;
    let access = &caller.access;
    Ok(json!({
        "access": {
            "role": access.role.as_str(),
            "all_accounts": access.all_accounts,
            "privileges": access.privileges.iter().map(|privilege| privilege.as_str()).collect::<Vec<_>>(),
        },
        "accounts": accounts.iter().map(|account| json!({
            "id": account.id,
            "address": account.address,
            "name": account.name,
            "provider": account.provider,
            "status": account.status,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AccountArgs {
    /// Only this account's. Every account the connection reaches when left out.
    pub account_id: Option<String>,
}

pub async fn list_labels(state: &AppState, caller: &Caller, args: AccountArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let accounts = chosen(&accounts, args.account_id.as_deref())?;
    let labels = labels_of(state, &accounts).await?;
    let labels: Vec<Value> = labels
        .iter()
        .map(|label| json!({ "id": label.id, "account_id": label.account_id, "name": label.name }))
        .collect();
    Ok(json!({ "labels": labels }))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// Gmail's search: words, "quoted phrases", -word, OR, from:, to:, subject:, has:attachment,
    /// is:unread, is:starred, in:inbox|sent|drafts|trash|spam|archive|snoozed, label:<name>,
    /// before:/after: YYYY/MM/DD, newer_than:/older_than: 2d, 3w, 1m, 1y. Empty matches all mail.
    #[serde(default)]
    pub query: String,
    /// Only this account's mail.
    pub account_id: Option<String>,
    #[serde(flatten)]
    pub filters: Filters,
    /// How many messages, newest first: 25 when left out, at most 100.
    pub limit: Option<u32>,
    /// The next_cursor of the page before, to go on after it.
    pub cursor: Option<String>,
}

fn read_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid), String> {
    let wrong = || "That cursor isn't one search gave.".to_string();
    let (millis, id) = cursor.split_once('_').ok_or_else(wrong)?;
    let millis: i64 = millis.parse().map_err(|_| wrong())?;
    Ok((crate::db::from_millis(millis), id.parse().map_err(|_| wrong())?))
}

pub async fn search(state: &AppState, caller: &Caller, args: SearchArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let me = own_addresses(&accounts);
    let ids = chosen(&accounts, args.account_id.as_deref())?;
    let limit = args.limit.unwrap_or(PAGE).clamp(1, MOST);
    let mut sql = matching(MESSAGE_COLUMNS, caller, ids.clone(), me, &args.query, &args.filters);
    if let Some(cursor) = args.cursor.as_deref().filter(|cursor| !cursor.is_empty()) {
        let (date, id) = read_cursor(cursor)?;
        sql.push(" AND (date, id) < (").push_bind(date).push(", ").push_bind(id).push(")");
    }
    sql.push(" ORDER BY date DESC, id DESC LIMIT ").push_bind(i64::from(limit) + 1);
    let mut rows: Vec<MessageRow> = sql.build_query_as().fetch_all(&state.db).await.map_err(failed)?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let labels = names(&labels_of(state, &ids).await?);
    let next = rows.last().filter(|_| more).map(|row| format!("{}_{}", crate::db::millis(row.date), row.id));
    Ok(json!({
        "messages": rows.iter().map(|row| message_json(row, &labels)).collect::<Vec<_>>(),
        "next_cursor": next,
    }))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SendersArgs {
    /// A search, as `search` takes it, whose mail is grouped. Empty groups all mail.
    #[serde(default)]
    pub query: String,
    pub account_id: Option<String>,
    #[serde(flatten)]
    pub filters: Filters,
    /// `sender` (the default) groups by address, `domain` by what comes after the @.
    pub group_by: Option<GroupBy>,
    /// How many senders, most mail first: 50 when left out, at most 100.
    pub limit: Option<u32>,
    /// How many to skip, for the next page.
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Sender,
    Domain,
}

#[derive(sqlx::FromRow)]
struct SenderRow {
    sender: String,
    name: Option<String>,
    messages: i64,
    unread: i64,
    in_inbox: i64,
    newest: DateTime<Utc>,
    bulk: bool,
    unsubscribe: Option<Json<mail_protocol::Unsubscribe>>,
    subjects: Vec<String>,
}

pub async fn list_senders(state: &AppState, caller: &Caller, args: SendersArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let me = own_addresses(&accounts);
    let ids = chosen(&accounts, args.account_id.as_deref())?;
    let limit = args.limit.unwrap_or(50).clamp(1, MOST);
    let offset = args.offset.unwrap_or(0);
    let key = match args.group_by.unwrap_or(GroupBy::Sender) {
        GroupBy::Sender => "from_email",
        GroupBy::Domain => "split_part(from_email, '@', 2)",
    };
    let select = format!(
        "{key} AS sender,
         (array_agg(from_name ORDER BY date DESC) FILTER (WHERE from_name IS NOT NULL))[1] AS name,
         count(*) AS messages, count(*) FILTER (WHERE unread) AS unread,
         count(*) FILTER (WHERE 'inbox' = ANY(labels)) AS in_inbox, max(date) AS newest, bool_and(bulk) AS bulk,
         (array_agg(unsubscribe ORDER BY date DESC) FILTER (WHERE unsubscribe IS NOT NULL))[1] AS unsubscribe,
         (array_agg(subject ORDER BY date DESC))[1:3] AS subjects"
    );
    let mut sql = matching(&select, caller, ids, me.clone(), &args.query, &args.filters);
    sql.push(" AND from_email <> '' AND NOT (from_email = ANY(").push_bind(me).push("))");
    sql.push(" GROUP BY 1 ORDER BY messages DESC, sender LIMIT ").push_bind(i64::from(limit) + 1);
    sql.push(" OFFSET ").push_bind(i64::from(offset));
    let mut rows: Vec<SenderRow> = sql.build_query_as().fetch_all(&state.db).await.map_err(failed)?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let senders: Vec<Value> = rows
        .iter()
        .map(|row| {
            let way = row.unsubscribe.as_ref().map(|way| match (&way.url, &way.mailto) {
                (Some(_), _) if way.one_click => "one_click",
                (_, Some(_)) => "email",
                _ => "web",
            });
            json!({
                "sender": row.sender,
                "name": row.name,
                "messages": row.messages,
                "unread": row.unread,
                "in_inbox": row.in_inbox,
                "newest": row.newest.to_rfc3339(),
                "bulk": row.bulk,
                "unsubscribe": way,
                "subjects": row.subjects,
            })
        })
        .collect();
    let next = more.then(|| offset + limit);
    Ok(json!({ "senders": senders, "next_offset": next }))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ThreadArgs {
    /// Any message of the thread.
    pub message_id: String,
}

async fn message_reached(state: &AppState, caller: &Caller, id: &str) -> Result<MessageRow, String> {
    let id = uuid(id, "message")?;
    let row = crate::db::message(&state.db, caller.user_id, id).await.map_err(failed)?;
    row.filter(|row| !row.deleted && caller.access.reaches(row.account_id)).ok_or_else(|| "No such message.".into())
}

async fn thread_rows(
    state: &AppState,
    caller: &Caller,
    account_id: Uuid,
    thread_id: &str,
) -> Result<Vec<MessageRow>, String> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE user_id = $1 AND account_id = $2 AND thread_id = $3 AND NOT deleted ORDER BY date"
    )))
    .bind(caller.user_id)
    .bind(account_id)
    .bind(thread_id)
    .fetch_all(&state.db)
    .await
    .map_err(failed)
}

pub async fn get_thread(state: &AppState, caller: &Caller, args: ThreadArgs) -> Answer {
    let message = message_reached(state, caller, &args.message_id).await?;
    let rows = thread_rows(state, caller, message.account_id, &message.thread_id).await?;
    let labels = names(&labels_of(state, &[message.account_id]).await?);
    let reads = caller.access.has(Privilege::ReadBodies);
    let first_body = rows.len().saturating_sub(THREAD_BODIES);
    let mut messages = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let mut json = message_json(row, &labels);
        if reads && index >= first_body {
            match sync::body(state, caller.user_id, &row.id.to_string()).await {
                Ok(body) => {
                    let text = mime::plain(&body);
                    let truncated = text.chars().count() > BODY_CHARS;
                    json["body"] = json!(text.chars().take(BODY_CHARS).collect::<String>());
                    json["body_truncated"] = json!(truncated);
                }
                Err(error) => {
                    tracing::warn!("mcp: couldn't fetch a body: {error:#}");
                    json["body_error"] = json!("The message couldn't be loaded from its provider.");
                }
            }
        }
        messages.push(json);
    }
    let mut thread = json!({
        "account_id": message.account_id,
        "thread_id": message.thread_id,
        "subject": rows.first().map(|row| row.subject.as_str()).unwrap_or_default(),
        "messages": messages,
    });
    if !reads {
        thread["note"] = json!("Bodies need the read_bodies privilege; these are the snippets.");
    }
    Ok(thread)
}

// ---------- changing ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Archive,
    /// Back to the inbox from the archive, the trash or spam.
    Inbox,
    Trash,
    Spam,
    Read,
    Unread,
    Star,
    Unstar,
    AddLabel,
    RemoveLabel,
    /// Out of the inbox until `until`.
    Snooze,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ModifyArgs {
    pub action: Action,
    /// Messages to act on. Each one's whole thread is changed, as the apps do.
    #[serde(default)]
    pub message_ids: Vec<String>,
    /// Or a search, as `search` takes it: every thread it matches is changed.
    pub query: Option<String>,
    pub account_id: Option<String>,
    #[serde(flatten)]
    pub filters: Filters,
    /// For add_label and remove_label: the label's id, or its name in each account.
    pub label: Option<String>,
    /// For snooze: when the threads come back, RFC 3339.
    pub until: Option<String>,
    /// With a query: false, the default, only counts the threads it would change and shows some.
    #[serde(default)]
    pub confirm: bool,
}

/// The ops `action` takes for these threads' messages, each thread as the apps act on it.
fn ops_for(
    action: Action,
    threads: &BTreeMap<(Uuid, String), Vec<MessageRow>>,
    labels: &HashMap<Uuid, Uuid>,
    until: i64,
) -> Vec<Op> {
    let mut ids: Vec<String> = Vec::new();
    let mut by_label: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
    for ((account_id, _), messages) in threads {
        let has = |message: &MessageRow, label: &str| message.labels.iter().any(|existing| existing == label);
        let pick = |test: &dyn Fn(&MessageRow) -> bool| -> Vec<String> {
            messages.iter().filter(|message| test(message)).map(|message| message.id.to_string()).collect()
        };
        let newest: Vec<String> = messages.last().map(|message| vec![message.id.to_string()]).unwrap_or_default();
        match action {
            Action::Archive | Action::Snooze => ids.extend(pick(&|message| has(message, role::INBOX))),
            Action::Trash => ids.extend(pick(&|message| !has(message, role::TRASH))),
            Action::Spam => ids.extend(pick(&|message| !has(message, role::SPAM))),
            Action::Read => ids.extend(pick(&|message| message.unread)),
            Action::Unread | Action::Star => ids.extend(newest),
            Action::Unstar => ids.extend(pick(&|message| message.starred)),
            Action::Inbox => ids.extend(pick(&|message| {
                !has(message, role::INBOX) && !has(message, role::SENT) && !has(message, role::DRAFTS)
            })),
            Action::AddLabel | Action::RemoveLabel => {
                let Some(label) = labels.get(account_id) else { continue };
                let label_text = label.to_string();
                let wanted = action == Action::AddLabel;
                by_label.entry(*label).or_default().extend(pick(&|message| has(message, &label_text) != wanted));
            }
        }
    }
    let chunks = |ids: Vec<String>, make: &dyn Fn(Vec<String>) -> Op| -> Vec<Op> {
        ids.chunks(OP_SIZE).map(|chunk| make(chunk.to_vec())).collect()
    };
    match action {
        Action::Archive => chunks(ids, &|ids| Op::Archive { ids }),
        Action::Inbox => chunks(ids, &|ids| Op::MoveToInbox { ids }),
        Action::Trash => chunks(ids, &|ids| Op::Trash { ids }),
        Action::Spam => chunks(ids, &|ids| Op::Spam { ids }),
        Action::Read => chunks(ids, &|ids| Op::SetUnread { ids, unread: false }),
        Action::Unread => chunks(ids, &|ids| Op::SetUnread { ids, unread: true }),
        Action::Star => chunks(ids, &|ids| Op::SetStarred { ids, starred: true }),
        Action::Unstar => chunks(ids, &|ids| Op::SetStarred { ids, starred: false }),
        Action::Snooze => chunks(ids, &|ids| Op::Snooze { ids, until }),
        Action::AddLabel | Action::RemoveLabel => by_label
            .into_iter()
            .flat_map(|(label, ids)| {
                let label = label.to_string();
                chunks(ids, &|ids| match action {
                    Action::AddLabel => Op::AddLabel { ids, label: label.clone() },
                    _ => Op::RemoveLabel { ids, label: label.clone() },
                })
            })
            .collect(),
    }
}

/// Applies the ops; the count is of the messages they changed.
async fn apply_all(state: &AppState, caller: &Caller, ops: Vec<Op>) -> Result<usize, String> {
    let mut changed = 0;
    for op in ops {
        let count = op.ids().len();
        match ops::apply(state, caller.user_id, &Uuid::new_v4().to_string(), op).await.map_err(failed)? {
            Outcome::Done { ok: true, .. } => changed += count,
            Outcome::Done { error, .. } => return Err(error.unwrap_or_else(|| "It couldn't be changed.".into())),
            Outcome::Waiting => {}
        }
    }
    Ok(changed)
}

/// Every message of these threads, by thread, oldest first in each.
async fn threads_of(
    state: &AppState,
    caller: &Caller,
    keys: Vec<(Uuid, String)>,
) -> Result<BTreeMap<(Uuid, String), Vec<MessageRow>>, String> {
    let (accounts, threads): (Vec<Uuid>, Vec<String>) = keys.into_iter().unzip();
    let rows: Vec<MessageRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE user_id = $1 AND NOT deleted
             AND (account_id, thread_id) IN (SELECT * FROM unnest($2::uuid[], $3::text[]))
         ORDER BY date"
    )))
    .bind(caller.user_id)
    .bind(&accounts)
    .bind(&threads)
    .fetch_all(&state.db)
    .await
    .map_err(failed)?;
    let mut by_thread: BTreeMap<(Uuid, String), Vec<MessageRow>> = BTreeMap::new();
    for row in rows {
        by_thread.entry((row.account_id, row.thread_id.clone())).or_default().push(row);
    }
    Ok(by_thread)
}

/// The label to use in each account, from an id or a name.
fn label_per_account(label: &str, labels: &[LabelName]) -> Result<HashMap<Uuid, Uuid>, String> {
    if let Ok(id) = label.trim().parse::<Uuid>() {
        let found = labels.iter().find(|known| known.id == id).ok_or("No such label. list_labels lists them.")?;
        return Ok(HashMap::from([(found.account_id, found.id)]));
    }
    let per_account: HashMap<Uuid, Uuid> = labels
        .iter()
        .filter(|known| known.name.eq_ignore_ascii_case(label.trim()))
        .map(|known| (known.account_id, known.id))
        .collect();
    match per_account.is_empty() {
        true => Err(format!("No account has a label named {label:?}. create_label makes one.")),
        false => Ok(per_account),
    }
}

pub async fn modify(state: &AppState, caller: &Caller, args: ModifyArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let ids = chosen(&accounts, args.account_id.as_deref())?;
    let me = own_addresses(&accounts);
    let labels = match (args.action, &args.label) {
        (Action::AddLabel | Action::RemoveLabel, None) => return Err("Say which label.".into()),
        (Action::AddLabel | Action::RemoveLabel, Some(label)) => {
            label_per_account(label, &labels_of(state, &ids).await?)?
        }
        _ => HashMap::new(),
    };
    let until = match (args.action, &args.until) {
        (Action::Snooze, None) => return Err("Say until when, as RFC 3339.".into()),
        (Action::Snooze, Some(until)) => DateTime::parse_from_rfc3339(until)
            .map_err(|_| format!("{until:?} isn't an RFC 3339 time."))?
            .timestamp_millis(),
        _ => 0,
    };
    if until != 0 && until <= Utc::now().timestamp_millis() {
        return Err("Snooze until a time to come.".into());
    }
    let (keys, more) = match &args.query {
        Some(query) if args.message_ids.is_empty() => {
            let mut sql = matching("DISTINCT account_id, thread_id", caller, ids.clone(), me, query, &args.filters);
            sql.push(" LIMIT ").push_bind(MOST_THREADS + 1);
            let mut keys: Vec<(Uuid, String)> = sql.build_query_as().fetch_all(&state.db).await.map_err(failed)?;
            let more = keys.len() as i64 > MOST_THREADS;
            keys.truncate(MOST_THREADS as usize);
            (keys, more)
        }
        Some(_) => return Err("Give message_ids or a query, not both.".into()),
        None if args.message_ids.is_empty() => return Err("Give message_ids or a query.".into()),
        None => {
            let mut keys = Vec::new();
            for id in &args.message_ids {
                let message = message_reached(state, caller, id).await?;
                if ids.contains(&message.account_id) {
                    keys.push((message.account_id, message.thread_id));
                }
            }
            (keys, false)
        }
    };
    let threads = threads_of(state, caller, keys).await?;
    if args.query.is_some() && !args.confirm {
        let all_labels = names(&labels_of(state, &ids).await?);
        let mut newest: Vec<&MessageRow> = threads.values().filter_map(|messages| messages.last()).collect();
        newest.sort_by_key(|message| std::cmp::Reverse(message.date));
        let sample: Vec<Value> = newest.iter().take(SAMPLE).map(|row| message_json(row, &all_labels)).collect();
        return Ok(json!({
            "would_change_threads": threads.len(),
            "more_than_that": more,
            "sample": sample,
            "note": "Nothing was changed. Call again with confirm: true to change them.",
        }));
    }
    let ops = ops_for(args.action, &threads, &labels, until);
    let changed = apply_all(state, caller, ops).await?;
    tracing::info!(
        "mcp: {} changed {changed} messages for user {} ({:?})",
        caller.client_name,
        caller.user_id,
        args.action
    );
    let mut answer = json!({ "changed_threads": threads.len(), "changed_messages": changed });
    if more {
        answer["note"] = json!(format!("More than {MOST_THREADS} threads matched. Run it again for the rest."));
    }
    Ok(answer)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateLabelArgs {
    pub account_id: String,
    pub name: String,
}

pub async fn create_label(state: &AppState, caller: &Caller, args: CreateLabelArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let [account_id] = chosen(&accounts, Some(&args.account_id))?[..] else { return Err("No such account.".into()) };
    let label_id = Uuid::new_v4();
    let op =
        Op::CreateLabel { account_id: account_id.to_string(), label_id: label_id.to_string(), name: args.name.clone() };
    apply_all(state, caller, vec![op]).await?;
    Ok(json!({ "id": label_id, "account_id": account_id, "name": args.name.trim() }))
}

/// Settings that hold for every account need access to every account.
fn every_account(caller: &Caller) -> Result<(), String> {
    match caller.access.all_accounts {
        true => Ok(()),
        false => Err("This connection reaches only some accounts, and this holds for all of them.".into()),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlockArgs {
    /// An address, or @domain for everyone there.
    pub sender: String,
    /// false unblocks. true when left out.
    pub blocked: Option<bool>,
}

pub async fn block_sender(state: &AppState, caller: &Caller, args: BlockArgs) -> Answer {
    every_account(caller)?;
    let sender = args.sender.trim().to_lowercase();
    let valid = match sender.strip_prefix('@') {
        Some(domain) => domain.contains('.') && !domain.contains('@'),
        None => sender.split('@').count() == 2 && !sender.starts_with('@') && !sender.ends_with('@'),
    };
    if !valid || sender.chars().any(char::is_whitespace) {
        return Err(format!("{:?} isn't an address or an @domain.", args.sender));
    }
    let blocked = args.blocked.unwrap_or(true);
    let value = blocked.then_some(json!(true));
    let op = Op::SetPreference { key: format!("blocked:{sender}"), value };
    apply_all(state, caller, vec![op]).await?;
    Ok(
        json!({ "sender": sender, "blocked": blocked, "note": "New mail from it goes to the trash. Mail already here stays where it is." }),
    )
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MuteArgs {
    /// Any message of the thread.
    pub message_id: String,
    /// false unmutes and brings the thread back to the inbox. true when left out.
    pub muted: Option<bool>,
}

pub async fn mute_thread(state: &AppState, caller: &Caller, args: MuteArgs) -> Answer {
    let message = message_reached(state, caller, &args.message_id).await?;
    let muted = args.muted.unwrap_or(true);
    let key = format!("muted:{}:{}", message.account_id, message.thread_id);
    let threads = threads_of(state, caller, vec![(message.account_id, message.thread_id.clone())]).await?;
    let mut ops = vec![Op::SetPreference { key, value: muted.then_some(json!(true)) }];
    ops.extend(ops_for(if muted { Action::Archive } else { Action::Inbox }, &threads, &HashMap::new(), 0));
    apply_all(state, caller, ops).await?;
    Ok(json!({ "thread_id": message.thread_id, "muted": muted }))
}

// ---------- rules ----------

#[derive(sqlx::FromRow)]
struct RuleRow {
    key: String,
    value: Value,
}

/// The rules the caller may see: those for a label of a reached account, and with every account,
/// those that hold for all mail.
async fn rules(state: &AppState, caller: &Caller, labels: &[LabelName]) -> Result<Vec<(String, Rule)>, String> {
    let rows: Vec<RuleRow> =
        sqlx::query_as("SELECT key, value FROM preferences WHERE user_id = $1 AND key LIKE 'rule:%' AND NOT deleted")
            .bind(caller.user_id)
            .fetch_all(&state.db)
            .await
            .map_err(failed)?;
    let mut found: Vec<(String, Rule)> = rows
        .into_iter()
        .filter_map(|row| Some((row.key.strip_prefix(RULE_KEY)?.to_string(), serde_json::from_value(row.value).ok()?)))
        .filter(|(_, rule): &(String, Rule)| match &rule.label {
            Some(label) => labels.iter().any(|known| known.id.to_string() == *label),
            None => caller.access.all_accounts,
        })
        .collect();
    found.sort_by_key(|(_, rule)| rule.order);
    Ok(found)
}

fn rule_json(id: &str, rule: &Rule, labels: &[LabelName]) -> Value {
    let label = rule.label.as_ref().and_then(|label| labels.iter().find(|known| known.id.to_string() == *label));
    json!({
        "id": id,
        "from": rule.from,
        "to": rule.to,
        "subject": rule.subject,
        "label": label.map(|label| json!({ "id": label.id, "account_id": label.account_id, "name": label.name })),
        "skip_inbox": rule.skip_inbox,
        "mark_read": rule.mark_read,
    })
}

pub async fn list_rules(state: &AppState, caller: &Caller) -> Answer {
    let accounts = reached(state, caller).await?;
    let ids: Vec<Uuid> = accounts.iter().map(|account| account.id).collect();
    let labels = labels_of(state, &ids).await?;
    let rules = rules(state, caller, &labels).await?;
    Ok(json!({ "rules": rules.iter().map(|(id, rule)| rule_json(id, rule, &labels)).collect::<Vec<_>>() }))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CreateRuleArgs {
    /// Senders: an address, or @domain for everyone there. Any one of them matches.
    #[serde(default)]
    pub from: Vec<String>,
    /// Recipients, To or Cc, as `from`.
    #[serde(default)]
    pub to: Vec<String>,
    /// Words the subject contains, case aside. Any one of them matches.
    #[serde(default)]
    pub subject: Vec<String>,
    /// The label to put on matching mail: its id, or its name with account_id. Then the rule holds
    /// for that account only; without a label it holds for every account.
    pub label: Option<String>,
    /// The account whose label `label` names.
    pub account_id: Option<String>,
    /// Take matching mail out of the inbox.
    #[serde(default)]
    pub skip_inbox: bool,
    #[serde(default)]
    pub mark_read: bool,
    /// Apply it to the mail already here too, not only to new mail.
    #[serde(default)]
    pub apply_to_existing: bool,
}

#[derive(sqlx::FromRow)]
struct RuleCandidate {
    id: Uuid,
    from_email: String,
    recipients: Json<mail_protocol::Recipients>,
    subject: String,
    labels: Vec<String>,
    unread: bool,
    starred: bool,
}

pub async fn create_rule(state: &AppState, caller: &Caller, args: CreateRuleArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let all: Vec<Uuid> = accounts.iter().map(|account| account.id).collect();
    let labels = labels_of(state, &all).await?;
    let label = match &args.label {
        None => {
            every_account(caller)?;
            None
        }
        Some(label) => {
            let in_accounts = chosen(&accounts, args.account_id.as_deref())?;
            let candidates: Vec<LabelName> = labels
                .iter()
                .filter(|known| in_accounts.contains(&known.account_id))
                .map(|known| LabelName { id: known.id, account_id: known.account_id, name: known.name.clone() })
                .collect();
            let per_account = label_per_account(label, &candidates)?;
            let [(account_id, label_id)] = per_account.into_iter().collect::<Vec<_>>()[..] else {
                return Err("More than one account has that label. Say which with account_id.".into());
            };
            Some((account_id, label_id))
        }
    };
    let existing = rules(state, caller, &labels).await?;
    let order = existing.iter().map(|(_, rule)| rule.order).max().map_or(0, |order| order + 1);
    let rule = Rule {
        from: args.from,
        to: args.to,
        subject: args.subject,
        label: label.map(|(_, label_id)| label_id.to_string()),
        skip_inbox: args.skip_inbox,
        mark_read: args.mark_read,
        order,
    };
    if !rule.does_something() {
        return Err("Give the rule something to do: a label, skip_inbox or mark_read.".into());
    }
    if [&rule.from, &rule.to, &rule.subject].iter().all(|entries| entries.iter().all(|entry| entry.trim().is_empty())) {
        return Err("Give the rule a sender, a recipient or subject words to match.".into());
    }
    let id = Uuid::new_v4().to_string();
    let op = Op::SetPreference { key: format!("{RULE_KEY}{id}"), value: Some(json!(rule)) };
    apply_all(state, caller, vec![op]).await?;
    let mut answer = json!({ "rule": rule_json(&id, &rule, &labels) });
    if args.apply_to_existing {
        let within = label.map_or(all, |(account_id, _)| vec![account_id]);
        let changed = apply_rule(state, caller, &rule, &accounts, within).await?;
        answer["changed_messages"] = json!(changed);
    }
    Ok(answer)
}

/// What the apps' "apply to the mail here" does: the user's own mail and Sent, Drafts, Spam and
/// Trash are left alone.
async fn apply_rule(
    state: &AppState,
    caller: &Caller,
    rule: &Rule,
    accounts: &[AccountRow],
    within: Vec<Uuid>,
) -> Result<usize, String> {
    let me: HashSet<String> = own_addresses(accounts).into_iter().collect();
    let rows: Vec<RuleCandidate> = sqlx::query_as(
        "SELECT id, from_email, recipients, subject, labels, unread, starred FROM messages
         WHERE user_id = $1 AND account_id = ANY($2) AND NOT deleted
             AND NOT labels && ARRAY['sent', 'drafts', 'spam', 'trash']",
    )
    .bind(caller.user_id)
    .bind(&within)
    .fetch_all(&state.db)
    .await
    .map_err(failed)?;
    let (mut labelled, mut archived, mut read) = (Vec::new(), Vec::new(), Vec::new());
    for row in rows {
        if me.contains(&row.from_email.to_lowercase()) {
            continue;
        }
        let to: Vec<&str> =
            row.recipients.to.iter().chain(&row.recipients.cc).map(|address| address.email.as_str()).collect();
        if !rule.matches(&row.from_email, &to, &row.subject) {
            continue;
        }
        let state = MessageState { labels: row.labels, unread: row.unread, starred: row.starred, snoozed_until: None };
        let id = row.id.to_string();
        if rule.label.as_ref().is_some_and(|label| !state.has(label)) {
            labelled.push(id.clone());
        }
        if rule.skip_inbox && state.has(role::INBOX) {
            archived.push(id.clone());
        }
        if rule.mark_read && state.unread {
            read.push(id);
        }
    }
    let mut ops = Vec::new();
    if let Some(label) = &rule.label {
        ops.extend(labelled.chunks(OP_SIZE).map(|ids| Op::AddLabel { ids: ids.to_vec(), label: label.clone() }));
    }
    ops.extend(archived.chunks(OP_SIZE).map(|ids| Op::Archive { ids: ids.to_vec() }));
    ops.extend(read.chunks(OP_SIZE).map(|ids| Op::SetUnread { ids: ids.to_vec(), unread: false }));
    apply_all(state, caller, ops).await
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RuleIdArgs {
    pub id: String,
}

pub async fn delete_rule(state: &AppState, caller: &Caller, args: RuleIdArgs) -> Answer {
    let accounts = reached(state, caller).await?;
    let ids: Vec<Uuid> = accounts.iter().map(|account| account.id).collect();
    let labels = labels_of(state, &ids).await?;
    let rules = rules(state, caller, &labels).await?;
    if !rules.iter().any(|(id, _)| *id == args.id) {
        return Err("No such rule. list_rules lists them.".into());
    }
    apply_all(state, caller, vec![Op::SetPreference { key: format!("{RULE_KEY}{}", args.id), value: None }]).await?;
    Ok(json!({ "deleted": args.id }))
}

// ---------- unsubscribing ----------

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Then {
    Archive,
    Trash,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnsubscribeArgs {
    /// Addresses to leave the lists of, as list_senders gives them. At most 25.
    pub senders: Vec<String>,
    /// Once unsubscribed, archive or trash each sender's mail too. Needs the organizer role.
    pub then: Option<Then>,
}

async fn unsubscribe_one(
    state: &AppState,
    caller: &Caller,
    accounts: &[Uuid],
    sender: String,
    then: Option<Then>,
) -> Value {
    let sender = sender.trim().to_lowercase();
    let newest: Result<Option<MessageRow>, sqlx::Error> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages WHERE user_id = $1 AND account_id = ANY($2) AND from_email = $3
             AND NOT deleted AND unsubscribe IS NOT NULL ORDER BY date DESC LIMIT 1"
    )))
    .bind(caller.user_id)
    .bind(accounts)
    .bind(&sender)
    .fetch_optional(&state.db)
    .await;
    let message = match newest {
        Ok(Some(message)) => message,
        Ok(None) => {
            return json!({ "sender": sender, "result": "no_way", "note": "No mail from it says how to unsubscribe." });
        }
        Err(error) => return json!({ "sender": sender, "result": "failed", "error": failed(error) }),
    };
    if unsubscribe_way(&message) == Some("web") {
        let url = message.unsubscribe.as_ref().and_then(|way| way.0.url.clone());
        return json!({ "sender": sender, "result": "web_page", "url": url, "note": "This list is left on its web page, which the user has to open." });
    }
    match unsubscribe::run(state, caller.user_id, &message.id.to_string()).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return json!({ "sender": sender, "result": "failed", "error": error }),
        Err(error) => return json!({ "sender": sender, "result": "failed", "error": failed(error) }),
    }
    let mut answer = json!({ "sender": sender, "result": "unsubscribed", "by": unsubscribe_way(&message) });
    let Some(then) = then else { return answer };
    let keys: Result<Vec<(Uuid, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT DISTINCT account_id, thread_id FROM messages
         WHERE user_id = $1 AND account_id = ANY($2) AND from_email = $3 AND NOT deleted",
    )
    .bind(caller.user_id)
    .bind(accounts)
    .bind(&sender)
    .fetch_all(&state.db)
    .await;
    let moved = async {
        let threads = threads_of(state, caller, keys.map_err(failed)?).await?;
        let action = match then {
            Then::Archive => Action::Archive,
            Then::Trash => Action::Trash,
        };
        apply_all(state, caller, ops_for(action, &threads, &HashMap::new(), 0)).await
    };
    match moved.await {
        Ok(count) => answer["moved_messages"] = json!(count),
        Err(error) => answer["move_error"] = json!(error),
    }
    answer
}

pub async fn unsubscribe(state: &AppState, caller: &Caller, args: UnsubscribeArgs) -> Answer {
    if args.then.is_some() && !caller.access.organizes() {
        return Err("Moving mail needs the organizer role. Leave out `then`.".into());
    }
    if args.senders.is_empty() || args.senders.len() > MOST_SENDERS {
        return Err(format!("Give 1 to {MOST_SENDERS} senders."));
    }
    let accounts: Vec<Uuid> = reached(state, caller).await?.iter().map(|account| account.id).collect();
    let results: Vec<Value> = futures_util::stream::iter(args.senders)
        .map(|sender| unsubscribe_one(state, caller, &accounts, sender, args.then))
        .buffered(UNSUBSCRIBING_AT_ONCE)
        .collect()
        .await;
    tracing::info!("mcp: {} unsubscribed user {} from {} senders", caller.client_name, caller.user_id, results.len());
    Ok(json!({ "results": results }))
}
