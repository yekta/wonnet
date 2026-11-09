//! The sheet a `wonnet://connect` link opens over everything, or a connected app's Change: who is
//! asking, the sentence that says what it may do, which accounts and which of their mail it
//! reaches (the filter, with its menu under the chip that opened it), what it reads of each
//! message and what it can do; then Approve or Deny (Save or Cancel). Closed any other way, a
//! request is denied.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, Entity, FocusHandle, Focusable as _, FontWeight, HighlightStyle,
    MouseDownEvent, Pixels, ScrollHandle, SharedString, StyledText, Subscription, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::models::{
    AccessChoice, AccessSummary, ConnectRequest, FilterChip, FilterLine, FilterOption, FilterValue, Part, Privilege,
    Role, SentencePart,
};
use crate::store::{ConnectChoices, ConnectState, FilterMenu, Store};
use crate::theme::*;
use crate::ui::{
    self, ActionButton, AddChip, Chip, ChoiceRow, ConditionChip, Lead, Mark, MarkRow, SheetSize, Surface, Symbol, Tone,
    Variant,
};

/// The most the menu's options take before they scroll.
const MENU_LIST: f32 = 340.;

pub struct ConnectView {
    store: Entity<Store>,
    /// The words typed in the filter's menu.
    query: Entity<InputState>,
    focus: FocusHandle,
    /// Where the filter's chips are, so a click on one is left to the chip.
    faces: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    options: ScrollHandle,
    /// The menu last drawn (its line and chip), the option highlighted in it, and its answers.
    shown: Option<(FilterLine, Option<FilterValue>, usize, u64)>,
    placeholder: String,
    _subscriptions: Vec<Subscription>,
}

impl ConnectView {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        let typed = cx.subscribe(&query, |this, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                let text = field.read(cx).value().to_string();
                this.store.update(cx, |store, cx| store.set_filter_query(text, cx));
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            store,
            query,
            focus,
            faces: Rc::default(),
            options: ScrollHandle::new(),
            shown: None,
            placeholder: String::new(),
            _subscriptions: vec![typed],
        }
    }

    /// Keeps the menu's field as the menu has it: its words, its placeholder and the focus, and
    /// the highlighted option in view.
    fn sync_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menu = match self.store.read(cx).connect.as_ref().map(|sheet| &sheet.state) {
            Some(ConnectState::Asking(choices)) => choices.menu.as_ref(),
            _ => None,
        };
        let Some(menu) = menu else {
            if self.shown.take().is_some() {
                window.focus(&self.focus, cx);
            }
            return;
        };
        let shown = (menu.line, menu.editing.clone(), menu.highlighted, menu.answers);
        let query = menu.query.clone();
        let placeholder = menu.options.as_ref().map(|options| options.placeholder.clone()).unwrap_or_default();
        let index = child_index(menu);
        let hovered = menu.hovered;
        if self.shown.as_ref() != Some(&shown) {
            let opened = self.shown.as_ref().is_none_or(|(line, editing, ..)| (*line, editing) != (shown.0, &shown.1));
            let answered = self.shown.as_ref().is_none_or(|(.., answers)| *answers != shown.3);
            if opened {
                self.options = ScrollHandle::new();
            }
            // A hover moves the highlight but never scrolls; the keys and a new list do.
            if let Some(index) = index.filter(|_| opened || answered || !hovered) {
                self.options.scroll_to_item(index);
            }
            self.shown = Some(shown);
        }
        self.query.update(cx, |field, cx| {
            if field.value() != query {
                field.set_value(query, window, cx);
            }
            if self.placeholder != placeholder {
                field.set_placeholder(placeholder.clone(), window, cx);
            }
        });
        self.placeholder = placeholder;
        let focus = self.query.read(cx).focus_handle(cx);
        if !focus.is_focused(window) {
            window.focus(&focus, cx);
        }
    }
}

