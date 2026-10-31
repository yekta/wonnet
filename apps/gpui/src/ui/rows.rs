//! Rows: what goes somewhere (the sidebar's), a choice under a field, a thing in a list, and the
//! tabs. Rows the user puts in order are dragged to their place.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Div, Hsla, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Stateful,
    Window, canvas, div, px,
};

use super::{Avatar, MenuEntry, OnClick, Symbol, icon, line, line_in, open_menu};
use crate::models::Address;
use crate::theme::*;

/// What a row offers on a right-click.
pub type RowMenu = Rc<dyn Fn(&mut Window, &mut App) -> Vec<MenuEntry>>;

fn right_click(id: SharedString, menu: RowMenu) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let entries = menu(window, cx);
        open_menu(id.clone(), entries, Some(event.position), window, cx);
    }
}

/// What a row starts with.
#[derive(Clone)]
pub enum Lead {
    None,
    Symbol(Symbol),
    Dot(ThemeColor),
    Avatar(Address),
}

/// A row that goes somewhere: a mailbox in the sidebar, Settings. `selected` is where the app
/// is; `count` its unread, shown when above zero.
#[derive(IntoElement)]
pub struct NavRow {
    id: SharedString,
    title: SharedString,
    lead: Lead,
    count: u32,
    selected: bool,
    indent: f32,
    vertical: f32,
    trailing: Vec<AnyElement>,
    on_click: Option<OnClick>,
    menu: Option<RowMenu>,
    dragged: Option<(SharedString, Dropped)>,
}

/// What is dragged when a row is moved: its id and a picture of it.
#[derive(Clone)]
pub struct Dragged {
    pub id: SharedString,
    pub title: SharedString,
}

impl Render for Dragged {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px(px(space::M))
            .py(px(space::XS))
            .rounded(px(radius::MD))
            .bg(cx.color(POPOVER))
            .border_1()
            .border_color(cx.color(BORDER_POPOVER))
            .shadow(cx.shadow(ShadowSize::Md, opacity::SHADOW_STRONGER))
            .text(TextStyle::Label, cx)
            .child(self.title.clone())
    }
}

impl NavRow {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            lead: Lead::None,
            count: 0,
            selected: false,
            indent: 0.,
            vertical: 0.,
            trailing: Vec::new(),
            on_click: None,
            menu: None,
            dragged: None,
        }
    }

    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.lead = Lead::Symbol(symbol);
        self
    }

    pub fn dot(mut self, color: ThemeColor) -> Self {
        self.lead = Lead::Dot(color);
        self
    }

    pub fn count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    pub fn vertical_padding(mut self, padding: f32) -> Self {
        self.vertical = padding;
        self
    }

    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
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

    /// Dragged onto another row of the same `group`, it is put before or after that row.
    pub fn movable(
        mut self,
        group: impl Into<SharedString>,
        dropped: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.dragged = Some((group.into(), Rc::new(dropped)));
        self
    }
}

impl RenderOnce for NavRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let fill = if self.selected { Some(cx.color(BACKGROUND_ACCENT_STRONGER)) } else { None };
        let lead = match &self.lead {
            Lead::Dot(color) => div().size(px(8.)).rounded_full().bg(cx.color(*color)).into_any_element(),
            Lead::Symbol(symbol) => icon(*symbol, 15.).into_any_element(),
            _ => div().into_any_element(),
        };
        let on_click = self.on_click.clone();
        let face = div()
            .flex()
            .items_center()
            .gap(px(space::M))
            .h(px(size::ROW + self.vertical * 2.))
            .pl(px(18. + self.indent))
            .pr(px(space::L))
            .text_color(cx.color(FOREGROUND))
            .when_some(fill, |face, fill| face.bg(fill))
            .when(!self.selected, |face| face.hover(|face| face.bg(cx.color(BACKGROUND_ACCENT))))
            .child(div().w(px(18.)).flex().justify_center().child(lead))
            .child(line_in(self.title.clone(), TextStyle::Label, FOREGROUND, cx).flex_1())
            .when(self.count > 0, |face| face.child(line(self.count.to_string(), TextStyle::Caption, cx).flex_none()))
            .children(self.trailing);
        let row = div()
            .id(self.id.clone())
            .py(px(size::ROW_GAP / 2.))
            .cursor_pointer()
            .on_click(move |event, window, cx| {
                if let Some(on_click) = &on_click {
                    on_click(event, window, cx);
                }
            })
            .when_some(self.menu.clone(), |row, menu| {
                row.on_mouse_down(MouseButton::Right, right_click(self.id.clone(), menu))
            })
            .child(face);
        let Some((group, dropped)) = self.dragged else { return row };
        let dragged = Dragged { id: self.id.clone(), title: self.title.clone() };
        movable(row, group, dragged, false, dropped, cx)
    }
}

