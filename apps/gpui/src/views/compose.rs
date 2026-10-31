//! Writing a message: who it goes to and from, its subject, its text with the signature and the
//! quote under it, and its files. It is kept as a draft as it is written. The bar on top has Send
//! alone; everything else about the message is in the toolbar under it. Sending waits for the
//! undo delay, or for a time picked to send it at.

use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Anchor, AnyElement, App, Context, Entity, ExternalPaths, FocusHandle, Focusable as _, KeyDownEvent, MouseButton,
    PathPromptOptions, SharedString, Subscription, Task, Window, anchored, deferred, div, px,
};
use gpui_component::input::{InputEvent, InputState, TextareaState};
use mail_core::api::Command;

use super::chooser::{Chooser, ChooserKind};
use crate::keys;
use crate::models::*;
use crate::store::{Compose, Confirmation, Snippet, Store};
use crate::theme::*;
use crate::ui::{self, ActionButton, Chip, ChoiceRow, IconButton, Lead, Symbol, Variant};

/// The most the attachments of a message can weigh together.
const LIMIT: i64 = 25 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Line {
    To,
    Cc,
    Bcc,
}

/// An address the message can be from: one of an account's identities, the first its own.
#[derive(Clone, Debug)]
struct Sender {
    account: AccountView,
    identity: Identity,
    first: bool,
}

impl Sender {
    fn all(accounts: &[AccountView]) -> Vec<Sender> {
        accounts
            .iter()
            .flat_map(|account| {
                let identities = if account.identities.is_empty() {
                    vec![Identity { name: None, email: account.address.clone(), signature: None }]
                } else {
                    account.identities.clone()
                };
                identities.into_iter().enumerate().map(|(index, identity)| Sender {
                    account: account.clone(),
                    identity,
                    first: index == 0,
                })
            })
            .collect()
    }

    fn id(&self) -> String {
        format!("{}\n{}", self.account.id, self.identity.email)
    }

    fn name(&self) -> String {
        self.identity.name.clone().unwrap_or_else(|| self.identity.email.clone())
    }

    fn detail(&self) -> String {
        if self.identity.name.is_none() { self.account.title.clone() } else { self.identity.email.clone() }
    }
}

pub struct ComposeView {
    store: Entity<Store>,
    compose: Compose,
    focus: FocusHandle,
    to: Entity<InputState>,
    cc: Entity<InputState>,
    bcc: Entity<InputState>,
    subject: Entity<InputState>,
    text: Entity<TextareaState>,
    sender_query: Entity<InputState>,
    picking_sender: bool,
    sender_highlight: usize,
    /// The address line being typed in, and the people it could mean.
    suggesting: Option<Line>,
    suggestions: Vec<Address>,
    highlighted: usize,
    suggest: Option<Task<()>>,
    /// The word after a `;` at the caret, while there is one.
    trigger: Option<String>,
    trigger_index: usize,
    show_quote: bool,
    notice: Option<String>,
    kept: Draft,
    edited: bool,
    closing: bool,
    autosave: Option<Task<()>>,
    picker: Option<Entity<Chooser>>,
    _subscriptions: Vec<Subscription>,
}

