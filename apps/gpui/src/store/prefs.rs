//! What the synced preferences hold, read and changed as both Apple apps do: the tabs, the
//! sidebar's order, the splits, the snippets, the signatures, the keybindings and the settings'
//! pages.

use std::collections::BTreeMap;

use gpui::Context;
use serde_json::{Value, json};

use super::{Confirmation, Store};
use crate::keys::{self, KeyCommand, KeyScope};
use crate::models::{AccountView, Mailbox};
use crate::ui::Symbol;

pub const SIDEBAR_KEY: &str = "sidebar";
pub const ACCOUNTS_KEY: &str = "accounts";
const TABS_KEY: &str = "tabs";
const STANDARD_TABS: [&str; 4] = ["inbox", "unread", "sent", "snoozed"];

pub fn ids(key: &str, preferences: &BTreeMap<String, Value>) -> Vec<String> {
    preferences
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(|value| value.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// The items in the order of `ids`, then the rest as they came.
pub fn sorted<T: Clone>(items: &[T], ids: &[String], id: impl Fn(&T) -> &String) -> Vec<T> {
    let rank = |item: &T| ids.iter().position(|named| named == id(item)).unwrap_or(ids.len());
    let mut ranked: Vec<(usize, usize, &T)> =
        items.iter().enumerate().map(|(index, item)| (rank(item), index, item)).collect();
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked.into_iter().map(|(_, _, item)| item.clone()).collect()
}

/// Text kept to put in messages, `snippet:<id>`. `{first_name}` is filled from the first recipient.
#[derive(Clone, Debug, PartialEq)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub text: String,
}

impl Snippet {
    pub fn key(&self) -> String {
        format!("snippet:{}", self.id)
    }

    pub fn all(preferences: &BTreeMap<String, Value>) -> Vec<Snippet> {
        let mut snippets: Vec<Snippet> = preferences
            .iter()
            .filter_map(|(key, value)| {
                let id = key.strip_prefix("snippet:")?;
                let name = value.get("name")?.as_str()?;
                let text = value.get("text").and_then(Value::as_str).unwrap_or_default();
                Some(Snippet { id: id.into(), name: name.into(), text: text.into() })
            })
            .collect();
        snippets.sort_by_key(|snippet| snippet.name.to_lowercase());
        snippets
    }

    pub fn filled(&self, first_name: Option<&str>) -> String {
        match first_name {
            Some(name) => self.text.replace("{first_name}", name),
            None => self.text.clone(),
        }
    }
}

/// A tab of Split Inbox the user made, `split:<id>`.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomSplit {
    pub id: String,
    pub name: String,
    /// Addresses, and domains written `@x.com`.
    pub from: Vec<String>,
    pub label: Option<String>,
    pub order: i64,
}

impl CustomSplit {
    pub fn key(&self) -> String {
        format!("split:{}", self.id)
    }

    pub fn value(&self) -> Value {
        json!({ "name": self.name, "from": self.from, "label": self.label, "order": self.order })
    }

    pub fn all(preferences: &BTreeMap<String, Value>) -> Vec<CustomSplit> {
        let mut splits: Vec<CustomSplit> = preferences
            .iter()
            .filter_map(|(key, value)| {
                let id = key.strip_prefix("split:")?;
                let name = value.get("name")?.as_str()?;
                let from = value
                    .get("from")
                    .and_then(Value::as_array)
                    .map(|from| from.iter().filter_map(|value| value.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let label = value.get("label").and_then(Value::as_str).map(String::from);
                let order = value.get("order").and_then(Value::as_f64).unwrap_or(0.0) as i64;
                Some(CustomSplit { id: id.into(), name: name.into(), from, label, order })
            })
            .collect();
        splits.sort_by(|one, other| (one.order, &one.name).cmp(&(other.order, &other.name)));
        splits
    }

    /// The senders as they are typed: addresses and `@domains`, split on commas, spaces or lines.
    pub fn senders(text: &str) -> Vec<String> {
        text.split(|character: char| ", ;\n".contains(character))
            .filter(|part| !part.is_empty())
            .map(str::to_lowercase)
            .collect()
    }
}

/// A page of the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    General,
    Accounts,
    Inbox,
    Reading,
    Writing,
    Keybindings,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 6] =
        [Self::General, Self::Accounts, Self::Inbox, Self::Reading, Self::Writing, Self::Keybindings];

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Accounts => "Accounts",
            Self::Inbox => "Inbox",
            Self::Reading => "Reading",
            Self::Writing => "Writing",
            Self::Keybindings => "Keybindings",
        }
    }

    pub fn symbol(self) -> Symbol {
        match self {
            Self::General => Symbol::Settings,
            Self::Accounts => Symbol::AtSign,
            Self::Inbox => Symbol::Inbox,
            Self::Reading => Symbol::MailOpen,
            Self::Writing => Symbol::Pencil,
            Self::Keybindings => Symbol::Keyboard,
        }
    }
}

