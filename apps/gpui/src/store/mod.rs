//! Everything the window shows and does, as the Mac app's `MailStore` has it: the list, the open
//! thread, the message being written, the sheets, the preferences and the toasts. Every action
//! changes what is shown at once and asks the core after; nothing waits on the network.

mod actions;
pub mod commands;
mod local;
mod prefs;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::rc::Rc;
use std::time::Duration;

use futures::FutureExt as _;
use futures::future::Shared;
use gpui::{AppContext as _, Context, Task};
use mail_core::api::{Command, Event};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub use commands::ThreadCommand;
pub use local::{Appearance, LocalSettings};
pub use prefs::{CustomSplit, SettingsEntry, SettingsSection, Snippet, label_id, uuid};

use crate::core::Core;
use crate::models::*;
use crate::platform::notify::{Answer, Notifier};

/// A note at the bottom of the window, sometimes with something to do about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub action: Option<String>,
    pub seconds: u64,
}

type StoreAction = Box<dyn FnOnce(&mut Store, &mut Context<Store>)>;

/// A question asked before something that can't be taken back.
#[derive(Clone)]
pub struct Confirmation {
    pub title: String,
    pub message: String,
    pub action: String,
    /// What leaves the question without doing it.
    pub cancel: &'static str,
    /// The action isn't a loss: Send, rather than Delete.
    pub safe: bool,
    pub perform: Rc<dyn Fn(&mut Store, &mut Context<Store>)>,
}

/// Threads being labelled, or moved to a label.
#[derive(Clone, Debug, PartialEq)]
pub struct Labeling {
    pub id: u64,
    pub threads: Vec<String>,
    pub moving: bool,
}

/// What the command palette lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteScope {
    Everything,
    Labels,
}

/// A message being written, and the number that tells it from the one before.
#[derive(Clone, Debug, PartialEq)]
pub struct Compose {
    pub id: u64,
    pub state: ComposeState,
}

pub struct Store {
    pub core: Core,
    pub local: LocalSettings,
    pub booted: bool,
    pub signed_in: bool,
    pub connection: String,
    pub server: String,
    listed_unified: Vec<Mailbox>,
    listed_accounts: Vec<AccountView>,
    pub mailbox: String,
    pub rows: Vec<ThreadRow>,
    /// False while the rows are still another mailbox's, until its page is read.
    pub list_ready: bool,
    pub total: usize,
    pub loading_more: bool,
    pub conversation: Option<Conversation>,
    pub selected: Option<String>,
    pub search_query: String,
    pub search_rows: Option<Vec<ThreadRow>>,
    pub toast: Option<Toast>,
    toast_action: Option<StoreAction>,
    pub compose: Option<Compose>,
    pub snoozing: Option<Vec<String>>,
    pub splits: Vec<SplitTab>,
    pub filter: Option<Filter>,
    pub selection: BTreeSet<String>,
    pub selection_anchor: Option<String>,
    pub labeling: Option<Labeling>,
    pub palette: Option<PaletteScope>,
    pub gallery_open: bool,
    pub settings: Option<SettingsSection>,
    pub settings_target: Option<String>,
    pub settings_query: String,
    pub confirmation: Option<Confirmation>,
    /// The sender shown in a sheet, on a window too narrow for the pane beside the thread.
    pub contact: Option<String>,
    arriving_thread: Option<String>,
    pub images_shown: HashSet<String>,
    pub sidebar_open: bool,
    /// The accounts opened in the sidebar to show their mailboxes.
    pub expanded: BTreeSet<String>,
    pub list_offset: f64,
    pub thread_offset: Option<f64>,
    pub body_heights: BTreeMap<String, f64>,
    pub body_width: f64,
    pub preferences: BTreeMap<String, Value>,
    pub preferences_loaded: bool,
    /// Attachments being downloaded, as `<message>/<index>`.
    pub downloading: HashSet<String>,
    pub unfolded_messages: HashSet<String>,
    pub folded_messages: HashSet<String>,
    pub original_messages: HashSet<String>,
    pub focused_message: Option<String>,
    /// Each compose's last save, so the next save and the send wait for it.
    draft_saves: BTreeMap<u64, Shared<Task<Option<String>>>>,
    search_request: u64,
    reloading: bool,
    stale_while_reloading: bool,
    more_while_reloading: bool,
    prefetched: HashSet<String>,
    next_id: u64,
    saved_ui: Option<String>,
    ui_save: Option<Task<()>>,
    /// The window is in front, where new mail needs no notification.
    pub active: bool,
    notifier: Notifier,
    /// When a sign-in in the browser started; the browser comes back to the app with it.
    pub signing_in: Option<std::time::Instant>,
}

