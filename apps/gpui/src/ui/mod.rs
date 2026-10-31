//! Every control the app is drawn with, as `packages/apple/DESIGN.md` describes them. A view
//! composes these and styles nothing by hand; what is missing is added here.

mod buttons;
mod fields;
mod icon;
mod mail;
mod menu;
mod rows;
mod settings;
mod surfaces;
mod text;

use std::rc::Rc;

use gpui::{
    AnyView, App, AppContext as _, ClickEvent, Context, Div, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

pub use buttons::*;
pub use fields::*;
pub use icon::*;
pub use mail::*;
pub use menu::*;
pub use rows::*;
pub use settings::*;
pub use surfaces::*;
pub use text::*;

use crate::theme::*;

/// What a click does.
pub type OnClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
/// The words over a control the pointer rests on.
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    pub fn view(text: impl Into<SharedString>, cx: &mut App) -> AnyView {
        let text = text.into();
        cx.new(|_| Tooltip { text }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().pl(px(space::S)).pt(px(space::XS)).child(
            div()
                .px(px(space::S))
                .py(px(space::XXS))
                .rounded(px(radius::SM))
                .bg(cx.color(POPOVER))
                .border_1()
                .border_color(cx.color(BORDER_POPOVER))
                .shadow(cx.shadow(ShadowSize::Sm, opacity::SHADOW))
                .text(TextStyle::Caption, cx)
                .text_color(cx.color(FOREGROUND))
                .child(self.text.clone()),
        )
    }
}

/// A hairline across.
pub fn rule(color: ThemeColor, cx: &App) -> Div {
    div().flex_none().h(px(size::HAIRLINE)).w_full().bg(cx.color(color))
}
/// Text in a style of the scale on one line, cut with an ellipsis.
pub fn line(text: impl Into<SharedString>, style: TextStyle, cx: &App) -> Div {
    div().text(style, cx).truncate().min_w_0().child(text.into())
}

/// Text in a style of the scale and a colour, on one line.
pub fn line_in(text: impl Into<SharedString>, style: TextStyle, color: ThemeColor, cx: &App) -> Div {
    div().text_in(style, color, cx).truncate().min_w_0().child(text.into())
}

/// Text in a style of the scale that wraps.
pub fn paragraph(text: impl Into<SharedString>, style: TextStyle, cx: &App) -> Div {
    div().text(style, cx).child(text.into())
}

pub fn paragraph_in(text: impl Into<SharedString>, style: TextStyle, color: ThemeColor, cx: &App) -> Div {
    div().text_in(style, color, cx).child(text.into())
}

/// A file's size as people read it: "12 KB".
pub fn file_size(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    match bytes {
        b if b < 1000.0 => format!("{b} bytes"),
        b if b < 1_000_000.0 => format!("{:.0} KB", b / 1000.0),
        b if b < 1_000_000_000.0 => format!("{:.1} MB", b / 1_000_000.0),
        b => format!("{:.1} GB", b / 1_000_000_000.0),
    }
}

/// A time as a toast says it: "Oct 31, 2026 at 9:00 AM".
pub fn format_time(milliseconds: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_millis_opt(milliseconds) {
        chrono::LocalResult::Single(time) => time.format("%b %-d, %Y at %-I:%M %p").to_string(),
        _ => String::new(),
    }
}