/// One group of settings, as the settings' search finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsEntry {
    pub id: String,
    pub title: String,
    pub section: SettingsSection,
    pub keywords: String,
}

impl SettingsEntry {
    fn new(id: &str, title: &str, section: SettingsSection, keywords: &str) -> Self {
        Self { id: id.into(), title: title.into(), section, keywords: keywords.into() }
    }

    pub fn all() -> Vec<SettingsEntry> {
        use SettingsSection::*;
        vec![
            Self::new("appearance", "Theme", General, "appearance light dark system mode"),
            Self::new("updates", "Updates", General, "version check for updates release about"),
            Self::new("server", "Server", General, "connection connected offline sync"),
            Self::new("accounts", "Accounts", Accounts, "add remove gmail jmap fastmail colour color notifications"),
            Self::new("signatures", "Signatures", Accounts, "sign off footer"),
            Self::new("sign-out", "Sign out", Accounts, "log out leave"),
            Self::new("split-inbox", "Split Inbox", Inbox, "important other newsletters notifications splits"),
            Self::new("tabs", "Tabs", Inbox, "mailboxes over the list order"),
            Self::new("selecting", "Selecting", Inbox, "select circles hover bulk batch checkbox"),
            Self::new(
                "mail",
                "Dark mode for mail",
                Reading,
                "designed newsletters colours remote images pictures privacy tracking",
            ),
            Self::new("blocked", "Blocked senders", Reading, "block unblock trash spam"),
            Self::new("sending", "Undo send", Writing, "delay cancel sending"),
            Self::new("snippets", "Snippets", Writing, "templates canned replies text"),
            Self::new("keybindings", "Keybindings", Keybindings, "keyboard shortcuts keys hotkeys superhuman"),
        ]
    }

    /// The groups every word of the query is found in, and the commands of the keys it names.
    pub fn matching(query: &str) -> Vec<SettingsEntry> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let keys = keys::all().into_iter().map(|command| {
            let keywords = format!("shortcut keys {}", command.group);
            Self::new(&format!("key-{}", command.id), &command.title, SettingsSection::Keybindings, &keywords)
        });
        Self::all()
            .into_iter()
            .chain(keys)
            .filter(|entry| {
                let text = format!("{} {} {}", entry.title, entry.section.title(), entry.keywords).to_lowercase();
                words.iter().all(|word| text.contains(word))
            })
            .collect()
    }
}

impl Store {
    // MARK: Tabs and the sidebar

    pub fn set_sidebar_order_mailboxes(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.set_preference(SIDEBAR_KEY, Some(json!(ids)), cx);
    }

    pub fn set_sidebar_order_accounts(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.set_preference(ACCOUNTS_KEY, Some(json!(ids)), cx);
    }

    fn tab_ids(&self) -> Vec<String> {
        match self.preferences.get(TABS_KEY).and_then(Value::as_array) {
            Some(_) => ids(TABS_KEY, &self.preferences),
            None => STANDARD_TABS.iter().map(|id| id.to_string()).collect(),
        }
    }

    /// The tabs as saved, in the user's order.
    pub fn saved_tabs(&self) -> Vec<Mailbox> {
        let all = self.all_mailboxes();
        self.tab_ids().iter().filter_map(|id| all.iter().find(|mailbox| &mailbox.id == id).cloned()).collect()
    }

    /// The tabs over the list, for the account on screen.
    pub fn tabs(&self) -> Vec<Mailbox> {
        let all = self.all_mailboxes();
        let mut shown = Vec::new();
        let mut tabs = Vec::new();
        for id in self.tab_ids() {
            let scoped = self.scoped_tab(&id);
            if shown.contains(&scoped) {
                continue;
            }
            if let Some(mailbox) = all.iter().find(|mailbox| mailbox.id == scoped) {
                tabs.push(mailbox.clone());
            }
            shown.push(scoped);
        }
        tabs
    }

    fn scoped_tab(&self, id: &str) -> String {
        match self.list_account() {
            Some(account) if !id.contains('/') => format!("{account}/{id}"),
            _ => id.to_string(),
        }
    }

