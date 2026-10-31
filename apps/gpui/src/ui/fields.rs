//! Fields, on GPUI Component's text inputs: a field with its label above, the search field that
//! narrows a list (with the keys that move through it), and the face of a combobox.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Entity, FocusHandle, Focusable as _, Global, Keystroke, MouseButton, SharedString, Window, div, px,
};
use gpui_component::input::{Input, InputState};

use super::{IconButton, Symbol, icon, line};
use crate::theme::*;

/// What a field does with the keys that move through the list under it. True when it took one.
pub type FieldKeys = Rc<dyn Fn(&str, &mut Window, &mut App) -> bool>;

/// The fields that take the keys of a list under them, with what they do with them, as they
/// were last drawn. The window looks here before the fields' own keys run.
#[derive(Default)]
pub struct ListKeyTargets {
    fields: RefCell<Vec<(FocusHandle, FieldKeys)>>,
    /// The sheets on screen, the last on top, by what closes them: Esc closes the top one.
    sheets: RefCell<Vec<Rc<dyn Fn(&mut Window, &mut App)>>>,
}

impl Global for ListKeyTargets {}

impl ListKeyTargets {
    pub fn clear(cx: &App) {
        if let Some(targets) = cx.try_global::<ListKeyTargets>() {
            targets.fields.borrow_mut().clear();
            targets.sheets.borrow_mut().clear();
        }
    }

    /// A sheet on screen, which Esc closes.
    pub fn sheet(close: Rc<dyn Fn(&mut Window, &mut App)>, cx: &App) {
        if let Some(targets) = cx.try_global::<ListKeyTargets>() {
            targets.sheets.borrow_mut().push(close);
        }
    }

    /// Gives a list key to the focused field that takes it. True when it did.
    pub fn handle(keystroke: &Keystroke, window: &mut Window, cx: &mut App) -> bool {
        let modifiers = &keystroke.modifiers;
        let plain = !modifiers.control && !modifiers.alt && !modifiers.platform;
        let key = match keystroke.key.as_str() {
            "up" | "down" | "enter" | "escape" | "tab" if plain && !modifiers.shift => keystroke.key.clone(),
            "backspace" if plain => "backspace".into(),
            _ => return false,
        };
        let Some(targets) = cx.try_global::<ListKeyTargets>() else { return false };
        let field =
            targets.fields.borrow().iter().find(|(focus, _)| focus.is_focused(window)).map(|(_, keys)| keys.clone());
        let sheet = targets.sheets.borrow().last().cloned();
        if field.is_some_and(|keys| keys(&key, window, cx)) {
            return true;
        }
        match sheet {
            Some(close) if key == "escape" => {
                close(window, cx);
                true
            }
            _ => false,
        }
    }
}

/// Hands the field's list keys (↑, ↓, Enter, Esc, Tab, ⌫) to `keys` before the field sees them.
pub fn with_list_keys(element: gpui::Div, focus: &FocusHandle, keys: FieldKeys, cx: &App) -> gpui::Div {
    if let Some(targets) = cx.try_global::<ListKeyTargets>() {
        let mut targets = targets.fields.borrow_mut();
        targets.retain(|(known, _)| known != focus);
        targets.push((focus.clone(), keys));
    }
    element
}

/// A field's words alone, for a box drawn around them here.
pub fn bare_input(state: &Entity<InputState>) -> Input {
    Input::new(state).appearance(false).px_0().py_0()
}

/// GPUI Component pads a box of lines inside; this takes it back so the words line up.
const TEXTAREA_INSET: f32 = 10.;

/// What a box of lines does with a paste before its text does: true when it took it.
pub type OnPaste = Rc<dyn Fn(&gpui::ClipboardItem, &mut Window, &mut App) -> bool>;

/// A box of lines' words alone, for a box drawn around them here.
pub fn bare_textarea(state: &Entity<gpui_component::input::TextareaState>, paste: Option<OnPaste>) -> gpui::Div {
    let mut textarea = gpui_component::input::Textarea::new(state).appearance(false).px_0().py_0();
    if let Some(paste) = paste {
        textarea = textarea.on_paste(move |item, window, cx| paste(item, window, cx));
    }
    div().mx(px(-TEXTAREA_INSET)).child(textarea)
}

/// The look of a search field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchStyle {
    /// On a hairline over a sheet's list.
    Sheet,
    /// The same, smaller, at the top of a popup.
    Popup,
    /// The top bar's capsule.
    Bar,
}