impl ComposeView {
    pub fn new(store: Entity<Store>, compose: Compose, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let typing = compose.state.typing.clone().unwrap_or_default();
        let draft = compose.state.draft.clone();
        let mut make =
            |value: String, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(value));
        let to = make(typing.to, cx);
        let cc = make(typing.cc, cx);
        let bcc = make(typing.bcc, cx);
        let subject = make(draft.subject.clone(), cx);
        let sender_query = cx.new(|cx| InputState::new(window, cx).placeholder("Find an address"));
        let text = cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 10_000).default_value(draft.text.clone()));
        let mut subscriptions = Vec::new();
        for (line, field) in [(Line::To, &to), (Line::Cc, &cc), (Line::Bcc, &bcc)] {
            subscriptions.push(cx.subscribe_in(field, window, move |this, _, event: &InputEvent, window, cx| {
                this.address_event(line, event, window, cx)
            }));
        }
        subscriptions.push(cx.subscribe(&subject, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.changed(cx);
            }
        }));
        subscriptions.push(cx.subscribe(&text, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.find_trigger(cx);
                this.changed(cx);
            }
        }));
        subscriptions.push(cx.subscribe(&sender_query, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.sender_highlight = 0;
                cx.notify();
            }
        }));
        if !store.read(cx).preferences_loaded {
            store.update(cx, |store, cx| store.load_preferences(cx));
        }
        let focus = cx.focus_handle();
        // A moment after, so the key that opened it isn't typed into it.
        let (first_to, first_text, empty) = (to.clone(), text.clone(), draft.to.is_empty());
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(60)).await;
            let _ = cx.update(|window, cx| {
                if empty {
                    first_to.update(cx, |field, cx| field.focus(window, cx));
                } else {
                    first_text.update(cx, |field, cx| field.focus(window, cx));
                }
            });
        })
        .detach();
        Self {
            store,
            kept: compose.state.draft.clone(),
            compose,
            focus,
            to,
            cc,
            bcc,
            subject,
            text,
            sender_query,
            picking_sender: false,
            sender_highlight: 0,
            suggesting: None,
            suggestions: Vec::new(),
            highlighted: 0,
            suggest: None,
            trigger: None,
            trigger_index: 0,
            show_quote: false,
            notice: None,
            edited: false,
            closing: false,
            autosave: None,
            picker: None,
            _subscriptions: subscriptions,
        }
    }

    fn field(&self, line: Line) -> &Entity<InputState> {
        match line {
            Line::To => &self.to,
            Line::Cc => &self.cc,
            Line::Bcc => &self.bcc,
        }
    }

    fn addresses(&mut self, line: Line) -> &mut Vec<Address> {
        let draft = &mut self.compose.state.draft;
        match line {
            Line::To => &mut draft.to,
            Line::Cc => &mut draft.cc,
            Line::Bcc => &mut draft.bcc,
        }
    }

    /// The compose as it is, with what is typed in the address fields, for the store to keep.
    fn live(&self, cx: &App) -> ComposeState {
        let mut state = self.compose.state.clone();
        state.draft.subject = self.subject.read(cx).value().to_string();
        state.draft.text = self.text.read(cx).value().to_string();
        state.typing = Some(Typing {
            to: self.to.read(cx).value().to_string(),
            cc: self.cc.read(cx).value().to_string(),
            bcc: self.bcc.read(cx).value().to_string(),
        });
        state
    }

    /// The compose with what is typed in the address fields made addresses.
    fn assembled(&self, cx: &App) -> ComposeState {
        let mut state = self.live(cx);
        state.draft.to.extend(typed_addresses(self.to.read(cx).value().as_ref()));
        state.draft.cc.extend(typed_addresses(self.cc.read(cx).value().as_ref()));
        state.draft.bcc.extend(typed_addresses(self.bcc.read(cx).value().as_ref()));
        state.typing = None;
        state
    }

    /// Notes what was written and keeps it as a draft a little after the typing stops.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let live = self.live(cx);
        self.store.update(cx, |store, cx| store.note_compose(live, cx));
        let assembled = self.assembled(cx);
        if assembled.draft == self.kept {
            return;
        }
        self.edited = true;
        self.autosave = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                if this.closing {
                    return;
                }
                let assembled = this.assembled(cx);
                this.kept = assembled.draft.clone();
                let id = this.compose.id;
                let save = this.store.update(cx, |store, cx| store.save_draft(id, &assembled, cx));
                cx.spawn(async move |this, cx| {
                    let saved = save.await;
                    let _ = this.update(cx, |this, _| this.compose.state.draft_id = saved);
                })
                .detach();
            });
        }));
        cx.notify();
    }

    // MARK: Addresses

    fn address_event(&mut self, line: Line, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Focus => {
                self.suggesting = Some(line);
                self.suggest(line, cx);
            }
            InputEvent::Blur => {
                if self.suggesting == Some(line) {
                    self.suggesting = None;
                    cx.notify();
                }
            }
            InputEvent::Change => {
                self.split(line, window, cx);
                self.suggest(line, cx);
                self.changed(cx);
            }
            InputEvent::PressEnter { .. } => {
                self.pick(line, window, cx);
            }
        }
    }

    /// Makes chips of what is before a comma, a semicolon or a line break.
    fn split(&mut self, line: Line, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.field(line).read(cx).value().to_string();
        if !typed.contains([',', ';', '\n']) {
            return;
        }
        let mut parts: Vec<&str> = typed.split([',', ';', '\n']).collect();
        let rest = parts.pop().unwrap_or_default().trim().to_string();
        for part in parts {
            self.add_text(line, part);
        }
        let field = self.field(line).clone();
        field.update(cx, |field, cx| field.set_value(rest, window, cx));
    }

    fn suggest(&mut self, line: Line, cx: &mut Context<Self>) {
        let query = self.field(line).read(cx).value().trim().to_string();
        if query.is_empty() {
            self.suggestions.clear();
            self.suggest = None;
            return cx.notify();
        }
        let taken: Vec<String> = self
            .compose
            .state
            .draft
            .to
            .iter()
            .chain(&self.compose.state.draft.cc)
            .chain(&self.compose.state.draft.bcc)
            .map(|address| address.email.to_lowercase())
            .collect();
        let core = self.store.read(cx).core.clone();
        self.suggest = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(60)).await;
            let found = core.call::<ContactList>(Command::Contacts { query, limit: 8 }).await;
            let _ = this.update(cx, |this, cx| {
                this.suggestions = found
                    .map(|list| list.contacts)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|address| !taken.contains(&address.email.to_lowercase()))
                    .collect();
                this.highlighted = 0;
                cx.notify();
            });
        }));
    }

    /// Takes the highlighted suggestion, or what was typed. False when there was nothing.
    fn pick(&mut self, line: Line, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(address) = self.suggestions.get(self.highlighted).cloned().filter(|_| self.suggesting == Some(line))
        {
            self.add(line, address);
        } else {
            let typed = self.field(line).read(cx).value().to_string();
            if typed.trim().is_empty() {
                return false;
            }
            self.add_text(line, &typed);
        }
        self.suggestions.clear();
        let field = self.field(line).clone();
        field.update(cx, |field, cx| field.set_value("", window, cx));
        self.changed(cx);
        true
    }

    fn add(&mut self, line: Line, address: Address) {
        let list = self.addresses(line);
        if !list.iter().any(|known| known.email.eq_ignore_ascii_case(&address.email)) {
            list.push(address);
        }
    }

    fn add_text(&mut self, line: Line, text: &str) {
        for address in typed_addresses(text) {
            self.add(line, address);
        }
    }

    fn address_key(&mut self, line: Line, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match key {
            "down" | "up" if self.suggesting == Some(line) && !self.suggestions.is_empty() => {
                let last = self.suggestions.len() - 1;
                self.highlighted =
                    if key == "down" { (self.highlighted + 1).min(last) } else { self.highlighted.saturating_sub(1) };
                cx.notify();
                true
            }
            "tab" | "enter" => self.pick(line, window, cx),
            "escape" if !self.suggestions.is_empty() => {
                self.suggestions.clear();
                cx.notify();
                true
            }
            "backspace" => {
                if !self.field(line).read(cx).value().is_empty() || self.addresses(line).is_empty() {
                    return false;
                }
                self.addresses(line).pop();
                self.changed(cx);
                true
            }
            _ => false,
        }
    }

    // MARK: Senders

    fn senders(&self, cx: &App) -> Vec<Sender> {
        Sender::all(&self.store.read(cx).accounts())
    }

    fn sender(&self, cx: &App) -> Option<Sender> {
        let draft = &self.compose.state.draft;
        self.senders(cx).into_iter().find(|option| {
            if option.account.id != draft.account_id {
                return false;
            }
            match &draft.from {
                None => option.first,
                Some(from) => option.identity.email.eq_ignore_ascii_case(&from.email),
            }
        })
    }

    fn choose(&mut self, sender: Sender, cx: &mut Context<Self>) {
        self.compose.state.draft.account_id = sender.account.id.clone();
        self.compose.state.draft.from = (!sender.first)
            .then(|| Address { name: sender.identity.name.clone(), email: sender.identity.email.clone() });
        self.compose.state.from = sender.identity.email.clone();
        self.picking_sender = false;
        self.changed(cx);
    }

    fn found_senders(&self, cx: &App) -> Vec<Sender> {
        let query = self.sender_query.read(cx).value().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        self.senders(cx)
            .into_iter()
            .filter(|sender| {
                let haystack =
                    format!("{} {} {} {}", sender.name(), sender.detail(), sender.identity.email, sender.account.title)
                        .to_lowercase();
                words.iter().all(|word| haystack.contains(word))
            })
            .collect()
    }

    // MARK: Snippets

    fn snippets(&self, cx: &App) -> Vec<Snippet> {
        Snippet::all(&self.store.read(cx).preferences)
    }

    /// The word after a `;` right before the caret.
    fn find_trigger(&mut self, cx: &mut Context<Self>) {
        if self.read_trigger(cx) {
            cx.notify();
        }
    }

    /// Reads the `;word` at the caret again, as the text or the caret moves. True when it changed.
    fn read_trigger(&mut self, cx: &App) -> bool {
        let state = self.text.read(cx);
        let text = state.value().to_string();
        let cursor = state.cursor().min(text.len());
        let before = &text[..cursor];
        let word_start = before.rfind(|character: char| character.is_whitespace()).map_or(0, |at| at + 1);
        let word = &before[word_start..];
        let trigger = word.strip_prefix(';').filter(|rest| !rest.contains(';')).map(String::from);
        if trigger == self.trigger {
            return false;
        }
        self.trigger = trigger;
        self.trigger_index = 0;
        true
    }

    fn trigger_matches(&self, cx: &App) -> Vec<Snippet> {
        let Some(trigger) = &self.trigger else { return Vec::new() };
        let trigger = trigger.to_lowercase();
        self.snippets(cx)
            .into_iter()
            .filter(|snippet| trigger.is_empty() || snippet.name.to_lowercase().contains(&trigger))
            .collect()
    }

    /// Puts a snippet in at the caret, with the first recipient's first name; in place of the
    /// `;word` that asked for it, when one did.
    fn put(&mut self, snippet: &Snippet, replacing: bool, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.assembled(cx).draft.to.first().and_then(first_name);
        let filled = snippet.filled(first.as_deref());
        let trigger = self.trigger.take();
        self.text.update(cx, |state, cx| {
            if replacing && let Some(trigger) = trigger {
                let cursor = state.cursor();
                let start = cursor.saturating_sub(trigger.len() + 1);
                state.set_selected_range(start..cursor, cx);
            }
            state.replace(filled, window, cx);
            state.focus(window, cx);
        });
        self.changed(cx);
    }

    fn trigger_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let matches = self.trigger_matches(cx);
        if matches.is_empty() {
            return false;
        }
        match key {
            "up" => self.trigger_index = self.trigger_index.saturating_sub(1),
            "down" => self.trigger_index = (self.trigger_index + 1).min(matches.len() - 1),
            "enter" | "tab" => {
                let snippet = matches[self.trigger_index.min(matches.len() - 1)].clone();
                self.put(&snippet, true, window, cx);
            }
            "escape" => self.trigger = None,
            _ => return false,
        }
        cx.notify();
        true
    }

    // MARK: Doing

    fn reveal(&mut self, line: Line, window: &mut Window, cx: &mut Context<Self>) {
        self.compose.state.show_cc = true;
        let field = self.field(line).clone();
        field.update(cx, |field, cx| field.focus(window, cx));
        cx.notify();
    }

    fn attach(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let _ = this.update(cx, |this, cx| this.add_files(paths, cx));
        })
        .detach();
    }

    /// Copies the files where the core reads them until it uploads them, off the window's thread.
    fn add_files(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        let folder = crate::platform::outgoing_dir();
        let staged = cx
            .background_spawn(async move { paths.iter().filter_map(|path| stage(path, &folder)).collect::<Vec<_>>() });
        cx.spawn(async move |this, cx| {
            let files = staged.await;
            let _ = this.update(cx, |this, cx| {
                let mut total: i64 = this.compose.state.draft.attachments.iter().map(|file| file.size).sum();
                for file in files {
                    if total + file.size > LIMIT {
                        this.notice =
                            Some(format!("{} wasn't added: attachments can be 25 MB together at most.", file.name));
                        continue;
                    }
                    total += file.size;
                    this.compose.state.draft.attachments.push(file);
                }
                this.changed(cx);
            });
        })
        .detach();
    }

    /// A paste of files or of a picture attaches them; text goes into the message.
    fn paste(&self, cx: &mut Context<Self>) -> ui::OnPaste {
        let this = cx.entity().downgrade();
        Rc::new(move |item, _, cx| {
            let mut paths = Vec::new();
            let mut pictures = Vec::new();
            for entry in item.entries() {
                match entry {
                    gpui::ClipboardEntry::ExternalPaths(found) => paths.extend(found.paths().iter().cloned()),
                    gpui::ClipboardEntry::Image(image) => pictures.push((image.format, image.bytes.clone())),
                    gpui::ClipboardEntry::String(_) => {}
                }
            }
            if paths.is_empty() && pictures.is_empty() {
                return false;
            }
            let _ = this.update(cx, |this, cx| {
                if !paths.is_empty() {
                    this.add_files(paths, cx);
                }
                if !pictures.is_empty() {
                    this.add_pictures(pictures, cx);
                }
            });
            true
        })
    }

    /// Writes pasted pictures where the core reads them, off the window's thread, and attaches them.
    fn add_pictures(&mut self, pictures: Vec<(gpui::ImageFormat, Vec<u8>)>, cx: &mut Context<Self>) {
        let folder = crate::platform::outgoing_dir();
        let written = cx.background_spawn(async move {
            pictures
                .into_iter()
                .filter_map(|(format, bytes)| {
                    let place = folder.join(uuid::Uuid::new_v4().to_string());
                    std::fs::create_dir_all(&place).ok()?;
                    let path = place.join(format!("Pasted image.{}", format.extension()));
                    std::fs::write(&path, bytes).ok()?;
                    Some(path)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let paths = written.await;
            let _ = this.update(cx, |this, cx| this.add_files(paths, cx));
        })
        .detach();
    }

    fn open_picker(&mut self, kind: ChooserKind, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity();
        let store = self.store.clone();
        let picker = cx.new(|cx| {
            let mut chooser = Chooser::new(store, kind.clone(), window, cx);
            let close = this.clone();
            chooser.closing = Some(Rc::new(move |_, cx| {
                close.update(cx, |this, cx| {
                    this.picker = None;
                    cx.notify();
                })
            }));
            let picked = this.clone();
            chooser.picked_time = Some(Rc::new(move |choice, window, cx| {
                let kind = kind.clone();
                picked.update(cx, |this, cx| {
                    this.picker = None;
                    match kind {
                        ChooserKind::Remind => {
                            this.compose.state.remind_at = Some(choice.into());
                            this.changed(cx);
                        }
                        _ => this.send(Some(choice.until), false, window, cx),
                    }
                    cx.notify();
                })
            }));
            let snippet = this.clone();
            chooser.picked_snippet = Some(Rc::new(move |chosen, window, cx| {
                snippet.update(cx, |this, cx| {
                    this.picker = None;
                    this.put(&chosen, false, window, cx);
                })
            }));
            chooser
        });
        self.picker = Some(picker);
        cx.notify();
    }

    /// Moves whoever made the introduction to Bcc and thanks them first thing.
    fn instant_intro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut draft = self.assembled(cx).draft;
        if draft.to.is_empty() {
            return;
        }
        let introducer = draft.to.remove(0);
        let cc = std::mem::take(&mut draft.cc);
        draft.to.extend(cc);
        let thanks = match first_name(&introducer) {
            Some(name) => format!("Thanks for the intro, {name}! Moving you to Bcc."),
            None => "Thanks for the intro! Moving you to Bcc.".into(),
        };
        draft.bcc.push(introducer);
        draft.text = format!("{thanks}\n\n{}", draft.text);
        let text = draft.text.clone();
        self.compose.state.draft = draft;
        self.compose.state.show_cc = true;
        for field in [self.to.clone(), self.cc.clone(), self.bcc.clone()] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        self.text.update(cx, |state, cx| state.set_value(text, window, cx));
        self.changed(cx);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.closing = true;
        let keep = self.edited || self.compose.state.keep || self.compose.state.draft_id.is_some();
        let compose = Compose { id: self.compose.id, state: self.assembled(cx) };
        self.store.update(cx, |store, cx| store.close_compose(compose, keep, cx));
    }

    fn confirm_discard(&mut self, cx: &mut Context<Self>) {
        let compose = self.compose.clone();
        let this = cx.entity().downgrade();
        self.store.update(cx, |store, cx| {
            store.confirmation = Some(Confirmation {
                title: "Discard this draft?".into(),
                message: "What you wrote is thrown away.".into(),
                action: "Discard".into(),
                cancel: "Keep it",
                safe: false,
                perform: Rc::new(move |store, cx| {
                    let _ = this.update(cx, |this, _| this.closing = true);
                    store.discard_draft(compose.clone(), cx);
                }),
            });
            cx.notify();
        });
    }

    /// Checks the message, asks about what looks forgotten, then sends.
    fn send(&mut self, at: Option<i64>, archive: bool, _: &mut Window, cx: &mut Context<Self>) {
        let draft = self.assembled(cx).draft;
        let everyone: Vec<&Address> = draft.to.iter().chain(&draft.cc).chain(&draft.bcc).collect();
        if everyone.is_empty() {
            self.notice = Some("Add who it goes to.".into());
            return cx.notify();
        }
        if let Some(wrong) = everyone.iter().find(|address| !is_valid(address)) {
            self.notice = Some(format!("“{}” isn't an address.", wrong.email));
            return cx.notify();
        }
        self.notice = None;
        let mut worries = Vec::new();
        let placeholders = placeholders(&draft.text);
        if !placeholders.is_empty() {
            let verb = if placeholders.len() == 1 { "needs" } else { "need" };
            worries.push(format!("{} still {verb} filling in.", placeholders.join(", ")));
        }
        if draft.attachments.is_empty() && draft.forward_attachments_of.is_none() && mentions_attachment(&draft.text) {
            worries.push("It mentions an attachment, but nothing is attached.".into());
        }
        if worries.is_empty() {
            return self.finish(at, archive, cx);
        }
        let this = cx.entity().downgrade();
        self.store.update(cx, |store, cx| {
            store.confirmation = Some(Confirmation {
                title: "Send anyway?".into(),
                message: worries.join(" "),
                action: "Send".into(),
                cancel: "Keep writing",
                safe: true,
                perform: Rc::new(move |_, cx| {
                    let _ = this.update(cx, |this, cx| this.finish(at, archive, cx));
                }),
            });
            cx.notify();
        });
    }

    fn finish(&mut self, at: Option<i64>, archive: bool, cx: &mut Context<Self>) {
        self.closing = true;
        let compose = Compose { id: self.compose.id, state: self.assembled(cx) };
        self.store.update(cx, |store, cx| store.send(compose, at, archive, cx));
    }

    fn cannot_send(&self, cx: &App) -> bool {
        let draft = self.assembled(cx).draft;
        draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty()
    }

    fn title(&self) -> &'static str {
        let draft = &self.compose.state.draft;
        if draft.in_reply_to.is_some() {
            return "Reply";
        }
        if draft.forward_attachments_of.is_some() || draft.subject.starts_with("Fwd:") {
            "Forward"
        } else {
            "New message"
        }
    }

    /// The shortcuts of writing: Ctrl+Enter sends, and the toolbar's.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_some() || self.store.read(cx).confirmation.is_some() {
            return;
        }
        let Some(stroke) = keys::stroke(&event.keystroke) else { return };
        let stroke = keys::normalized(&stroke);
        let done = match stroke.as_str() {
            "mod+enter" => {
                self.send(None, false, window, cx);
                true
            }
            "mod+shift+enter" if self.compose.state.thread.is_some() => {
                self.send(None, true, window, cx);
                true
            }
            "mod+shift+u" => {
                self.attach(cx);
                true
            }
            "mod+;" => {
                self.open_picker(ChooserKind::Snippet, window, cx);
                true
            }
            "mod+shift+l" => {
                self.open_picker(ChooserKind::SendLater, window, cx);
                true
            }
            "mod+shift+h" => {
                self.open_picker(ChooserKind::Remind, window, cx);
                true
            }
            "mod+shift+i" if self.compose.state.draft.in_reply_to.is_some() => {
                self.instant_intro(window, cx);
                true
            }
            "mod+shift+," => {
                self.confirm_discard(cx);
                true
            }
            "mod+w" => {
                self.close(cx);
                true
            }
            "mod+shift+c" => {
                self.reveal(Line::Cc, window, cx);
                true
            }
            "mod+shift+b" => {
                self.reveal(Line::Bcc, window, cx);
                true
            }
            "mod+shift+f" if self.senders(cx).len() > 1 => {
                self.picking_sender = true;
                self.sender_query.update(cx, |query, cx| {
                    query.set_value("", window, cx);
                    query.focus(window, cx);
                });
                true
            }
            "escape" if self.picking_sender => {
                self.picking_sender = false;
                true
            }
            "escape" => {
                self.close(cx);
                true
            }
            _ => false,
        };
        if done {
            cx.stop_propagation();
            cx.notify();
        }
    }
}