/// The place among the list's children (section titles, then options) of the highlighted option.
fn child_index(menu: &FilterMenu) -> Option<usize> {
    let options = menu.options.as_ref()?;
    let mut child = 0;
    let mut option = 0;
    for section in &options.sections {
        child += usize::from(section.title.is_some());
        for _ in &section.options {
            if option == menu.highlighted {
                return Some(child);
            }
            child += 1;
            option += 1;
        }
    }
    None
}

impl Render for ConnectView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.faces.borrow_mut().clear();
        self.sync_menu(window, cx);
        let store = self.store.clone();
        let close = move |_: &mut Window, cx: &mut App| store.update(cx, |store, cx| store.close_connect(cx));
        let Some(sheet) = self.store.read(cx).connect.as_ref() else { return div().into_any_element() };
        let (title, content) = match &sheet.state {
            ConnectState::Loading => (untitled(sheet.changing), loading(cx)),
            ConnectState::Failed(error) => (untitled(sheet.changing), failed(error, &self.store, cx)),
            ConnectState::Asking(choices) => {
                let client = &choices.request.client;
                let title = if choices.changing() { format!("Change {client}") } else { format!("Connect {client}") };
                (title, self.asking(choices, cx))
            }
            ConnectState::Approved(client) => (format!("Connect {client}"), approved(client, &self.store, cx)),
        };
        let sheet = ui::sheet("connect", title, SheetSize::Large, BACKGROUND, Vec::new(), content, close.clone(), cx);
        div().absolute().inset_0().track_focus(&self.focus).child(ui::scrim(sheet, close, cx)).into_any_element()
    }
}

fn untitled(changing: bool) -> String {
    if changing { "Change an application".into() } else { "Connect an application".into() }
}

fn close_button(id: &str, title: &str, variant: Variant, store: &Entity<Store>) -> ActionButton {
    let store = store.clone();
    ActionButton::new(SharedString::from(id.to_string()), title.to_string())
        .variant(variant)
        .on_click(move |_, _, cx| store.update(cx, |store, cx| store.close_connect(cx)))
}

fn loading(cx: &App) -> Div {
    div().flex_1().flex().items_center().justify_center().child(ui::spinner(20., cx.color(MUTED_FOREGROUND)))
}

fn failed(error: &str, store: &Entity<Store>, cx: &App) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(div().flex_1().p(px(space::XL)).child(ui::notice(
            error.to_string(),
            Tone::Error,
            Some(Symbol::CircleAlert),
            None,
            cx,
        )))
        .child(ui::footer_bar(
            vec![close_button("connect-close", "Close", Variant::Outline, store).into_any_element()],
            cx,
        ))
}

fn approved(client: &str, store: &Entity<Store>, cx: &App) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::M))
        .p(px(space::XL))
        .child(ui::icon(Symbol::CircleCheck, 36.).text_color(cx.color(SUCCESS)))
        .child(div().text(TextStyle::Subheading, cx).child("Approved"))
        .child(
            ui::paragraph_in(
                format!(
                    "Your browser takes you back to {client}. You can change or revoke it in Settings → Connected apps."
                ),
                TextStyle::Label,
                MUTED_FOREGROUND,
                cx,
            )
            .max_w(px(size::FORM_WIDTH))
            .text_center(),
        )
        .child(
            div()
                .pt(px(space::S))
                .child(close_button("connect-done", "Done", Variant::Primary, store).tooltip("Done (Enter)")),
        )
}

