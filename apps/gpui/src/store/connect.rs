//! An application asking to connect, from a `wonnet://connect` link, or one connected being
//! changed, and the applications connected, as Settings lists them.

use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use gpui::Context;
use mail_core::api::Command;

use super::{Confirmation, Store};
use crate::models::{
    Access, AccessSummary, ConnectRequest, ConnectedApp, ConnectedApps, Empty, FilterEdit, FilterKind, FilterLine,
    FilterOption, FilterOptions, FilterValue, McpServer,
};

/// The sheet a `wonnet://connect` link or a connected app's Change opens; `id` tells it from the
/// one before.
pub struct ConnectSheet {
    pub id: u64,
    /// It changes a connected app rather than answering a request.
    pub changing: bool,
    pub state: ConnectState,
}

pub enum ConnectState {
    Loading,
    Failed(String),
    Asking(Box<ConnectChoices>),
    /// Approved: the application's name.
    Approved(String),
}

/// The request, the access chosen so far, and what the core says it comes to.
pub struct ConnectChoices {
    pub request: ConnectRequest,
    pub access: Access,
    pub summary: Option<AccessSummary>,
    pub menu: Option<FilterMenu>,
    /// The approval (or the change) is on its way, or waits for the summaries to settle.
    pub answering: bool,
    pub error: Option<String>,
    /// A summary is on its way: what changes meanwhile waits for it, so each builds on the last.
    asking: bool,
    changed: bool,
    edits: VecDeque<FilterEdit>,
}

impl ConnectChoices {
    fn new(request: ConnectRequest) -> Self {
        Self {
            access: request.access.clone(),
            request,
            summary: None,
            menu: None,
            answering: false,
            error: None,
            asking: false,
            changed: false,
            edits: VecDeque::new(),
        }
    }

    /// It changes a connected app rather than answering a request.
    pub fn changing(&self) -> bool {
        self.request.app.is_some()
    }

    /// Nothing stands in the way of approving it.
    pub fn ready(&self) -> bool {
        self.summary.as_ref().is_some_and(|summary| summary.problem.is_none())
    }

    fn settled(&self) -> bool {
        !self.asking && !self.changed && self.edits.is_empty()
    }
}

/// The filter's menu: the line it adds to, the chip it changes (`editing`), the kind picked, the
/// words typed and the options the core found for them. Only the answer to the latest ask is
/// kept, and the options shown stay until it comes.
pub struct FilterMenu {
    pub line: FilterLine,
    pub editing: Option<FilterValue>,
    pub scope: Option<FilterKind>,
    pub query: String,
    pub options: Option<FilterOptions>,
    /// The option the arrow keys are on, of `picks`.
    pub highlighted: usize,
    /// How many answers came, to tell a new list from the one before.
    pub answers: u64,
    /// The pointer moved the highlight last, not the keys: the list doesn't scroll to it.
    pub hovered: bool,
    asked: u64,
}

/// What a key does to the menu.
#[derive(Debug, PartialEq, Eq)]
pub enum MenuKeyResult {
    Ignored,
    Handled,
    /// The menu asks again: the kind or the words changed.
    Changed,
    Pick(usize),
    Close,
}

impl FilterMenu {
    pub fn new(line: FilterLine, editing: Option<FilterValue>) -> Self {
        let scope = editing.as_ref().map(|value| value.kind);
        Self {
            line,
            editing,
            scope,
            query: String::new(),
            options: None,
            highlighted: 0,
            answers: 0,
            hovered: false,
            asked: 0,
        }
    }

    /// Whether the chip (or with `None`, the line's add chip) opened it.
    pub fn opened_by(&self, line: FilterLine, editing: Option<&FilterValue>) -> bool {
        self.line == line && self.editing.as_ref() == editing
    }

    pub fn picks(&self) -> Vec<&FilterOption> {
        self.options.iter().flat_map(FilterOptions::options).collect()
    }

    /// The options for the latest ask: the first list starts on the checked option.
    fn take(&mut self, options: FilterOptions) {
        let first = self.options.is_none();
        self.highlighted = if first { options.options().position(|option| option.checked).unwrap_or(0) } else { 0 };
        self.options = Some(options);
        self.answers += 1;
    }

    /// Narrows it to a kind, or takes the kind away.
    pub fn narrow(&mut self, kind: Option<FilterKind>) {
        self.scope = kind;
        self.query.clear();
        self.highlighted = 0;
    }

    pub fn set_query(&mut self, query: String) -> bool {
        if self.query == query {
            return false;
        }
        self.query = query;
        self.highlighted = 0;
        true
    }