/// A copy of a file where the core can read it until it uploads it.
fn stage(path: &Path, folder: &Path) -> Option<DraftAttachment> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let place = folder.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&place).ok()?;
    let target = place.join(&name);
    std::fs::copy(path, &target).ok()?;
    let size = std::fs::metadata(&target).ok()?.len() as i64;
    let mime = mime_of(&name);
    Some(DraftAttachment { name, mime, size, upload: None, path: Some(target.display().to_string()) })
}

/// A file's type, by its extension.
fn mime_of(name: &str) -> String {
    let extension = Path::new(name).extension().map(|ext| ext.to_string_lossy().to_lowercase()).unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Whether the text speaks of an attachment.
fn mentions_attachment(text: &str) -> bool {
    const WORDS: [&str; 8] =
        ["attach", "attached", "attaches", "attaching", "attachment", "attachments", "enclosed", "attachement"];
    text.split(|character: char| !character.is_alphanumeric()).any(|word| WORDS.contains(&word.to_lowercase().as_str()))
}

/// The `{placeholders}` of a snippet still in the text.
fn placeholders(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else { break };
        let name = &after[..end];
        if !name.is_empty() && name.chars().all(|character| character.is_ascii_alphabetic() || character == '_') {
            found.push(format!("{{{name}}}"));
        }
        rest = &after[end..];
    }
    found
}

