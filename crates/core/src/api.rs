//! The JSON the apps and the core exchange: commands in, events out. Each command's answer is
//! written beside it.
//!
//! Mailbox ids: `inbox`, `unread`, `starred`, `snoozed`, `sent`, `drafts`, `archive`, `spam`,
//! `trash` for every account together; `<account>/<mailbox>` for one account; `<account>/label/<label>` for a
//! label. With Split Inbox on, an inbox's splits are `inbox:important`, `inbox:other` and
//! `inbox:<split id>` (and `<account>/inbox:other`, ...); a thread is in one split only, and an
//! inbox's own id shows Important. `inbox:all` is the last tab, every thread of the inbox.
//!
//! The synced preferences (`Preferences`, `SetPreference`), by key:
//! - `split_inbox`: `true` splits the inbox into Important, the custom splits, and Other (mail
//!   that is `bulk`).
//! - `split:<id>`: a custom split, `{"name": "Team", "from": ["ann@x.com", "@x.com"],
//!   "label": "<label id>" | null, "order": 0}`. A thread is in it when a message's sender matches
//!   `from` (an address, or a domain written `@x.com`, which takes its subdomains too) or a
//!   message has `label` (`mail_protocol::Split`).
//! - `signature:<account id>`: text put under what is written from that account. Without one,
//!   the identity's own signature from the provider is used.
//! - `snippet:<id>`: `{"name": "Thanks", "text": "Thanks {first_name}!"}`.
//! - `blocked:<address or @domain>`: `true`. The server trashes new mail from it.
//! - `muted:<thread>`: `true`. The server archives new mail in it.
//! - `rule:<id>`: `{"from": ["@github.com"], "to": [], "subject": ["[dux-shell]"], "label":
//!   "<label id>" | null, "skip_inbox": true, "mark_read": false, "order": 0}`. The server applies
//!   it to new mail (`mail_protocol::Rule`); `ApplyRule` applies it to the mail already here.
//! - `notify:<account id>`: `false` stops notifications for that account.
//! - `remote_images`: `false` leaves the images a message loads from the web out until the user
//!   asks for them, so the sender doesn't learn the mail was opened. By default they are shown.
//! - `dark_mail`: `false` keeps designed mail as its sender made it in the dark scheme. By
//!   default its colours are turned to dark, and each page can be asked for the original.

use std::collections::BTreeMap;

use mail_protocol::access::Access;
use mail_protocol::api::Platform;
use mail_protocol::{Address, Attachment, Draft, Identity, Rule};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::render::drafts::ReplyKind;
use crate::render::html::Line;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Where the core keeps its database.
    pub data_dir: String,
    /// The server to use until the user sets another.
    #[serde(default)]
    pub server_url: Option<String>,
    /// Made-up mail instead of the server's, for looking at the apps.
    #[serde(default)]
    pub demo: bool,
}

/// Where the app is, kept on this device so the next start shows the same screen. Every field
/// has a default, so a state written by an older app still reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub mailbox: String,
    pub filter: Option<Filter>,
    /// The thread open on screen.
    pub thread: Option<String>,
    /// The row the keyboard is on.
    pub selected: Option<String>,
    /// How many rows the list had loaded, to load as many again.
    pub rows: usize,
    /// How far the list was scrolled, in points.
    pub list_offset: f64,
    /// How far the open thread was scrolled, in points; none until it is scrolled.
    pub thread_offset: Option<f64>,
    /// The threads picked to act on together.
    pub selection: Vec<String>,
    pub search: String,
    /// The compose sheet as the app keeps it, with what was typed.
    pub compose: Option<Value>,
    pub sidebar: bool,
    /// The open thread's messages unfolded by hand, and the one the keyboard is on.
    pub unfolded: Vec<String>,
    pub focused: Option<String>,
    /// Remote images were allowed in the open thread.
    pub images: bool,
    /// How tall each shown body of the open thread was, by message id, at `body_width` points
    /// wide: the cards open at their size before the pages are laid out again.
    pub heights: BTreeMap<String, f64>,
    pub body_width: f64,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mailbox: "inbox".into(),
            filter: None,
            thread: None,
            selected: None,
            rows: 0,
            list_offset: 0.0,
            thread_offset: None,
            selection: Vec::new(),
            search: String::new(),
            compose: None,
            sidebar: false,
            unfolded: Vec::new(),
            focused: None,
            images: false,
            heights: BTreeMap::new(),
            body_width: 0.0,
        }
    }
}