impl ConnectView {
    fn asking(&self, choices: &ConnectChoices, cx: &App) -> Div {
        let Some(summary) = &choices.summary else { return loading(cx) };
        let request = &choices.request;
        let store = &self.store;
        let error =
            choices.error.clone().map(|error| ui::notice(error, Tone::Error, Some(Symbol::CircleAlert), None, cx));
        let deny = store.clone();
        let changing = choices.changing();
        let deny_button = ActionButton::new("connect-deny", if changing { "Cancel" } else { "Deny" })
            .variant(Variant::Ghost)
            .disabled(choices.answering)
            .on_click(move |_, _, cx| {
                deny.update(cx, |store, cx| match changing {
                    true => store.close_connect(cx),
                    false => store.deny_connect(cx),
                })
            });
        let approve = store.clone();
        let (verb, tooltip) = if changing { ("Save", "Save (Enter)") } else { ("Approve", "Approve (Enter)") };
        let approve_button = ActionButton::new("connect-approve", verb)
            .symbol(Symbol::Check)
            .variant(Variant::Primary)
            .tooltip(tooltip)
            .pending(choices.answering)
            .disabled(!choices.ready())
            .on_click(move |_, _, cx| approve.update(cx, |store, cx| store.approve_connect(cx)));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div().id("connect-choices").flex_1().min_h_0().overflow_y_scroll().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::XL))
                        .p(px(space::XL))
                        .child(asker(request, cx))
                        .child(ui::settings_group(
                            "Accounts",
                            Some("Whose mail it can reach."),
                            vec![ui::mark_list(account_rows(choices, store), cx).into_any_element()],
                            cx,
                        ))
                        .child(ui::settings_group(
                            "Filter",
                            Some("Which of that mail it can find. What's kept out isn't there for it."),
                            self.filter_rows(choices, summary, cx),
                            cx,
                        ))
                        .child(ui::settings_group(
                            "What it reads",
                            Some("Of each message it can reach."),
                            vec![ui::mark_list(read_rows(choices, store), cx).into_any_element()],
                            cx,
                        ))
                        .child(ui::settings_group(
                            "What it can do",
                            Some("With the mail it reaches."),
                            vec![ui::mark_list(ability_rows(choices, store), cx).into_any_element()],
                            cx,
                        ))
                        .children(error),
                ),
            )
            // What it all comes to, beside the button that approves it: down here it grows into the
            // bar, so nothing on the page moves when it changes.
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(space::L))
                    .px(px(space::XL - size::BUTTON_GAP / 2.))
                    .py(px(space::M))
                    .border_t_1()
                    .border_color(cx.color(BORDER))
                    .child(sentence(&summary.sentence, cx).flex_1().min_w_0().pl(px(size::BUTTON_GAP / 2.)))
                    .child(div().flex_none().flex().child(deny_button).child(approve_button)),
            )
    }

    // MARK: The filter

    fn filter_rows(&self, choices: &ConnectChoices, summary: &AccessSummary, cx: &App) -> Vec<AnyElement> {
        vec![
            self.filter_line(FilterLine::Only, choices, summary, cx).into_any_element(),
            ui::settings_divider(cx).into_any_element(),
            self.filter_line(FilterLine::Never, choices, summary, cx).into_any_element(),
            ui::settings_divider(cx).into_any_element(),
            reach(summary, cx).into_any_element(),
        ]
    }

    /// A line of chips, each with the word that joins it to the one before, then the add chip and,
    /// on an empty line, what that means.
    fn filter_line(&self, line: FilterLine, choices: &ConnectChoices, summary: &AccessSummary, cx: &App) -> Div {
        let chips = summary.chips(line);
        let menu = choices.menu.as_ref();
        let name = match line {
            FilterLine::Only => "only",
            FilterLine::Never => "never",
        };
        let mut items: Vec<AnyElement> = Vec::new();
        for (index, chip) in chips.iter().enumerate() {
            if let Some(joiner) = &chip.joiner {
                items.push(beside_chip(joiner.clone(), cx).into_any_element());
            }
            let value = chip.filter_value();
            let open = menu.is_some_and(|menu| menu.opened_by(line, Some(&value)));
            let face = self.condition_chip(SharedString::from(format!("filter-{name}-{index}")), line, chip, open);
            let menu = menu.filter(|_| open);
            items.push(self.with_menu(face, menu, cx));
        }
        let adding = menu.is_some_and(|menu| menu.opened_by(line, None));
        let title = if line == FilterLine::Only && chips.is_empty() { "Narrow it" } else { "Add" };
        let store = self.store.clone();
        let add = AddChip::new(SharedString::from(format!("filter-{name}-add")), title, move |_, _, cx| {
            store.update(cx, |store, cx| store.toggle_filter_menu(line, None, cx))
        })
        .open(adding);
        items.push(self.with_menu(add, menu.filter(|_| adding), cx));
        if chips.is_empty() {
            let empty = if line == FilterLine::Only { &summary.only_empty } else { &summary.never_empty };
            items.push(beside_chip(empty.clone(), cx).into_any_element());
        }
        let label = if line == FilterLine::Only { "Only" } else { "Never" };
        div()
            .flex()
            .items_start()
            .gap(px(space::M))
            .px(px(space::L))
            .py(px(space::M))
            .child(
                div()
                    .flex_none()
                    .w(px(48.))
                    .h(px(size::CHIP))
                    .flex()
                    .items_center()
                    .text_in(TextStyle::Label, MUTED_FOREGROUND, cx)
                    .child(label),
            )
            .child(div().flex_1().min_w_0().flex().flex_wrap().items_center().gap(px(space::XS)).children(items))
    }

    fn condition_chip(&self, id: SharedString, line: FilterLine, chip: &FilterChip, open: bool) -> ConditionChip {
        let (opening, removing) = (self.store.clone(), self.store.clone());
        let (edited, removed) = (chip.filter_value(), chip.filter_value());
        ConditionChip::new(
            id,
            chip.label.clone(),
            chip.text.clone(),
            move |_, _, cx| {
                let edited = edited.clone();
                opening.update(cx, |store, cx| store.toggle_filter_menu(line, Some(edited), cx))
            },
            move |_, _, cx| {
                let removed = removed.clone();
                removing.update(cx, |store, cx| store.remove_filter(line, removed, cx))
            },
        )
        .help(chip.help.clone())
        .empty(chip.matches == 0)
        .open(open)
    }

    /// The menu: the field (with the kind picked as a tag), the options in their sections, and
    /// Remove when it changes a chip. Drawn once its first options are in, so it never shows empty.
    /// A chip, and the menu it opened under it, or over it (the field next to the chip) when the
    /// menu at its tallest doesn't fit under it.
    fn with_menu(&self, face: impl IntoElement, menu: Option<&FilterMenu>, cx: &App) -> AnyElement {
        let under = menu.and_then(|menu| self.menu(menu, false, cx));
        let over = menu.and_then(|menu| self.menu(menu, true, cx));
        let footer = if menu.is_some_and(|menu| menu.editing.is_some()) { size::REGULAR } else { 0. };
        let tallest = size::REGULAR + MENU_LIST + footer + 2. * size::HAIRLINE;
        ui::popup_under(face, under).over(over, tallest).faces(self.faces.clone()).into_any_element()
    }

    /// The menu: over its face (`over`), the field goes at its bottom, next to the face, so what
    /// is typed stays put as the list changes.
    fn menu(&self, menu: &FilterMenu, over: bool, cx: &App) -> Option<AnyElement> {
        let options = menu.options.as_ref()?;
        let tag = menu.scope.and(options.scope.clone()).map(|scope| {
            let store = self.store.clone();
            Chip::new("filter-scope", scope)
                .remove_help("Show every kind")
                .remove(move |_, _, cx| store.update(cx, |store, cx| store.narrow_filter_menu(None, cx)))
                .into_any_element()
        });
        let store = self.store.clone();
        let keys: ui::FieldKeys =
            Rc::new(move |key, _, cx| store.update(cx, |store, cx| store.filter_menu_key(key, cx)));
        let store = self.store.clone();
        let clear = Rc::new(move |_: &mut Window, cx: &mut App| {
            store.update(cx, |store, cx| store.set_filter_query(String::new(), cx))
        });
        let field = ui::tagged_search_field(
            "filter-search",
            &self.query,
            if over { ui::SearchStyle::PopupBottom } else { ui::SearchStyle::Popup },
            Symbol::Search,
            tag,
            keys,
            clear,
            cx,
        );
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut index = 0;
        for (number, section) in options.sections.iter().enumerate() {
            if let Some(title) = &section.title {
                rows.push(section_title(number, title, cx).into_any_element());
            }
            for option in &section.options {
                rows.push(self.option_row(index, option, index == menu.highlighted).into_any_element());
                index += 1;
            }
        }
        if let Some(note) = &options.note {
            rows.push(
                div()
                    .p(px(space::S + 2.))
                    .text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)
                    .child(note.clone())
                    .into_any_element(),
            );
        }
        let footer = menu.editing.clone().map(|editing| {
            let store = self.store.clone();
            let line = menu.line;
            div()
                .flex()
                .items_center()
                .gap(px(space::S))
                .h(px(size::REGULAR))
                .pl(px(space::M))
                .pr(px(space::S - size::BUTTON_GAP / 2.))
                .border_t_1()
                .border_color(cx.color(BORDER_POPOVER))
                .child(
                    ui::line_in("Pick another to replace it", TextStyle::Footnote, MUTED_STRONGER_FOREGROUND, cx)
                        .flex_1(),
                )
                .child(
                    ActionButton::new("filter-remove", "Remove")
                        .symbol(Symbol::Trash)
                        .variant(Variant::Destructive)
                        .size(ControlSize::Small)
                        .surface(Surface::Popover)
                        .on_click(move |_, _, cx| {
                            let editing = editing.clone();
                            store.update(cx, |store, cx| store.remove_filter(line, editing, cx))
                        }),
                )
        });
        let list = div()
            .id("filter-options")
            .flex_shrink(1.)
            .min_h_0()
            .max_h(px(MENU_LIST))
            .overflow_y_scroll()
            .track_scroll(&self.options)
            .p(px(space::S))
            .flex()
            .flex_col()
            .children(rows);
        let field = field.into_any_element();
        let (top, bottom) = if over { (None, Some(field)) } else { (Some(field), None) };
        let faces = self.faces.clone();
        let store = self.store.clone();
        Some(
            ui::popup_card(cx)
                .id("filter-menu")
                .max_h_full()
                .occlude()
                .w(px(size::POPUP_WIDTH))
                .flex()
                .flex_col()
                .overflow_hidden()
                .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
                    if faces.borrow().iter().any(|face| face.contains(&event.position)) {
                        return;
                    }
                    store.update(cx, |store, cx| store.close_filter_menu(cx));
                })
                .children(top)
                .child(list)
                .children(footer)
                .children(bottom)
                .into_any_element(),
        )
    }

    fn option_row(&self, index: usize, option: &FilterOption, highlighted: bool) -> ChoiceRow {
        let store = self.store.clone();
        let lead = match (&option.avatar, &option.symbol) {
            (Some(avatar), _) => Lead::Avatar(avatar.clone()),
            (None, Some(symbol)) => Lead::Symbol(Symbol::named(symbol)),
            (None, None) => Lead::None,
        };
        // A detail beside a count goes on a line of its own, under the title.
        let two_lines = option.detail.is_some() && option.count.is_some();
        let row = ChoiceRow::new(SharedString::from(format!("filter-option-{}", option.id)), option.title.clone(), {
            move |_, _, cx| store.update(cx, |store, cx| store.pick_filter_option(index, cx))
        })
        .lead(lead)
        .checked(option.checked)
        .highlighted(highlighted)
        .on_popover()
        .on_hover({
            let store = self.store.clone();
            move |_, cx| store.update(cx, |store, cx| store.hover_filter_option(index, cx))
        });
        let row = match (two_lines, &option.detail) {
            (true, Some(detail)) => row.subtitle(detail.clone()),
            _ => row,
        };
        match option.count.as_ref().or(option.detail.as_ref()) {
            Some(detail) => row.detail(detail.clone()),
            None => row,
        }
    }
}