/// The search icon, the words and Clear, over a list or in the top bar.
pub fn search_field(
    id: &str,
    state: &Entity<InputState>,
    style: SearchStyle,
    symbol: Symbol,
    keys: FieldKeys,
    clear: Rc<dyn Fn(&mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement + use<> {
    let height = if style == SearchStyle::Sheet { size::SEARCH } else { size::REGULAR };
    let empty = state.read(cx).value().is_empty();
    let text_size = if style == SearchStyle::Sheet { 15. } else { 13. };
    let focus = state.read(cx).focus_handle(cx);
    let field = div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .items_center()
        .gap(px(space::S))
        .h(px(height))
        .pl(px(space::M))
        .pr(px(if empty { space::M } else { (height - size::SMALL) / 2. - size::BUTTON_GAP / 2. }))
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&focus, cx))
        .child(icon(symbol, 14.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(FONT)
                .text_size(px(text_size))
                .text_color(cx.color(FOREGROUND))
                .child(bare_input(state)),
        )
        .when(!empty, |field| {
            field.child(
                IconButton::new(SharedString::from(format!("{id}-clear")), Symbol::X, "Clear")
                    .size(ControlSize::Small)
                    .plain()
                    .quiet()
                    .on_click(move |_, window, cx| clear(window, cx)),
            )
        });
    let field = match style {
        SearchStyle::Bar => field.rounded_full().border_1().border_color(cx.color(BORDER_INPUT)),
        SearchStyle::Sheet => field.border_b_1().border_color(cx.color(BORDER_CARD)),
        SearchStyle::Popup => field.border_b_1().border_color(cx.color(BORDER_POPOVER)),
    };
    with_list_keys(div().w_full().child(field), &state.read(cx).focus_handle(cx), keys, cx)
}

/// A text field in a box, with its label above if it has one; a box of `lines` with more.
pub fn input_field(id: &str, label: Option<&str>, state: &Entity<InputState>, cx: &App) -> impl IntoElement + use<> {
    let focus = state.read(cx).focus_handle(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(space::XXS))
        .w_full()
        .when_some(label, |column, label| {
            column.child(
                div().text(TextStyle::Footnote, cx).font_weight(gpui::FontWeight::SEMIBOLD).child(label.to_string()),
            )
        })
        .child(
            div()
                .id(SharedString::from(id.to_string()))
                .flex()
                .items_center()
                .h(px(size::FIELD))
                .px(px(space::S + 2.))
                .rounded(px(radius::SM))
                .bg(cx.color(INPUT))
                .border_1()
                .border_color(cx.color(BORDER_INPUT))
                .cursor_text()
                .font_family(FONT)
                .text_size(px(TextStyle::Body.size()))
                .text_color(cx.color(FOREGROUND))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&focus, cx))
                .child(div().flex_1().min_w_0().child(bare_input(state))),
        )
}

/// A box of several lines, with its label above if it has one.
pub fn text_box(
    id: &str,
    label: Option<&str>,
    state: &Entity<gpui_component::input::TextareaState>,
    lines: usize,
    cx: &App,
) -> impl IntoElement + use<> {
    let focus = state.read(cx).focus_handle(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(space::XXS))
        .w_full()
        .when_some(label, |column, label| {
            column.child(
                div().text(TextStyle::Footnote, cx).font_weight(gpui::FontWeight::SEMIBOLD).child(label.to_string()),
            )
        })
        .child(
            div()
                .id(SharedString::from(id.to_string()))
                .h(px(lines as f32 * 18. + 16.))
                .px(px(space::S + 2.))
                .py(px(space::S))
                .rounded(px(radius::SM))
                .bg(cx.color(INPUT))
                .border_1()
                .border_color(cx.color(BORDER_INPUT))
                .cursor_text()
                .font_family(FONT)
                .text_size(px(TextStyle::Body.size()))
                .text_color(cx.color(FOREGROUND))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&focus, cx))
                .child(bare_textarea(state, None).h_full()),
        )
}

/// The look of a combobox's face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboboxStyle {
    /// A boxed field with a label above it.
    Field,
    /// Only its words, for a line of a message's head.
    Inline,
}

/// The face of a choice from a list: the choice in force and a chevron.
pub fn combobox_face(
    id: &str,
    lead: Option<AnyElement>,
    title: Option<SharedString>,
    detail: Option<SharedString>,
    placeholder: &str,
    style: ComboboxStyle,
    open: bool,
    help: Option<SharedString>,
    on_click: super::OnClick,
    cx: &App,
) -> impl IntoElement + use<> {
    let field = style == ComboboxStyle::Field;
    let rest = if field { Some(cx.color(INPUT)) } else { None };
    div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .items_center()
        .gap(px(space::S))
        .min_h(px(if field { size::FIELD } else { size::SMALL }))
        .px(px(if field { space::S + 2. } else { space::XXS }))
        .rounded(px(radius::SM))
        .when_some(rest, |face, rest| face.bg(rest))
        .when(field, |face| face.border_1().border_color(cx.color(BORDER_INPUT)))
        .when(open, |face| face.bg(cx.color(BACKGROUND_ACCENT)))
        .hover(|face| face.bg(cx.color(BACKGROUND_ACCENT)))
        .cursor_pointer()
        .when_some(help, |face, help| face.tooltip(move |_, cx| super::Tooltip::view(help.clone(), cx)))
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .children(lead)
        .child(match title {
            Some(title) => line(title, TextStyle::Body, cx).into_any_element(),
            None => line(placeholder.to_string(), TextStyle::Body, cx)
                .text_color(cx.color(MUTED_STRONGER_FOREGROUND))
                .into_any_element(),
        })
        .when_some(detail, |face, detail| face.child(line(detail, TextStyle::Caption, cx)))
        .when(field, |face| face.child(div().flex_1()))
        .child(icon(Symbol::ChevronDown, 12.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)))
}
