//! What the core answers, as `crates/core/src/api.rs` writes it.

use std::collections::BTreeMap;

pub use mail_core::api::{Action, Filter, UiState};
pub use mail_core::render::drafts::ReplyKind;
pub use mail_protocol::{Address, Attachment, Draft, DraftAttachment, Identity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ThreadRow {
    pub id: String,
    pub account_id: String,
    pub color: String,
    pub senders: String,
    pub subject: String,
    pub snippet: String,
    pub date: String,
    pub timestamp: i64,
    pub unread: bool,
    pub starred: bool,
    pub attachment: bool,
    pub snoozed: bool,
    #[serde(default)]
    pub draft_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mailbox {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub unread: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AccountView {
    pub id: String,
    pub address: String,
    pub name: String,
    pub title: String,
    pub provider: String,
    pub color: String,
    pub status: String,
    pub identities: Vec<Identity>,
    pub mailboxes: Vec<Mailbox>,
}

impl AccountView {
    /// The longest name the core takes.
    pub const NAME_MAX: usize = 40;
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mailboxes {
    pub unified: Vec<Mailbox>,
    pub accounts: Vec<AccountView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ThreadPage {
    pub rows: Vec<ThreadRow>,
    pub total: usize,
    pub splits: Vec<SplitTab>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SplitTab {
    pub mailbox: String,
    pub name: String,
    pub unread: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MessageItem {
    pub id: String,
    pub from_name: String,
    pub from_email: String,
    pub initials: String,
    pub to: String,
    pub date: String,
    pub snippet: String,
    pub unread: bool,
    pub folded: bool,
    pub html: Option<String>,
    pub shape: Vec<ShapeLine>,
    pub failed: bool,
    pub blocked_images: bool,
    pub designed: bool,
    pub attachments: Vec<Attachment>,
}

/// A line of a page's placeholder: text of so many characters, or an image.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeLine {
    Text(u32),
    Image { width: u32, height: u32 },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub account_id: String,
    pub color: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub starred: bool,
    pub unread: bool,
    pub muted: bool,
    pub labels: Vec<LabelChip>,
    pub unsubscribe: bool,
    pub draft_id: Option<String>,
    pub person: Option<String>,
    pub messages: Vec<MessageItem>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LabelChip {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftReply {
    #[serde(default)]
    pub id: Option<String>,
    pub draft: Draft,
    pub from: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActReply {
    pub message: Option<String>,
    pub undo: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageReply {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdReply {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathReply {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AvatarReply {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HtmlReply {
    pub html: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrlReply {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimeChoice {
    pub id: String,
    pub name: String,
    pub label: String,
    /// Unix milliseconds.
    pub until: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeChoices {
    pub choices: Vec<TimeChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContactList {
    pub contacts: Vec<Address>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Person {
    pub name: String,
    pub email: String,
    pub initials: String,
    pub threads: Vec<ThreadRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreferenceList {
    pub values: Vec<PreferenceValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreferenceValue {
    pub key: String,
    pub value: Value,
}

impl PreferenceList {
    pub fn map(self) -> BTreeMap<String, Value> {
        self.values.into_iter().map(|value| (value.key, value.value)).collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    pub signed_in: bool,
    pub server: Option<String>,
    pub connection: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Boot {
    pub status: Status,
    pub mailboxes: Mailboxes,
    pub page: ThreadPage,
    pub thread: Option<Conversation>,
    pub search: Option<Vec<ThreadRow>>,
    pub preferences: PreferenceList,
    pub ui: UiState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendReply {
    pub op_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CancelReply {
    pub draft: Draft,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchReply {
    pub request: u64,
    pub rows: Vec<ThreadRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Empty {}

/// What a draft being written holds beyond the draft, kept with the UI state so a restart
/// brings it back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Typing {
    pub to: String,
    pub cc: String,
    pub bcc: String,
}

/// A message being written, as the UI state keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComposeState {
    pub draft: Draft,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub show_cc: bool,
    #[serde(default)]
    pub draft_id: Option<String>,
    /// The thread it answers, to archive with sending.
    #[serde(default)]
    pub thread: Option<String>,
    #[serde(default)]
    pub remind_at: Option<SavedTime>,
    /// Closing keeps it as a draft even unchanged: a message brought back from sending.
    #[serde(default)]
    pub keep: bool,
    #[serde(default)]
    pub typing: Option<Typing>,
}

/// A `TimeChoice` as the UI state keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTime {
    pub id: String,
    pub name: String,
    pub label: String,
    pub until: i64,
}

impl From<TimeChoice> for SavedTime {
    fn from(choice: TimeChoice) -> Self {
        Self { id: choice.id, name: choice.name, label: choice.label, until: choice.until }
    }
}

impl ComposeState {
    pub fn new(draft: Draft, from: String) -> Self {
        let show_cc = !draft.cc.is_empty() || !draft.bcc.is_empty();
        Self { draft, from, show_cc, draft_id: None, thread: None, remind_at: None, keep: false, typing: None }
    }

    pub fn has_content(&self) -> bool {
        let draft = &self.draft;
        !draft.to.is_empty()
            || !draft.cc.is_empty()
            || !draft.bcc.is_empty()
            || !draft.subject.is_empty()
            || !draft.text.is_empty()
            || !draft.attachments.is_empty()
    }
}

/// A new message, for a notification.
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
pub struct NewMailItem {
    pub thread: String,
    pub from: String,
    pub email: String,
    pub subject: String,
    pub snippet: String,
}

/// The answer to `UnreadInInbox`.
#[derive(Debug, Clone, Deserialize)]
pub struct ThreadIds {
    pub threads: Vec<String>,
}

/// Reads "Ann <ann@example.com>" or a bare address.
pub fn parse_address(text: &str) -> Option<Address> {
    let text = text.trim();
    if let (Some(open), Some(close)) = (text.rfind('<'), text.rfind('>'))
        && open < close
    {
        let email = text[open + 1..close].trim();
        let name = text[..open].trim().trim_matches('"').trim();
        let name = (!name.is_empty()).then(|| name.to_string());
        return email.contains('@').then(|| Address { name, email: email.to_string() });
    }
    (text.contains('@') && !text.contains(' ')).then(|| Address { name: None, email: text.to_string() })
}

/// Typed or pasted text as addresses; what can't be read is kept as written, to show as wrong.
pub fn typed_addresses(text: &str) -> Vec<Address> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if let Some(address) = parse_address(text) {
        return vec![address];
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|word| word.contains('@')) {
        return words
            .iter()
            .map(|word| parse_address(word).unwrap_or(Address { name: None, email: word.to_string() }))
            .collect();
    }
    vec![Address { name: None, email: text.to_string() }]
}

/// Whether mail can be sent to the address.
pub fn is_valid(address: &Address) -> bool {
    let bad = |character: char| character.is_whitespace() || "<>,;\"".contains(character);
    let Some((local, domain)) = address.email.split_once('@') else { return false };
    let Some((host, top)) = domain.rsplit_once('.') else { return false };
    !local.is_empty() && !host.is_empty() && !top.is_empty() && !address.email.chars().any(bad) && !domain.contains('@')
}

/// The first word of the name, for "Hi {first_name}".
pub fn first_name(address: &Address) -> Option<String> {
    let first = address.name.as_ref()?.split(' ').next()?;
    (!first.contains('@') && !first.is_empty()).then(|| first.to_string())
}

/// The first letters of the first two words of the name, else the address's first letter.
pub fn initials(address: &Address) -> String {
    let letters: String = address
        .name
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if !letters.is_empty() {
        return letters;
    }
    address.email.chars().next().map(|first| first.to_uppercase().collect()).unwrap_or_else(|| "?".into())
}

pub fn address_text(address: &Address) -> String {
    match &address.name {
        Some(name) => format!("{name} <{}>", address.email),
        None => address.email.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_typed_addresses() {
        let found = typed_addresses("Ann Lee <ann@example.com>");
        assert_eq!(found, vec![Address { name: Some("Ann Lee".into()), email: "ann@example.com".into() }]);
        assert_eq!(typed_addresses("a@x.com b@y.com").len(), 2);
        assert!(!is_valid(&typed_addresses("not an address")[0]));
        assert!(is_valid(&typed_addresses("bob@example.org")[0]));
        assert_eq!(initials(&found[0]), "AL");
    }
}
