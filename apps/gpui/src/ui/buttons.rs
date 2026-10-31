//! Buttons. Every one hovers at once, can be disabled, can carry an icon and has a pending
//! state, with the spinner where the icon goes. Half of the button gap is padded inside each
//! hit area, so faces side by side show the gap while their hit areas touch.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, ClickEvent, Hsla, MouseButton, SharedString, Window, div, px};

use super::{MenuEntry, OnClick, Symbol, Tooltip, icon, icon_in, menu_under, open_menu, spinner};
use crate::theme::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// Filled in the primary colour: the one thing a page is for.
    Primary,
    /// Filled in green: what finishes a thing done.
    Success,
    /// Newton's pill.
    #[default]
    Outline,
    /// Words alone.
    Ghost,
    /// A pill in red.
    Destructive,
}

/// What a button that opens a menu offers, built when it is clicked.
pub type MenuBuilder = Rc<dyn Fn(&mut Window, &mut App) -> Vec<MenuEntry>>;

fn click_or_menu(
    id: &SharedString,
    pending: bool,
    disabled: bool,
    on_click: Option<OnClick>,
    menu: Option<MenuBuilder>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let id = id.clone();
    move |event, window, cx| {
        cx.stop_propagation();
        if pending || disabled {
            return;
        }
        if let Some(menu) = &menu {
            let entries = menu(window, cx);
            open_menu(id.clone(), entries, None, window, cx);
            return;
        }
        if let Some(on_click) = &on_click {
            on_click(event, window, cx);
        }
    }
}

/// A button with words: `primary`, `success`, `outline`, `ghost` or `destructive`.
#[derive(IntoElement)]
pub struct ActionButton {
    id: SharedString,
    title: SharedString,
    symbol: Option<Symbol>,
    variant: Variant,
    size: ControlSize,
    pending: bool,
    wide: bool,
    icon_only: bool,
    disabled: bool,
    chevron: bool,
    tooltip: Option<SharedString>,
    on_click: Option<OnClick>,
    menu: Option<MenuBuilder>,
}

impl ActionButton {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            symbol: None,
            variant: Variant::Outline,
            size: ControlSize::Regular,
            pending: false,
            wide: false,
            icon_only: false,
            disabled: false,
            chevron: false,
            tooltip: None,
            on_click: None,
            menu: None,
        }
    }

    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }

    pub fn wide(mut self) -> Self {
        self.wide = true;
        self
    }

    pub fn icon_only(mut self, icon_only: bool) -> Self {
        self.icon_only = icon_only;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A chevron after the words: the button opens a choice.
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
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

impl RenderOnce for ActionButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let height = self.size.height();
        let (foreground, hover_foreground) = match self.variant {
            Variant::Primary => (PRIMARY_FOREGROUND, PRIMARY_FOREGROUND),
            Variant::Success => (SUCCESS_FOREGROUND, SUCCESS_FOREGROUND),
            Variant::Outline => (PRIMARY, PRIMARY),
            Variant::Ghost => (MUTED_FOREGROUND, FOREGROUND),
            Variant::Destructive => (DESTRUCTIVE, DESTRUCTIVE),
        };
        let (fill, hover_fill): (Option<Hsla>, Hsla) = match self.variant {
            Variant::Primary => (Some(cx.color(PRIMARY)), cx.tint(PRIMARY, opacity::LIT)),
            Variant::Success => (Some(cx.color(SUCCESS)), cx.tint(SUCCESS, opacity::LIT)),
            _ => (None, cx.color(BACKGROUND_ACCENT)),
        };
        let border = match self.variant {
            Variant::Outline => Some(cx.tint(PRIMARY, opacity::COLOR_BORDER)),
            Variant::Destructive => Some(cx.tint(DESTRUCTIVE, opacity::COLOR_BORDER)),
            _ => None,
        };
        let group = self.id.clone();
        let live = !self.disabled && !self.pending;
        let icon_side = self.size.icon() - 1.;
        let glyph = if self.pending {
            Some(spinner(icon_side, cx.color(foreground)).into_any_element())
        } else {
            self.symbol.map(|symbol| icon(symbol, icon_side).into_any_element())
        };
        let face = div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(space::XS))
            .h(px(height))
            .rounded_full()
            .text_with(self.size.text(), cx.color(foreground))
            .when_some(fill, |face, fill| face.bg(fill))
            .when_some(border, |face, border| face.border_1().border_color(border))
            .when(live, |face| {
                face.group_hover(group.clone(), |face| face.bg(hover_fill).text_color(cx.color(hover_foreground)))
            })
            .when(self.icon_only, |face| face.w(px(height)))
            .when(!self.icon_only, |face| face.px(px(height / 2.)))
            .when(self.wide && !self.icon_only, |face| face.w_full())
            .when_some(glyph, |face, glyph| {
                face.child(div().when(!self.icon_only, |glyph| glyph.ml(px(-3.))).child(glyph))
            })
            .when(!self.icon_only, |face| face.child(div().whitespace_nowrap().child(self.title.clone())))
            .when(self.chevron, |face| face.child(div().mr(px(-3.)).child(icon(Symbol::ChevronDown, icon_side))));
        let tooltip = self.tooltip.clone().or_else(|| self.icon_only.then(|| self.title.clone()));
        let menu = menu_under(&self.id, cx);
        div()
            .id(self.id.clone())
            .group(group)
            .flex()
            .flex_none()
            .px(px(size::BUTTON_GAP / 2.))
            .when(self.wide, |hit| hit.w_full().flex_1())
            .when(self.disabled, |hit| hit.opacity(cx.number(opacity::DISABLED)))
            .when(live, |hit| hit.cursor_pointer())
            .when_some(tooltip, |hit, text| hit.tooltip(move |_, cx| Tooltip::view(text.clone(), cx)))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(click_or_menu(&self.id, self.pending, self.disabled, self.on_click, self.menu))
            .child(face)
            .children(menu)
    }
}

