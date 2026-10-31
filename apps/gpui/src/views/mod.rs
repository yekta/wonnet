//! The window: the list or the open thread under the top bar, the sidebar sliding over them, the
//! message being written and the settings in their place, the sheets over everything, and the
//! keys, as the Mac app has them.

mod body;
mod chooser;
mod compose;
mod failure;
mod gallery;
mod list;
mod onboarding;
mod person;
mod settings;
mod sidebar;
mod thread;
mod topbar;

use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, KeyDownEvent, MouseButton, SharedString, Subscription, Window, WindowAppearance,
    div, px,
};
use gpui_component::input::{InputEvent, InputState};
use mail_core::api::Event;

use crate::core::Core;
use crate::keys::{self, KeyScope};
use crate::models::{Filter, ReplyKind};
use crate::platform::notify::{Answer, Notifier};
use crate::store::{LocalSettings, PaletteScope, SettingsSection, Store, ThreadCommand};
use crate::theme::*;
use crate::ui::{self, ActionButton, IconButton, Symbol, Variant};
use crate::updater::{UpdateState, Updater};

pub use chooser::Chooser;
pub use compose::ComposeView;
pub use failure::show_start_failure;
pub use thread::ThreadPane;

/// A key being recorded for a keybinding: the strokes so far, and what to do with the key.
pub struct Recording {
    pub strokes: Vec<String>,
    pub done: std::rc::Rc<dyn Fn(String, &mut Window, &mut App)>,
    pub id: SharedString,
}

impl gpui::Global for Recording {}

pub struct MailWindow {
    pub store: Entity<Store>,
    pub updater: Entity<Updater>,
    focus: FocusHandle,
    search: Entity<InputState>,
    list: Entity<list::ThreadList>,
    thread: Entity<ThreadPane>,
    compose: Option<(u64, Entity<ComposeView>)>,
    chooser: Option<(String, Entity<Chooser>)>,
    settings: Option<Entity<settings::SettingsView>>,
    onboarding: Option<Entity<onboarding::OnboardingView>>,
    gallery: Option<Entity<gallery::Gallery>>,
    contact: Option<(String, Entity<person::PersonView>)>,
    /// The first key of a longer one, as G of G I, and when it was pressed.
    started: Option<(String, Instant)>,
    focus_search: bool,
    /// The theme the window is drawn in, as last applied.
    applied: Option<crate::store::Appearance>,
    _subscriptions: Vec<Subscription>,
}