/// The answer to `Boot`: the `Status`, the `Mailboxes`, the `ThreadPage` of the saved mailbox,
/// the saved thread's `ThreadView` when it is still there, the local matches of the saved
/// search, the `Preferences` and the saved state itself.
#[derive(Debug, Clone, Serialize)]
pub struct Boot {
    pub status: Value,
    pub mailboxes: Value,
    pub page: ThreadPage,
    pub thread: Option<Value>,
    pub search: Option<Vec<ThreadRow>>,
    pub preferences: Preferences,
    pub ui: UiState,
}

#[derive(Debug, Deserialize)]
pub struct Envelope {
    pub id: u64,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Archive,
    Trash,
    Read,
    Unread,
    Star,
    Unstar,
    Inbox,
    Spam,
    /// Needs `until`. A thread that isn't in the inbox (one the user sent) comes back to it then:
    /// that is "remind me". A reply ends the snooze early.
    Snooze,
    /// Needs `label`.
    AddLabel,
    /// Needs `label`.
    RemoveLabel,
    /// Adds `label` and takes the thread out of the inbox.
    Move,
    /// Archives the thread and keeps later mail in it out of the inbox.
    Mute,
    Unmute,
    /// Leaves the list of the thread's newest message that has a way to. Answers `url` when the
    /// list only takes a visit to a web page, for the app to open.
    Unsubscribe,
    /// Blocks the sender of the thread's newest message not from the user, and trashes the thread.
    Block,
    /// Deletes every message of the thread for good. It can't be undone.
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Unread,
    Starred,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Status,
    /// A `Boot`: everything the app's first frame needs, in the state it was closed in.
    Boot,
    /// Keeps where the app is, to come back to it at the next start. `{}`.
    SaveUi {
        ui: UiState,
    },
    SetServer {
        url: String,
    },
    /// Answers with the URL to open in a browser; the core keeps the secret behind it.
    SignInGoogle,
    SignInMicrosoft,
    /// The `wonnet://auth?...` URL the browser came back with.
    FinishSignIn {
        url: String,
    },
    AddJmapAccount {
        url: String,
        username: String,
        password: String,
    },
    DevLogin {
        email: String,
    },
    SignOut,
    RemoveAccount {
        account: String,
    },
    /// `color` is `account-1` to `account-10`. `{}`.
    SetAccountColor {
        account: String,
        color: String,
    },
    /// An empty `name` takes the name away. `{}`.
    SetAccountName {
        account: String,
        name: String,
    },
    /// `{unified: [Mailbox], accounts: [AccountView]}`.
    Mailboxes,
    /// A `ThreadPage`. `unread` is the inbox's unread threads, and those of `keep` still in the
    /// inbox: the threads on screen, so one read there stays until the mailbox is left. The
    /// Unread filter keeps them too.
    Threads {
        mailbox: String,
        #[serde(default)]
        offset: usize,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        filter: Option<Filter>,
        #[serde(default)]
        keep: Vec<String>,
    },
    /// Fetches the bodies of these threads ahead of their opening, as the list scrolls past them
    /// or the keyboard moves near them. `{}`.
    Prefetch {
        threads: Vec<String>,
    },
    /// A `ThreadView`. Marks the thread read, unless it is only a `peek`: the thread beside the
    /// open one, drawn while a swipe turns to it. `images` shows the remote images of this
    /// thread even with `remote_images` off.
    OpenThread {
        thread: String,
        #[serde(default)]
        images: bool,
        #[serde(default)]
        peek: bool,
    },
    /// An `ActReply`.
    Act {
        action: Action,
        threads: Vec<String>,
        /// When a snooze ends, in Unix milliseconds.
        #[serde(default)]
        until: Option<i64>,
        /// A label's id, for `add_label`, `remove_label` and `move`.
        #[serde(default)]
        label: Option<String>,
    },
    /// Takes back the last `Act`, `ArchiveAll`, `ReadAll` or `ApplyRule` that answered `undo: true`. `{message}`, or an
    /// error when there is nothing to undo.
    Undo,
    /// Archives every thread a mailbox shows (older than `before`, when given; only the unread or
    /// starred ones with `filter`): "get me to zero". A split's id archives that split only. An
    /// `ActReply`.
    ArchiveAll {
        mailbox: String,
        #[serde(default)]
        before: Option<i64>,
        #[serde(default)]
        filter: Option<Filter>,
        /// As `Threads`'s.
        #[serde(default)]
        keep: Vec<String>,
    },
    /// Marks every unread message in the threads a mailbox shows read (only its starred ones with
    /// that filter). A split's id marks that split only. An `ActReply`.
    ReadAll {
        mailbox: String,
        #[serde(default)]
        filter: Option<Filter>,
    },
    /// Does what a rule does to new mail to the mail already here. An `ActReply`.
    ApplyRule {
        rule: Rule,
    },
    /// Deletes every message in Trash or Spam for good: `trash`, `spam`, or an account's. It
    /// can't be undone. An `ActReply`.
    EmptyMailbox {
        mailbox: String,
    },
    /// `{id}`: the new label's id, usable at once.
    CreateLabel {
        account: String,
        name: String,
    },
    /// The times a snooze, a reminder or a send-later can be for. Empty `text` gives the usual
    /// choices; otherwise what the text says ("tomorrow 9am", "in 2 hours", "fri", "dec 3").
    /// `{choices: [TimeChoice]}`.
    ParseTime {
        text: String,
    },
    /// Addresses the user wrote to or heard from, best first. `{contacts: [Address]}`.
    Contacts {
        query: String,
        #[serde(default = "default_contacts")]
        limit: usize,
    },
    /// Everyone in the device's contacts with a name and an address, one `Address` for each of
    /// their addresses. Their names replace the ones mail came with. `{}`.
    SetAddressBook {
        people: Vec<Address>,
    },
    /// A `Person`: someone, and the latest threads with them.
    Person {
        email: String,
    },
    /// `{path}`: a sender's picture from the server (their Gravatar or their company's logo) as
    /// a file on this device, or null when they have none.
    Avatar {
        email: String,
    },
    /// A `Preferences`: every synced preference.
    Preferences,
    /// `value` null removes the key. `{}`.
    SetPreference {
        key: String,
        #[serde(default)]
        value: Option<Value>,
    },
    /// A new message, written in `mailbox`: `{draft, from}`. It is from the mailbox's account, or
    /// in every account's mailboxes from the address the user last sent from.
    NewDraft {
        #[serde(default)]
        mailbox: Option<String>,
    },
    /// `{draft, from}`. The draft keeps the text quoted in `quote`. It answers `message`, else
    /// the thread's last message from someone else.
    ReplyDraft {
        thread: String,
        kind: ReplyKind,
        #[serde(default)]
        message: Option<String>,
    },
    /// Keeps a draft, on this device at once and on the user's other devices through the server.
    /// `{id}`; a new one is made when `draft_id` is none.
    SaveDraft {
        #[serde(default)]
        draft_id: Option<String>,
        draft: Draft,
    },
    /// `{id, draft, from}`.
    OpenDraft {
        draft_id: String,
    },
    /// `{}`.
    DeleteDraft {
        draft_id: String,
    },
    /// Sends after `delay` seconds (the undo window), or at `send_at` (Unix milliseconds). Adds
    /// the signature, folds the quote in and makes the HTML. Uploads the attachments that have a
    /// `path` first. Deletes the saved draft `draft_id`. `remind_at` brings the thread back to the
    /// inbox then if nobody answered. `{op_id, send_at}`.
    Send {
        draft: Draft,
        #[serde(default)]
        delay: u64,
        #[serde(default)]
        send_at: Option<i64>,
        #[serde(default)]
        remind_at: Option<i64>,
        #[serde(default)]
        draft_id: Option<String>,
    },
    /// `{draft, id}`: the draft as it was before sending, to edit again. It is kept as the saved
    /// draft `id` (the send's `draft_id` when it had one), so it stays in Drafts.
    CancelSend {
        op_id: String,
    },
    /// Downloads an attachment, or finds it already downloaded. `{path}`: a file on this device.
    OpenAttachment {
        message: String,
        /// Its place in the message's `attachments`.
        index: usize,
    },
    /// `{html}`: the whole thread, every message open, as a page to print.
    PrintThread {
        thread: String,
    },
    /// `{request, rows}`. Takes Gmail's operators: `from:`, `to:`, `subject:`, `has:attachment`,
    /// `is:unread`, `is:starred`, `in:<mailbox>`, `label:<name>`, `before:`/`after:` (YYYY/MM/DD),
    /// `newer_than:`/`older_than:` (2d, 3w, 1m, 1y), `"exact words"`, `-word` and `OR`.
    /// Every account's mail, or only `account`'s.
    Search {
        query: String,
        #[serde(default)]
        account: Option<String>,
    },
    /// Asks the server to check every account with its provider now, reconnecting first when
    /// offline. `{}` once the server has, or after a while when it can't be reached.
    Refresh,
    /// The app is on screen again. A link that went quiet while it was away is made anew, and
    /// one that was offline reconnects at once instead of after its backoff. `{}` at once.
    CameBack,
    /// `{}` once the core is caught up with the server and has sent everything waiting (but
    /// sends waiting for their time), or after 25 s. For an app that runs in the background a
    /// short while, as for a notification's action.
    Settle,
    /// This device, to push new mail to, as `DeviceRequest` describes it: who carries its
    /// pushes (APNs when left out), its token, the app's bundle id or package name, whether it is
    /// a development build, and the 32 bytes (base64) the notifications are sealed with. Kept,
    /// and given to the server at every sign-in. `{}` once the server has it (at once when signed
    /// out), or the server's error.
    SetPushDevice {
        #[serde(default)]
        platform: Platform,
        token: String,
        topic: String,
        #[serde(default)]
        sandbox: bool,
        key: String,
    },
    /// `{threads}`: those of these threads that are still unread in an inbox, whose
    /// notifications stay.
    UnreadInInbox {
        threads: Vec<String>,
    },
    /// `{address}` at once: the MCP server applications connect to.
    McpServer,
    /// A `ConnectedAppsView`, from the server.
    ConnectedApps,
    /// Ends an application's access at once. `{}`.
    RevokeConnectedApp {
        app: String,
    },
    /// A `ConnectRequestView`: the request a `wonnet://connect?key=…` URL opens, from the server.
    /// The demo answers `wonnet://connect?key=demo` with a made-up one.
    ConnectRequest {
        url: String,
    },
    /// The user's answer to a request. `access` is needed to approve it, as `mail_protocol::access::
    /// Access` writes it: `{"role": "reader" | "organizer", "all_accounts": true, "accounts":
    /// [account ids], "reads": ["subject", "body", "attachments"], "privileges": ["unsubscribe"],
    /// "only": {...}, "never": {...}}`. `{}`.
    AnswerConnectRequest {
        key: String,
        approve: bool,
        #[serde(default)]
        access: Option<Access>,
    },
    /// A `ConnectRequestView` of a connected app, to change what it may do: its `app` is set,
    /// its `key` empty, and `access` is what it may do now. From the last `ConnectedApps`.
    EditConnectedApp {
        app: String,
    },
    /// What a connected app may do from now on. `{}`.
    SetConnectedAppAccess {
        app: String,
        access: Access,
    },
    /// An `AccessSummary` of `access` after `edit`, for the sheet that chooses it: ask again on
    /// every change. `client` is the app's name, for the sentence. Answers can come out of
    /// order; an app keeps the answer to its latest ask.
    AccessSummary {
        client: String,
        access: Access,
        #[serde(default)]
        edit: Option<FilterEdit>,
    },
    /// A `FilterOptions`: the menu of what to add to a line of the filter of `access`, of every
    /// kind or of `kind`, matching `query`; `editing` is the chip it changes, checked in the
    /// menu. Answers can come out of order, as `AccessSummary`'s.
    FilterOptions {
        access: Access,
        line: FilterLine,
        #[serde(default)]
        kind: Option<FilterKind>,
        #[serde(default)]
        query: String,
        #[serde(default)]
        editing: Option<FilterValue>,
    },
}