impl Render for ComposeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.read_trigger(cx);
        let this = cx.entity();
        let cannot = self.cannot_send(cx);
        let back = this.clone();
        let sending = this.clone();
        let header = super::topbar::window_bar(
            vec![
                ui::header_bar(
                    Some(self.title().into()),
                    vec![
                        IconButton::new("compose-back", Symbol::ArrowLeft, "Back (Esc)")
                            .on_click(move |_, _, cx| back.update(cx, |this, cx| this.close(cx)))
                            .into_any_element(),
                    ],
                    None,
                    vec![
                        ActionButton::new("send", "Send")
                            .symbol(Symbol::Send)
                            .variant(Variant::Primary)
                            .disabled(cannot)
                            .tooltip(format!("Send ({})", keys::label("mod+enter")))
                            .on_click(move |_, window, cx| {
                                sending.update(cx, |this, cx| this.send(None, false, window, cx))
                            })
                            .into_any_element(),
                    ],
                    cx,
                )
                .flex_1()
                .px(px(0.))
                .pr(px(space::S))
                .into_any_element(),
            ],
            cx,
        );
        let card = ui::page_card(cx)
            .id("compose-card")
            .w_full()
            .max_w(px(size::CARD_WIDTH))
            .min_h(window.bounds().size.height - px(size::BAR * 2. + size::CARD_MARGIN_Y * 2. + 2.))
            .flex()
            .flex_col()
            .when_some(self.notice.clone(), |card, notice| {
                card.child(div().px(px(size::CARD_INSET)).py(px(space::M)).child(ui::notice(
                    notice,
                    ui::Tone::Error,
                    Some(Symbol::CircleAlert),
                    None,
                    cx,
                )))
                .child(div().mx(px(size::CARD_INSET)).child(ui::rule(CARD_RULE, cx)))
            })
            .children(self.fields(window, cx))
            .child(self.message(cx));
        let page = div().id("compose-scroll").flex_1().min_h_0().overflow_y_scroll().child(
            div().w_full().flex().justify_center().px(px(size::CARD_MARGIN_X)).py(px(size::CARD_MARGIN_Y)).child(card),
        );
        let picker = self.picker.clone().map(|picker| {
            let close = this.clone();
            ui::scrim(
                picker,
                move |_, cx| {
                    close.update(cx, |this, cx| {
                        this.picker = None;
                        cx.notify();
                    })
                },
                cx,
            )
        });
        let drop_target = this.clone();
        div()
            .id("compose")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.color(BACKGROUND))
            .on_key_down(cx.listener(Self::key_down))
            .capture_action(cx.listener(|this, action: &gpui_component::input::Enter, window, cx| {
                // A field takes Ctrl+Enter for itself before the shortcuts hear it.
                if action.secondary && this.picker.is_none() {
                    cx.stop_propagation();
                    this.send(None, false, window, cx);
                }
            }))
            .on_drop(move |paths: &ExternalPaths, _, cx| {
                let paths = paths.paths().to_vec();
                drop_target.update(cx, |this, cx| this.add_files(paths, cx))
            })
            .child(header)
            .child(page)
            .child(ui::rule(BORDER, cx))
            .child(self.toolbar(cx))
            .children(self.snippet_menu(cx))
            .child(
                div()
                    .id("compose-drop")
                    .absolute()
                    .inset(px(space::XS))
                    .rounded(px(radius::SM))
                    .drag_over::<ExternalPaths>(|outline, _, _, cx| {
                        outline.border_2().border_dashed().border_color(cx.color(PRIMARY))
                    }),
            )
            .children(picker)
    }
}

