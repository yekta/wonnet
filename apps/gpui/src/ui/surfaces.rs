//! What things rest on: the bar across a page, a sheet over the window, cards, popovers, the
//! banner and the toast at the bottom, notices and the empty state.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Div, MouseButton, SharedString, Window, div, px};

use super::{ActionButton, IconButton, Symbol, Variant, icon, line, paragraph_in, rule, spinner};
use crate::theme::*;

/// How big a sheet is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetSize {
    /// A short list: labels, times, a person.
    Small,
    /// A list to scroll: the palette.
    Medium,
    /// A page.
    Large,
}

impl SheetSize {
    pub fn frame(self) -> (f32, f32) {
        match self {
            Self::Small => (400., 420.),
            Self::Medium => (560., 460.),
            Self::Large => (560., 640.),
        }
    }
}

/// A bar across the top of a page or a sheet: what leads, the title or `center` in the middle,
/// and what trails.
pub fn header_bar(
    title: Option<SharedString>,
    leading: Vec<AnyElement>,
    center: Option<AnyElement>,
    trailing: Vec<AnyElement>,
    cx: &App,
) -> Div {
    let side = |children: Vec<AnyElement>| div().flex().items_center().gap(px(space::S + 2.)).children(children);
    let bar = div().relative().flex().items_center().h(px(size::BAR)).px(px(space::L)).gap(px(space::S + 2.));
    let bar = match center {
        Some(center) => bar
            .child(side(leading).flex_1().min_w_0())
            .child(div().flex_none().child(center))
            .child(side(trailing).flex_1().min_w_0().justify_end()),
        None => bar.child(side(leading).min_w_0()).child(div().flex_1()).child(side(trailing)),
    };
    match title {
        Some(title) => bar.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(line(title, TextStyle::Subheading, cx).max_w(px(320.))),
        ),
        None => bar,
    }
}

/// Every sheet's shell over the window: its header with the close, its content, at its size.
/// `background` is the page's colour unless it is a card's.
pub fn sheet<E: IntoElement, F: Fn(&mut Window, &mut App) + 'static>(
    id: &str,
    title: impl Into<SharedString>,
    size: SheetSize,
    background: ThemeColor,
    trailing: Vec<AnyElement>,
    content: E,
    close: F,
    cx: &App,
) -> AnyElement {
    let (width, height) = size.frame();
    let rule_color = if background == BACKGROUND { BORDER } else { BORDER_CARD };
    let close: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(close);
    super::ListKeyTargets::sheet(close.clone(), cx);
    let close_button = IconButton::new(SharedString::from(format!("{id}-close")), Symbol::X, "Close (Esc)")
        .plain()
        .on_click(move |_, window, cx| close(window, cx));
    div()
        .id(SharedString::from(id.to_string()))
        .occlude()
        .w(px(width))
        .h(px(height))
        .max_h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::LG))
        .bg(cx.color(background))
        .border_1()
        .border_color(cx.color(BORDER_POPOVER))
        .shadow(cx.shadow(ShadowSize::Xl, opacity::SHADOW_STRONGER))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(header_bar(Some(title.into()), vec![close_button.into_any_element()], None, trailing, cx).flex_none())
        .child(rule(rule_color, cx))
        .child(div().flex_1().min_h_0().flex().flex_col().child(content))
        .into_any_element()
}

/// The dimmed window behind a sheet; a click on it closes the sheet.
pub fn scrim(content: impl IntoElement, close: impl Fn(&mut Window, &mut App) + 'static, cx: &App) -> AnyElement {
    div()
        .id("scrim")
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .pt(px(size::BAR + space::L))
        .pb(px(space::XL))
        .bg(cx.tint(OVERLAY, opacity::OVERLAY))
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| close(window, cx))
        .child(content)
        .into_any_element()
}

/// The card that fills the page: an open thread, a message being written.
pub fn page_card(cx: &App) -> Div {
    div()
        .rounded(px(radius::SM))
        .bg(cx.color(CARD_FILL))
        .border_1()
        .border_color(cx.color(BORDER))
        .shadow(cx.shadow(ShadowSize::Lg, opacity::SHADOW))
}
/// A card floating over the page, for suggestions and short menus.
pub fn popup_card(cx: &App) -> Div {
    div()
        .rounded(px(radius::MD))
        .bg(cx.color(POPOVER))
        .border_1()
        .border_color(cx.color(BORDER_POPOVER))
        .shadow(cx.shadow(ShadowSize::Md, opacity::SHADOW_STRONGER))
}

/// A section's name, and maybe a muted line about it.
pub fn section_heading(title: impl Into<SharedString>, detail: Option<SharedString>, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XXS))
        .child(div().text(TextStyle::Subheading, cx).child(title.into()))
        .when_some(detail, |heading, detail| {
            heading.child(paragraph_in(detail, TextStyle::Caption, MUTED_FOREGROUND, cx))
        })
}

/// The tone of a notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Error,
}