fn default_limit() -> usize {
    100
}

fn default_contacts() -> usize {
    8
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Reply {
        id: u64,
        ok: bool,
        value: Value,
    },
    /// Local data changed. `threads` are the threads that changed; `mailboxes` that the lists or
    /// their counts may have; `preferences` that a synced preference did.
    Changed {
        mailboxes: bool,
        threads: Vec<String>,
        preferences: bool,
    },
    /// `connecting`, `online`, `offline` or `signed_out`.
    Connection {
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Sent {
        op_id: String,
    },
    /// `draft` is the draft as it was before sending, kept again in Drafts as the saved draft
    /// `draft_id`: open that one rather than saving a copy.
    SendFailed {
        op_id: String,
        error: String,
        draft: Box<Draft>,
        #[serde(skip_serializing_if = "Option::is_none")]
        draft_id: Option<String>,
    },
    /// The server's matches for a search, merged with the local ones.
    SearchResults {
        request: u64,
        rows: Vec<ThreadRow>,
    },
    /// Mail that just arrived in an inbox, unread and not from the user, for a notification.
    /// Not sent for the mail of a first sync, nor for accounts with `notify:<account>` false.
    NewMail {
        messages: Vec<NewMail>,
    },
    /// An application got or lost access to the user's mail, here or on another device: ask for
    /// `ConnectedApps` again while they are shown.
    ConnectedAppsChanged,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewMail {
    pub thread: String,
    pub account_id: String,
    /// The sender's name.
    pub from: String,
    pub email: String,
    pub initials: String,
    pub subject: String,
    pub snippet: String,
}

/// One row of a thread list, ready to draw.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThreadRow {
    pub id: String,
    pub account_id: String,
    /// The account's theme colour, `account-1` to `account-10`.
    pub color: String,
    /// "Alice, me (3)".
    pub senders: String,
    pub subject: String,
    pub snippet: String,
    /// "9:41 AM", "Yesterday", "Mar 3".
    pub date: String,
    pub timestamp: i64,
    pub unread: bool,
    pub starred: bool,
    pub attachment: bool,
    pub snoozed: bool,
    /// The row is a saved draft, not a thread: open it with `OpenDraft`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_id: Option<String>,
}

/// A list of threads. `splits` are the tabs over an inbox when Split Inbox is on, empty
/// otherwise; the one shown is the one whose `mailbox` was asked for.
#[derive(Debug, Clone, Serialize)]
pub struct ThreadPage {
    pub rows: Vec<ThreadRow>,
    pub total: usize,
    pub splits: Vec<SplitTab>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitTab {
    pub mailbox: String,
    pub name: String,
    pub unread: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mailbox {
    pub id: String,
    pub name: String,
    /// The Lucide icon it is drawn with.
    pub symbol: String,
    pub unread: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountView {
    pub id: String,
    pub address: String,
    /// What the user calls it; empty when they haven't named it.
    pub name: String,
    /// What it is shown as: its name, or its address.
    pub title: String,
    pub provider: String,
    pub color: String,
    pub status: String,
    pub identities: Vec<Identity>,
    /// Its mailboxes, then its labels (`<account>/label/<label>`).
    pub mailboxes: Vec<Mailbox>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelChip {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadView {
    pub id: String,
    pub account_id: String,
    pub color: String,
    pub subject: String,
    /// Who wrote in the thread, "Roger Cotes".
    pub from: String,
    /// Who they wrote to, apart from those who wrote, "me@example.com".
    pub to: String,
    pub starred: bool,
    pub unread: bool,
    pub muted: bool,
    /// The thread's custom labels.
    pub labels: Vec<LabelChip>,
    /// Whether `Act` `unsubscribe` can do something.
    pub unsubscribe: bool,
    /// The saved reply draft of this thread, to open with `OpenDraft`.
    pub draft_id: Option<String>,
    /// The address of the person the thread is with, for the contact pane.
    pub person: Option<String>,
    pub messages: Vec<MessageView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub from_name: String,
    pub from_email: String,
    pub initials: String,
    /// "to Ann, Bob".
    pub to: String,
    pub date: String,
    pub snippet: String,
    pub unread: bool,
    /// Older messages start folded to their snippet, as Newton showed them.
    pub folded: bool,
    /// The page to show, or none while the body is on its way.
    pub html: Option<String>,
    /// The lines of the page's text and its images, for a placeholder about as tall as the page
    /// until it is drawn.
    pub shape: Vec<Line>,
    /// The server couldn't fetch the body; opening the thread again later asks again.
    pub failed: bool,
    /// Remote images were left out; `OpenThread` with `images` shows them.
    pub blocked_images: bool,
    /// The mail brings its own design: in the dark scheme its colours are turned, and the page
    /// can show the original.
    pub designed: bool,
    /// Open one with `OpenAttachment` and its place in this list.
    pub attachments: Vec<Attachment>,
}

/// What an `Act` did. `message` is for a toast ("Archived."); `undo` says `Undo` can take it
/// back; `url` is a page to open to finish unsubscribing.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ActReply {
    pub message: Option<String>,
    pub undo: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimeChoice {
    pub id: String,
    /// "Tomorrow".
    pub name: String,
    /// "Tue 8:00 AM".
    pub label: String,
    /// Unix milliseconds.
    pub until: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Person {
    pub name: String,
    pub email: String,
    pub initials: String,
    pub threads: Vec<ThreadRow>,
}

/// The answer to `Preferences`. A list, not a map: the apps' JSON decoders rewrite map keys.
#[derive(Debug, Clone, Serialize)]
pub struct Preferences {
    pub values: Vec<PreferenceValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreferenceValue {
    pub key: String,
    pub value: Value,
}

/// The answer to `NewDraft`, `ReplyDraft` and `OpenDraft`: the draft and the address it is
/// written from.
#[derive(Debug, Clone, Serialize)]
pub struct DraftReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub draft: Draft,
    pub from: String,
}

/// The answer to `Contacts`.
#[derive(Debug, Clone, Serialize)]
pub struct Contacts {
    pub contacts: Vec<Address>,
}

/// The answer to `ConnectedApps`: the address applications connect to, and the ones the user let
/// in, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectedAppsView {
    pub address: String,
    pub apps: Vec<ConnectedAppView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConnectedAppView {
    pub id: String,
    pub name: String,
    pub verified: bool,
    /// What it may do, in one sentence: "Reads the subject of mail in Work, and only looks."
    pub access: String,
    /// "Approved Nov 5 · Used 5 minutes ago".
    pub used: String,
}

/// The answer to `ConnectRequest` and `EditConnectedApp`: who is asking, the choices of what it
/// may do, ready to draw, and the access the sheet starts with. A request starts with every
/// account, all of their mail, the subject, and only looking.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectRequestView {
    /// For `AnswerConnectRequest`; empty when an app is changed.
    pub key: String,
    /// For `SetConnectedAppAccess`, when it changes a connected app.
    pub app: Option<String>,
    pub client: String,
    pub verified: bool,
    /// "Published by claude.ai", or that it registered itself.
    pub publisher: String,
    /// "Sends you back to claude.ai"; empty when an app is changed.
    pub back_to: String,
    /// It goes back to an application on this device, not to a website: a warning.
    pub on_device: bool,
    pub accounts: Vec<AccessChoice>,
    /// The parts it may read: `subject`, `body`, `attachments`.
    pub reads: Vec<AccessChoice>,
    /// `reader` (only looks) and `organizer`.
    pub roles: Vec<AccessChoice>,
    /// `unsubscribe`.
    pub privileges: Vec<AccessChoice>,
    pub access: Access,
}

/// A line of the filter: `only` narrows what an app reaches, `never` keeps mail from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterLine {
    Only,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKind {
    From,
    To,
    Label,
    Subject,
    Body,
    Attachment,
    Date,
}

/// One condition: a sender (an address, `@domain` or text), a recipient, a label's id or a
/// mailbox, subject or body words, a file name, or an age, `newer:30d` or `older:1y`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterValue {
    pub kind: FilterKind,
    pub value: String,
}

/// A change to a line of the filter: takes `remove` away and puts `add` in, either or both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterEdit {
    pub line: FilterLine,
    #[serde(default)]
    pub remove: Option<FilterValue>,
    #[serde(default)]
    pub add: Option<FilterValue>,
}

/// A run of the sentence, in bold when `strong`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SentencePart {
    pub text: String,
    pub strong: bool,
}

/// The answer to `AccessSummary`: the access to keep (after the edit, tidied), the sentence that
/// says it, how much it reaches (`fraction` fills the bar) and each line's chips. `problem` is
/// why it can't be approved yet; `warning` a chip that changes nothing.
#[derive(Debug, Clone, Serialize)]
pub struct AccessSummary {
    pub access: Access,
    pub sentence: Vec<SentencePart>,
    /// "Reaches 14,212 of 15,630 messages in Work".
    pub reach: String,
    pub fraction: f64,
    pub only: Vec<FilterChip>,
    pub never: Vec<FilterChip>,
    /// What an empty line says: "All mail in Work", "Nothing kept out".
    pub only_empty: String,
    pub never_empty: String,
    pub warning: Option<String>,
    pub problem: Option<String>,
}

/// A condition as a chip: `label` is its kind's word ("from", "labelled"), `text` its value
/// ("@chase.com", "Medical"), `joiner` the "or" or "and" before it. `matches` is how many
/// messages it matches; none, and it is drawn as a warning.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FilterChip {
    pub kind: FilterKind,
    pub value: String,
    pub label: String,
    pub text: String,
    pub joiner: Option<String>,
    pub matches: usize,
    /// "1,204 messages".
    pub help: String,
}

/// The answer to `FilterOptions`: the field's placeholder, the kind picked (as the tag in the
/// field), the options in sections, and a note when there is nothing to pick.
#[derive(Debug, Clone, Serialize)]
pub struct FilterOptions {
    pub placeholder: String,
    pub scope: Option<String>,
    pub sections: Vec<FilterSection>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterSection {
    pub title: Option<String>,
    pub options: Vec<FilterOption>,
}

/// One option of the menu. Without a `value` it picks its `kind`, narrowing the menu to it;
/// with one it adds that condition, or replaces the chip being changed. It starts with a
/// person's `avatar` or a Lucide `symbol`; `count` is how many messages it matches; `checked`
/// is the chip being changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FilterOption {
    pub id: String,
    pub kind: FilterKind,
    pub value: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub symbol: Option<String>,
    pub avatar: Option<Address>,
    pub count: Option<String>,
    pub checked: bool,
}

/// One choice of what an application may do. `id` is an account's id, a role or a privilege;
/// `symbol` is the Lucide icon it is drawn with, `color` an account's.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccessChoice {
    pub id: String,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
