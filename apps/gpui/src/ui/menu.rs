//! Menus: the few choices a button or a right-click offers, on a popover under the button or at
//! the pointer. One is open at a time; a click elsewhere or Esc closes it, the arrows and Enter
//! pick from it.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    Anchor, AnyElement, App, Global, MouseButton, Pixels, Point, SharedString, Window, anchored, deferred, div, point,
    px,
};

use super::{Symbol, icon};
use crate::theme::*;

pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub enum MenuEntry {
    Item {
        title: SharedString,
        symbol: Option<Symbol>,
        destructive: bool,
        checked: bool,
        disabled: bool,
        action: MenuAction,
    },
    Heading(SharedString),
    Separator,
}

impl MenuEntry {
    pub fn item(title: impl Into<SharedString>, action: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self::Item {
            title: title.into(),
            symbol: None,
            destructive: false,
            checked: false,
            disabled: false,
            action: Rc::new(action),
        }
    }

    pub fn symbol(mut self, new: Symbol) -> Self {
        if let Self::Item { symbol, .. } = &mut self {
            *symbol = Some(new);
        }
        self
    }

    pub fn destructive(mut self) -> Self {
        if let Self::Item { destructive, .. } = &mut self {
            *destructive = true;
        }
        self
    }

    pub fn checked(mut self, on: bool) -> Self {
        if let Self::Item { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }

    pub fn disabled(mut self, off: bool) -> Self {
        if let Self::Item { disabled, .. } = &mut self {
            *disabled = off;
        }
        self
    }

    fn pickable(&self) -> bool {
        matches!(self, Self::Item { disabled: false, .. })
    }
}

/// The open menu, which the control that opened it draws.
pub struct OpenMenu {
    pub id: SharedString,
    pub entries: Vec<MenuEntry>,
    /// Where a right-click opened it; a button's opens under the button.
    pub at: Option<Point<Pixels>>,
    pub highlighted: Option<usize>,
}

#[derive(Default)]
pub struct Menus {
    pub open: Option<OpenMenu>,
}

impl Global for Menus {}

pub fn open_menu(
    id: impl Into<SharedString>,
    entries: Vec<MenuEntry>,
    at: Option<Point<Pixels>>,
    window: &mut Window,
    cx: &mut App,
) {
    if entries.is_empty() {
        return;
    }
    cx.set_global(Menus { open: Some(OpenMenu { id: id.into(), entries, at, highlighted: None }) });
    window.refresh();
}

pub fn close_menu(window: &mut Window, cx: &mut App) {
    if cx.try_global::<Menus>().is_some_and(|menus| menus.open.is_some()) {
        cx.set_global(Menus::default());
        window.refresh();
    }
}

pub fn menu_open(cx: &App) -> bool {
    cx.try_global::<Menus>().is_some_and(|menus| menus.open.is_some())
}
/// The keys of an open menu: the arrows move, Enter picks, Esc closes. True when one was used.
pub fn menu_key(key: &str, window: &mut Window, cx: &mut App) -> bool {
    let Some(open) = cx.try_global::<Menus>().and_then(|menus| menus.open.as_ref()) else { return false };
    let pickable: Vec<usize> =
        open.entries.iter().enumerate().filter(|(_, entry)| entry.pickable()).map(|(index, _)| index).collect();
    let position = open.highlighted.and_then(|highlighted| pickable.iter().position(|index| *index == highlighted));
    let step = |by: isize| {
        let count = pickable.len() as isize;
        let next = match position {
            Some(at) => (at as isize + by).rem_euclid(count),
            None if by > 0 => 0,
            None => count - 1,
        };
        pickable.get(next as usize).copied()
    };
    match key {
        "escape" => close_menu(window, cx),
        "down" | "up" => {
            let next = step(if key == "down" { 1 } else { -1 });
            if let Some(open) = cx.global_mut::<Menus>().open.as_mut() {
                open.highlighted = next;
            }
            window.refresh();
        }
        "enter" | "space" => {
            let action = open.highlighted.and_then(|index| match &open.entries[index] {
                MenuEntry::Item { action, disabled: false, .. } => Some(action.clone()),
                _ => None,
            });
            close_menu(window, cx);
            if let Some(action) = action {
                action(window, cx);
            }
        }
        _ => return false,
    }
    true
}

/// The open menu drawn by the control `id`, under it, when it is that control's.
pub fn menu_under(id: &str, cx: &App) -> Option<AnyElement> {
    let open = cx.try_global::<Menus>()?.open.as_ref()?;
    if open.id.as_ref() != id || open.at.is_some() {
        return None;
    }
    Some(
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(space::S))
                .child(div().mt(px(space::XXS)).child(menu_card(open, cx))),
        )
        .with_priority(10)
        .into_any_element(),
    )
}