/// An icon in a thin circle, as Newton's toolbars drew them.
#[derive(IntoElement)]
pub struct IconButton {
    id: SharedString,
    symbol: Symbol,
    help: SharedString,
    size: ControlSize,
    active: bool,
    circled: bool,
    quiet: bool,
    strong: bool,
    pending: bool,
    disabled: bool,
    on_card: bool,
    on_popover: bool,
    on_click: Option<OnClick>,
    menu: Option<MenuBuilder>,
}

impl IconButton {
    pub fn new(id: impl Into<SharedString>, symbol: Symbol, help: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            symbol,
            help: help.into(),
            size: ControlSize::Regular,
            active: false,
            circled: true,
            quiet: false,
            strong: false,
            pending: false,
            disabled: false,
            on_card: false,
            on_popover: false,
            on_click: None,
            menu: None,
        }
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Lit in the primary colour: a reminder set, a filter on.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn plain(mut self) -> Self {
        self.circled = false;
        self
    }

    /// Fainter, for a control that should not call for attention.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    /// In the foreground's colour, for an icon on a bar.
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }

    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// On the card that fills the page, whose hairlines are its own.
    pub fn on_card(mut self) -> Self {
        self.on_card = true;
        self
    }

    pub fn on_popover(mut self) -> Self {
        self.on_popover = true;
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

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let side = self.size.height();
        let (tint, hover_tint) = if self.active {
            (PRIMARY, PRIMARY)
        } else if self.quiet {
            (MUTED_STRONGER_FOREGROUND, MUTED_FOREGROUND)
        } else if self.strong {
            (FOREGROUND, FOREGROUND)
        } else {
            (MUTED_FOREGROUND, FOREGROUND)
        };
        let hover_fill = if self.on_popover { POPOVER_ACCENT_STRONGER } else { BACKGROUND_ACCENT };
        let ring = if self.on_card { CARD_RULE } else { BORDER };
        let group = self.id.clone();
        let live = !self.disabled && !self.pending;
        let glyph = if self.pending {
            spinner(self.size.icon(), cx.color(tint)).into_any_element()
        } else {
            icon(self.symbol, self.size.icon()).into_any_element()
        };
        let menu = menu_under(&self.id, cx);
        let open = menu.is_some();
        let face = div()
            .size(px(side))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .text_color(cx.color(tint))
            .when(self.circled, |face| face.border_1().border_color(cx.color(ring)))
            .when(open, |face| face.bg(cx.color(hover_fill)).text_color(cx.color(hover_tint)))
            .when(live, |face| {
                face.group_hover(group.clone(), |face| face.bg(cx.color(hover_fill)).text_color(cx.color(hover_tint)))
            })
            .child(glyph);
        let help = self.help.clone();
        div()
            .id(self.id.clone())
            .group(group)
            .flex_none()
            .px(px(size::BUTTON_GAP / 2.))
            .when(self.disabled, |hit| hit.opacity(cx.number(opacity::DISABLED)))
            .when(live, |hit| hit.cursor_pointer())
            .tooltip(move |_, cx| Tooltip::view(help.clone(), cx))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(click_or_menu(&self.id, self.pending, self.disabled, self.on_click, self.menu))
            .child(face)
            .children(menu)
    }
}

/// A button that is only its content: a star, a sender's name, a word in a toast. `tint` is the
/// colour it hovers in, as a wash of it.
#[derive(IntoElement)]
pub struct PlainButton {
    id: SharedString,
    help: Option<SharedString>,
    tint: Option<ThemeColor>,
    content: AnyElement,
    on_click: Option<OnClick>,
}