    /// ↑ ↓ move over the options, Return picks, Esc takes the words, then the kind (when adding),
    /// then closes; Backspace with nothing typed takes the kind (when adding).
    pub fn key(&mut self, key: &str) -> MenuKeyResult {
        let count = self.picks().len();
        self.hovered = false;
        match key {
            "up" => self.highlighted = self.highlighted.saturating_sub(1),
            "down" => self.highlighted = (self.highlighted + 1).min(count.saturating_sub(1)),
            "enter" if self.highlighted < count => return MenuKeyResult::Pick(self.highlighted),
            "enter" => {}
            "escape" if !self.query.is_empty() => {
                self.set_query(String::new());
                return MenuKeyResult::Changed;
            }
            "escape" if self.scope.is_some() && self.editing.is_none() => {
                self.narrow(None);
                return MenuKeyResult::Changed;
            }
            "escape" => return MenuKeyResult::Close,
            "backspace" if self.query.is_empty() && self.scope.is_some() && self.editing.is_none() => {
                self.narrow(None);
                return MenuKeyResult::Changed;
            }
            _ => return MenuKeyResult::Ignored,
        }
        MenuKeyResult::Handled
    }

    /// The edit an option makes: none for a kind, which narrows the menu; a value adds itself, or
    /// takes the place of the chip being changed.
    pub fn edit_for(&self, option: &FilterOption) -> Option<FilterEdit> {
        let value = option.value.clone()?;
        Some(FilterEdit {
            line: self.line,
            remove: self.editing.clone(),
            add: Some(FilterValue { kind: option.kind, value }),
        })
    }
}

/// Settings' Connected apps: the address to connect one to, and the ones connected.
#[derive(Default)]
pub struct Connected {
    pub address: Option<String>,
    pub address_error: Option<String>,
    pub apps: Option<Vec<ConnectedApp>>,
    pub loading: bool,
    pub error: Option<String>,
    /// The apps whose Revoke is on its way.
    pub revoking: HashSet<String>,
}

fn deny(key: String) -> Command {
    Command::AnswerConnectRequest { key, approve: false, access: None }
}

impl Store {
    // MARK: A request to connect, or a change

    /// Opens the sheet over everything at once, and asks the server about the request.
    pub fn open_connect(&mut self, url: String, cx: &mut Context<Self>) {
        self.open_sheet(Command::ConnectRequest { url }, false, cx);
    }

    /// Opens the sheet for a connected app, to change what it may do.
    pub fn edit_connected_app(&mut self, app: String, cx: &mut Context<Self>) {
        self.open_sheet(Command::EditConnectedApp { app }, true, cx);
    }

    fn open_sheet(&mut self, command: Command, changing: bool, cx: &mut Context<Self>) {
        self.close_connect(cx);
        let id = self.next_id();
        self.connect = Some(ConnectSheet { id, changing, state: ConnectState::Loading });
        self.ask(command, cx, move |this, result: Result<ConnectRequest, String>, cx| {
            let Some(sheet) = this.connect.as_mut().filter(|sheet| sheet.id == id) else {
                if let Ok(request) = result
                    && request.app.is_none()
                {
                    this.core.send(deny(request.key));
                }
                return;
            };
            match result {
                Ok(request) => {
                    sheet.state = ConnectState::Asking(Box::new(ConnectChoices::new(request)));
                    this.refresh_access(None, cx);
                }
                Err(error) => sheet.state = ConnectState::Failed(error),
            }
        });
        cx.notify();
    }

    /// The choices of the sheet `id`, while it asks.
    fn choices_of(&mut self, id: u64) -> Option<&mut ConnectChoices> {
        match self.connect.as_mut() {
            Some(ConnectSheet { id: open, state: ConnectState::Asking(choices), .. }) if *open == id => Some(choices),
            _ => None,
        }
    }

    /// The choices of the sheet open, and its id.
    fn choices(&mut self) -> Option<(u64, &mut ConnectChoices)> {
        match self.connect.as_mut() {
            Some(ConnectSheet { id, state: ConnectState::Asking(choices), .. }) => Some((*id, choices)),
            _ => None,
        }
    }

    /// Changes the access at once, then asks what it comes to.
    pub fn change_access(&mut self, change: impl FnOnce(&mut Access), cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        change(&mut choices.access);
        choices.error = None;
        self.refresh_access(None, cx);
    }

    /// Asks the core what the access comes to after `edit`, one ask after the other: what
    /// changes while one is on its way is sent once it is back.
    fn refresh_access(&mut self, edit: Option<FilterEdit>, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        match edit {
            Some(edit) => choices.edits.push_back(edit),
            None => choices.changed = true,
        }
        if !choices.asking {
            self.ask_summary(cx);
        }
        cx.notify();
    }

