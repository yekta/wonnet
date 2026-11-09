//! An application asking to connect, from a `wonnet://connect` link, and the applications
//! connected, as Settings lists them.

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use gpui::Context;
use mail_core::api::Command;
use mail_protocol::access::{Access, Privilege, Role};

use super::{Confirmation, Store};
use crate::models::{ConnectRequest, ConnectedApp, ConnectedApps, Empty, McpServer};

/// The sheet a `wonnet://connect` link opens; `id` tells it from the one before.
pub struct ConnectSheet {
    pub id: u64,
    pub state: ConnectState,
}

pub enum ConnectState {
    Loading,
    Failed(String),
    Asking(Box<ConnectChoices>),
    /// Approved: the application's name.
    Approved(String),
}

/// The request, and what the user chose so far.
pub struct ConnectChoices {
    pub request: ConnectRequest,
    pub all_accounts: bool,
    pub accounts: BTreeSet<String>,
    pub role: String,
    pub privileges: BTreeSet<String>,
    /// The approval is on its way.
    pub answering: bool,
    pub error: Option<String>,
}

impl ConnectChoices {
    fn new(request: ConnectRequest) -> Self {
        Self {
            request,
            all_accounts: true,
            accounts: BTreeSet::new(),
            role: Role::Reader.as_str().into(),
            privileges: BTreeSet::new(),
            answering: false,
            error: None,
        }
    }

    /// Some mail is chosen, so it can be approved.
    pub fn ready(&self) -> bool {
        self.all_accounts || !self.accounts.is_empty()
    }

    fn access(&self) -> Access {
        Access {
            role: Role::ALL.into_iter().find(|role| role.as_str() == self.role).unwrap_or(Role::Reader),
            all_accounts: self.all_accounts,
            accounts: if self.all_accounts { Vec::new() } else { self.accounts.iter().cloned().collect() },
            privileges: Privilege::ALL
                .into_iter()
                .filter(|privilege| self.privileges.contains(privilege.as_str()))
                .collect(),
        }
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
    // MARK: A request to connect

    /// Opens the sheet over everything at once, and asks the server about the request.
    pub fn open_connect(&mut self, url: String, cx: &mut Context<Self>) {
        self.close_connect(cx);
        let id = self.next_id();
        self.connect = Some(ConnectSheet { id, state: ConnectState::Loading });
        self.ask(Command::ConnectRequest { url }, cx, move |this, result: Result<ConnectRequest, String>, _| {
            let Some(sheet) = this.connect.as_mut().filter(|sheet| sheet.id == id) else {
                if let Ok(request) = result {
                    this.core.send(deny(request.key));
                }
                return;
            };
            sheet.state = match result {
                Ok(request) => ConnectState::Asking(Box::new(ConnectChoices::new(request))),
                Err(error) => ConnectState::Failed(error),
            };
        });
        cx.notify();
    }

    /// Changes what the user chose.
    pub fn choose(&mut self, change: impl FnOnce(&mut ConnectChoices), cx: &mut Context<Self>) {
        let Some(ConnectSheet { state: ConnectState::Asking(choices), .. }) = &mut self.connect else { return };
        change(choices);
        choices.error = None;
        cx.notify();
    }

    /// Closes the sheet; a request left unanswered is denied quietly, so the browser stops waiting.
    pub fn close_connect(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.connect.take() else { return };
        if let ConnectState::Asking(choices) = sheet.state
            && !choices.answering
        {
            self.core.send(deny(choices.request.key));
        }
        cx.notify();
    }

    /// Enter: Approve while it asks, Done once approved.
    pub fn connect_default(&mut self, cx: &mut Context<Self>) {
        match self.connect.as_ref().map(|sheet| &sheet.state) {
            Some(ConnectState::Asking(_)) => self.approve_connect(cx),
            Some(ConnectState::Approved(_)) => self.close_connect(cx),
            _ => {}
        }
    }

    pub fn deny_connect(&mut self, cx: &mut Context<Self>) {
        let Some(ConnectSheet { state: ConnectState::Asking(choices), .. }) = &self.connect else { return };
        if choices.answering {
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

    pub fn approve_connect(&mut self, cx: &mut Context<Self>) {
        let Some(ConnectSheet { id, state: ConnectState::Asking(choices) }) = &mut self.connect else { return };
        if choices.answering || !choices.ready() {
            return;
        }
        choices.answering = true;
        choices.error = None;
        let id = *id;
        let client = choices.request.client.clone();
        let command = Command::AnswerConnectRequest {
            key: choices.request.key.clone(),
            approve: true,
            access: Some(choices.access()),
        };
        self.ask(command, cx, move |this, result: Result<Empty, String>, cx| {
            let Some(sheet) = this.connect.as_mut().filter(|sheet| sheet.id == id) else {
                if let Err(error) = result {
                    this.show(error, cx);
                }
                return;
            };
            let ConnectState::Asking(choices) = &mut sheet.state else { return };
            match result {
                Ok(_) => sheet.state = ConnectState::Approved(client),
                Err(error) => {
                    choices.answering = false;
                    choices.error = Some(error);
                }
            }
        });
        cx.notify();
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