/// Words on a line of chips, as tall as a chip: a joiner, or what an empty line means.
fn beside_chip(text: String, cx: &App) -> Div {
    div()
        .flex_none()
        .h(px(size::CHIP))
        .flex()
        .items_center()
        .text_in(TextStyle::Footnote, MUTED_STRONGER_FOREGROUND, cx)
        .child(text)
}

fn section_title(number: usize, title: &str, cx: &App) -> impl IntoElement + use<> {
    div()
        .id(("filter-section", number))
        .flex_none()
        .h(px(26.))
        .flex()
        .items_end()
        .px(px(space::S + 2.))
        .pb(px(2.))
        .text_in(TextStyle::Footnote, MUTED_STRONGER_FOREGROUND, cx)
        .font_weight(FontWeight::MEDIUM)
        .child(title.to_uppercase())
}

/// How much of the mail it reaches, as a bar and in words, and a condition that changes nothing.
fn reach(summary: &AccessSummary, cx: &App) -> Div {
    let fraction = summary.fraction.clamp(0., 1.) as f32;
    let filled = if fraction > 0. { (fraction * REACH_BAR).max(4.) } else { 0. };
    div()
        .flex()
        .flex_col()
        .gap(px(space::S))
        .px(px(space::L))
        .py(px(space::M))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::M))
                .child(
                    div()
                        .flex_none()
                        .w(px(REACH_BAR))
                        .h(px(4.))
                        .rounded_full()
                        .bg(cx.color(BACKGROUND_SECONDARY))
                        .child(div().h_full().w(px(filled)).rounded_full().bg(cx.color(MUTED_FOREGROUND))),
                )
                .child(ui::line(summary.reach.clone(), TextStyle::Caption, cx).flex_1()),
        )
        .when_some(summary.warning.clone(), |block, warning| {
            block.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(space::XS))
                    .child(
                        div()
                            .h(TextStyle::Caption.line_height())
                            .flex()
                            .items_center()
                            .child(ui::icon(Symbol::CircleAlert, 12.).text_color(cx.color(WARNING))),
                    )
                    .child(ui::paragraph_in(warning, TextStyle::Caption, WARNING, cx).flex_1()),
            )
        })
}