/// A line about something: a hint, an error, a note that something was left out; with a button
/// at its end that does the one thing about it.
pub fn notice(
    text: impl Into<SharedString>,
    tone: Tone,
    symbol: Option<Symbol>,
    action: Option<ActionButton>,
    cx: &App,
) -> Div {
    let color = if tone == Tone::Error { DESTRUCTIVE } else { MUTED_STRONGER_FOREGROUND };
    div()
        .flex()
        .items_center()
        .gap(px(space::S))
        .w_full()
        .when_some(symbol, |row, symbol| row.child(icon(symbol, 13.).text_color(cx.color(color))))
        .child(paragraph_in(text, TextStyle::Caption, color, cx).flex_shrink(1.))
        .when_some(action, |row, action| row.child(action.variant(Variant::Ghost)))
}

/// A list with nothing in it: an icon and a calm line.
pub fn empty_state(symbol: Symbol, message: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::M))
        .child(icon(symbol, 30.).text_color(cx.color(MUTED_STRONGEST_FOREGROUND)))
        .child(div().text_in(TextStyle::Body, MUTED_STRONGER_FOREGROUND, cx).child(message.into()))
}

/// A card at the bottom that stays until what it says is done: its icon in a wash of `tint`, a
/// title over a line (or a progress bar beside it), an action and a close.
pub struct Banner {
    pub symbol: Symbol,
    pub tint: ThemeColor,
    pub title: SharedString,
    pub detail: SharedString,
    pub progress: Option<f32>,
    pub pending: bool,
    pub action: Option<ActionButton>,
    pub close: Option<IconButton>,
}

pub fn banner(banner: Banner, cx: &App) -> impl IntoElement + use<> {
    let badge = div()
        .size(px(34.))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.tint(banner.tint, opacity::COLOR_TINT))
        .text_color(cx.color(banner.tint))
        .child(if banner.pending {
            spinner(16., cx.color(banner.tint)).into_any_element()
        } else {
            icon(banner.symbol, 16.).into_any_element()
        });
    let detail = line(banner.detail.clone(), TextStyle::Caption, cx);
    let under = match banner.progress {
        Some(fraction) => div()
            .flex()
            .items_center()
            .gap(px(space::S))
            .child(div().flex_1().h(px(4.)).rounded_full().bg(cx.color(POPOVER_ACCENT)).child(
                div().h_full().rounded_full().bg(cx.color(banner.tint)).w(gpui::relative(fraction.clamp(0., 1.))),
            ))
            .child(detail.w(px(36.)).flex().justify_end()),
        None => detail,
    };
    div()
        .w(px(size::BANNER_WIDTH))
        .flex()
        .items_center()
        .gap(px(space::M))
        .pl(px(space::M))
        .pr(px(if banner.close.is_some() { space::XXS } else { space::M - size::BUTTON_GAP / 2. }))
        .py(px(space::M))
        .rounded(px(radius::LG))
        .bg(cx.color(POPOVER))
        .border_1()
        .border_color(cx.color(BORDER_POPOVER))
        .shadow(cx.shadow(ShadowSize::Lg, opacity::SHADOW))
        .occlude()
        .child(badge)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(line(banner.title.clone(), TextStyle::LabelStrong, cx))
                .child(under),
        )
        .child(
            div().flex().items_center().children(banner.action).children(banner.close.map(|close| close.on_popover())),
        )
}

/// The note at the bottom: a line and maybe an action, gone after a while.
pub fn toast(
    message: SharedString,
    action: Option<(SharedString, super::OnClick)>,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap(px(space::L))
        .px(px(space::L + 2.))
        .h(px(size::SEARCH))
        .rounded_full()
        .bg(cx.color(POPOVER))
        .border_1()
        .border_color(cx.color(BORDER))
        .shadow(cx.shadow(ShadowSize::Md, opacity::SHADOW))
        .occlude()
        .child(div().text_in(TextStyle::Label, FOREGROUND, cx).whitespace_nowrap().child(message))
        .when_some(action, |toast, (title, on_click)| {
            toast.child(
                super::PlainButton::new(
                    "toast-action",
                    div().text_in(TextStyle::LabelStrong, PRIMARY, cx).child(title),
                )
                .on_click(move |event, window, cx| on_click(event, window, cx)),
            )
        })
}

/// Cancel and Save under a form.
pub fn form_buttons(
    id: &str,
    save: &str,
    save_disabled: bool,
    on_save: super::OnClick,
    on_cancel: super::OnClick,
) -> Div {
    div()
        .flex()
        .items_center()
        .child(
            ActionButton::new(SharedString::from(format!("{id}-cancel")), "Cancel")
                .variant(Variant::Ghost)
                .on_click(move |event, window, cx| on_cancel(event, window, cx)),
        )
        .child(
            ActionButton::new(SharedString::from(format!("{id}-save")), save.to_string())
                .variant(Variant::Primary)
                .disabled(save_disabled)
                .on_click(move |event, window, cx| on_save(event, window, cx)),
        )
}