    /// The saved id a tab over the list comes from.
    pub fn saved_tab(&self, id: &str) -> String {
        self.tab_ids().into_iter().find(|saved| self.scoped_tab(saved) == id).unwrap_or_else(|| id.to_string())
    }

    pub fn set_tabs(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.set_preference(TABS_KEY, Some(json!(ids)), cx);
    }

    pub fn is_tab(&self, id: &str) -> bool {
        self.tab_ids().iter().any(|tab| tab == id)
    }

    pub fn toggle_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        let mut ids = self.tab_ids();
        match ids.iter().position(|tab| tab == id) {
            Some(index) => {
                ids.remove(index);
            }
            None => ids.push(id.to_string()),
        }
        self.set_tabs(ids, cx);
    }

    pub fn move_tab(&mut self, tab: &str, index: usize, cx: &mut Context<Self>) {
        let mut ids: Vec<String> =
            self.saved_tabs().into_iter().map(|mailbox| mailbox.id).filter(|id| id != tab).collect();
        ids.insert(index.min(ids.len()), tab.to_string());
        self.set_tabs(ids, cx);
    }

    /// A mailbox's name, with its account's name after it when it is one account's other than
    /// the account on screen.
    pub fn tab_name(&self, mailbox: &Mailbox, shown_in: Option<&str>) -> String {
        let parts: Vec<&str> = mailbox.id.split('/').collect();
        let accounts = self.accounts();
        if parts.len() > 1
            && accounts.len() > 1
            && Some(parts[0]) != shown_in
            && let Some(account) = accounts.iter().find(|account| account.id == parts[0])
        {
            return format!("{} · {}", mailbox.name, account.title);
        }
        mailbox.name.clone()
    }

    /// The mailboxes that could be tabs and aren't yet.
    pub fn mailboxes_for_tabs(&self) -> (Vec<Mailbox>, Vec<(AccountView, Vec<Mailbox>)>) {
        let unified = self.unified().into_iter().filter(|mailbox| !self.is_tab(&mailbox.id)).collect();
        let accounts = self
            .accounts()
            .into_iter()
            .filter_map(|account| {
                let left: Vec<Mailbox> =
                    account.mailboxes.iter().filter(|mailbox| !self.is_tab(&mailbox.id)).cloned().collect();
                (!left.is_empty()).then_some((account, left))
            })
            .collect();
        (unified, accounts)
    }

    // MARK: Settings

    pub fn open_settings(&mut self, section: SettingsSection, target: Option<String>, cx: &mut Context<Self>) {
        self.settings = Some(section);
        self.settings_target = target;
        self.sidebar_open = false;
        self.load_preferences(cx);
        cx.notify();
    }

    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings = None;
        self.settings_target = None;
        self.settings_query.clear();
        cx.notify();
    }

    pub fn connection_text(&self) -> &'static str {
        match self.connection.as_str() {
            "online" => "Connected",
            "connecting" => "Connecting",
            _ => "Offline. Changes wait and are sent once connected.",
        }
    }

    pub fn status_of(&self, account: &AccountView) -> String {
        match account.status.as_str() {
            "syncing" => "Syncing".into(),
            "reauth" => "Sign in again: remove the account and add it back".into(),
            "error" => "Can't reach the provider right now".into(),
            _ if account.provider == "gmail" => "Gmail".into(),
            _ => "JMAP".into(),
        }
    }

    pub fn subtitle_of(&self, account: &AccountView) -> String {
        if account.name.is_empty() {
            return self.status_of(account);
        }
        format!("{} · {}", account.address, self.status_of(account))
    }

    pub fn confirm_removing(&mut self, account: &AccountView, cx: &mut Context<Self>) {
        let id = account.id.clone();
        self.confirmation = Some(Confirmation {
            title: format!("Remove {}?", account.address),
            message: format!("Its mail leaves this {} and your other devices. You can add it again.", device()),
            action: "Remove".into(),
            cancel: "Cancel",
            safe: false,
            perform: std::rc::Rc::new(move |this, cx| this.remove_account(id.clone(), cx)),
        });
        cx.notify();
    }

    pub fn confirm_signing_out(&mut self, cx: &mut Context<Self>) {
        self.confirmation = Some(Confirmation {
            title: "Sign out of Wonnet?".into(),
            message: format!("Every account leaves this {}. Signing in again brings them back.", device()),
            action: "Sign out".into(),
            cancel: "Cancel",
            safe: false,
            perform: std::rc::Rc::new(|this, cx| {
                this.close_settings(cx);
                this.sign_out(cx);
            }),
        });
        cx.notify();
    }

    /// The senders and domains whose new mail goes to the trash.
    pub fn blocked_senders(&self) -> Vec<String> {
        self.preferences
            .iter()
            .filter(|(key, value)| key.starts_with("blocked:") && value.as_bool() != Some(false))
            .map(|(key, _)| key["blocked:".len()..].to_string())
            .collect()
    }

    pub fn unblock(&mut self, address: &str, cx: &mut Context<Self>) {
        self.set_preference(&format!("blocked:{address}"), None, cx);
    }

    pub fn custom_splits(&self) -> Vec<CustomSplit> {
        CustomSplit::all(&self.preferences)
    }

    pub fn new_split(&self) -> CustomSplit {
        let order = self.custom_splits().iter().map(|split| split.order).max().unwrap_or(-1) + 1;
        CustomSplit { id: uuid(), name: String::new(), from: Vec::new(), label: None, order }
    }

    pub fn save_split(&mut self, split: &CustomSplit, cx: &mut Context<Self>) {
        self.set_preference(&split.key(), Some(split.value()), cx);
    }

    pub fn remove_split(&mut self, split: &CustomSplit, cx: &mut Context<Self>) {
        self.set_preference(&split.key(), None, cx);
    }

    pub fn move_custom_split(&mut self, split: &CustomSplit, index: usize, cx: &mut Context<Self>) {
        let mut reordered: Vec<CustomSplit> =
            self.custom_splits().into_iter().filter(|other| other.id != split.id).collect();
        reordered.insert(index.min(reordered.len()), split.clone());
        for (order, split) in reordered.iter_mut().enumerate() {
            if split.order != order as i64 {
                split.order = order as i64;
                self.save_split(split, cx);
            }
        }
    }

    /// Every account's labels, for a split to take mail of.
    pub fn split_labels(&self) -> Vec<(String, String)> {
        let accounts = self.accounts();
        let several = accounts.len() > 1;
        accounts
            .iter()
            .flat_map(|account| {
                account.mailboxes.iter().filter_map(move |mailbox| {
                    let id = label_id(&mailbox.id)?;
                    let name =
                        if several { format!("{} · {}", mailbox.name, account.title) } else { mailbox.name.clone() };
                    Some((id, name))
                })
            })
            .collect()
    }

    pub fn summary_of(&self, split: &CustomSplit) -> String {
        let mut parts = split.from.clone();
        if let Some(label) = &split.label
            && let Some((_, name)) = self.split_labels().into_iter().find(|(id, _)| id == label)
        {
            parts.push(format!("label {name}"));
        }
        if parts.is_empty() { "Nothing yet".into() } else { parts.join(", ") }
    }

    pub fn snippets(&self) -> Vec<Snippet> {
        Snippet::all(&self.preferences)
    }

    pub fn save_snippet(&mut self, snippet: &Snippet, cx: &mut Context<Self>) {
        let value = json!({ "name": snippet.name.trim(), "text": snippet.text });
        self.set_preference(&snippet.key(), Some(value), cx);
    }

    /// The signature the provider keeps for the account, used while the user's own is empty.
    pub fn provided_signature(&self, account: &AccountView) -> Option<String> {
        account
            .identities
            .first()
            .and_then(|identity| identity.signature.clone())
            .filter(|signature| !signature.is_empty())
    }

    pub fn provider_name(&self, account: &AccountView) -> &'static str {
        if account.provider == "gmail" { "Gmail" } else { "your provider" }
    }

    pub fn own_signature(&self, account: &AccountView) -> String {
        self.preferences
            .get(&format!("signature:{}", account.id))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    pub fn set_signature(&mut self, text: &str, account: &AccountView, cx: &mut Context<Self>) {
        let value = (!text.is_empty()).then(|| json!(text));
        self.set_preference(&format!("signature:{}", account.id), value, cx);
    }

    /// What goes under a message from an identity: the user's own signature for the account, or
    /// else the provider's.
    pub fn signature(&self, account: &str, email: Option<&str>) -> Option<String> {
        if let Some(own) = self.preferences.get(&format!("signature:{account}")).and_then(Value::as_str)
            && !own.is_empty()
        {
            return Some(own.to_string());
        }
        let accounts = self.accounts();
        let identities = accounts.iter().find(|candidate| candidate.id == account).map(|found| &found.identities)?;
        let identity =
            identities.iter().find(|identity| Some(identity.email.as_str()) == email).or(identities.first())?;
        identity.signature.clone().filter(|signature| !signature.is_empty())
    }

    // MARK: Keybindings

    /// The command's keys: the user's, or its defaults.
    pub fn keys(&self, command: &KeyCommand) -> Vec<String> {
        match self.preferences.get(&command.preference_key()).and_then(Value::as_array) {
            Some(values) => values.iter().filter_map(|value| value.as_str().map(String::from)).collect(),
            None => command.defaults.clone(),
        }
    }

    pub fn keys_of(&self, id: &str) -> Vec<String> {
        keys::all().iter().find(|command| command.id == id).map(|command| self.keys(command)).unwrap_or_default()
    }

    pub fn keys_changed(&self, command: &KeyCommand) -> bool {
        self.preferences.contains_key(&command.preference_key())
    }

    /// The command's first key, as the palette and the tooltips say it.
    pub fn key_label(&self, id: &str) -> Option<String> {
        self.keys_of(id).first().map(|key| keys::label(key))
    }

    /// The text with the command's first key after it: "Archive (E)".
    pub fn key_help(&self, text: &str, id: &str) -> String {
        match self.key_label(id) {
            Some(key) => format!("{text} ({key})"),
            None => text.to_string(),
        }
    }

    /// The command the key runs where `scope` is.
    pub fn command_for(&self, key: &str, scope: KeyScope) -> Option<KeyCommand> {
        let key = keys::normalized(key);
        keys::all().into_iter().find(|command| {
            command.scope.overlaps(scope) && self.keys(command).iter().any(|own| keys::normalized(own) == key)
        })
    }

    /// Whether a key starts a longer one that works where `scope` is, as G starts G I.
    pub fn starts_sequence(&self, stroke: &str, scope: KeyScope) -> bool {
        let start = format!("{} ", keys::normalized(stroke));
        keys::all().iter().any(|command| {
            command.scope.overlaps(scope)
                && self.keys(command).iter().any(|key| keys::normalized(key).starts_with(&start))
        })
    }

    /// Puts the key in place of the command's `replacing` key, or after its keys, and takes it
    /// from the commands that had it where this one works.
    pub fn set_key(&mut self, key: &str, command: &KeyCommand, replacing: Option<&str>, cx: &mut Context<Self>) {
        let mut taken = Vec::new();
        for other in keys::all() {
            if other.id == command.id || !other.scope.overlaps(command.scope) {
                continue;
            }
            let had = self.keys(&other);
            let kept: Vec<String> = had.iter().filter(|own| !keys::clash(own, key)).cloned().collect();
            if kept.len() == had.len() {
                continue;
            }
            self.set_keys(kept, &other, cx);
            taken.push(other.title.clone());
        }
        let mut mine: Vec<String> = self.keys(command).into_iter().filter(|own| own != key).collect();
        match replacing.and_then(|old| mine.iter().position(|own| own == old)) {
            Some(index) => mine[index] = key.to_string(),
            None => mine.push(key.to_string()),
        }
        self.set_keys(mine, command, cx);
        if !taken.is_empty() {
            self.show(format!("{} was taken from {}.", keys::label(key), taken.join(", ")), cx);
        }
    }

    pub fn remove_key(&mut self, key: &str, command: &KeyCommand, cx: &mut Context<Self>) {
        let kept = self.keys(command).into_iter().filter(|own| own != key).collect();
        self.set_keys(kept, command, cx);
    }

    /// Gives the command its defaults back, taking them from the commands that took them since.
    pub fn reset_keys(&mut self, command: &KeyCommand, cx: &mut Context<Self>) {
        self.set_preference(&command.preference_key(), None, cx);
        for key in &command.defaults {
            for other in keys::all() {
                if other.id == command.id || !other.scope.overlaps(command.scope) {
                    continue;
                }
                let had = self.keys(&other);
                if had.iter().any(|own| keys::clash(own, key)) {
                    let kept = had.into_iter().filter(|own| !keys::clash(own, key)).collect();
                    self.set_keys(kept, &other, cx);
                }
            }
        }
    }

    pub fn reset_all_keys(&mut self, cx: &mut Context<Self>) {
        for command in keys::all() {
            if self.keys_changed(&command) {
                self.set_preference(&command.preference_key(), None, cx);
            }
        }
    }

    fn set_keys(&mut self, keys: Vec<String>, command: &KeyCommand, cx: &mut Context<Self>) {
        let value = (keys != command.defaults).then(|| json!(keys));
        self.set_preference(&command.preference_key(), value, cx);
    }
}

/// The label of a label's mailbox, `<account>/label/<label>`.
pub fn label_id(mailbox: &str) -> Option<String> {
    mailbox.split_once("/label/").map(|(_, label)| label.to_string())
}

/// What this device is called in a sentence: "leaves this computer".
pub fn device() -> &'static str {
    "computer"
}

pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
