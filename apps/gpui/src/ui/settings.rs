//! The settings' pieces: a group's name over a card of rows, each what it is about on the left
//! and its controls on the right.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, ClickEvent, Div, MouseButton, SharedString, Window, div, px};

use super::{Dragged, Dropped, MenuEntry, OnClick, RowMenu, icon, line, movable, open_menu, paragraph};
use crate::theme::*;

/// A group of a settings page: its name and a line about it, over a card of rows.
pub fn settings_group(title: &str, detail: Option<&str>, rows: Vec<AnyElement>, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::S + 2.))
        .child(
            super::section_heading(title.to_string(), detail.map(|detail| SharedString::from(detail.to_string())), cx)
                .px(px(space::XXS)),
        )
        .child(settings_card(rows, cx))
}

/// The card of a group's rows.
pub fn settings_card(rows: Vec<AnyElement>, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::MD))
        .bg(cx.color(CARD))
        .border_1()
        .border_color(cx.color(BORDER))
        .children(rows)
}

/// One row of a group: what it is about on the left, its controls on the right. A control at
/// either end is as far from the card's side as from the row's top and bottom.
pub fn settings_row(leading: impl IntoElement, trailing: Vec<AnyElement>) -> Div {
    let has_controls = !trailing.is_empty();
    div()
        .flex()
        .items_center()
        .gap(px(space::M))
        .min_h(px(size::SETTINGS_ROW))
        .py(px(space::S))
        .pl(px(space::L))
        .pr(px(if has_controls { (size::SETTINGS_ROW - size::REGULAR) / 2. - size::BUTTON_GAP / 2. } else { space::L }))
        .child(div().flex_1().min_w_0().child(leading))
        .child(div().flex().flex_none().items_center().children(trailing))
}

/// A row of a card's list the user puts in order: it is dragged onto another to take its place,
/// offers a menu on a right-click, and with a click of its own hovers and opens. The card clips to
/// a rectangle, so the first row rounds its own corners inside its border. The card's last row
/// is always its Add.
#[derive(IntoElement)]
pub struct MovableRow {
    id: SharedString,
    group: SharedString,
    leading: AnyElement,
    trailing: Option<AnyElement>,
    first: bool,
    on_click: Option<OnClick>,
    menu: Option<RowMenu>,
    dropped: Dropped,
}

impl MovableRow {
    pub fn new(
        id: impl Into<SharedString>,
        group: impl Into<SharedString>,
        leading: impl IntoElement,
        dropped: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            group: group.into(),
            leading: leading.into_any_element(),
            trailing: None,
            first: false,
            on_click: None,
            menu: None,
            dropped: Rc::new(dropped),
        }
    }

    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// The card's first row.
    pub fn first(mut self, first: bool) -> Self {
        self.first = first;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn menu(mut self, builder: impl Fn(&mut Window, &mut App) -> Vec<MenuEntry> + 'static) -> Self {
        self.menu = Some(Rc::new(builder));
        self
    }
}

impl RenderOnce for MovableRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let corner = px(radius::MD - size::HAIRLINE);
        let menu_id = self.id.clone();
        let dragged = Dragged { id: self.id.clone(), title: self.id.clone() };
        let row = div()
            .id(self.id)
            .border_b_1()
            .border_color(cx.color(BORDER_CARD))
            .when(self.first, |row| row.rounded_t(corner))
            .when_some(self.on_click, |row, on_click| {
                row.cursor_pointer()
                    .hover(|row| row.bg(cx.color(FORM_CARD_ACCENT)))
                    .on_click(move |event, window, cx| on_click(event, window, cx))
            })
            .when_some(self.menu, |row, menu| {
                row.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    let entries = menu(window, cx);
                    open_menu(menu_id.clone(), entries, Some(event.position), window, cx);
                })
            })
            .child(settings_row(self.leading, self.trailing.into_iter().collect()));
        movable(row, self.group, dragged, false, self.dropped, cx)
    }
}

/// What a row is about: its name, a line under it, and a symbol or a dot before it. `heading`
/// sets its name as a heading, for the row that heads a card.
pub fn settings_label(
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
    lead: super::Lead,
    heading: bool,
    cx: &App,
) -> Div {
    let lead = match lead {
        super::Lead::Dot(color) => {
            let side = if heading { 10. } else { 8. };
            Some(div().size(px(side)).flex_none().rounded_full().bg(cx.color(color)).into_any_element())
        }
        super::Lead::Symbol(symbol) => {
            Some(icon(symbol, 14.).text_color(cx.color(MUTED_FOREGROUND)).into_any_element())
        }
        _ => None,
    };
    div().flex().items_center().gap(px(space::S + 2.)).children(lead).child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(line(title, if heading { TextStyle::Subheading } else { TextStyle::Body }, cx))
            .when_some(detail, |column, detail| column.child(paragraph(detail, TextStyle::Caption, cx))),
    )
}

/// The hairline between a group's rows.
pub fn settings_divider(cx: &App) -> Div {
    super::rule(BORDER_CARD, cx)
}

/// A row that only says something: nothing set yet.
pub fn settings_note(text: impl Into<SharedString>, cx: &App) -> Div {
    settings_row(div().text_in(TextStyle::Label, MUTED_FOREGROUND, cx).child(text.into()), Vec::new())
}