impl ComposeView {
    fn fields(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut fields = Vec::new();
        let this = cx.entity();
        let reveal = (!self.compose.state.show_cc).then(|| {
            let this = this.clone();
            ActionButton::new("cc-bcc", "Cc Bcc")
                .variant(Variant::Ghost)
                .size(ControlSize::Small)
                .on_click(move |_, window, cx| this.update(cx, |this, cx| this.reveal(Line::Cc, window, cx)))
        });
        fields.push(self.address_line(Line::To, reveal, window, cx));
        if self.compose.state.show_cc {
            fields.push(self.address_line(Line::Cc, None, window, cx));
            fields.push(self.address_line(Line::Bcc, None, window, cx));
        }
        if self.senders(cx).len() > 1 {
            fields
                .push(field_row("From", false, None, self.from(window, cx).into_any_element(), cx).into_any_element());
        }
        let subject_focus = self.subject.read(cx).focus_handle(cx);
        let subject = div().flex_1().text(TextStyle::Body, cx).child(ui::bare_input(&self.subject));
        fields
            .push(field_row("Subject", false, Some(subject_focus), subject.into_any_element(), cx).into_any_element());
        fields
    }

    fn address_line(
        &self,
        line: Line,
        accessory: Option<ActionButton>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = match line {
            Line::To => "To",
            Line::Cc => "Cc",
            Line::Bcc => "Bcc",
        };
        let this = cx.entity();
        let addresses = match line {
            Line::To => &self.compose.state.draft.to,
            Line::Cc => &self.compose.state.draft.cc,
            Line::Bcc => &self.compose.state.draft.bcc,
        };
        let chips = addresses.iter().enumerate().map(|(index, address)| {
            let remove = this.clone();
            let valid = is_valid(address);
            Chip::new(
                SharedString::from(format!("{label}-{index}")),
                address.name.clone().unwrap_or_else(|| address.email.clone()),
            )
            .on_card()
            .invalid(!valid)
            .help(if valid { address_text(address) } else { format!("“{}” isn't an address", address.email) })
            .remove(move |_, _, cx| {
                remove.update(cx, |this, cx| {
                    this.addresses(line).remove(index);
                    this.changed(cx);
                })
            })
            .into_any_element()
        });
        let keys_target = this.clone();
        let keys: ui::FieldKeys =
            Rc::new(move |key, window, cx| keys_target.update(cx, |this, cx| this.address_key(line, key, window, cx)));
        let field = ui::with_list_keys(
            div()
                .min_w(px(80.))
                .flex_1()
                .h(px(size::CHIP))
                .flex()
                .items_center()
                .text(TextStyle::Body, cx)
                .child(ui::bare_input(self.field(line))),
            &self.field(line).read(cx).focus_handle(cx),
            keys,
            cx,
        );
        let focus = self.field(line).read(cx).focus_handle(cx);
        let content = div().flex_1().flex().flex_wrap().gap(px(space::XS)).items_center().children(chips).child(field);
        let row = field_row(
            label,
            true,
            Some(focus),
            content
                .child(
                    div()
                        .flex_none()
                        .children(accessory.map(|button| div().mr(px(-size::BUTTON_GAP / 2.)).child(button))),
                )
                .into_any_element(),
            cx,
        );
        let suggestions = (self.suggesting == Some(line) && !self.suggestions.is_empty()).then(|| {
            let rows = self.suggestions.iter().enumerate().map(|(index, address)| {
                let pick = this.clone();
                let chosen = address.clone();
                let mut row = ChoiceRow::new(
                    SharedString::from(format!("suggest-{index}")),
                    address.name.clone().unwrap_or_else(|| address.email.clone()),
                    move |_, window, cx| {
                        let chosen = chosen.clone();
                        pick.update(cx, |this, cx| {
                            this.add(line, chosen);
                            this.suggestions.clear();
                            let field = this.field(line).clone();
                            field.update(cx, |field, cx| field.set_value("", window, cx));
                            this.changed(cx);
                        })
                    },
                )
                .lead(Lead::Avatar(address.clone()))
                .highlighted(index == self.highlighted)
                .on_popover();
                if address.name.is_some() {
                    row = row.detail(address.email.clone());
                }
                row
            });
            deferred(
                anchored().anchor(Anchor::TopLeft).child(
                    ui::popup_card(cx)
                        .w(px(size::POPUP_WIDTH))
                        .p(px(space::XS))
                        .ml(px(size::CARD_INSET + 56. + space::S + 2.))
                        .children(rows),
                ),
            )
            .with_priority(5)
        });
        div().relative().child(row).children(suggestions).into_any_element()
    }

