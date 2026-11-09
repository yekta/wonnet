//! The settings' pieces: a group's name over a card of rows, each what it is about on the left
//! and its controls on the right.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AlignItems, AnyElement, App, Bounds, ClickEvent, Display, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, LayoutId, MouseButton, Pixels, SharedString, Style, Window, div, point, px,
};

use super::{Dragged, Dropped, MenuEntry, OnClick, RowMenu, icon, line, movable, open_menu, paragraph};
use crate::theme::*;

/// A group of a settings page: its name and a line about it, over a card of rows.
pub fn settings_group(title: &str, detail: Option<&str>, rows: Vec<AnyElement>, cx: &App) -> Div {
    group(title, detail, settings_card(rows, cx).into_any_element(), cx)
}

/// A group whose content is a field of its own, as the MCP server's address: on the page, as
/// wide as the cards, with no card around it.
pub fn settings_group_on_page(title: &str, detail: Option<&str>, content: AnyElement, cx: &App) -> Div {
    group(title, detail, content, cx)
}

fn group(title: &str, detail: Option<&str>, content: AnyElement, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::S + 2.))
        .child(
            super::section_heading(title.to_string(), detail.map(|detail| SharedString::from(detail.to_string())), cx)
                .px(px(space::XXS)),
        )
        .child(content)
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

/// One row of a group: what it is about on the left, its controls on the right. In a row of one
/// line a control at the end is as far from the card's side as from the row's top and bottom; in
/// a taller one it is as far in as the words are from the other side, so controls line up.
pub fn settings_row(leading: impl IntoElement, trailing: Vec<AnyElement>) -> Div {
    let has_controls = !trailing.is_empty();
    div()
        .flex()
        .items_center()
        .gap(px(space::M))
        .min_h(px(size::SETTINGS_ROW))
        .py(px(space::S))
        .pl(px(space::L))
        .pr(px(if has_controls { 0. } else { space::L }))
        .child(div().flex_1().min_w_0().child(leading))
        .when(has_controls, |row| {
            row.child(EdgeControls(div().flex().flex_none().items_center().children(trailing).into_any_element()))
        })
}

/// A row's controls, set in from its end once the row's height is known: by as much as there is
/// above and below them in a row of one line, by `space::L` in a taller one. The room is kept at its widest, and the controls
/// moved out into what they don't need of it. The last control pads half the button gap inside.
struct EdgeControls(AnyElement);

