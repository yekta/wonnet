//! The settings' pieces: a group's name over a card of rows, each what it is about on the left
//! and its controls on the right.

use gpui::prelude::*;
use gpui::{AnyElement, App, Div, SharedString, div, px};

use super::{icon, line, paragraph};
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
