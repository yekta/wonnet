//! Wonnet for Windows and Linux: the Mac app's window, drawn with GPUI, over the Rust core,
//! which runs in this process.

#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod assets;
mod core;
mod keys;
mod models;
mod platform;
mod store;
mod theme;
mod ui;
mod updater;
mod views;

use std::sync::Arc;

use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};
use mail_core::api::Config;

use crate::core::Core;
use crate::store::LocalSettings;
use crate::theme::Look;
use crate::views::MailWindow;

/// The server a new install uses.
const SERVER: &str = match option_env!("MAIL_SERVER_URL") {
    Some(url) => url,
    None => "https://api.wonnet.app",
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|arg| arg == "--demo");
    let links: Vec<String> = args.iter().filter(|arg| arg.starts_with("wonnet://")).cloned().collect();
    let base = platform::data_dir();
    let data = if demo { base.join("Demo") } else { base.clone() };
    if let Err(error) = std::fs::create_dir_all(&data) {
        eprintln!("The data folder {} couldn't be made: {error}", data.display());
    }
    if args.iter().any(|arg| arg == updater::AFTER_UPDATE) {
        platform::instance::wait_for_previous(&base);
    } else if !demo && platform::instance::hand_over(&base, &links) {
        return;
    }
    platform::prune_outgoing();
    let config = Config { data_dir: data.display().to_string(), server_url: Some(SERVER.into()), demo };
    let (core, events) = match Core::start(config) {
        Ok(started) => started,
        Err(error) => return views::show_start_failure(format!("{error:#}")),
    };
    let (heard, starts) = async_channel::unbounded();
    for link in links {
        let _ = heard.send_blocking(link);
    }
    if !demo {
        platform::instance::listen(&base, heard.clone());
        platform::scheme::register();
        #[cfg(target_os = "windows")]
        platform::notify::register();
    }
    let local = LocalSettings::load(data.join("settings.json"));

    application().with_assets(assets::Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        assets::load_fonts(cx);
        cx.set_global(Look::default());
        cx.set_global(ui::Menus::default());
        cx.set_global(ui::Photos::new(core.clone()));
        let options = window_options(cx);
        let opened = cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| MailWindow::new(core, events, starts, local, demo, window, cx));
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        });
        if let Err(error) = opened {
            eprintln!("The window couldn't open: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}

/// On Linux the app runs on X11 (XWayland on a Wayland desktop): WebKitGTK's pages can only be
/// laid into an X11 window.
fn application() -> gpui::Application {
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some_and(|display| !display.is_empty()) {
        return gpui_platform::linux(gpui::WindowingModes::X11);
    }
    gpui_platform::application()
}

fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Wonnet".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(18.), px(18.))),
        }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(860.), px(520.))),
        app_id: Some("wonnet".into()),
        icon: window_icon(),
        window_decorations: Some(gpui::WindowDecorations::Client),
        ..Default::default()
    }
}

fn window_icon() -> Option<Arc<image::RgbaImage>> {
    let png = include_bytes!("../assets/icon/wonnet.png");
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    Some(Arc::new(image.to_rgba8()))
}