impl IntoElement for EdgeControls {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EdgeControls {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let controls = self.0.request_layout(window, cx);
        let mut style = Style { display: Display::Flex, flex_shrink: 0., ..Style::default() };
        style.align_self = Some(AlignItems::Stretch);
        style.align_items = Some(AlignItems::Center);
        // As tall as the row, past its padding.
        style.margin.top = px(-space::S).into();
        style.margin.bottom = px(-space::S).into();
        style.padding.right = px(space::L - size::BUTTON_GAP / 2.).into();
        (window.request_layout(style, [controls], cx), controls)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        controls: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let height = window.layout_bounds(*controls).size.height;
        let one_line = bounds.size.height <= px(size::SETTINGS_ROW);
        let room = if one_line { ((bounds.size.height - height) / 2.).min(px(space::L)) } else { px(space::L) };
        window.with_element_offset(point(px(space::L) - room, px(0.)), |window| {
            self.0.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
    }
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
    let lines = detail.is_some();
    div()
        .flex()
        .items_center()
        .gap(px(space::S + 2.))
        .when(lines, |label| label.py(px(LINES_INSET)))
        .children(lead)
        .child(
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

/// What words of several lines add to a row's padding, so they are `space::M` from its top and
/// bottom.
const LINES_INSET: f32 = space::M - space::S;

/// What a row is about when it says more: its name, a chip after it, and lines under it.
pub fn settings_item(
    title: impl Into<SharedString>,
    chip: Option<AnyElement>,
    lines: Vec<SharedString>,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .when(!lines.is_empty(), |item| item.py(px(LINES_INSET)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::S))
                .child(line(title, TextStyle::Body, cx).flex_shrink(1.))
                .children(chip),
        )
        .children(lines.into_iter().map(|text| paragraph(text, TextStyle::Caption, cx)))
}

/// The hairline between a group's rows.
pub fn settings_divider(cx: &App) -> Div {
    super::rule(BORDER_CARD, cx)
}

/// A row that only says something: nothing set yet.
pub fn settings_note(text: impl Into<SharedString>, cx: &App) -> Div {
    settings_row(div().text_in(TextStyle::Label, MUTED_FOREGROUND, cx).child(text.into()), Vec::new())
}

/// How a choice shows that it is picked: a round mark for one of a few, a square one for each of
/// several that is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Radio,
    Check,
}

/// A choice on a form's card: its mark, a symbol or a dot, its name and a line about it. A click
/// anywhere on it picks it. `indent` sets it under the choice it belongs to. A `locked` choice is
/// always so: a lock stands in its mark's place, and it can't be clicked or hovered.
#[derive(IntoElement)]
pub struct MarkRow {
    id: SharedString,
    title: SharedString,
    detail: Option<SharedString>,
    lead: super::Lead,
    mark: Mark,
    checked: bool,
    locked: bool,
    indent: f32,
    /// The first or the last row of its card, whose corners its fill follows: the card's clip is
    /// square.
    first: bool,
    last: bool,
    on_click: OnClick,
}

impl MarkRow {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        mark: Mark,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            lead: super::Lead::None,
            mark,
            checked: false,
            locked: false,
            indent: 0.,
            first: false,
            last: false,
            on_click: Rc::new(on_click),
        }
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn lead(mut self, lead: super::Lead) -> Self {
        self.lead = lead;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }
}

impl RenderOnce for MarkRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = self.id.clone();
        let on_click = self.on_click.clone();
        // The words are as far from the row's top and bottom as the Mac's: `space::S + 2.`, the
        // label's own inset for a detail included.
        let inset = if self.detail.is_some() { space::S + 2. - LINES_INSET } else { space::S };
        let label = settings_label(self.title.clone(), self.detail.clone(), self.lead.clone(), false, cx);
        let corner = px(radius::MD - size::HAIRLINE);
        div()
            .id(self.id.clone())
            .group(group.clone())
            .when(self.first, |row| row.rounded_t(corner))
            .when(self.last, |row| row.rounded_b(corner))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(space::M))
            .min_h(px(size::ROW))
            .py(px(inset))
            .pl(px(space::L + self.indent))
            .pr(px(space::L))
            .when(!self.locked, |row| {
                row.cursor_pointer()
                    .hover(|row| row.bg(cx.color(FORM_CARD_ACCENT)))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(move |event, window, cx| on_click(event, window, cx))
            })
            .child(match self.locked {
                true => {
                    icon(super::Symbol::Lock, 14.).size(px(size::MARK)).text_color(cx.color(MUTED_STRONGER_FOREGROUND))
                }
                false => mark(self.mark, self.checked, group, cx),
            })
            .child(label.flex_1().min_w_0())
    }
}

/// The mark itself: filled in the primary colour when picked, else a ring that darkens on hover.
fn mark(kind: Mark, checked: bool, hover_group: SharedString, cx: &App) -> Div {
    let side = size::MARK;
    let face = div().size(px(side)).flex_none().flex().items_center().justify_center();
    let face = match kind {
        Mark::Radio => face.rounded_full(),
        Mark::Check => face.rounded(px(radius::XS)),
    };
    if !checked {
        return face
            .border(px(size::SELECT_LINE))
            .border_color(cx.color(MUTED_STRONGEST_FOREGROUND))
            .group_hover(hover_group, |face| face.border_color(cx.color(MUTED_FOREGROUND)));
    }
    let face = face.bg(cx.color(PRIMARY));
    match kind {
        Mark::Radio => face.child(div().size(px(side * 0.4)).rounded_full().bg(cx.color(PRIMARY_FOREGROUND))),
        Mark::Check => face.child(icon(super::Symbol::Check, side * 0.75).text_color(cx.color(PRIMARY_FOREGROUND))),
    }
}

/// Choices on a form's card, as the Mac's: each across the whole card, hovering in a fill as wide
/// as it, with a hairline between one and the next.
pub fn mark_list(rows: Vec<MarkRow>, cx: &App) -> Div {
    let count = rows.len();
    let mut list = div().flex().flex_col();
    for (index, mut row) in rows.into_iter().enumerate() {
        if index > 0 {
            list = list.child(settings_divider(cx));
        }
        row.first = index == 0;
        row.last = index + 1 == count;
        list = list.child(row);
    }
    list
}
