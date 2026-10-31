//! What the window shows when the mail store can't open: what went wrong, and Quit.

use gpui::prelude::*;
use gpui::{App, Context, SharedString, Window, WindowBounds, WindowOptions, div, px, size};

use crate::theme::*;
use crate::ui::{self, ActionButton, Symbol, Variant};

struct StartFailure {
    error: SharedString,
}

impl Render for StartFailure {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(space::L))
            .p(px(space::XXL))
            .bg(cx.color(BACKGROUND))
            .font_family(FONT)
            .child(ui::icon(Symbol::CircleAlert, 30.).text_color(cx.color(DESTRUCTIVE)))
            .child(div().text(TextStyle::Heading, cx).child("The mail store couldn't open."))
            .child(ui::paragraph_in(self.error.clone(), TextStyle::Caption, MUTED_FOREGROUND, cx))
            .child(ActionButton::new("quit", "Quit").variant(Variant::Primary).on_click(|_, _, cx| cx.quit()))
    }
}

/// Shows why the app can't start, in a window of its own.
pub fn show_start_failure(error: String) {
    gpui_platform::application().with_assets(crate::assets::Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        crate::assets::load_fonts(cx);
        cx.set_global(Look::default());
        let bounds = gpui::Bounds::centered(None, size(px(520.), px(320.)), cx);
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        let opened = cx.open_window(options, |window, cx| {
            let view = cx.new(|_| StartFailure { error: error.into() });
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        });
        if opened.is_err() {
            cx.quit();
        }
        cx.activate(true);
    });
}