/// A dragged row, and the group it moves in.
#[derive(Clone)]
pub struct DragGroup(pub SharedString, pub Dragged);

/// What a drop on a row does: the dragged row's id, and whether it lands after the row.
pub type Dropped = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

/// Whether the pointer is past the middle of a row, so a row dropped there goes after it.
fn past_middle(bounds: Bounds<Pixels>, pointer: Point<Pixels>, horizontal: bool) -> bool {
    if horizontal { pointer.x > bounds.center().x } else { pointer.y > bounds.center().y }
}

/// Makes a row one that is dragged onto the others of its `group`, landing before or after
/// the one it is let go over, by which half of it the pointer is in. A line shows where.
pub fn movable(
    element: Stateful<Div>,
    group: SharedString,
    dragged: Dragged,
    horizontal: bool,
    dropped: Dropped,
    cx: &App,
) -> Stateful<Div> {
    let bounds: Rc<Cell<Bounds<Pixels>>> = Rc::default();
    let measured = bounds.clone();
    let over = bounds.clone();
    let accepts = group.clone();
    let target = dragged.id.clone();
    let line = cx.color(PRIMARY);
    element
        .relative()
        .child(canvas(move |area, _, _| measured.set(area), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .on_drag(DragGroup(group, dragged), |drag, _, _, cx| cx.new(|_| drag.1.clone()))
        // A drag in the window's bar moves the row, not the window.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .can_drop(move |value, _, _| value.downcast_ref::<DragGroup>().is_some_and(|drag| drag.0 == accepts))
        .drag_over::<DragGroup>(move |row, _, window, _| {
            match (horizontal, past_middle(over.get(), window.mouse_position(), horizontal)) {
                (true, true) => row.border_r_2(),
                (true, false) => row.border_l_2(),
                (false, true) => row.border_b_2(),
                (false, false) => row.border_t_2(),
            }
            .border_color(line)
        })
        .on_drop(move |drag: &DragGroup, window, cx| {
            if drag.1.id != target {
                let after = past_middle(bounds.get(), window.mouse_position(), horizontal);
                dropped(&drag.1.id, after, window, cx);
            }
        })
}

/// The ids with `dragged` put before or after `target`.
pub fn reordered(mut ids: Vec<String>, dragged: &str, target: &str, after: bool) -> Vec<String> {
    if dragged == target {
        return ids;
    }
    ids.retain(|id| id != dragged);
    let index = ids.iter().position(|id| id == target).map_or(ids.len(), |index| index + usize::from(after));
    ids.insert(index, dragged.to_string());
    ids
}

/// One choice of a short list: a suggestion, a time, a snippet, a sender. `checked` is the one
/// in force; `highlighted` the one the arrow keys are on.
#[derive(IntoElement)]
pub struct ChoiceRow {
    id: SharedString,
    title: SharedString,
    detail: Option<SharedString>,
    lead: Lead,
    checked: bool,
    highlighted: bool,
    on_popover: bool,
    on_click: OnClick,
}

impl ChoiceRow {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            lead: Lead::None,
            checked: false,
            highlighted: false,
            on_popover: false,
            on_click: Rc::new(on_click),
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn lead(mut self, lead: Lead) -> Self {
        self.lead = lead;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    pub fn on_popover(mut self) -> Self {
        self.on_popover = true;
        self
    }
}

impl RenderOnce for ChoiceRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (hover, highlight): (Hsla, Hsla) = if self.on_popover {
            (cx.color(POPOVER_ACCENT), cx.color(POPOVER_ACCENT_STRONGER))
        } else {
            (cx.color(BACKGROUND_ACCENT_LARGER), cx.color(BACKGROUND_ACCENT_LARGER_STRONGER))
        };
        let lead = match &self.lead {
            Lead::None => None,
            Lead::Symbol(symbol) => {
                Some(icon(*symbol, 14.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)).into_any_element())
            }
            Lead::Dot(color) => Some(
                div()
                    .w(px(14.))
                    .flex()
                    .justify_center()
                    .child(div().size(px(8.)).rounded_full().bg(cx.color(*color)))
                    .into_any_element(),
            ),
            Lead::Avatar(address) => {
                Some(Avatar::new(&crate::models::initials(address), &address.email, 24.).into_any_element())
            }
        };
        let on_click = self.on_click.clone();
        div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(px(space::S + 2.))
            .min_h(px(size::ROW))
            .px(px(space::S + 2.))
            .rounded(px(radius::SM))
            .cursor_pointer()
            .when(self.highlighted, |row| row.bg(highlight))
            .when(!self.highlighted, |row| row.hover(move |row| row.bg(hover)))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .children(lead)
            .child(line(self.title.clone(), TextStyle::Body, cx).flex_1())
            .when_some(self.detail.clone(), |row, detail| {
                row.child(line(detail, TextStyle::Caption, cx).flex_shrink(1.).max_w(px(220.)))
            })
            .when(self.checked, |row| row.child(icon(Symbol::Check, 14.).text_color(cx.color(FOREGROUND))))
    }
}
/// Tabs on a line with counts, the chosen one underlined: a split inbox's.
pub fn tab_strip(tabs: Vec<(SharedString, SharedString, u32, bool, OnClick)>, cx: &App) -> impl IntoElement + use<> {
    div().flex().items_center().children(tabs.into_iter().map(|(id, title, count, chosen, on_click)| {
        let color = if chosen { FOREGROUND } else { MUTED_FOREGROUND };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(space::XS))
            .h(px(size::ROW))
            .px(px(space::S + 1.))
            .border_b_2()
            .border_color(if chosen { cx.color(PRIMARY) } else { gpui::transparent_black() })
            .text_in(TextStyle::Label, color, cx)
            .cursor_pointer()
            .hover(|tab| tab.text_color(cx.color(FOREGROUND)))
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .child(div().whitespace_nowrap().child(title))
            .when(count > 0, |tab| tab.child(line(count.to_string(), TextStyle::Footnote, cx)))
    }))
}