impl PlainButton {
    pub fn new(id: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self { id: id.into(), help: None, tint: None, content: content.into_any_element(), on_click: None }
    }

    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn tint(mut self, tint: ThemeColor) -> Self {
        self.tint = Some(tint);
        self
    }
    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PlainButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let fill = match self.tint {
            Some(tint) => cx.tint(tint, opacity::COLOR_TINT),
            None => cx.color(BACKGROUND_ACCENT),
        };
        let on_click = self.on_click.clone();
        div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(size::SMALL))
            .min_h(px(size::SMALL))
            .px(px(space::XXS))
            .rounded(px(radius::SM))
            .cursor_pointer()
            .hover(move |face| face.bg(fill))
            .when_some(self.help.clone(), |face, help| face.tooltip(move |_, cx| Tooltip::view(help.clone(), cx)))
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                if let Some(on_click) = &on_click {
                    on_click(event, window, cx);
                }
            })
            .child(self.content)
    }
}

/// A thread's star: in the star colour when starred, and on hover over a wash of it.
#[derive(IntoElement)]
pub struct StarButton {
    id: SharedString,
    starred: bool,
    side: f32,
    on_click: OnClick,
}

impl StarButton {
    pub fn new(
        id: impl Into<SharedString>,
        starred: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), starred, side: 20., on_click: Rc::new(on_click) }
    }
}

impl RenderOnce for StarButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = self.id.clone();
        let symbol = if self.starred { Symbol::StarFilled } else { Symbol::Star };
        let rest = if self.starred { STAR } else { MUTED_STRONGER_FOREGROUND };
        let on_click = self.on_click.clone();
        let glyph = div()
            .text_color(cx.color(rest))
            .group_hover(group.clone(), |glyph| glyph.text_color(cx.color(STAR)))
            .child(if symbol == Symbol::StarFilled {
                icon_in(symbol, self.side, cx.color(STAR))
            } else {
                icon(symbol, self.side)
            });
        div()
            .id(self.id.clone())
            .group(group)
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(size::SMALL))
            .min_h(px(size::SMALL))
            .px(px(space::XXS))
            .rounded(px(radius::SM))
            .cursor_pointer()
            .hover(|face| face.bg(cx.tint(STAR, opacity::COLOR_TINT)))
            .tooltip(move |_, cx| Tooltip::view(if self.starred { "Unstar" } else { "Star" }, cx))
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_click(event, window, cx);
            })
            .child(glyph)
    }
}

/// An icon on a bordered circle of the page's colour over a page: back to the top of a list.
#[derive(IntoElement)]
pub struct FloatingButton {
    id: SharedString,
    symbol: Symbol,
    help: SharedString,
    small: bool,
    on_click: OnClick,
}

impl FloatingButton {
    pub fn new(
        id: impl Into<SharedString>,
        symbol: Symbol,
        help: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), symbol, help: help.into(), small: false, on_click: Rc::new(on_click) }
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
}

impl RenderOnce for FloatingButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let side = if self.small { 40. } else { 56. };
        let on_click = self.on_click.clone();
        let help = self.help.clone();
        div()
            .id(self.id.clone())
            .size(px(side))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(cx.color(BACKGROUND))
            .border_1()
            .border_color(cx.color(BORDER))
            .shadow(cx.shadow(ShadowSize::Sm, opacity::SHADOW))
            .text_color(cx.color(FOREGROUND))
            .cursor_pointer()
            .hover(|face| face.bg(cx.color(BACKGROUND_ACCENT)))
            .tooltip(move |_, cx| Tooltip::view(help.clone(), cx))
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .child(icon(self.symbol, if self.small { ControlSize::Large.icon() } else { 22. }))
    }
}

/// The circle that selects threads: empty, or filled in the primary colour with a check.
#[derive(IntoElement)]
pub struct SelectCircle {
    id: SharedString,
    checked: bool,
    help: SharedString,
    on_click: OnClick,
}

impl SelectCircle {
    pub fn new(
        id: impl Into<SharedString>,
        checked: bool,
        help: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), checked, help: help.into(), on_click: Rc::new(on_click) }
    }
}

impl RenderOnce for SelectCircle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = self.id.clone();
        let help = self.help.clone();
        let on_click = self.on_click.clone();
        div()
            .id(self.id.clone())
            .group(group.clone())
            .flex_none()
            .px(px(size::BUTTON_GAP / 2.))
            .cursor_pointer()
            .tooltip(move |_, cx| Tooltip::view(help.clone(), cx))
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .child(div().size(px(size::REGULAR)).flex().items_center().justify_center().child(select_circle(
                self.checked,
                Some(group),
                cx,
            )))
    }
}

/// The circle itself: filled with a check when selected, else a ring that darkens on hover.
pub fn select_circle(checked: bool, hover_group: Option<SharedString>, cx: &App) -> impl IntoElement + use<> {
    let side = size::SELECT_CIRCLE;
    let circle = div().size(px(side)).flex_none().rounded_full().flex().items_center().justify_center();
    if checked {
        return circle.bg(cx.color(PRIMARY)).child(icon_in(Symbol::Check, side * 0.67, cx.color(PRIMARY_FOREGROUND)));
    }
    circle.border(px(size::SELECT_LINE)).border_color(cx.color(BORDER)).when_some(hover_group, |circle, group| {
        circle.group_hover(group, |circle| circle.border_color(cx.color(MUTED_FOREGROUND)))
    })
}