    /// The address the message goes from, and the choice of another.
    fn from(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let sender = self.sender(cx);
        let open = this.clone();
        let face = ui::combobox_face(
            "from",
            sender.as_ref().map(|sender| {
                div().size(px(8.)).rounded_full().bg(cx.color(account_color(&sender.account.color))).into_any_element()
            }),
            sender.as_ref().map(|sender| SharedString::from(sender.name())),
            sender.as_ref().map(|sender| SharedString::from(sender.detail())),
            "Find an address",
            ui::ComboboxStyle::Inline,
            self.picking_sender,
            Some(format!("Send as another address ({})", keys::label("mod+shift+f")).into()),
            Rc::new(move |_, window, cx| {
                open.update(cx, |this, cx| {
                    this.picking_sender = !this.picking_sender;
                    this.sender_highlight = 0;
                    if this.picking_sender {
                        this.sender_query.update(cx, |query, cx| {
                            query.set_value("", window, cx);
                            query.focus(window, cx);
                        });
                    }
                    cx.notify();
                })
            }),
            cx,
        );
        let popup = self.picking_sender.then(|| {
            let found = self.found_senders(cx);
            let current = sender.as_ref().map(Sender::id);
            let highlighted = self.sender_highlight;
            let rows: Vec<_> = found
                .iter()
                .enumerate()
                .map(|(index, option)| {
                    let pick = this.clone();
                    let chosen = option.clone();
                    ChoiceRow::new(SharedString::from(format!("sender-{index}")), option.name(), move |_, _, cx| {
                        let chosen = chosen.clone();
                        pick.update(cx, |this, cx| this.choose(chosen, cx))
                    })
                    .detail(option.detail())
                    .lead(Lead::Dot(account_color(&option.account.color)))
                    .checked(current.as_ref() == Some(&option.id()))
                    .highlighted(index == highlighted)
                    .on_popover()
                    .into_any_element()
                })
                .collect();
            let keys_target = this.clone();
            let keys: ui::FieldKeys = Rc::new(move |key, _, cx| {
                keys_target.update(cx, |this, cx| {
                    let found = this.found_senders(cx);
                    match key {
                        "down" => {
                            this.sender_highlight = (this.sender_highlight + 1).min(found.len().saturating_sub(1))
                        }
                        "up" => this.sender_highlight = this.sender_highlight.saturating_sub(1),
                        "enter" => {
                            if let Some(chosen) = found.get(this.sender_highlight).cloned() {
                                this.choose(chosen, cx);
                            }
                        }
                        "escape" => this.picking_sender = false,
                        _ => return false,
                    }
                    cx.notify();
                    true
                })
            });
            let clear = this.clone();
            let empty = rows.is_empty();
            let query = self.sender_query.clone();
            deferred(
                anchored().anchor(Anchor::TopLeft).snap_to_window_with_margin(px(space::S)).child(
                    ui::popup_card(cx)
                        .id("from-popup")
                        .occlude()
                        .mt(px(space::XXS))
                        .w(px(size::POPUP_WIDTH))
                        .on_mouse_down_out(move |_, _, cx| {
                            clear.update(cx, |this, cx| {
                                this.picking_sender = false;
                                cx.notify();
                            })
                        })
                        .child(ui::search_field(
                            "from-search",
                            &self.sender_query,
                            ui::SearchStyle::Popup,
                            Symbol::Search,
                            keys,
                            Rc::new(move |window, cx| query.update(cx, |field, cx| field.set_value("", window, cx))),
                            cx,
                        ))
                        .child(
                            div()
                                .id("from-list")
                                .max_h(px(8. * size::ROW))
                                .overflow_y_scroll()
                                .p(px(space::S))
                                .children(rows)
                                .when(empty, |list| {
                                    list.child(
                                        div()
                                            .p(px(space::S + 2.))
                                            .text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)
                                            .child("No address by that name."),
                                    )
                                }),
                        ),
                ),
            )
            .with_priority(5)
        });
        div().ml(px(-space::XXS)).child(face).children(popup)
    }

    fn message(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = self.store.read(cx);
        let sender = self.sender(cx);
        let signature = store.signature(
            &self.compose.state.draft.account_id,
            sender.as_ref().map(|sender| sender.identity.email.as_str()),
        );
        let quote = self.compose.state.draft.quote.clone().filter(|quote| !quote.is_empty());
        let this = cx.entity();
        let attachments: Vec<_> = self
            .compose
            .state
            .draft
            .attachments
            .iter()
            .enumerate()
            .map(|(index, file)| {
                let remove = this.clone();
                Chip::new(SharedString::from(format!("attachment-{index}")), file.name.clone())
                    .symbol(Symbol::Paperclip)
                    .detail(ui::file_size(file.size))
                    .on_card()
                    .remove(move |_, _, cx| {
                        remove.update(cx, |this, cx| {
                            this.compose.state.draft.attachments.remove(index);
                            this.changed(cx);
                        })
                    })
                    .into_any_element()
            })
            .chain(self.compose.state.remind_at.clone().map(|remind| {
                let remove = this.clone();
                Chip::new("remind", format!("Remind me {} if nobody replies", remind.label))
                    .symbol(Symbol::AlarmClock)
                    .on_card()
                    .remove(move |_, _, cx| {
                        remove.update(cx, |this, cx| {
                            this.compose.state.remind_at = None;
                            this.changed(cx);
                        })
                    })
                    .into_any_element()
            }))
            .collect();
        let toggle = this.clone();
        let show_quote = self.show_quote;
        let focus = self.text.read(cx).focus_handle(cx);
        div()
            .id("compose-message")
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(space::L))
            .px(px(size::CARD_INSET))
            .py(px(space::L))
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&focus, cx))
            .child(div().w_full().text(TextStyle::Body, cx).child(ui::bare_textarea(&self.text, Some(self.paste(cx)))))
            .when_some(signature, |message, signature| {
                message.child(ui::paragraph_in(signature, TextStyle::Body, MUTED_STRONGER_FOREGROUND, cx))
            })
            .when_some(quote, |message, quote| {
                message
                    .child(
                        div().flex().child(
                            Chip::new("quote", "•••")
                                .on_card()
                                .help(if show_quote { "Hide the quoted text" } else { "Show the quoted text" })
                                .action(move |_, _, cx| {
                                    toggle.update(cx, |this, cx| {
                                        this.show_quote = !this.show_quote;
                                        cx.notify();
                                    })
                                }),
                        ),
                    )
                    .when(show_quote, |message| {
                        message.child(
                            div()
                                .pl(px(space::M))
                                .border_l_2()
                                .border_color(cx.color(CARD_RULE))
                                .child(ui::paragraph_in(quote, TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)),
                        )
                    })
            })
            .when(!attachments.is_empty(), |message| {
                message.child(div().flex().flex_wrap().gap(px(space::XS)).children(attachments))
            })
    }

    /// Everything about the message but sending it: its files, a snippet, when to send it, a
    /// reminder, and the way to throw it away at the far end.
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let button = |id: &'static str,
                      symbol: Symbol,
                      help: String,
                      act: Rc<dyn Fn(&mut ComposeView, &mut Window, &mut Context<ComposeView>)>| {
            let this = this.clone();
            IconButton::new(id, symbol, help).on_click(move |_, window, cx| {
                let act = act.clone();
                this.update(cx, |view, cx| act(view, window, cx))
            })
        };
        let label = keys::label;
        let thread = self.compose.state.thread.is_some();
        let reply = self.compose.state.draft.in_reply_to.is_some();
        let reminded = self.compose.state.remind_at.is_some();
        let cannot = self.cannot_send(cx);
        div()
            .flex()
            .items_center()
            .h(px(size::BAR))
            .px(px(space::L))
            .child(button(
                "attach",
                Symbol::Paperclip,
                format!("Attach files ({})", label("mod+shift+u")),
                Rc::new(|view, _, cx| view.attach(cx)),
            ))
            .child(button(
                "snippets",
                Symbol::Zap,
                format!("Snippets ({}), or type ; and a name", label("mod+;")),
                Rc::new(|view, window, cx| view.open_picker(ChooserKind::Snippet, window, cx)),
            ))
            .child(button(
                "send-later",
                Symbol::CalendarClock,
                format!("Send later ({})", label("mod+shift+l")),
                Rc::new(|view, window, cx| view.open_picker(ChooserKind::SendLater, window, cx)),
            ))
            .child(
                button(
                    "remind",
                    Symbol::AlarmClock,
                    format!("Remind me if nobody replies ({})", label("mod+shift+h")),
                    Rc::new(|view, window, cx| view.open_picker(ChooserKind::Remind, window, cx)),
                )
                .active(reminded),
            )
            .when(thread, |bar| {
                bar.child(
                    button(
                        "send-archive",
                        Symbol::Archive,
                        format!("Send and archive ({})", label("mod+shift+enter")),
                        Rc::new(|view, window, cx| view.send(None, true, window, cx)),
                    )
                    .disabled(cannot),
                )
            })
            .when(reply, |bar| {
                bar.child(button(
                    "intro",
                    Symbol::Handshake,
                    format!("Instant Intro: move who introduced you to Bcc ({})", label("mod+shift+i")),
                    Rc::new(|view, window, cx| view.instant_intro(window, cx)),
                ))
            })
            .child(div().flex_1())
            .child(button(
                "discard",
                Symbol::Trash,
                format!("Discard the draft ({})", label("mod+shift+,")),
                Rc::new(|view, _, cx| view.confirm_discard(cx)),
            ))
    }

    /// The snippets matching what follows a `;`, over the toolbar.
    fn snippet_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let matches = self.trigger_matches(cx);
        if matches.is_empty() {
            return None;
        }
        let this = cx.entity();
        let rows = matches.into_iter().enumerate().map(|(index, snippet)| {
            let pick = this.clone();
            let chosen = snippet.clone();
            ChoiceRow::new(
                SharedString::from(format!("trigger-{index}")),
                snippet.name.clone(),
                move |_, window, cx| {
                    let chosen = chosen.clone();
                    pick.update(cx, |this, cx| this.put(&chosen, true, window, cx))
                },
            )
            .detail(snippet.text.replace('\n', " "))
            .lead(Lead::Symbol(Symbol::Zap))
            .highlighted(index == self.trigger_index)
            .on_popover()
        });
        let keys_target = this.clone();
        Some(div().absolute().left(px(space::L)).bottom(px(size::BAR + space::XXS)).w(px(size::POPUP_WIDTH)).child(
            ui::with_list_keys(
                ui::popup_card(cx).p(px(space::XS)).children(rows),
                &self.text.read(cx).focus_handle(cx),
                Rc::new(move |key, window, cx| keys_target.update(cx, |this, cx| this.trigger_key(key, window, cx))),
                cx,
            ),
        ))
    }
}