impl Store {
    pub fn new(core: Core, local: LocalSettings, notifier: Notifier) -> Self {
        Self {
            core,
            local,
            booted: false,
            signed_in: false,
            connection: "offline".into(),
            server: String::new(),
            listed_unified: Vec::new(),
            listed_accounts: Vec::new(),
            mailbox: "inbox".into(),
            rows: Vec::new(),
            list_ready: true,
            total: 0,
            loading_more: false,
            conversation: None,
            selected: None,
            search_query: String::new(),
            search_rows: None,
            toast: None,
            toast_action: None,
            compose: None,
            snoozing: None,
            splits: Vec::new(),
            filter: None,
            selection: BTreeSet::new(),
            selection_anchor: None,
            labeling: None,
            palette: None,
            gallery_open: false,
            settings: None,
            settings_target: None,
            settings_query: String::new(),
            confirmation: None,
            contact: None,
            arriving_thread: None,
            images_shown: HashSet::new(),
            sidebar_open: false,
            expanded: BTreeSet::new(),
            list_offset: 0.0,
            thread_offset: None,
            body_heights: BTreeMap::new(),
            body_width: 0.0,
            preferences: BTreeMap::new(),
            preferences_loaded: false,
            downloading: HashSet::new(),
            unfolded_messages: HashSet::new(),
            folded_messages: HashSet::new(),
            original_messages: HashSet::new(),
            focused_message: None,
            draft_saves: BTreeMap::new(),
            search_request: 0,
            reloading: false,
            stale_while_reloading: false,
            more_while_reloading: false,
            prefetched: HashSet::new(),
            next_id: 1,
            saved_ui: None,
            ui_save: None,
            active: true,
            notifier,
            signing_in: None,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Asks the core, then does `done` with its answer here.
    pub fn ask<T: DeserializeOwned + Send + 'static>(
        &self,
        command: Command,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<T, String>, &mut Context<Self>) + 'static,
    ) {
        let answer = self.core.call::<T>(command);
        cx.spawn(async move |this, cx| {
            let result = answer.await;
            let _ = this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks the core and shows what went wrong, if anything did.
    pub fn ask_or_show<T: DeserializeOwned + Send + 'static>(
        &self,
        command: Command,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        self.ask(command, cx, |this, result, cx| match result {
            Ok(value) => done(this, value, cx),
            Err(error) => this.show(error, cx),
        });
    }

    // MARK: What is shown

    /// The rows the list shows: the search's while searching.
    pub fn visible_rows(&self) -> &[ThreadRow] {
        self.search_rows.as_deref().unwrap_or(&self.rows)
    }

    /// Nothing to list, and no page on its way that could fill it.
    pub fn list_empty(&self) -> bool {
        match &self.search_rows {
            Some(rows) => rows.is_empty(),
            None => self.rows.is_empty() && self.list_ready,
        }
    }

    /// The mailbox without its split: `inbox` for `inbox:other`.
    pub fn base_mailbox(&self) -> String {
        let mut parts: Vec<&str> = self.mailbox.split('/').collect();
        match parts.last() {
            Some(last) if last.starts_with("inbox:") => {
                parts.pop();
                parts.push("inbox");
                parts.join("/")
            }
            _ => self.mailbox.clone(),
        }
    }

    pub fn unified(&self) -> Vec<Mailbox> {
        prefs::sorted(&self.listed_unified, &prefs::ids(prefs::SIDEBAR_KEY, &self.preferences), |mailbox| &mailbox.id)
    }

    pub fn accounts(&self) -> Vec<AccountView> {
        prefs::sorted(&self.listed_accounts, &prefs::ids(prefs::ACCOUNTS_KEY, &self.preferences), |account| &account.id)
    }

    pub fn all_mailboxes(&self) -> Vec<Mailbox> {
        let mut all = self.unified();
        all.extend(self.accounts().into_iter().flat_map(|account| account.mailboxes));
        all
    }

    /// The account whose mailbox is on screen; none in the mailboxes of every account together.
    pub fn list_account(&self) -> Option<String> {
        let parts: Vec<&str> = self.mailbox.split('/').collect();
        (parts.len() > 1).then(|| parts[0].to_string())
    }

    pub fn in_unread(&self) -> bool {
        self.base_mailbox().rsplit('/').next() == Some("unread")
    }

    /// The mailbox's name, or "Search" while searching.
    pub fn mailbox_name(&self) -> String {
        if self.search_rows.is_some() {
            return "Search".into();
        }
        self.base_mailbox_name()
    }

    pub fn base_mailbox_name(&self) -> String {
        let (name, account) = self.base_mailbox_title();
        match account {
            Some(account) => format!("{name} · {account}"),
            None => name,
        }
    }

    /// The mailbox's name and, when it is one account's, that account's name.
    pub fn base_mailbox_title(&self) -> (String, Option<String>) {
        let base = self.base_mailbox();
        let parts: Vec<&str> = base.split('/').collect();
        let name = self.all_mailboxes().into_iter().find(|mailbox| mailbox.id == base).map(|mailbox| mailbox.name);
        let name = name.unwrap_or_else(|| "Inbox".into());
        let accounts = self.accounts();
        if parts.len() > 1
            && let Some(account) = accounts.iter().find(|account| account.id == parts[0])
        {
            return (name, Some(account.title.clone()));
        }
        let name = if name == "Inbox" && accounts.len() > 1 { "All Inboxes".into() } else { name };
        (name, None)
    }

    /// The mailbox's mail is still coming.
    pub fn mailbox_syncing(&self) -> bool {
        let accounts = self.accounts();
        if self.connection == "connecting" && accounts.is_empty() {
            return true;
        }
        let base = self.base_mailbox();
        let parts: Vec<&str> = base.split('/').collect();
        let mut syncing = accounts.iter().filter(|account| account.status == "syncing");
        if parts.len() > 1 {
            return syncing.any(|account| account.id == parts[0]);
        }
        syncing.next().is_some()
    }

    /// Whether a sheet, the palette or a question is up, when the window's keys don't apply.
    pub fn sheet_open(&self) -> bool {
        self.compose.is_some()
            || self.snoozing.is_some()
            || self.labeling.is_some()
            || self.palette.is_some()
            || self.settings.is_some()
            || self.gallery_open
            || self.confirmation.is_some()
            || self.contact.is_some()
    }

    // MARK: Starting

    /// Shows the screen the app was left on, all of it at once.
    pub fn boot(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Boot, cx, |this, result: Result<Boot, String>, cx| {
            this.booted = true;
            let boot = match result {
                Ok(boot) => boot,
                Err(error) => return this.show(error, cx),
            };
            let ui = boot.ui;
            this.signed_in = boot.status.signed_in;
            this.connection = boot.status.connection;
            this.server = boot.status.server.unwrap_or_default();
            this.listed_unified = boot.mailboxes.unified;
            this.listed_accounts = boot.mailboxes.accounts;
            this.preferences = boot.preferences.map();
            this.preferences_loaded = true;
            this.mailbox = ui.mailbox.clone();
            this.filter = ui.filter;
            this.rows = boot.page.rows;
            this.total = boot.page.total;
            this.splits = boot.page.splits;
            this.selected = ui.selected.clone();
            this.selection = ui.selection.iter().cloned().collect();
            this.search_query = ui.search.clone();
            this.search_rows = boot.search;
            this.unfolded_messages = ui.unfolded.iter().cloned().collect();
            this.body_heights = ui.heights.clone();
            this.body_width = ui.body_width;
            if ui.images
                && let Some(thread) = &boot.thread
            {
                this.images_shown.insert(thread.id.clone());
            }
            this.conversation = boot.thread;
            this.focused_message = ui.focused.clone();
            this.list_offset = ui.list_offset;
            this.thread_offset = ui.thread_offset;
            this.sidebar_open = ui.sidebar;
            if let Some(mut restored) = ui.compose.and_then(|value| serde_json::from_value::<ComposeState>(value).ok())
            {
                restored.keep = restored.keep || restored.has_content();
                let id = this.next_id();
                this.compose = Some(Compose { id, state: restored });
            }
            this.saved_ui = serde_json::to_string(&this.ui_state()).ok();
            if !this.search_query.trim().is_empty() {
                let query = this.search_query.clone();
                this.search(query, cx);
            }
        });
    }

    // MARK: Where the app is

    pub fn ui_state(&self) -> UiState {
        UiState {
            mailbox: self.mailbox.clone(),
            filter: self.filter,
            thread: self.conversation.as_ref().map(|conversation| conversation.id.clone()),
            selected: self.selected.clone(),
            rows: self.rows.len(),
            list_offset: self.list_offset,
            thread_offset: self.thread_offset,
            selection: self.selection.iter().cloned().collect(),
            search: self.search_query.clone(),
            compose: self.compose.as_ref().and_then(|compose| serde_json::to_value(&compose.state).ok()),
            sidebar: self.sidebar_open,
            unfolded: self.unfolded_messages.iter().cloned().collect(),
            focused: self.focused_message.clone(),
            images: self.conversation.as_ref().is_some_and(|open| self.images_shown.contains(&open.id)),
            heights: self.body_heights.clone(),
            body_width: self.body_width,
        }
    }

    /// Saves a little after the state changes, however much changes in the meantime.
    pub fn schedule_ui_save(&mut self, cx: &mut Context<Self>) {
        if !self.booted || self.ui_save.is_some() {
            return;
        }
        self.ui_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let _ = this.update(cx, |this, _| {
                this.ui_save = None;
                this.save_ui();
            });
        }));
    }

    pub fn save_ui(&mut self) {
        if !self.booted {
            return;
        }
        let ui = self.ui_state();
        let text = serde_json::to_string(&ui).ok();
        if text == self.saved_ui {
            return;
        }
        self.saved_ui = text;
        self.core.send(Command::SaveUi { ui });
    }

    /// Saves and waits for it to be written, before the app quits.
    pub fn save_ui_now(&mut self) -> impl Future<Output = ()> + use<> {
        self.ui_save = None;
        let answer = self.core.call::<Empty>(Command::SaveUi { ui: self.ui_state() });
        let booted = self.booted;
        async move {
            if booted {
                let _ = answer.await;
            }
        }
    }

    pub fn note_compose(&mut self, live: ComposeState, cx: &mut Context<Self>) {
        let Some(compose) = &mut self.compose else { return };
        if compose.state == live {
            return;
        }
        compose.state = live;
        cx.notify();
    }

    pub fn note_scroll(&mut self, offset: f64) {
        self.list_offset = offset;
    }

    pub fn note_thread_scroll(&mut self, offset: f64) {
        self.thread_offset = Some(offset);
    }

    /// A body of the open thread says how tall its page is at this width; a new width starts
    /// the heights over.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub fn note_body_height(&mut self, message: &str, width: f64, height: f64) {
        if width <= 0.0 {
            return;
        }
        if (width - self.body_width).abs() > 0.5 {
            self.body_heights.clear();
            self.body_width = width;
        }
        self.body_heights.insert(message.to_string(), height);
    }

    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub fn body_height(&self, message: &str, width: f64) -> Option<f64> {
        ((width - self.body_width).abs() <= 0.5).then(|| self.body_heights.get(message).copied()).flatten()
    }

    /// Asks the server to check every account now.
    pub fn check_mail(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Refresh, cx, |_, _: Result<Empty, String>, _| {});
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Status, cx, |this, status: Result<Status, String>, cx| {
            if let Ok(status) = status {
                this.signed_in = status.signed_in;
                this.connection = status.connection;
                this.server = status.server.unwrap_or_default();
            }
            this.reload_mailboxes(cx);
            this.reload_threads(cx);
        });
    }

    pub fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Changed { mailboxes, threads, preferences } => {
                for thread in &threads {
                    self.prefetched.remove(thread);
                }
                self.settle_notifications(Some(&threads), cx);
                if preferences {
                    self.load_preferences(cx);
                }
                if mailboxes {
                    self.reload_mailboxes(cx);
                }
                self.reload_threads(cx);
                if let Some(open) = &self.conversation
                    && threads.contains(&open.id)
                {
                    self.reopen(cx);
                }
                if let Some(arriving) = self.arriving_thread.clone()
                    && threads.contains(&arriving)
                    && self.selected.as_ref() == Some(&arriving)
                {
                    self.open(&arriving, cx);
                }
            }
            Event::Connection { state, .. } => {
                self.signed_in = state != "signed_out";
                if state == "signed_out" {
                    self.notifier.take_down_all();
                    self.conversation = None;
                    self.rows.clear();
                    self.listed_unified.clear();
                    self.listed_accounts.clear();
                    self.selection.clear();
                }
                self.connection = state;
            }
            Event::Sent { .. } => self.show("Sent.", cx),
            Event::SendFailed { error, draft, draft_id, .. } => {
                let draft = *draft;
                self.show_with(error, "Edit", 4, cx, move |this, cx| {
                    let mut state = ComposeState::new(draft, String::new());
                    state.draft_id = draft_id;
                    state.keep = true;
                    this.open_compose(state, cx);
                });
            }
            Event::SearchResults { request, rows } => {
                if request == self.search_request && self.search_rows.is_some() {
                    self.search_rows =
                        serde_json::to_value(rows).ok().and_then(|rows| serde_json::from_value(rows).ok());
                }
            }
            Event::NewMail { messages } => {
                let messages: Vec<NewMailItem> = serde_json::to_value(messages)
                    .ok()
                    .and_then(|value| serde_json::from_value(value).ok())
                    .unwrap_or_default();
                if !self.active {
                    self.notify(messages, cx);
                }
            }
            Event::Error { message } => self.show(message, cx),
            Event::Reply { .. } => {}
        }
        cx.notify();
    }

    // MARK: Notifications

    /// Tells of new mail from its sender, with their picture.
    fn notify(&mut self, messages: Vec<NewMailItem>, cx: &mut Context<Self>) {
        for message in messages.into_iter().rev().take(5) {
            let photo = self.core.call::<AvatarReply>(Command::Avatar { email: message.email.to_lowercase() });
            let notifier = self.notifier.clone();
            cx.background_spawn(async move {
                let photo = photo.await.ok().and_then(|reply| reply.path).map(std::path::PathBuf::from);
                notifier.show(message, photo);
            })
            .detach();
        }
    }

    /// Takes down the notifications of these threads (all shown, when none) that are no longer
    /// unread in an inbox.
    pub fn settle_notifications(&mut self, threads: Option<&[String]>, cx: &mut Context<Self>) {
        let shown = self.notifier.threads();
        let changed: Vec<String> =
            shown.into_iter().filter(|thread| threads.is_none_or(|threads| threads.contains(thread))).collect();
        if changed.is_empty() {
            return;
        }
        let unread = self.core.call::<ThreadIds>(Command::UnreadInInbox { threads: changed.clone() });
        let notifier = self.notifier.clone();
        cx.background_spawn(async move {
            let Ok(unread) = unread.await else { return };
            let gone: Vec<String> = changed.into_iter().filter(|thread| !unread.threads.contains(thread)).collect();
            notifier.take_down(&gone);
        })
        .detach();
    }

    /// Does what a notification was clicked for. Its buttons act at once, the server told
    /// before and after, as the app may be about to close.
    pub fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        let (action, thread) = match answer {
            Answer::Open(thread) => return self.show_thread(&thread, cx),
            Answer::Reply(thread) => {
                self.show_thread(&thread, cx);
                return self.reply(ReplyKind::Reply, Some(thread), None, cx);
            }
            Answer::Archive(thread) => (Action::Archive, thread),
            Answer::Read(thread) => (Action::Read, thread),
        };
        let core = self.core.clone();
        cx.background_spawn(async move {
            let _ = core.call::<Empty>(Command::Settle).await;
            let command = Command::Act { action, threads: vec![thread], until: None, label: None };
            let _ = core.call::<Value>(command).await;
            let _ = core.call::<Empty>(Command::Settle).await;
        })
        .detach();
    }

    pub fn reload_mailboxes(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Mailboxes, cx, |this, found: Result<Mailboxes, String>, _| {
            let Ok(found) = found else { return };
            if found.unified != this.listed_unified {
                this.listed_unified = found.unified;
            }
            if found.accounts != this.listed_accounts {
                this.listed_accounts = found.accounts;
            }
        });
    }

    /// The inbox's unread count, for the window's title.
    pub fn inbox_unread(&self) -> u32 {
        self.listed_unified.iter().find(|mailbox| mailbox.id == "inbox").map_or(0, |mailbox| mailbox.unread)
    }

    fn page_command(&self, offset: usize, limit: usize) -> Command {
        Command::Threads {
            mailbox: self.mailbox.clone(),
            offset,
            limit,
            filter: self.filter,
            keep: self.kept_rows().unwrap_or_default(),
        }
    }

    /// In Unread or with its filter, the threads on screen, which stay while they are in it.
    pub fn kept_rows(&self) -> Option<Vec<String>> {
        let unread = self.in_unread() || self.filter == Some(Filter::Unread);
        (unread && self.list_ready).then(|| self.rows.iter().map(|row| row.id.clone()).collect())
    }

    /// Reads the list again.
    pub fn reload_threads(&mut self, cx: &mut Context<Self>) {
        if self.reloading {
            self.stale_while_reloading = true;
            return;
        }
        self.reloading = true;
        self.stale_while_reloading = false;
        let asked = (self.mailbox.clone(), self.filter);
        let command = self.page_command(0, self.rows.len().max(200));
        self.ask(command, cx, move |this, page: Result<ThreadPage, String>, cx| {
            this.reloading = false;
            if let Ok(page) = page
                && asked == (this.mailbox.clone(), this.filter)
            {
                this.take_page(page);
            }
            if this.stale_while_reloading {
                return this.reload_threads(cx);
            }
            if this.more_while_reloading {
                this.more_while_reloading = false;
                this.load_more(cx);
            }
        });
    }

    fn take_page(&mut self, page: ThreadPage) {
        let old_index = self.selected.as_ref().and_then(|id| self.rows.iter().position(|row| &row.id == id));
        if page.rows != self.rows {
            self.rows = page.rows;
        }
        self.total = page.total;
        self.splits = page.splits;
        self.list_ready = true;
        if self.search_rows.is_some() {
            return;
        }
        if !self.selection.is_empty() {
            let shown: HashSet<&String> = self.rows.iter().map(|row| &row.id).collect();
            self.selection.retain(|id| shown.contains(id));
        }
        if let (Some(selected), Some(old_index)) = (&self.selected, old_index)
            && !self.rows.iter().any(|row| &row.id == selected)
        {
            self.selected = self.rows.get(old_index.min(self.rows.len().saturating_sub(1))).map(|row| row.id.clone());
        }
    }

    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.rows.len() >= self.total {
            return;
        }
        if self.reloading {
            self.more_while_reloading = true;
            return;
        }
        self.reloading = true;
        self.loading_more = true;
        let asked = (self.mailbox.clone(), self.filter);
        let offset = self.rows.len();
        let command = self.page_command(offset, 200);
        self.ask(command, cx, move |this, page: Result<ThreadPage, String>, cx| {
            if let Ok(page) = page
                && asked == (this.mailbox.clone(), this.filter)
                && offset == this.rows.len()
            {
                this.rows.extend(page.rows);
                this.total = page.total;
            }
            this.reloading = false;
            this.loading_more = false;
            this.more_while_reloading = false;
            if this.stale_while_reloading {
                this.reload_threads(cx);
            }
        });
    }

    pub fn select_mailbox(&mut self, mailbox: &str, cx: &mut Context<Self>) {
        self.mailbox = mailbox.to_string();
        self.conversation = None;
        self.search_rows = None;
        self.search_query.clear();
        self.list_ready = false;
        self.selection.clear();
        self.selection_anchor = None;
        self.selected = None;
        self.list_offset = 0.0;
        self.reload_threads(cx);
        cx.notify();
    }

    /// One of the mailboxes every account has, in the account on screen when one is.
    pub fn go(&mut self, kind: &str, cx: &mut Context<Self>) {
        let mailbox = match self.list_account() {
            Some(account) => format!("{account}/{kind}"),
            None => kind.to_string(),
        };
        self.select_mailbox(&mailbox, cx);
    }

    pub fn toggle_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        let next = if self.filter == Some(filter) { None } else { Some(filter) };
        self.set_filter(next, cx);
    }

    pub fn set_filter(&mut self, filter: Option<Filter>, cx: &mut Context<Self>) {
        if filter == self.filter {
            return;
        }
        self.filter = filter;
        self.selection.clear();
        self.list_ready = false;
        self.list_offset = 0.0;
        self.reload_threads(cx);
        cx.notify();
    }

    /// The next or previous tab of a split inbox.
    pub fn move_split(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.splits.is_empty() {
            return;
        }
        let count = self.splits.len() as isize;
        let index = self.splits.iter().position(|split| split.mailbox == self.mailbox).unwrap_or(0) as isize;
        let next = self.splits[((index + step + count) % count) as usize].mailbox.clone();
        if next != self.mailbox {
            self.select_mailbox(&next, cx);
        }
    }

    // MARK: Threads

    pub fn row(&self, thread: &str) -> Option<&ThreadRow> {
        self.visible_rows().iter().find(|row| row.id == thread)
    }

    pub fn open(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.open_thread(thread, false, cx);
    }

    /// Opens the thread of a notification, whose mail may not have synced yet.
    pub fn show_thread(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.open_thread(thread, true, cx);
    }

    fn open_thread(&mut self, thread: &str, arriving: bool, cx: &mut Context<Self>) {
        self.selected = Some(thread.to_string());
        self.arriving_thread = None;
        if let Some(draft) = self.row(thread).and_then(|row| row.draft_id.clone()) {
            return self.open_draft(&draft, cx);
        }
        if self.conversation.as_ref().map(|open| open.id.as_str()) != Some(thread) {
            self.unfolded_messages.clear();
            self.folded_messages.clear();
            self.original_messages.clear();
            self.focused_message = None;
            self.thread_offset = None;
            self.body_heights.clear();
        }
        let wanted = thread.to_string();
        let command =
            Command::OpenThread { thread: wanted.clone(), images: self.images_shown.contains(thread), peek: false };
        self.ask(command, cx, move |this, result: Result<Conversation, String>, cx| match result {
            Ok(conversation) => {
                if this.selected.as_ref() == Some(&wanted) {
                    let first = this.conversation.as_ref().map(|open| open.id != conversation.id).unwrap_or(true);
                    if first {
                        this.focused_message = this
                            .focused_message
                            .clone()
                            .filter(|id| conversation.messages.iter().any(|m| &m.id == id))
                            .or_else(|| {
                                conversation
                                    .messages
                                    .iter()
                                    .find(|message| message.unread)
                                    .map(|message| message.id.clone())
                            });
                    }
                    this.conversation = Some(conversation);
                }
            }
            Err(error) => {
                if arriving && this.selected.as_ref() == Some(&wanted) {
                    this.arriving_thread = Some(wanted);
                } else {
                    this.show(error, cx);
                }
            }
        });
        cx.notify();
    }

    fn reopen(&mut self, cx: &mut Context<Self>) {
        let Some(open) = self.conversation.as_ref().map(|open| open.id.clone()) else { return };
        let command =
            Command::OpenThread { thread: open.clone(), images: self.images_shown.contains(&open), peek: false };
        self.ask(command, cx, move |this, fresh: Result<Conversation, String>, _| {
            let Ok(fresh) = fresh else { return };
            if this.conversation.as_ref().is_some_and(|conversation| conversation.id == open) {
                this.conversation = Some(fresh);
            }
        });
    }

    pub fn show_images(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.conversation else { return };
        self.images_shown.insert(open.id.clone());
        self.reopen(cx);
    }

    /// Whether designed mail is turned dark in dark mode.
    pub fn dark_mail(&self) -> bool {
        self.flag("dark_mail", true)
    }

    /// Whether hovering a row shows the circle that selects it.
    pub fn select_circles(&self) -> bool {
        self.flag("select_circles", true)
    }

    pub fn toggle_original(&mut self, message: &str, cx: &mut Context<Self>) {
        if !self.original_messages.remove(message) {
            self.original_messages.insert(message.to_string());
        }
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.conversation = None;
        cx.notify();
    }

    /// The thread an action from the keyboard or the toolbar is for.
    pub fn current(&self) -> Option<String> {
        self.conversation.as_ref().map(|open| open.id.clone()).or_else(|| self.selected.clone())
    }

    /// The threads an action is for: the selection over the list, else the current thread.
    pub fn targets(&self) -> Vec<String> {
        if self.conversation.is_some() || self.selection.is_empty() {
            return self.current().into_iter().collect();
        }
        self.visible_rows().iter().filter(|row| self.selection.contains(&row.id)).map(|row| row.id.clone()).collect()
    }

    /// The threads an action on a row is for: the whole selection when the row is in it.
    pub fn targets_for(&self, thread: &str) -> Vec<String> {
        if self.selection.contains(thread) { self.targets() } else { vec![thread.to_string()] }
    }

    pub fn act(
        &mut self,
        action: Action,
        given: &[String],
        until: Option<i64>,
        label: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let threads: Vec<String> = given.iter().filter(|thread| !is_draft(thread)).cloned().collect();
        if threads.is_empty() {
            return;
        }
        let leaving = matches!(
            action,
            Action::Archive
                | Action::Trash
                | Action::Spam
                | Action::Snooze
                | Action::Inbox
                | Action::Move
                | Action::Mute
                | Action::Block
        );
        if leaving {
            let gone: HashSet<String> = threads.iter().cloned().collect();
            self.step_away(&gone, cx);
            for thread in &threads {
                self.selection.remove(thread);
            }
        }
        let command = Command::Act { action, threads, until, label };
        self.ask_or_show(command, cx, |this, reply: ActReply, cx| this.report(reply, cx));
        cx.notify();
    }

    /// Shows what an action did, with Undo when the core can take it back.
    pub fn report(&mut self, reply: ActReply, cx: &mut Context<Self>) {
        if let Some(url) = reply.url {
            cx.open_url(&url);
        }
        let Some(message) = reply.message else { return };
        if !reply.undo {
            return self.show(message, cx);
        }
        self.show_with(message, "Undo", 4, cx, |this, cx| this.undo(cx));
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Undo, cx, |this, result: Result<MessageReply, String>, cx| match result {
            Ok(reply) => this.show(reply.message, cx),
            Err(error) => this.show(error, cx),
        });
    }

    pub fn toggle_star(&mut self, threads: &[String], cx: &mut Context<Self>) {
        let action = if self.all_starred(threads) { Action::Unstar } else { Action::Star };
        self.act(action, threads, None, None, cx);
    }

    pub fn toggle_read(&mut self, threads: &[String], cx: &mut Context<Self>) {
        let unread = self.any_unread(threads);
        self.act(if unread { Action::Read } else { Action::Unread }, threads, None, None, cx);
        if !unread && self.conversation.as_ref().is_some_and(|open| threads.contains(&open.id)) {
            self.conversation = None;
        }
    }

    pub fn all_starred(&self, threads: &[String]) -> bool {
        if let (Some(open), [only]) = (&self.conversation, threads)
            && &open.id == only
        {
            return open.starred;
        }
        let found: Vec<&ThreadRow> = self.visible_rows().iter().filter(|row| threads.contains(&row.id)).collect();
        !found.is_empty() && found.iter().all(|row| row.starred)
    }

    pub fn any_unread(&self, threads: &[String]) -> bool {
        self.visible_rows().iter().any(|row| threads.contains(&row.id) && row.unread)
    }

    /// The row after the given one that isn't leaving with it, else the one before.
    fn neighbour(&self, thread: &str, leaving: &HashSet<String>) -> Option<String> {
        let list = self.visible_rows();
        let index = list.iter().position(|row| row.id == thread)?;
        list[index + 1..]
            .iter()
            .find(|row| !leaving.contains(&row.id))
            .or_else(|| list[..index].iter().rev().find(|row| !leaving.contains(&row.id)))
            .map(|row| row.id.clone())
    }

    /// Opens the next thread after ones that leave the list, as Newton did.
    fn step_away(&mut self, leaving: &HashSet<String>, cx: &mut Context<Self>) {
        if let Some(opened) = self.conversation.as_ref().map(|open| open.id.clone())
            && leaving.contains(&opened)
        {
            match self.neighbour(&opened, leaving) {
                Some(next) => self.open(&next, cx),
                None => self.conversation = None,
            }
            return;
        }
        if let Some(selected) = self.selected.clone()
            && leaving.contains(&selected)
        {
            self.selected = self.neighbour(&selected, leaving);
        }
    }

    /// Moves the keyboard's selection, or the open thread, by `step` rows.
    pub fn move_by(&mut self, step: isize, cx: &mut Context<Self>) {
        let list = self.visible_rows();
        if list.is_empty() {
            return;
        }
        let count = list.len() as isize;
        let index = self
            .current()
            .and_then(|id| list.iter().position(|row| row.id == id))
            .map(|index| index as isize)
            .unwrap_or(if step > 0 { -1 } else { count });
        let next_index = (index + step).clamp(0, count - 1) as usize;
        let next = list[next_index].id.clone();
        if self.conversation.is_some() {
            self.open(&next, cx);
        } else {
            self.selected = Some(next);
        }
        self.ready_around(next_index, cx);
        cx.notify();
    }

    /// Fetches the bodies of the rows around one about to be opened, and the next page near the end.
    fn ready_around(&mut self, index: usize, cx: &mut Context<Self>) {
        let list = self.visible_rows();
        let around: Vec<String> =
            list[index.saturating_sub(2)..(index + 5).min(list.len())].iter().map(|row| row.id.clone()).collect();
        let near_end = index + 20 >= list.len();
        self.prefetch(around);
        if near_end {
            self.load_more(cx);
        }
    }

    /// Asks the core for the bodies of threads about to be opened.
    pub fn prefetch(&mut self, threads: Vec<String>) {
        let fresh: Vec<String> = threads.into_iter().filter(|thread| self.prefetched.insert(thread.clone())).collect();
        if !fresh.is_empty() {
            self.core.send(Command::Prefetch { threads: fresh });
        }
    }

    // MARK: Writing

    /// Opens a message to write. One already open is kept as a draft first, as closing it would.
    pub fn open_compose(&mut self, state: ComposeState, cx: &mut Context<Self>) {
        if let Some(open) = self.compose.take() {
            let keep = open.state.keep || open.state.draft_id.is_some() || open.state.has_content();
            self.close_compose(open, keep, cx);
        }
        let id = self.next_id();
        self.compose = Some(Compose { id, state });
        self.palette = None;
        self.settings = None;
    }

    pub fn new_message(&mut self, cx: &mut Context<Self>) {
        self.start_draft(Vec::new(), cx);
    }

    pub fn write_to(&mut self, address: Address, cx: &mut Context<Self>) {
        self.start_draft(vec![address], cx);
    }

    fn start_draft(&mut self, to: Vec<Address>, cx: &mut Context<Self>) {
        self.ask_or_show(Command::NewDraft { account: None }, cx, move |this, made: DraftReply, cx| {
            let mut draft = made.draft;
            draft.to = to;
            this.open_compose(ComposeState::new(draft, made.from), cx);
        });
    }

    /// Answers `message`, else the thread's last message from someone else.
    pub fn reply(&mut self, kind: ReplyKind, thread: Option<String>, message: Option<String>, cx: &mut Context<Self>) {
        let Some(thread) = thread.or_else(|| self.current()) else { return };
        let command = Command::ReplyDraft { thread: thread.clone(), kind, message };
        self.ask_or_show(command, cx, move |this, reply: DraftReply, cx| {
            let mut state = ComposeState::new(reply.draft, reply.from);
            state.show_cc = !state.draft.cc.is_empty();
            state.thread = Some(thread);
            this.open_compose(state, cx);
        });
    }

    pub fn open_draft(&mut self, id: &str, cx: &mut Context<Self>) {
        let wanted = id.to_string();
        self.ask_or_show(Command::OpenDraft { draft_id: id.to_string() }, cx, move |this, opened: DraftReply, cx| {
            let thread = this
                .conversation
                .as_ref()
                .filter(|open| open.draft_id.as_ref() == Some(&wanted))
                .map(|open| open.id.clone());
            let mut state = ComposeState::new(opened.draft, opened.from);
            state.draft_id = Some(opened.id.unwrap_or(wanted));
            state.thread = thread;
            this.open_compose(state, cx);
        });
    }

    /// Keeps what a compose holds, after any save of it still on its way, and answers the saved
    /// draft's id.
    pub fn save_draft(
        &mut self,
        id: u64,
        state: &ComposeState,
        cx: &mut Context<Self>,
    ) -> Shared<Task<Option<String>>> {
        self.keep_draft(id, state, cx);
        self.draft_saves[&id].clone()
    }

    /// Keeps the draft, without waiting for it.
    pub fn keep_draft(&mut self, id: u64, state: &ComposeState, cx: &mut Context<Self>) {
        let previous = self.draft_saves.get(&id).cloned();
        let known = state.draft_id.clone();
        let draft = state.draft.clone();
        let core = self.core.clone();
        let save = cx.background_spawn(async move {
            let id = match previous {
                Some(previous) => previous.await.or(known),
                None => known,
            };
            let answer = core.call::<IdReply>(Command::SaveDraft { draft_id: id.clone(), draft }).await;
            answer.map(|reply| reply.id).ok().or(id)
        });
        self.draft_saves.insert(id, save.shared());
    }

    /// The saved draft's id once the last save of this compose is done.
    fn saved_id(&mut self, id: u64, known: Option<String>) -> impl Future<Output = Option<String>> + use<> {
        let save = self.draft_saves.remove(&id);
        async move {
            match save {
                Some(save) => save.await.or(known),
                None => known,
            }
        }
    }

    /// Closes the compose; what was written is kept as a draft.
    pub fn close_compose(&mut self, compose: Compose, keep: bool, cx: &mut Context<Self>) {
        self.compose = None;
        if keep {
            self.keep_draft(compose.id, &compose.state, cx);
            self.show("Draft saved.", cx);
        }
        let saved = self.saved_id(compose.id, None);
        cx.background_spawn(saved).detach();
        cx.notify();
    }

    pub fn discard_draft(&mut self, compose: Compose, cx: &mut Context<Self>) {
        self.compose = None;
        let saved = self.saved_id(compose.id, compose.state.draft_id.clone());
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            if let Some(id) = saved.await {
                let _ = core.call::<Empty>(Command::DeleteDraft { draft_id: id }).await;
            }
            let _ = this.update(cx, |this, cx| this.show("Draft discarded.", cx));
        })
        .detach();
        cx.notify();
    }

    /// Sends after the undo delay, or at `send_at`; archives the thread it answers with `archive`.
    pub fn send(&mut self, compose: Compose, send_at: Option<i64>, archive: bool, cx: &mut Context<Self>) {
        self.compose = None;
        if archive && let Some(thread) = &compose.state.thread {
            self.act(Action::Archive, std::slice::from_ref(thread), None, None, cx);
        }
        let saved = self.saved_id(compose.id, compose.state.draft_id.clone());
        let core = self.core.clone();
        let delay = self.local.undo_delay;
        cx.spawn(async move |this, cx| {
            let draft_id = saved.await;
            let command = Command::Send {
                draft: compose.state.draft.clone(),
                delay,
                send_at,
                remind_at: compose.state.remind_at.as_ref().map(|time| time.until),
                draft_id: draft_id.clone(),
            };
            let result = core.call::<SendReply>(command).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(sent) => {
                        let op = sent.op_id;
                        if let Some(at) = send_at {
                            let when = crate::ui::format_time(at);
                            this.show_with(format!("Sending {when}."), "Undo", 4, cx, move |this, cx| {
                                this.undo_send(op, cx)
                            });
                        } else if delay > 0 {
                            this.show_with("Sending", "Undo", delay, cx, move |this, cx| this.undo_send(op, cx));
                        }
                    }
                    Err(error) => {
                        this.show(error, cx);
                        let mut again = compose.state;
                        again.draft_id = draft_id;
                        this.open_compose(again, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn undo_send(&mut self, op_id: String, cx: &mut Context<Self>) {
        self.ask_or_show(Command::CancelSend { op_id }, cx, |this, cancelled: CancelReply, cx| {
            let account = this.accounts().into_iter().find(|account| account.id == cancelled.draft.account_id);
            let from = cancelled.draft.from.as_ref().map(|from| from.email.clone());
            let from = from.or(account.map(|account| account.address)).unwrap_or_default();
            let mut state = ComposeState::new(cancelled.draft, from);
            state.draft_id = cancelled.id;
            state.keep = true;
            this.open_compose(state, cx);
        });
    }

    // MARK: The open thread

    /// An attachment as a file on this device, downloaded first if it has to be.
    pub fn open_attachment(
        &mut self,
        message: &str,
        index: usize,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, String, &mut Context<Self>) + 'static,
    ) {
        let key = format!("{message}/{index}");
        self.downloading.insert(key.clone());
        let command = Command::OpenAttachment { message: message.to_string(), index };
        self.ask(command, cx, move |this, result: Result<PathReply, String>, cx| {
            this.downloading.remove(&key);
            match result {
                Ok(found) => done(this, found.path, cx),
                Err(error) => this.show(error, cx),
            }
        });
        cx.notify();
    }

    /// Whether a message of the open thread shows only its line.
    pub fn is_folded(&self, message: &MessageItem) -> bool {
        self.folded_messages.contains(&message.id) || (message.folded && !self.unfolded_messages.contains(&message.id))
    }

    pub fn toggle_fold(&mut self, message: &MessageItem, cx: &mut Context<Self>) {
        if self.is_folded(message) {
            self.folded_messages.remove(&message.id);
            self.unfolded_messages.insert(message.id.clone());
        } else {
            self.folded_messages.insert(message.id.clone());
            self.unfolded_messages.remove(&message.id);
        }
        cx.notify();
    }

    pub fn any_message_folded(&self) -> bool {
        self.conversation.as_ref().is_some_and(|open| open.messages.iter().any(|message| self.is_folded(message)))
    }

    /// Unfolds every message of the open thread, or folds them all when none is folded.
    pub fn toggle_all_messages(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.conversation else { return };
        let ids: Vec<String> = open.messages.iter().map(|message| message.id.clone()).collect();
        if self.any_message_folded() {
            self.folded_messages.clear();
            self.unfolded_messages.extend(ids);
        } else {
            self.folded_messages = ids.into_iter().collect();
            self.unfolded_messages.clear();
        }
        cx.notify();
    }

    /// Goes to the next or the previous message of the open thread, unfolding it.
    pub fn move_message(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(open) = &self.conversation else { return };
        let messages = &open.messages;
        if messages.is_empty() {
            return;
        }
        let count = messages.len() as isize;
        let index = self
            .focused_message
            .as_ref()
            .and_then(|id| messages.iter().position(|message| &message.id == id))
            .map(|index| index as isize)
            .unwrap_or(if step > 0 { -1 } else { count });
        let next = messages[(index + step).clamp(0, count - 1) as usize].id.clone();
        self.unfolded_messages.insert(next.clone());
        self.folded_messages.remove(&next);
        self.focused_message = Some(next);
        cx.notify();
    }

    // MARK: Search

    pub fn search(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_query = query.clone();
        if query.trim().is_empty() {
            self.search_rows = None;
            cx.notify();
            return;
        }
        let command = Command::Search { query: query.clone(), account: self.list_account() };
        self.ask(command, cx, move |this, found: Result<SearchReply, String>, _| {
            let Ok(found) = found else { return };
            if this.search_query != query {
                return;
            }
            this.search_request = found.request;
            this.search_rows = Some(found.rows);
        });
    }

    pub fn end_search(&mut self, cx: &mut Context<Self>) {
        self.search_query.clear();
        self.search_rows = None;
        cx.notify();
    }

    // MARK: Accounts

    /// Opens Google's sign-in in the browser, which comes back to `wonnet://auth`.
    pub fn sign_in_with_google(&mut self, cx: &mut Context<Self>) {
        self.ask_or_show(Command::SignInGoogle, cx, |this, start: UrlReply, cx| {
            this.signing_in = Some(std::time::Instant::now());
            cx.open_url(&start.url);
        });
    }

    /// Forgets a sign-in in the browser that the app came back to without: one left there.
    pub fn forget_sign_in(&mut self) {
        if self.signing_in.is_some_and(|started| started.elapsed() > Duration::from_secs(20)) {
            self.signing_in = None;
        }
    }

    pub fn finish_sign_in(&mut self, url: String, cx: &mut Context<Self>) {
        self.signing_in = None;
        self.ask(Command::FinishSignIn { url }, cx, |this, result: Result<Empty, String>, cx| match result {
            Ok(_) => this.refresh(cx),
            Err(error) => this.show(error, cx),
        });
    }

    pub fn add_jmap(
        &mut self,
        url: String,
        username: String,
        password: String,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<(), String>, &mut Context<Self>) + 'static,
    ) {
        self.ask(Command::AddJmapAccount { url, username, password }, cx, |this, result: Result<Empty, String>, cx| {
            if result.is_ok() {
                this.refresh(cx);
            }
            done(this, result.map(|_| ()), cx);
        });
    }

    pub fn set_server(
        &mut self,
        url: String,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<(), String>, &mut Context<Self>) + 'static,
    ) {
        self.ask(Command::SetServer { url }, cx, |this, result: Result<Empty, String>, cx| {
            if result.is_ok() {
                this.refresh(cx);
            }
            done(this, result.map(|_| ()), cx);
        });
    }

    pub fn remove_account(&mut self, account: String, cx: &mut Context<Self>) {
        self.ask(Command::RemoveAccount { account }, cx, |this, _: Result<Empty, String>, cx| {
            this.show("Removing the account", cx)
        });
    }

    pub fn set_account_color(&mut self, account: &str, color: &str, cx: &mut Context<Self>) {
        let command = Command::SetAccountColor { account: account.to_string(), color: color.to_string() };
        self.ask(command, cx, |this, result: Result<Empty, String>, cx| {
            if let Err(error) = result {
                this.show(error, cx);
            }
        });
    }

    /// Names the account, or takes its name away when `name` is empty.
    pub fn set_account_name(&mut self, account: &AccountView, name: &str, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if name == account.name {
            return;
        }
        let command = Command::SetAccountName { account: account.id.clone(), name };
        self.ask(command, cx, |this, result: Result<Empty, String>, cx| {
            if let Err(error) = result {
                this.show(error, cx);
            }
        });
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::SignOut, cx, |this, _: Result<Empty, String>, cx| this.refresh(cx));
    }

    // MARK: Toasts

    pub fn show(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        let id = self.next_id();
        self.toast = Some(Toast { id, message: message.into(), action: None, seconds: 4 });
        self.toast_action = None;
        self.hide_later(id, 4, cx);
        cx.notify();
    }

    pub fn show_with(
        &mut self,
        message: impl Into<String>,
        action: &str,
        seconds: u64,
        cx: &mut Context<Self>,
        perform: impl FnOnce(&mut Store, &mut Context<Store>) + 'static,
    ) {
        let id = self.next_id();
        self.toast = Some(Toast { id, message: message.into(), action: Some(action.into()), seconds });
        self.toast_action = Some(Box::new(perform));
        self.hide_later(id, seconds.max(1), cx);
        cx.notify();
    }

    fn hide_later(&self, id: u64, seconds: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(seconds)).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|toast| toast.id == id) {
                    this.toast = None;
                    this.toast_action = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn toast_tapped(&mut self, cx: &mut Context<Self>) {
        self.toast = None;
        if let Some(action) = self.toast_action.take() {
            action(self, cx);
        }
        cx.notify();
    }

    /// Fills in what a preference changed for every view that reads it.
    pub fn set_preference(&mut self, key: &str, value: Option<Value>, cx: &mut Context<Self>) {
        match &value {
            Some(value) => self.preferences.insert(key.to_string(), value.clone()),
            None => self.preferences.remove(key),
        };
        let reopen = key == "remote_images" || key == "dark_mail";
        let command = Command::SetPreference { key: key.to_string(), value };
        self.ask(command, cx, move |this, result: Result<Empty, String>, cx| match result {
            Ok(_) if reopen => this.reopen(cx),
            Ok(_) => {}
            Err(error) => {
                this.show(error, cx);
                this.load_preferences(cx);
            }
        });
        cx.notify();
    }

    pub fn load_preferences(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::Preferences, cx, |this, list: Result<PreferenceList, String>, _| {
            let Ok(list) = list else { return };
            this.preferences = list.map();
            this.preferences_loaded = true;
        });
    }

    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.preferences.get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    /// A preference that is on or off, removed while it is as it starts.
    pub fn set_flag(&mut self, key: &str, on: bool, default: bool, cx: &mut Context<Self>) {
        self.set_preference(key, (on != default).then(|| json!(on)), cx);
    }
}

/// A saved draft's row, not a thread's.
pub fn is_draft(thread: &str) -> bool {
    thread.starts_with("draft:")
}