/// A tab of the pill tabs over the list: an icon, a name and a count, the chosen one filled.
pub struct PillTab {
    pub id: SharedString,
    pub symbol: Symbol,
    pub title: SharedString,
    pub count: u32,
    pub chosen: bool,
    pub on_click: OnClick,
    pub menu: RowMenu,
}

/// Tabs on a line, dragged to reorder, each with a menu on a right-click.
pub fn pill_tabs(
    tabs: Vec<PillTab>,
    moved: Rc<dyn Fn(&SharedString, &SharedString, bool, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement + use<> {
    div().flex().items_center().children(tabs.into_iter().map(|tab| {
        let (color, fill) =
            if tab.chosen { (FOREGROUND, Some(BACKGROUND_ACCENT_STRONGER)) } else { (MUTED_FOREGROUND, None) };
        let count_color = if tab.chosen { MUTED_FOREGROUND } else { MUTED_STRONGER_FOREGROUND };
        let group = SharedString::from(format!("{}-hit", tab.id));
        let on_click = tab.on_click.clone();
        let target = tab.id.clone();
        let moved = moved.clone();
        let dragged = Dragged { id: tab.id.clone(), title: tab.title.clone() };
        let dropped: Dropped = Rc::new(move |dragged, after, window, cx| moved(dragged, &target, after, window, cx));
        let face = div()
            .flex()
            .items_center()
            .gap(px(space::XS))
            .h(px(size::REGULAR))
            .px(px(space::M))
            .rounded(px(radius::MD))
            .text_in(TextStyle::Label, color, cx)
            .when_some(fill, |face, fill| face.bg(cx.color(fill)))
            .when(!tab.chosen, |face| {
                face.group_hover(group.clone(), |face| {
                    face.bg(cx.color(BACKGROUND_ACCENT)).text_color(cx.color(FOREGROUND))
                })
            })
            .child(icon(tab.symbol, ControlSize::Regular.icon() - 1.))
            .child(div().whitespace_nowrap().child(tab.title.clone()))
            .when(tab.count > 0, |face| {
                face.child(line_in(tab.count.to_string(), TextStyle::Footnote, count_color, cx))
            });
        let tab_element = div()
            .id(tab.id.clone())
            .group(group)
            .px(px(space::XXS / 2.))
            .cursor_pointer()
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .on_mouse_down(MouseButton::Right, right_click(tab.id.clone(), tab.menu.clone()))
            .child(face);
        movable(tab_element, "tabs".into(), dragged, true, dropped, cx)
    }))
}

#[cfg(test)]
mod tests {
    use super::reordered;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn puts_a_dropped_row_on_the_side_of_the_row_it_fell_on() {
        let list = ids(&["a", "b", "c", "d"]);
        assert_eq!(reordered(list.clone(), "a", "b", true), ids(&["b", "a", "c", "d"]));
        assert_eq!(reordered(list.clone(), "a", "d", true), ids(&["b", "c", "d", "a"]));
        assert_eq!(reordered(list.clone(), "d", "a", false), ids(&["d", "a", "b", "c"]));
        assert_eq!(reordered(list.clone(), "c", "b", false), ids(&["a", "c", "b", "d"]));
        assert_eq!(reordered(list.clone(), "b", "b", true), list);
    }
}