/// The open menu at the pointer, when a right-click opened it.
pub fn menu_at_pointer(cx: &App) -> Option<AnyElement> {
    let open = cx.try_global::<Menus>()?.open.as_ref()?;
    let at = open.at?;
    Some(
        deferred(
            anchored()
                .position(at + point(px(2.), px(2.)))
                .snap_to_window_with_margin(px(space::S))
                .child(menu_card(open, cx)),
        )
        .with_priority(10)
        .into_any_element(),
    )
}

fn menu_card(open: &OpenMenu, cx: &App) -> impl IntoElement + use<> {
    let rows = open.entries.iter().enumerate().map(|(index, entry)| match entry {
        MenuEntry::Separator => {
            div().my(px(space::XXS)).h(px(size::HAIRLINE)).bg(cx.color(BORDER_POPOVER)).into_any_element()
        }
        MenuEntry::Heading(title) => div()
            .px(px(space::S + 2.))
            .pt(px(space::S))
            .pb(px(space::XXS))
            .text(TextStyle::Footnote, cx)
            .child(title.clone())
            .into_any_element(),
        MenuEntry::Item { title, symbol, destructive, checked, disabled, action } => {
            let color = if *destructive { DESTRUCTIVE } else { FOREGROUND };
            let highlighted = open.highlighted == Some(index);
            let action = action.clone();
            let disabled = *disabled;
            div()
                .id(("menu-item", index))
                .flex()
                .items_center()
                .gap(px(space::S + 2.))
                .h(px(size::SMALL + 2.))
                .px(px(space::S + 2.))
                .rounded(px(radius::SM))
                .text(TextStyle::Label, cx)
                .text_color(cx.color(color))
                .when(highlighted, |row| row.bg(cx.color(POPOVER_ACCENT_STRONGER)))
                .when(!disabled, |row| row.hover(|row| row.bg(cx.color(POPOVER_ACCENT))))
                .when(disabled, |row| row.opacity(cx.number(opacity::DISABLED)))
                .child(match symbol {
                    Some(symbol) => icon(*symbol, 15.).into_any_element(),
                    None => div().into_any_element(),
                })
                .child(div().flex_1().whitespace_nowrap().child(title.clone()))
                .when(*checked, |row| row.child(icon(Symbol::Check, 14.)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, cx| {
                    if disabled {
                        return;
                    }
                    close_menu(window, cx);
                    action(window, cx);
                })
                .into_any_element()
        }
    });
    div()
        .id("menu")
        .occlude()
        .min_w(px(200.))
        .max_w(px(size::POPUP_WIDTH))
        .p(px(space::XS))
        .rounded(px(radius::MD))
        .bg(cx.color(POPOVER))
        .border_1()
        .border_color(cx.color(BORDER_POPOVER))
        .shadow(cx.shadow(ShadowSize::Md, opacity::SHADOW_STRONGER))
        .on_mouse_down_out(|_, window, cx| close_menu(window, cx))
        .children(rows)
}