    fn ask_summary(&mut self, cx: &mut Context<Self>) {
        let Some((id, choices)) = self.choices() else { return };
        let edit = choices.edits.pop_front();
        choices.changed = false;
        choices.asking = true;
        let command =
            Command::AccessSummary { client: choices.request.client.clone(), access: choices.access.clone(), edit };
        self.ask(command, cx, move |this, result: Result<AccessSummary, String>, cx| {
            let Some(choices) = this.choices_of(id) else { return };
            choices.asking = false;
            match result {
                // What changed here since the ask stays; the filter is the core's, which applied the edit.
                Ok(summary) if choices.changed => {
                    choices.access.only = summary.access.only.clone();
                    choices.access.never = summary.access.never.clone();
                    choices.summary = Some(summary);
                }
                Ok(summary) => {
                    choices.access = summary.access.clone();
                    choices.summary = Some(summary);
                }
                Err(error) => {
                    choices.error = Some(error);
                    choices.answering = false;
                }
            }
            if !choices.settled() {
                this.ask_summary(cx);
            } else if choices.answering {
                this.send_answer(cx);
            }
        });
    }

    // MARK: The filter's menu

    /// Opens the menu from a chip (`editing`) or a line's add chip; from the one that opened it,
    /// closes it.
    pub fn toggle_filter_menu(&mut self, line: FilterLine, editing: Option<FilterValue>, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        if choices.menu.as_ref().is_some_and(|menu| menu.opened_by(line, editing.as_ref())) {
            choices.menu = None;
        } else {
            choices.menu = Some(FilterMenu::new(line, editing));
            self.ask_filter_options(cx);
        }
        cx.notify();
    }

    pub fn close_filter_menu(&mut self, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        if choices.menu.take().is_some() {
            cx.notify();
        }
    }