const REACH_BAR: f32 = 120.;

/// The sentence that says what the app may do, its strong runs in bold.
fn sentence(parts: &[SentencePart], cx: &App) -> Div {
    let mut text = String::new();
    let mut strong = Vec::new();
    for part in parts {
        let start = text.len();
        text.push_str(&part.text);
        if part.strong {
            strong.push((
                start..text.len(),
                HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() },
            ));
        }
    }
    div().text(TextStyle::Body, cx).child(StyledText::new(text).with_highlights(strong))
}

/// Who is asking, whether it is verified, where it sends the browser, and the warnings; for a
/// connected app being changed, who it is.
fn asker(request: &ConnectRequest, cx: &App) -> Div {
    let symbol = if request.verified { Symbol::ShieldCheck } else { Symbol::Box };
    let chip = match request.verified {
        true => Chip::new("connect-verified", "Verified").symbol(Symbol::ShieldCheck).tone(Tone::Success),
        false => Chip::new("connect-verified", "Unverified").symbol(Symbol::CircleAlert).tone(Tone::Warning),
    };
    let asking = request.app.is_none();
    let on_device = (asking && request.on_device).then(|| {
        ui::notice("Sends you back to an application on this device", Tone::Warning, Some(Symbol::Monitor), None, cx)
    });
    let back_to = asking.then(|| ui::notice(request.back_to.clone(), Tone::Muted, Some(Symbol::ArrowRight), None, cx));
    let started = asking.then(|| {
        ui::notice("Only approve if you started this yourself", Tone::Warning, Some(Symbol::CircleAlert), None, cx)
    });
    ui::settings_card(
        vec![
            div()
                .flex()
                .flex_col()
                .gap(px(space::M))
                .p(px(space::L))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S))
                        .child(ui::icon(symbol, 20.).text_color(cx.color(FOREGROUND)))
                        .child(ui::line(request.client.clone(), TextStyle::Heading, cx).flex_shrink(1.))
                        .child(chip),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::XS))
                        .child(ui::notice(request.publisher.clone(), Tone::Muted, Some(Symbol::Globe), None, cx))
                        .children(back_to)
                        .children(on_device)
                        .children(started),
                )
                .into_any_element(),
        ],
        cx,
    )
}