impl MailWindow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        core: Core,
        events: async_channel::Receiver<Event>,
        starts: async_channel::Receiver<String>,
        local: LocalSettings,
        demo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (answers, answered) = async_channel::unbounded();
        let store = cx.new(|_| Store::new(core, local, Notifier::new(answers)));
        let updater = cx.new(|_| Updater::new());
        if !demo {
            updater.update(cx, |updater, cx| updater.start(cx));
        }
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let list = cx.new(|cx| list::ThreadList::new(store.clone(), cx));
        let thread = cx.new(|cx| ThreadPane::new(store.clone(), window, cx));

        cx.set_global(ui::ListKeyTargets::default());
        let mut subscriptions = vec![
            cx.intercept_keystrokes(|event, window, cx| {
                if ui::ListKeyTargets::handle(&event.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            }),
            cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(store, window, cx)),
            cx.observe(&updater, |_, _, cx| cx.notify()),
            cx.subscribe_in(&search, window, Self::search_event),
            cx.observe_window_appearance(window, |this, window, cx| this.apply_look(window, cx)),
            cx.observe_window_activation(window, |this, window, cx| {
                let active = window.is_window_active();
                this.store.update(cx, |store, cx| {
                    store.active = active;
                    if active {
                        store.forget_sign_in();
                    }
                    if active && store.booted {
                        store.reload_mailboxes(cx);
                        store.settle_notifications(None, cx);
                    } else {
                        store.save_ui();
                    }
                });
            }),
        ];
        let saving = store.clone();
        subscriptions.push(cx.on_app_quit(move |_, cx| {
            let save = saving.update(cx, |store, _| store.save_ui_now());
            async move {
                save.await;
            }
        }));
        store.update(cx, |store, cx| store.boot(cx));

        Self::pump(events, cx, |this, event, cx| this.store.update(cx, |store, cx| store.handle(event, cx)));
        Self::pump(starts, cx, |this, line, cx| this.heard(line, cx));
        Self::pump(answered, cx, Self::answer);

        let mut this = Self {
            store,
            updater,
            focus,
            search,
            list,
            thread,
            compose: None,
            chooser: None,
            settings: None,
            onboarding: None,
            gallery: None,
            contact: None,
            started: None,
            focus_search: false,
            applied: None,
            _subscriptions: subscriptions,
        };
        this.apply_look(window, cx);
        this
    }

    /// Hands what arrives on a channel to the window, one at a time.
    fn pump<T: 'static>(
        receiver: async_channel::Receiver<T>,
        cx: &mut Context<Self>,
        handle: impl Fn(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(item) = receiver.recv().await {
                if this.update(cx, |this, cx| handle(this, item, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// A later start handed over a link, or asked to be shown.
    fn heard(&mut self, line: String, cx: &mut Context<Self>) {
        if let Some(answer) = Answer::from_link(&line) {
            return self.answer(answer, cx);
        }
        cx.activate(true);
        if line.starts_with("wonnet://") {
            self.store.update(cx, |store, cx| store.finish_sign_in(line, cx));
        }
    }

    /// A notification was clicked: the window comes forward unless a button did it all.
    fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if matches!(answer, Answer::Open(_) | Answer::Reply(_)) {
            cx.activate(true);
        }
        self.store.update(cx, |store, cx| store.answer(answer, cx));
    }

    fn apply_look(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let appearance = self.store.read(cx).local.appearance;
        self.applied = Some(appearance);
        let dark = match appearance {
            crate::store::Appearance::Dark => true,
            crate::store::Appearance::Light => false,
            crate::store::Appearance::System => {
                matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark)
            }
        };
        cx.set_global(Look { dark });
        apply_component_theme(dark, window, cx);
        cx.refresh_windows();
    }

    fn store_changed(&mut self, store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        store.update(cx, |store, cx| store.schedule_ui_save(cx));
        if self.applied != Some(store.read(cx).local.appearance) {
            self.apply_look(window, cx);
        }
        let query = store.read(cx).search_query.clone();
        if self.search.read(cx).value() != query {
            self.search.update(cx, |search, cx| search.set_value(query, window, cx));
        }
        self.sync_views(window, cx);
        cx.notify();
    }

    /// Makes the views of what is open: the message being written, a sheet, the settings.
    fn sync_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open_before = (
            self.compose.is_some(),
            self.chooser.is_some(),
            self.settings.is_some(),
            self.gallery.is_some(),
            self.contact.is_some(),
        );
        self.make_views(window, cx);
        let open_now = (
            self.compose.is_some(),
            self.chooser.is_some(),
            self.settings.is_some(),
            self.gallery.is_some(),
            self.contact.is_some(),
        );
        let closed = (open_before.0 && !open_now.0)
            || (open_before.1 && !open_now.1)
            || (open_before.2 && !open_now.2)
            || (open_before.3 && !open_now.3)
            || (open_before.4 && !open_now.4);
        if closed {
            window.focus(&self.focus, cx);
        }
    }

    fn make_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let compose = store.compose.clone();
        let chooser = chooser::wanted(store);
        let settings = store.settings.is_some();
        let onboarding = store.booted && !store.signed_in;
        let gallery = store.gallery_open;
        let contact = store.contact.clone();
        match compose {
            Some(compose) if self.compose.as_ref().map(|(id, _)| *id) != Some(compose.id) => {
                let store = self.store.clone();
                self.compose = Some((compose.id, cx.new(|cx| ComposeView::new(store, compose, window, cx))));
            }
            None if self.compose.is_some() => self.compose = None,
            _ => {}
        }
        match chooser {
            Some((key, kind)) if self.chooser.as_ref().map(|(open, _)| open) != Some(&key) => {
                let store = self.store.clone();
                self.chooser = Some((key, cx.new(|cx| Chooser::new(store, kind, window, cx))));
            }
            None if self.chooser.is_some() => self.chooser = None,
            _ => {}
        }
        if settings && self.settings.is_none() {
            let store = self.store.clone();
            let updater = self.updater.clone();
            self.settings = Some(cx.new(|cx| settings::SettingsView::new(store, updater, window, cx)));
        } else if !settings && let Some(closed) = self.settings.take() {
            closed.update(cx, |settings, cx| settings.save_names(cx));
        }
        if onboarding && self.onboarding.is_none() {
            let store = self.store.clone();
            self.onboarding = Some(cx.new(|cx| onboarding::OnboardingView::new(store, true, window, cx)));
        } else if !onboarding {
            self.onboarding = None;
        }
        match contact {
            Some(email) if self.contact.as_ref().map(|(shown, _)| shown) != Some(&email) => {
                let store = self.store.clone();
                let shown = email.clone();
                self.contact = Some((shown, cx.new(|cx| person::PersonView::new(store, email, cx))));
            }
            None => self.contact = None,
            _ => {}
        }
        if gallery && self.gallery.is_none() {
            let store = self.store.clone();
            self.gallery = Some(cx.new(|_| gallery::Gallery::new(store)));
        } else if !gallery {
            self.gallery = None;
        }
    }

    fn search_event(&mut self, state: &Entity<InputState>, event: &InputEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let InputEvent::Change = event {
            let query = state.read(cx).value().to_string();
            self.store.update(cx, |store, cx| {
                if store.search_query != query {
                    store.search(query, cx);
                }
            });
        }
    }

    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_value("", window, cx));
        self.store.update(cx, |store, cx| store.end_search(cx));
        window.focus(&self.focus, cx);
    }

    // MARK: Keys

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.record(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        if ui::menu_open(cx) {
            if ui::menu_key(&event.keystroke.key, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        let Some(stroke) = keys::stroke(&event.keystroke) else { return };
        if self.handle_key(&stroke, window, cx) {
            cx.stop_propagation();
        }
    }

    /// The keys go to a key being recorded in the settings, when one is.
    fn record(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(recording) = cx.try_global::<Recording>() else { return false };
        if recording.strokes.len() > 1 {
            return true;
        }
        let modifiers = &event.keystroke.modifiers;
        let plain = !modifiers.control && !modifiers.alt && !modifiers.platform && !modifiers.shift;
        if plain && event.keystroke.key == "escape" {
            cx.remove_global::<Recording>();
            window.refresh();
            return true;
        }
        let Some(stroke) = keys::stroke(&event.keystroke) else { return true };
        let held = stroke.contains("mod+") || stroke.contains("ctrl+") || stroke.contains("alt+");
        let first = cx.global_mut::<Recording>().strokes.is_empty();
        cx.global_mut::<Recording>().strokes.push(stroke);
        window.refresh();
        if !first || held {
            finish_recording(window, cx);
            return true;
        }
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(900)).await;
            let _ = cx.update(|window, cx| {
                if cx.try_global::<Recording>().is_some_and(|recording| recording.strokes.len() == 1) {
                    finish_recording(window, cx);
                }
            });
        })
        .detach();
        true
    }

    /// The keys of the window, as the keybindings set them; ignored while typing, but for the
    /// ones with Ctrl, which work everywhere as the Mac's menu shortcuts do.
    fn handle_key(&mut self, stroke: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.read(cx);
        let typing = !self.focus.is_focused(window);
        let plain = !stroke.contains('+') || stroke.starts_with("shift+");
        if let Some(confirmation) = store.confirmation.clone() {
            match stroke {
                "escape" => self.store.update(cx, |store, cx| {
                    store.confirmation = None;
                    cx.notify();
                }),
                "enter" => self.store.update(cx, |store, cx| {
                    store.confirmation = None;
                    (confirmation.perform)(store, cx);
                }),
                _ => return false,
            }
            return true;
        }
        if store.settings.is_some() {
            if stroke == "escape" && !typing {
                self.store.update(cx, |store, cx| store.close_settings(cx));
                return true;
            }
            return false;
        }
        if store.sheet_open() || !store.signed_in || !store.booted {
            return false;
        }
        if typing {
            if plain {
                return false;
            }
            const EVERYWHERE: [&str; 7] =
                ["compose", "settings", "print", "palette", "keybindings", "unsubscribe", "zero"];
            let scope = if store.conversation.is_some() { KeyScope::Thread } else { KeyScope::List };
            let Some(command) = store.command_for(stroke, scope) else { return false };
            if !EVERYWHERE.contains(&command.id.as_str()) && !command.id.starts_with("go.account.") {
                return false;
            }
            return self.perform(&command.id, window, cx);
        }
        if stroke == "escape" {
            return self.escape(window, cx);
        }
        let scope = if store.conversation.is_some() { KeyScope::Thread } else { KeyScope::List };
        if let Some((first, at)) = self.started.take()
            && at.elapsed() < Duration::from_millis(1500)
            && let Some(command) = store.command_for(&format!("{first} {stroke}"), scope)
        {
            return self.perform(&command.id, window, cx);
        }
        if let Some(command) = store.command_for(stroke, scope) {
            return self.perform(&command.id, window, cx);
        }
        if !store.starts_sequence(stroke, scope) {
            return false;
        }
        self.started = Some((stroke.to_string(), Instant::now()));
        true
    }

    /// Does the command, or says it doesn't apply here, so the key goes on.
    fn perform(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.clone();
        let in_thread = store.read(cx).conversation.is_some();
        if let Some(command) = ThreadCommand::from_id(id)
            && command != ThreadCommand::ExpandAll
        {
            store.update(cx, |store, cx| store.run(command, None, cx));
            return true;
        }
        if let Some(number) = id.strip_prefix("go.account.") {
            let number: usize = number.parse().unwrap_or(0);
            let accounts = store.read(cx).accounts();
            if number > accounts.len() {
                return false;
            }
            let mailbox = if number == 0 { "inbox".to_string() } else { format!("{}/inbox", accounts[number - 1].id) };
            store.update(cx, |store, cx| store.select_mailbox(&mailbox, cx));
            return true;
        }
        if id == "go.label" {
            store.update(cx, |store, cx| store.show_palette(PaletteScope::Labels, cx));
            return true;
        }
        if let Some(kind) = id.strip_prefix("go.") {
            store.update(cx, |store, cx| store.go(kind, cx));
            return true;
        }
        match id {
            "next" => store.update(cx, |store, cx| store.move_by(1, cx)),
            "previous" => store.update(cx, |store, cx| store.move_by(-1, cx)),
            "open" => {
                let selected = store.read(cx).selected.clone();
                let Some(selected) = selected.filter(|_| !in_thread) else { return false };
                store.update(cx, |store, cx| store.open(&selected, cx));
            }
            "top" | "bottom" => {
                let count = store.read(cx).visible_rows().len() as isize;
                store.update(cx, |store, cx| store.move_by(if id == "top" { -count } else { count }, cx));
            }
            "nextMessage" | "previousMessage" => {
                if !in_thread {
                    return false;
                }
                store.update(cx, |store, cx| store.move_message(if id == "nextMessage" { 1 } else { -1 }, cx));
            }
            "expandAll" => {
                if !in_thread {
                    return false;
                }
                store.update(cx, |store, cx| store.toggle_all_messages(cx));
            }
            "nextSplit" | "previousSplit" => {
                if store.read(cx).splits.is_empty() || in_thread {
                    return false;
                }
                store.update(cx, |store, cx| store.move_split(if id == "nextSplit" { 1 } else { -1 }, cx));
            }
            "search" => {
                if in_thread {
                    store.update(cx, |store, cx| store.close(cx));
                }
                self.focus_search = true;
                cx.notify();
            }
            "palette" => store.update(cx, |store, cx| store.show_palette(PaletteScope::Everything, cx)),
            "select" => {
                if in_thread {
                    return false;
                }
                store.update(cx, |store, cx| {
                    if store.selected.is_none() {
                        store.move_by(1, cx);
                    }
                    if let Some(current) = store.selected.clone() {
                        store.toggle_selection(&current, cx);
                    }
                });
            }
            "selectNext" => store.update(cx, |store, cx| store.extend_selection(1, cx)),
            "selectPrevious" => store.update(cx, |store, cx| store.extend_selection(-1, cx)),
            "selectAll" => {
                if in_thread || store.read(cx).visible_rows().is_empty() {
                    return false;
                }
                store.update(cx, |store, cx| store.select_all(cx));
            }
            "removeLabel" => store.update(cx, |store, cx| store.remove_shown_label(cx)),
            "undo" => store.update(cx, |store, cx| store.undo(cx)),
            "compose" => store.update(cx, |store, cx| store.new_message(cx)),
            "replyAllInThread" => {
                if !in_thread {
                    return false;
                }
                store.update(cx, |store, cx| store.reply(ReplyKind::ReplyAll, None, None, cx));
            }
            "unreadOnly" => store.update(cx, |store, cx| store.toggle_filter(Filter::Unread, cx)),
            "starredOnly" => store.update(cx, |store, cx| store.toggle_filter(Filter::Starred, cx)),
            "zero" => store.update(cx, |store, cx| store.archive_all(cx)),
            "settings" => store.update(cx, |store, cx| store.open_settings(SettingsSection::General, None, cx)),
            "keybindings" => store.update(cx, |store, cx| store.open_settings(SettingsSection::Keybindings, None, cx)),
            _ => return false,
        }
        let _ = window;
        true
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.read(cx);
        if store.sidebar_open {
            self.store.update(cx, |store, cx| {
                store.sidebar_open = false;
                cx.notify();
            });
        } else if store.conversation.is_some() {
            self.store.update(cx, |store, cx| store.close(cx));
        } else if !store.selection.is_empty() {
            self.store.update(cx, |store, cx| store.clear_selection(cx));
        } else if store.search_rows.is_some() {
            self.clear_search(window, cx);
        } else if store.filter.is_some() {
            self.store.update(cx, |store, cx| store.set_filter(None, cx));
        } else {
            return false;
        }
        true
    }

    // MARK: Drawing

    fn overlays(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let mut overlays = Vec::new();
        let store = self.store.read(cx);
        if let Some((_, chooser)) = &self.chooser {
            let store = self.store.clone();
            overlays.push(
                ui::scrim(chooser.clone(), move |_, cx| store.update(cx, chooser::close_all), cx).into_any_element(),
            );
        }
        if let Some(gallery) = &self.gallery {
            let store = self.store.clone();
            overlays.push(
                ui::scrim(
                    gallery.clone(),
                    move |_, cx| {
                        store.update(cx, |store, cx| {
                            store.gallery_open = false;
                            cx.notify();
                        })
                    },
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some((_, person)) = &self.contact {
            let close = self.store.clone();
            let close_sheet = self.store.clone();
            let shut = move |_: &mut Window, cx: &mut App| {
                close.update(cx, |store, cx| {
                    store.contact = None;
                    cx.notify();
                })
            };
            let sheet = ui::sheet(
                "contact",
                "Contact",
                ui::SheetSize::Small,
                CARD,
                Vec::new(),
                person.clone(),
                shut.clone(),
                cx,
            );
            overlays.push(
                ui::scrim(
                    sheet,
                    move |_, cx| {
                        close_sheet.update(cx, |store, cx| {
                            store.contact = None;
                            cx.notify();
                        })
                    },
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some(confirmation) = store.confirmation.clone() {
            overlays.push(confirmation_alert(self.store.clone(), confirmation, cx).into_any_element());
        }
        let toast = store.toast.clone().map(|toast| {
            let store = self.store.clone();
            let action = toast.action.clone().map(|title| {
                let on_click: ui::OnClick =
                    std::rc::Rc::new(move |_, _, cx| store.update(cx, |store, cx| store.toast_tapped(cx)));
                (SharedString::from(title), on_click)
            });
            ui::toast(toast.message.into(), action, cx).into_any_element()
        });
        let banner = self.update_banner(cx);
        if toast.is_some() || banner.is_some() {
            overlays.push(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(space::M))
                    .pb(px(space::XL))
                    .children(toast)
                    .children(banner)
                    .into_any_element(),
            );
        }
        overlays.extend(ui::menu_at_pointer(cx));
        let _ = window;
        overlays
    }

    /// A new version: the offer, the download as it goes, and the restart that finishes it.
    fn update_banner(&self, cx: &App) -> Option<gpui::AnyElement> {
        let updater = self.updater.read(cx);
        if !updater.banner_shown() {
            return None;
        }
        let current = crate::updater::label(crate::updater::VERSION);
        let handle = self.updater.clone();
        let close = {
            let handle = handle.clone();
            IconButton::new("update-close", Symbol::X, "Close")
                .size(ControlSize::Small)
                .plain()
                .quiet()
                .on_click(move |_, _, cx| handle.update(cx, |updater, cx| updater.close_banner(cx)))
        };
        let button =
            |id: &'static str, title: &'static str, variant: Variant, act: fn(&mut Updater, &mut Context<Updater>)| {
                let handle = handle.clone();
                ActionButton::new(id, title)
                    .variant(variant)
                    .size(ControlSize::Small)
                    .on_click(move |_, _, cx| handle.update(cx, act))
            };
        let banner = match &updater.state {
            UpdateState::Idle => return None,
            UpdateState::Checking => ui::Banner {
                symbol: Symbol::RefreshCw,
                tint: PROCESS,
                title: "Checking for updates".into(),
                detail: format!("You have {current}.").into(),
                progress: None,
                pending: true,
                action: None,
                close: None,
            },
            UpdateState::UpToDate => ui::Banner {
                symbol: Symbol::CircleCheck,
                tint: SUCCESS,
                title: "Up to date".into(),
                detail: format!("{current} is the newest version.").into(),
                progress: None,
                pending: false,
                action: None,
                close: Some(close),
            },
            UpdateState::Available(version) => ui::Banner {
                symbol: Symbol::CircleArrowDown,
                tint: PRIMARY,
                title: format!("{} is out", crate::updater::label(version)).into(),
                detail: format!("You have {current}.").into(),
                progress: None,
                pending: false,
                action: Some(button("update-install", "Update", Variant::Primary, |updater, cx| updater.install(cx))),
                close: Some(close),
            },
            UpdateState::Downloading(version, fraction) => ui::Banner {
                symbol: Symbol::CircleArrowDown,
                tint: PROCESS,
                title: format!("Downloading {}", crate::updater::label(version)).into(),
                detail: format!("{}%", (fraction * 100.) as u32).into(),
                progress: Some(*fraction),
                pending: false,
                action: None,
                close: None,
            },
            UpdateState::Installing(version) => ui::Banner {
                symbol: Symbol::CircleArrowDown,
                tint: PROCESS,
                title: format!("Installing {}", crate::updater::label(version)).into(),
                detail: format!("Replacing {current}.").into(),
                progress: None,
                pending: true,
                action: None,
                close: None,
            },
            UpdateState::Ready(version) => ui::Banner {
                symbol: Symbol::CircleCheck,
                tint: SUCCESS,
                title: format!("{} is installed", crate::updater::label(version)).into(),
                detail: "Restart to start using it.".into(),
                progress: None,
                pending: false,
                action: Some(button("update-restart", "Restart", Variant::Success, |updater, cx| updater.relaunch(cx))),
                close: Some(close),
            },
            UpdateState::Failed(message) => ui::Banner {
                symbol: Symbol::CircleAlert,
                tint: DESTRUCTIVE,
                title: "Update failed".into(),
                detail: message.clone().into(),
                progress: None,
                pending: false,
                action: Some(button("update-retry", "Try again", Variant::Outline, |updater, cx| updater.retry(cx))),
                close: Some(close),
            },
        };
        Some(ui::banner(banner, cx).into_any_element())
    }
}

fn finish_recording(window: &mut Window, cx: &mut App) {
    let Some(recording) = cx.try_global::<Recording>() else { return };
    let key = recording.strokes.join(" ");
    let done = recording.done.clone();
    cx.remove_global::<Recording>();
    window.refresh();
    if !key.is_empty() {
        done(key, window, cx);
    }
}

/// The question before something that can't be taken back.
fn confirmation_alert(
    store: Entity<Store>,
    confirmation: crate::store::Confirmation,
    cx: &App,
) -> impl IntoElement + use<> {
    let cancel = store.clone();
    let confirm = store.clone();
    let perform = confirmation.perform.clone();
    let close = move |_: &mut Window, cx: &mut App| {
        cancel.update(cx, |store, cx| {
            store.confirmation = None;
            cx.notify();
        })
    };
    let close_button = close.clone();
    ui::scrim(
        div()
            .id("confirmation")
            .occlude()
            .self_start()
            .w(px(380.))
            .mt(px(80.))
            .h_auto()
            .flex()
            .flex_col()
            .gap(px(space::L))
            .p(px(space::XL))
            .rounded(px(radius::LG))
            .bg(cx.color(POPOVER))
            .border_1()
            .border_color(cx.color(BORDER_POPOVER))
            .shadow(cx.shadow(ShadowSize::Xl, opacity::SHADOW_STRONGER))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text(TextStyle::Heading, cx).child(confirmation.title.clone()))
            .child(ui::paragraph_in(confirmation.message.clone(), TextStyle::Body, MUTED_FOREGROUND, cx))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .mr(px(-size::BUTTON_GAP / 2.))
                    .child(
                        ActionButton::new("confirm-cancel", confirmation.cancel)
                            .variant(Variant::Ghost)
                            .on_click(move |_, window, cx| close_button(window, cx)),
                    )
                    .child(
                        ActionButton::new("confirm-do", confirmation.action.clone())
                            .variant(if confirmation.safe { Variant::Primary } else { Variant::Destructive })
                            .on_click(move |_, _, cx| {
                                let perform = perform.clone();
                                confirm.update(cx, |store, cx| {
                                    store.confirmation = None;
                                    perform(store, cx);
                                })
                            }),
                    ),
            ),
        close,
        cx,
    )
}

/// GPUI Component's controls drawn in Wonnet's colours and face.
fn apply_component_theme(dark: bool, window: &mut Window, cx: &mut App) {
    let mode = if dark { gpui_component::ThemeMode::Dark } else { gpui_component::ThemeMode::Light };
    gpui_component::Theme::change(mode, Some(window), cx);
    let color = |token: ThemeColor| token.hsla(dark);
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = FONT.into();
    theme.font_size = px(16.);
    theme.radius = px(radius::SM);
    theme.radius_lg = px(radius::LG);
    theme.focus_ring = false;
    theme.shadow = false;
    let colors = &mut theme.colors;
    colors.background = color(BACKGROUND);
    colors.foreground = color(FOREGROUND);
    colors.border = color(BORDER);
    colors.input = color(BORDER_INPUT);
    colors.ring = color(RING);
    colors.caret = color(PRIMARY);
    colors.selection = color(PRIMARY).opacity(opacity::COLOR_TINT_STRONGER.value(dark) * 1.5);
    colors.muted = color(BACKGROUND_SECONDARY);
    colors.muted_foreground = color(MUTED_STRONGER_FOREGROUND);
    colors.popover = color(POPOVER);
    colors.popover_foreground = color(FOREGROUND);
    colors.accent = color(POPOVER_ACCENT);
    colors.accent_foreground = color(FOREGROUND);
    colors.primary = color(PRIMARY);
    colors.primary_foreground = color(PRIMARY_FOREGROUND);
    colors.scrollbar = gpui::transparent_black();
    colors.scrollbar_thumb = color(MUTED_STRONGEST_FOREGROUND);
    colors.scrollbar_thumb_hover = color(MUTED_STRONGER_FOREGROUND);
    colors.overlay = color(OVERLAY).opacity(opacity::OVERLAY.value(dark));
    colors.title_bar = color(BACKGROUND);
    colors.title_bar_border = color(BORDER);
    colors.window_border = color(BORDER);
    colors.list = color(BACKGROUND);
    colors.list_hover = color(BACKGROUND_ACCENT_LARGER);
    colors.list_active = color(BACKGROUND_ACCENT_LARGER_STRONGER);
}

impl Render for MailWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ui::ListKeyTargets::clear(cx);
        if self.focus_search {
            self.focus_search = false;
            self.search.update(cx, |search, cx| search.focus(window, cx));
        }
        let store = self.store.read(cx);
        let covered = self.chooser.is_some()
            || self.gallery.is_some()
            || store.confirmation.is_some()
            || store.contact.is_some()
            || store.sidebar_open
            || ui::menu_open(cx);
        let bottom = if store.toast.is_some() || self.updater.read(cx).banner_shown() { px(110.) } else { px(0.) };
        let hidden =
            self.compose.is_some() || store.settings.is_some() || store.conversation.is_none() || !store.signed_in;
        cx.set_global(body::Occlusion { all: covered, bottom });
        if hidden {
            let thread = self.thread.clone();
            cx.defer(move |cx| thread.update(cx, |thread, cx| thread.hide_pages(cx)));
        }
        let store = self.store.read(cx);
        let unread = store.inbox_unread();
        window.set_window_title(&if unread > 0 { format!("Wonnet ({unread})") } else { "Wonnet".to_string() });
        let page = if !store.booted {
            div().size_full().into_any_element()
        } else if !store.signed_in {
            match &self.onboarding {
                Some(onboarding) => onboarding.clone().into_any_element(),
                None => div().into_any_element(),
            }
        } else if let Some(settings) = &self.settings {
            settings.clone().into_any_element()
        } else if let Some((_, compose)) = &self.compose {
            compose.clone().into_any_element()
        } else {
            self.mail(window, cx).into_any_element()
        };
        let overlays = self.overlays(window, cx);
        div()
            .id("window")
            .track_focus(&self.focus)
            .key_context("Wonnet")
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.color(BACKGROUND))
            .font_family(FONT)
            .text_color(cx.color(FOREGROUND))
            .child(page)
            .children(overlays)
    }
}

impl MailWindow {
    /// The list, the thread over it, the bar on top, and the sidebar over them all.
    fn mail(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = self.store.read(cx);
        let open = store.conversation.is_some();
        let sidebar_open = store.sidebar_open;
        let bar = topbar::top_bar(self, window, cx);
        let page = if open { self.thread.clone().into_any_element() } else { self.list.clone().into_any_element() };
        let store = self.store.clone();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(bar)
            .child(div().flex_1().min_h_0().child(page))
            .when(sidebar_open, |page| page.child(sidebar::sidebar(store, cx)))
    }
}