    pub fn set_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        let Some(menu) = self.choices().and_then(|(_, choices)| choices.menu.as_mut()) else { return };
        if menu.set_query(query) {
            self.ask_filter_options(cx);
        }
    }

    pub fn narrow_filter_menu(&mut self, kind: Option<FilterKind>, cx: &mut Context<Self>) {
        let Some(menu) = self.choices().and_then(|(_, choices)| choices.menu.as_mut()) else { return };
        menu.narrow(kind);
        self.ask_filter_options(cx);
    }

    /// A key in the menu's field; true when the menu took it.
    pub fn filter_menu_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(menu) = self.choices().and_then(|(_, choices)| choices.menu.as_mut()) else { return false };
        match menu.key(key) {
            MenuKeyResult::Ignored => return false,
            MenuKeyResult::Handled => cx.notify(),
            MenuKeyResult::Changed => self.ask_filter_options(cx),
            MenuKeyResult::Pick(index) => self.pick_filter_option(index, cx),
            MenuKeyResult::Close => self.close_filter_menu(cx),
        }
        true
    }

    /// The pointer on an option highlights it.
    pub fn hover_filter_option(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(menu) = self.choices().and_then(|(_, choices)| choices.menu.as_mut()) else { return };
        if menu.highlighted == index {
            return;
        }
        menu.highlighted = index;
        menu.hovered = true;
        cx.notify();
    }

    /// A kind narrows the menu to it; a value is added (or takes the place of the chip being
    /// changed) and the menu closes.
    pub fn pick_filter_option(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        let Some(menu) = choices.menu.as_mut() else { return };
        let Some(option) = menu.picks().get(index).map(|option| (*option).clone()) else { return };
        let Some(edit) = menu.edit_for(&option) else {
            menu.narrow(Some(option.kind));
            self.ask_filter_options(cx);
            return;
        };
        choices.menu = None;
        self.refresh_access(Some(edit), cx);
    }

    /// Takes a chip away; the menu closes.
    pub fn remove_filter(&mut self, line: FilterLine, value: FilterValue, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        choices.menu = None;
        self.refresh_access(Some(FilterEdit { line, remove: Some(value), add: None }), cx);
    }

    fn ask_filter_options(&mut self, cx: &mut Context<Self>) {
        let asked = self.next_id();
        let Some((id, choices)) = self.choices() else { return };
        let Some(menu) = choices.menu.as_mut() else { return };
        menu.asked = asked;
        let command = Command::FilterOptions {
            access: choices.access.clone(),
            line: menu.line,
            kind: menu.scope,
            query: menu.query.clone(),
            editing: menu.editing.clone(),
        };
        self.ask(command, cx, move |this, result: Result<FilterOptions, String>, cx| {
            let menu = this.choices_of(id).and_then(|choices| choices.menu.as_mut()).filter(|menu| menu.asked == asked);
            let Some(menu) = menu else { return };
            match result {
                Ok(options) => menu.take(options),
                Err(error) => this.show(error, cx),
            }
        });
        cx.notify();
    }

    // MARK: Answering

    /// Closes the sheet; a request left unanswered is denied quietly, so the browser stops waiting.
    pub fn close_connect(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.connect.take() else { return };
        if let ConnectState::Asking(choices) = sheet.state
            && !choices.answering
            && !choices.changing()
        {
            self.core.send(deny(choices.request.key));
        }
        cx.notify();
    }

    /// Enter: Approve (or Save) while it asks, Done once approved.
    pub fn connect_default(&mut self, cx: &mut Context<Self>) {
        match self.connect.as_ref().map(|sheet| &sheet.state) {
            Some(ConnectState::Asking(_)) => self.approve_connect(cx),
            Some(ConnectState::Approved(_)) => self.close_connect(cx),
            _ => {}
        }
    }

    pub fn deny_connect(&mut self, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        if choices.answering || choices.changing() {
            return;
        }
        let (key, client) = (choices.request.key.clone(), choices.request.client.clone());
        self.connect = None;
        self.show(format!("Denied {client}."), cx);
        self.ask(deny(key), cx, |this, result: Result<Empty, String>, cx| {
            if let Err(error) = result {
                this.show(error, cx);
            }
        });
    }

    /// Approve, or Save for a connected app: sent once the changes on their way are back, so it
    /// sends the access they made.
    pub fn approve_connect(&mut self, cx: &mut Context<Self>) {
        let Some((_, choices)) = self.choices() else { return };
        if choices.answering || !choices.ready() {
            return;
        }
        choices.answering = true;
        choices.error = None;
        choices.menu = None;
        if choices.settled() {
            self.send_answer(cx);
        }
        cx.notify();
    }

    fn send_answer(&mut self, cx: &mut Context<Self>) {
        let Some((id, choices)) = self.choices() else { return };
        if !choices.ready() {
            choices.answering = false;
            return;
        }
        let client = choices.request.client.clone();
        let app = choices.request.app.clone();
        let command = match &app {
            Some(app) => Command::SetConnectedAppAccess { app: app.clone(), access: choices.access.clone() },
            None => Command::AnswerConnectRequest {
                key: choices.request.key.clone(),
                approve: true,
                access: Some(choices.access.clone()),
            },
        };
        self.ask(command, cx, move |this, result: Result<Empty, String>, cx| {
            let Some(choices) = this.choices_of(id) else {
                if let Err(error) = result {
                    this.show(error, cx);
                }
                return;
            };
            if let Err(error) = result {
                choices.answering = false;
                choices.error = Some(error);
                return;
            }
            if app.is_none() {
                this.connect = Some(ConnectSheet { id, changing: false, state: ConnectState::Approved(client) });
                return;
            }
            this.connect = None;
            this.show("Saved.", cx);
            this.load_connected(cx);
        });
    }

    // MARK: Connected apps

    /// The address to connect an application to, at once, and the ones connected, from the server.
    pub fn load_connected(&mut self, cx: &mut Context<Self>) {
        self.ask(Command::McpServer, cx, |this, result: Result<McpServer, String>, _| match result {
            Ok(server) => {
                this.connected.address = Some(server.address);
                this.connected.address_error = None;
            }
            Err(error) => this.connected.address_error = Some(error),
        });
        self.connected.loading = true;
        self.connected.error = None;
        self.ask(Command::ConnectedApps, cx, |this, result: Result<ConnectedApps, String>, _| {
            this.connected.loading = false;
            match result {
                Ok(found) => {
                    this.connected.address = Some(found.address);
                    this.connected.address_error = None;
                    this.connected.apps = Some(found.apps);
                }
                Err(error) => this.connected.error = Some(error),
            }
        });
    }

    pub fn confirm_revoking(&mut self, app: &ConnectedApp, cx: &mut Context<Self>) {
        let id = app.id.clone();
        self.confirmation = Some(Confirmation {
            title: format!("Revoke {}?", app.name),
            message: "It can no longer reach your mail. You can connect it again.".into(),
            action: "Revoke".into(),
            cancel: "Cancel",
            safe: false,
            perform: Rc::new(move |this, cx| this.revoke(id.clone(), cx)),
        });
        cx.notify();
    }

    fn revoke(&mut self, app: String, cx: &mut Context<Self>) {
        self.connected.revoking.insert(app.clone());
        let command = Command::RevokeConnectedApp { app: app.clone() };
        self.ask(command, cx, move |this, result: Result<Empty, String>, cx| {
            this.connected.revoking.remove(&app);
            match result {
                Ok(_) => {
                    if let Some(apps) = &mut this.connected.apps {
                        apps.retain(|connected| connected.id != app);
                    }
                }
                Err(error) => this.show(error, cx),
            }
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FilterSection;

    fn option(id: &str, value: Option<&str>, checked: bool) -> FilterOption {
        FilterOption {
            id: id.into(),
            kind: FilterKind::From,
            value: value.map(Into::into),
            title: id.into(),
            detail: None,
            symbol: None,
            avatar: None,
            count: None,
            checked,
        }
    }

    fn options(list: Vec<FilterOption>) -> FilterOptions {
        let (first, rest) = list.split_at(1);
        FilterOptions {
            placeholder: String::new(),
            scope: None,
            sections: vec![
                FilterSection { title: None, options: first.to_vec() },
                FilterSection { title: Some("More".into()), options: rest.to_vec() },
            ],
            note: None,
        }
    }

    fn chase() -> FilterValue {
        FilterValue { kind: FilterKind::From, value: "@chase.com".into() }
    }

    #[test]
    fn starts_on_the_checked_option_then_on_the_first() {
        let mut menu = FilterMenu::new(FilterLine::Never, Some(chase()));
        assert_eq!(menu.scope, Some(FilterKind::From));
        menu.take(options(vec![
            option("a", Some("a"), false),
            option("b", Some("b"), false),
            option("c", Some("c"), true),
        ]));
        assert_eq!(menu.highlighted, 2);
        menu.set_query("x".into());
        assert_eq!(menu.highlighted, 0);
        menu.take(options(vec![option("a", Some("a"), false), option("c", Some("c"), true)]));
        assert_eq!(menu.highlighted, 0);
    }

    #[test]
    fn moves_over_the_options_without_wrapping() {
        let mut menu = FilterMenu::new(FilterLine::Only, None);
        menu.take(options(vec![option("a", None, false), option("b", None, false), option("c", None, false)]));
        assert_eq!(menu.key("up"), MenuKeyResult::Handled);
        assert_eq!(menu.highlighted, 0);
        menu.key("down");
        menu.key("down");
        menu.key("down");
        assert_eq!(menu.highlighted, 2);
        assert_eq!(menu.key("enter"), MenuKeyResult::Pick(2));
        assert_eq!(menu.key("left"), MenuKeyResult::Ignored);
    }

    #[test]
    fn escape_takes_the_words_then_the_kind_then_closes() {
        let mut menu = FilterMenu::new(FilterLine::Only, None);
        menu.narrow(Some(FilterKind::Label));
        menu.set_query("med".into());
        assert_eq!(menu.key("escape"), MenuKeyResult::Changed);
        assert_eq!((menu.query.as_str(), menu.scope), ("", Some(FilterKind::Label)));
        assert_eq!(menu.key("escape"), MenuKeyResult::Changed);
        assert_eq!(menu.scope, None);
        assert_eq!(menu.key("escape"), MenuKeyResult::Close);
    }

    #[test]
    fn keeps_the_kind_of_the_chip_being_changed() {
        let mut menu = FilterMenu::new(FilterLine::Never, Some(chase()));
        assert_eq!(menu.key("backspace"), MenuKeyResult::Ignored);
        assert_eq!(menu.key("escape"), MenuKeyResult::Close);
        let mut adding = FilterMenu::new(FilterLine::Never, None);
        adding.narrow(Some(FilterKind::From));
        assert_eq!(adding.key("backspace"), MenuKeyResult::Changed);
        assert_eq!(adding.scope, None);
        adding.set_query("a".into());
        assert_eq!(adding.key("backspace"), MenuKeyResult::Ignored);
    }

    #[test]
    fn a_value_adds_or_replaces_and_a_kind_narrows() {
        let changing = FilterMenu::new(FilterLine::Never, Some(chase()));
        let edit = changing.edit_for(&option("x", Some("@x.com"), false)).unwrap();
        assert_eq!(edit.remove, Some(chase()));
        assert_eq!(edit.add, Some(FilterValue { kind: FilterKind::From, value: "@x.com".into() }));
        assert!(changing.edit_for(&option("from", None, false)).is_none());
        let adding = FilterMenu::new(FilterLine::Only, None);
        assert_eq!(adding.edit_for(&option("x", Some("@x.com"), false)).unwrap().remove, None);
    }
}