// MARK: The choices

/// What a click on a choice changes in the access.
fn changing(
    store: &Entity<Store>,
    change: impl Fn(&mut crate::models::Access) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let store = store.clone();
    move |_, _, cx| store.update(cx, |store, cx| store.change_access(&change, cx))
}

/// Puts `item` in the list, or takes it out.
fn toggle<T: PartialEq + Clone>(list: &mut Vec<T>, item: &T) {
    match list.iter().position(|known| known == item) {
        Some(index) => {
            list.remove(index);
        }
        None => list.push(item.clone()),
    }
}

fn account_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<MarkRow> {
    let access = &choices.access;
    let mut rows = vec![
        MarkRow::new("connect-every-account", "Every account", Mark::Radio, changing(store, |a| a.all_accounts = true))
            .detail("Including the ones you add later.")
            .lead(Lead::Symbol(Symbol::Inbox))
            .checked(access.all_accounts),
        MarkRow::new(
            "connect-some-accounts",
            "Only some accounts",
            Mark::Radio,
            changing(store, |a| a.all_accounts = false),
        )
        .detail("Only the ones you pick.")
        .lead(Lead::Symbol(Symbol::ListFilter))
        .checked(!access.all_accounts),
    ];
    if access.all_accounts {
        return rows;
    }
    rows.extend(choices.request.accounts.iter().map(|account| {
        let id = account.id.clone();
        let color = account_color(account.color.as_deref().unwrap_or_default());
        MarkRow::new(
            SharedString::from(format!("connect-account-{}", account.id)),
            account.title.clone(),
            Mark::Check,
            changing(store, move |a| toggle(&mut a.accounts, &id)),
        )
        .detail(account.detail.clone())
        .lead(Lead::Dot(color))
        .checked(access.accounts.contains(&account.id))
        .indent(size::MARK + space::M)
    }));
    rows
}