/// A line of a message's head: its label, what it holds, and a hairline under it.
fn field_row(
    label: &str,
    wraps: bool,
    focus: Option<FocusHandle>,
    content: AnyElement,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .when(wraps, |row| row.items_start())
                .when(!wraps, |row| row.items_center())
                .gap(px(space::S + 2.))
                .px(px(size::CARD_INSET))
                .when(wraps, |row| row.py(px(space::S)))
                .min_h(px(size::SEARCH))
                .text(TextStyle::Body, cx)
                .when_some(focus, |row, focus| {
                    row.cursor_text().on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&focus, cx))
                })
                .child(
                    div()
                        .w(px(56.))
                        .flex_none()
                        .when(wraps, |label| label.h(px(size::CHIP)).flex().items_center())
                        .text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)
                        .child(label.to_string()),
                )
                .child(div().flex_1().min_w_0().flex().items_center().child(content)),
        )
        .child(div().mx(px(size::CARD_INSET)).child(ui::rule(CARD_RULE, cx)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_what_looks_forgotten() {
        assert!(mentions_attachment("See the attached file."));
        assert!(!mentions_attachment("I'm attaché to the embassy"));
        assert_eq!(placeholders("Hi {first_name}, see {link}"), vec!["{first_name}", "{link}"]);
        assert_eq!(mime_of("a.PDF"), "application/pdf");
    }
}