fn read_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<MarkRow> {
    let request = &choices.request;
    let always = choice_row("connect-read", &request.always, Mark::Check, |_, _, _| {}).checked(true).locked(true);
    let reads = request.reads.iter().filter_map(|choice| {
        let part = Part::ALL.into_iter().find(|part| part.as_str() == choice.id)?;
        let row =
            choice_row("connect-read", choice, Mark::Check, changing(store, move |a| toggle(&mut a.reads, &part)));
        Some(row.checked(choices.access.reads.contains(&part)))
    });
    std::iter::once(always).chain(reads).collect()
}

fn ability_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<MarkRow> {
    let request = &choices.request;
    let roles = request.roles.iter().filter_map(|choice| {
        let role = Role::ALL.into_iter().find(|role| role.as_str() == choice.id)?;
        let row = choice_row("connect-role", choice, Mark::Radio, changing(store, move |a| a.role = role));
        Some(row.checked(choices.access.role == role))
    });
    let privileges = request.privileges.iter().filter_map(|choice| {
        let privilege = Privilege::ALL.into_iter().find(|privilege| privilege.as_str() == choice.id)?;
        let row = choice_row(
            "connect-privilege",
            choice,
            Mark::Check,
            changing(store, move |a| toggle(&mut a.privileges, &privilege)),
        );
        Some(row.checked(choices.access.privileges.contains(&privilege)))
    });
    roles.chain(privileges).collect()
}

fn choice_row(
    group: &str,
    choice: &AccessChoice,
    mark: Mark,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> MarkRow {
    let lead = choice.symbol.as_deref().map_or(Lead::None, |name| Lead::Symbol(Symbol::named(name)));
    MarkRow::new(SharedString::from(format!("{group}-{}", choice.id)), choice.title.clone(), mark, on_click)
        .detail(choice.detail.clone())
        .lead(lead)
}
