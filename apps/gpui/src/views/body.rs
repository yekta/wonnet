//! A message's body: the core's page in the system's web view (WebView2 on Windows, WebKitGTK
//! on Linux, WKWebView on a Mac for development), as tall as the page, its links opening in the
//! browser.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{Context, Entity, Global, Pixels, ScrollHandle, Window, div, img, px};

use crate::models::MessageItem;
use crate::store::Store;
use crate::theme::*;
use crate::ui;

/// What covers the thread now: a sheet, a menu, the sidebar (`all`), or the toast and the
/// banner at the bottom. A web view, drawn by the system over the window, keeps out of it.
#[derive(Clone, Copy, Default)]
pub struct Occlusion {
    pub all: bool,
    pub bottom: Pixels,
}

impl Global for Occlusion {}

pub struct BodyView {
    store: Entity<Store>,
    message: MessageItem,
    /// The thread's scroll, which a web view's wheel turns.
    scroll: ScrollHandle,
    web: Option<web::Page>,
    /// The web view is being made, or couldn't be.
    making: bool,
    failed: bool,
    /// A picture of the page, drawn under it: what shows while a menu or a sheet covers the
    /// page, as GPUI can't draw over the system's view.
    snapshot: Option<web::Snapshot>,
    /// The pictures asked for, the last of which is taken once the page settles.
    asked: u64,
    /// The page changed while it was off the screen: it's pictured once it's back.
    stale: bool,
}

impl BodyView {
    pub fn new(store: Entity<Store>, message: MessageItem, scroll: ScrollHandle, cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, cx| {
            let Some(snapshot) = this.snapshot.take() else { return };
            cx.drop_image(snapshot.image, None);
        })
        .detach();
        Self { store, message, scroll, web: None, making: false, failed: false, snapshot: None, asked: 0, stale: false }
    }

    /// Pictures the page once it has settled for a moment.
    fn want_snapshot(&mut self, cx: &mut Context<Self>) {
        if !web::SNAPSHOTS {
            return;
        }
        self.asked += 1;
        let asked = self.asked;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            let _ = this.update(cx, |this, _| {
                let Some(page) = &this.web else { return };
                if this.asked == asked && !page.snapshot() {
                    this.stale = true;
                }
            });
        })
        .detach();
    }

    pub fn set_message(&mut self, message: MessageItem, cx: &mut Context<Self>) {
        if message == self.message {
            return;
        }
        let changed = message.html != self.message.html;
        self.message = message;
        if changed && let Some(page) = &mut self.web {
            page.load(self.message.html.as_deref().unwrap_or_default(), cx.dark());
        }
        cx.notify();
    }

    /// Takes the page off the screen while the thread isn't drawn.
    pub fn hide(&mut self) {
        let Some(page) = &mut self.web else { return };
        page.place(gpui::Bounds::default(), None, 1.);
        page.shown = None;
    }

    /// Makes the page's web view: after the frame is drawn, as the system's views can't be
    /// added while the window draws.
    fn make_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.web.is_some() {
            return;
        }
        let (said, heard) = async_channel::unbounded();
        self.web = web::Page::new(window, self.message.html.as_deref().unwrap_or_default(), cx.dark(), said, cx);
        if self.web.is_none() {
            self.failed = true;
        }
        let scroll = self.scroll.clone();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(message) = heard.recv().await {
                let done = match message {
                    // Outside this body's update: what the key does may update it.
                    web::Said::Key(keystroke) => cx.update(|window, cx| {
                        window.dispatch_keystroke(keystroke, cx);
                    }),
                    web::Said::Height(height) => this.update(cx, |this, cx| {
                        let Some(page) = &mut this.web else { return };
                        // Said when the page is laid out, a new one too: it's scrolled again.
                        page.scrolled = None;
                        page.height = Some(height.max(20.));
                        cx.notify();
                    }),
                    web::Said::Wheel(delta) => this.update(cx, |_, cx| {
                        web::wheel(&scroll, delta);
                        cx.notify();
                    }),
                    web::Said::Done => this.update(cx, |this, _| {
                        let Some(page) = &this.web else { return };
                        page.give_back_keys();
                    }),
                    web::Said::Drawn => this.update(cx, |this, cx| this.want_snapshot(cx)),
                    web::Said::Shot(shot) => {
                        let developed = cx.background_executor().spawn(async move { shot.develop() }).await;
                        this.update_in(cx, |this, window, cx| {
                            let Some(snapshot) = developed else { return };
                            if let Some(old) = this.snapshot.replace(snapshot) {
                                cx.drop_image(old.image, Some(window));
                            }
                            cx.notify();
                        })
                    }
                };
                if done.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
}

impl Render for BodyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.web.is_none() && !self.failed && !self.making {
            self.making = true;
            cx.defer_in(window, |this, window, cx| {
                this.making = false;
                this.make_page(window, cx);
            });
        }
        let store = self.store.read(cx);
        let original = store.original_messages.contains(&self.message.id);
        let kept = store.body_height(&self.message.id, store.body_width).map(|height| height as f32);
        if self.failed {
            return ui::paragraph_in(
                "This message can't be shown here.",
                TextStyle::Body,
                MUTED_STRONGER_FOREGROUND,
                cx,
            )
            .into_any_element();
        }
        if let Some(page) = &mut self.web {
            page.wanted_original = original;
        }
        let height = self.web.as_ref().and_then(|page| page.height).or(kept);
        let entity = cx.entity();
        let message_id = self.message.id.clone();
        let placeholder = height.is_none().then(|| ui::body_placeholder(self.message.shape.clone(), cx));
        let snapshot = self.snapshot.as_ref().filter(|_| height.is_some()).map(|snapshot| {
            img(snapshot.image.clone())
                .absolute()
                .top(px(snapshot.top))
                .left_0()
                .w(px(snapshot.width))
                .h(px(snapshot.height))
        });
        div()
            .relative()
            .w_full()
            .when_some(height, |frame, height| frame.h(px(height)))
            .children(placeholder)
            .children(snapshot)
            .child(
                gpui::canvas(
                    move |bounds, window, cx| {
                        let occlusion = cx.try_global::<Occlusion>().copied().unwrap_or_default();
                        let mask = window.content_mask().bounds;
                        let mut visible = bounds.intersect(&mask);
                        let floor = window.bounds().size.height - occlusion.bottom;
                        if visible.origin.y + visible.size.height > floor {
                            visible.size.height = (floor - visible.origin.y).max(px(0.));
                        }
                        let shown = (!occlusion.all).then_some(visible);
                        let scale = window.scale_factor();
                        let width: f32 = bounds.size.width.into();
                        let cut = f32::from(visible.origin.y - bounds.origin.y);
                        let tall = f32::from(visible.size.height);
                        let entity = entity.clone();
                        let message_id = message_id.clone();
                        // The system's view moves once the frame is drawn.
                        cx.defer(move |cx| {
                            entity.update(cx, |body, cx| {
                                let Some(page) = &mut body.web else { return };
                                page.place(bounds, shown, scale);
                                page.apply_original();
                                page.rescheme(cx.dark());
                                let pictured =
                                    body.snapshot.as_ref().is_some_and(|snapshot| snapshot.covers(cut, tall, width));
                                if tall > 1. && !page.hidden && (body.stale || !pictured) {
                                    body.stale = false;
                                    body.want_snapshot(cx);
                                }
                                let Some(page) = &body.web else { return };
                                let Some(height) = page.height else { return };
                                body.store.update(cx, |store, _| {
                                    store.note_body_height(&message_id, width as f64, height as f64)
                                });
                            })
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }
}

impl Drop for BodyView {
    fn drop(&mut self) {
        if let Some(page) = &self.web {
            let _ = page.view.set_visible(false);
        }
    }
}

mod web {
    //! The system's web view, as a child of the window, laid over the body's place.

    use std::sync::Arc;

    use gpui::{App, Bounds, Keystroke, Modifiers, Pixels, RenderImage, ScrollHandle, point, px};
    use wry::dpi::{PhysicalPosition, PhysicalSize};
    use wry::{Rect, WebView, WebViewBuilder};

    #[cfg(not(target_os = "macos"))]
    use crate::theme::CARD_FILL;

    /// Whether pages can be pictured here: not on a Mac, where the app runs only for development
    /// and a covered page is hidden instead.
    pub const SNAPSHOTS: bool = !cfg!(target_os = "macos");

    /// What the page tells the app: how tall it is, the wheel turned over it, a key pressed in
    /// it (the app's shortcuts work over a page too), a click that selected nothing, after which
    /// the keys go back to the app, that it looks different now, and a picture of it.
    pub enum Said {
        Height(f32),
        Wheel(f32),
        Key(Keystroke),
        Done,
        Drawn,
        #[cfg_attr(target_os = "macos", allow(dead_code))]
        Shot(Shot),
    }

    /// A picture of the part of the page on the screen, `top` down from its top, taken at the
    /// window's `scale`.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub struct Shot {
        capture: Capture,
        top: f32,
        scale: f32,
    }

    enum Capture {
        /// Cairo's pixels: blue, green, red and alpha, the colours multiplied by the alpha.
        #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
        Bgra { width: u32, height: u32, data: Vec<u8> },
        #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
        Png(Vec<u8>),
    }

    /// The picture as GPUI draws it, and where in the page it goes.
    pub struct Snapshot {
        pub image: Arc<RenderImage>,
        pub top: f32,
        pub width: f32,
        pub height: f32,
    }

    impl Snapshot {
        /// Whether it shows all of the page from `top` down for `height`, at this width.
        pub fn covers(&self, top: f32, height: f32, width: f32) -> bool {
            (self.width - width).abs() < 1. && self.top <= top + 1. && self.top + self.height >= top + height - 1.
        }
    }

    impl Shot {
        /// Turns the picture into GPUI's, off the window's thread.
        pub fn develop(self) -> Option<Snapshot> {
            let pixels = match self.capture {
                Capture::Bgra { width, height, mut data } => {
                    for pixel in data.as_chunks_mut::<4>().0 {
                        let alpha = pixel[3] as u32;
                        if alpha == 0 || alpha == 255 {
                            continue;
                        }
                        for channel in &mut pixel[..3] {
                            *channel = (*channel as u32 * 255 / alpha).min(255) as u8;
                        }
                    }
                    image::RgbaImage::from_raw(width, height, data)?
                }
                Capture::Png(bytes) => {
                    let mut pixels =
                        image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.into_rgba8();
                    for pixel in pixels.as_chunks_mut::<4>().0 {
                        pixel.swap(0, 2);
                    }
                    pixels
                }
            };
            let (width, height) = (pixels.width() as f32 / self.scale, pixels.height() as f32 / self.scale);
            let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
            Some(Snapshot { image, top: self.top, width, height })
        }
    }

    const SCRIPT: &str = r#"
(function () {
  function fit(body) {
    body.style.width = ''; body.style.transform = '';
    var wide = document.documentElement.scrollWidth, shown = document.documentElement.clientWidth;
    if (wide <= shown + 1) { return; }
    body.style.width = wide + 'px'; body.style.transformOrigin = '0 0';
    body.style.transform = 'scale(' + shown / wide + ')';
  }
  function report() {
    var body = document.body;
    if (!body) { return; }
    fit(body);
    window.ipc.postMessage('height:' + Math.ceil(body.getBoundingClientRect().height));
  }
  function drawn() { window.ipc.postMessage('drawn'); }
  window.wonnetDrawn = drawn;
  document.addEventListener('DOMContentLoaded', function () {
    document.documentElement.style.overflow = 'hidden';
    new ResizeObserver(report).observe(document.body);
    report();
    drawn();
  });
  document.addEventListener('toggle', function () { report(); drawn(); }, true);
  document.addEventListener('load', drawn, true);
  window.addEventListener('load', function () { report(); drawn(); });
  window.addEventListener('resize', function () {
    report();
    window.scrollTo(0, window.cut || 0);
  });
  window.addEventListener('wheel', function (event) {
    event.preventDefault();
    window.ipc.postMessage('wheel:' + event.deltaY);
  }, { passive: false });
  document.addEventListener('keydown', function (event) {
    var copy = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'c';
    if (copy && String(window.getSelection())) {
      event.preventDefault();
      document.execCommand('copy');
      return;
    }
    event.preventDefault();
    window.ipc.postMessage('key:' + JSON.stringify({
      key: event.key, ctrl: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, meta: event.metaKey
    }));
  }, true);
  document.addEventListener('mouseup', function () {
    setTimeout(function () {
      if (!String(window.getSelection())) { window.ipc.postMessage('done'); }
    }, 0);
  });
})();
"#;

    #[derive(serde::Deserialize)]
    struct Pressed {
        key: String,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    }

    /// The keystroke for a key the page reports, as `KeyboardEvent` names it.
    pub fn keystroke(json: &str) -> Option<Keystroke> {
        let pressed: Pressed = serde_json::from_str(json).ok()?;
        let key = match pressed.key.as_str() {
            "Shift" | "Control" | "Alt" | "Meta" | "CapsLock" | "Dead" | "Unidentified" => return None,
            "ArrowUp" => "up".to_string(),
            "ArrowDown" => "down".to_string(),
            "ArrowLeft" => "left".to_string(),
            "ArrowRight" => "right".to_string(),
            " " => "space".to_string(),
            key => key.to_lowercase(),
        };
        let typed = pressed.key.chars().count() == 1 && !pressed.ctrl && !pressed.meta;
        Some(Keystroke {
            modifiers: Modifiers {
                control: pressed.ctrl,
                alt: pressed.alt,
                shift: pressed.shift,
                platform: pressed.meta,
                function: false,
            },
            key,
            key_char: typed.then_some(pressed.key),
        })
    }

    pub struct Page {
        pub view: WebView,
        pub height: Option<f32>,
        pub shown: Option<Bounds<Pixels>>,
        /// How far the page is scrolled, unknown since it was laid out.
        pub scrolled: Option<f32>,
        pub original: bool,
        pub wanted_original: bool,
        pub hidden: bool,
        /// The scheme the page was loaded in.
        pub dark: bool,
        /// The window's scale the page is drawn at.
        scale: f32,
        html: String,
        said: async_channel::Sender<Said>,
    }

    impl Page {
        pub fn new(
            window: &gpui::Window,
            html: &str,
            dark: bool,
            said: async_channel::Sender<Said>,
            cx: &mut App,
        ) -> Option<Self> {
            let told = said.clone();
            #[cfg(target_os = "macos")]
            let view = {
                let _ = cx;
                described(WebViewBuilder::new(), html, dark, said).with_transparent(true).build_as_child(window).ok()?
            };
            #[cfg(target_os = "windows")]
            let view = {
                let _ = cx;
                described(WebViewBuilder::new(), html, dark, said)
                    .with_background_color(background(dark))
                    .build_as_child(window)
                    .ok()?
            };
            #[cfg(target_os = "linux")]
            let view = {
                if !gtk_loop::start(cx) {
                    return None;
                }
                let parent = gtk_loop::parent(window)?;
                let view = gtk_loop::with_context(|context| {
                    described(WebViewBuilder::new_with_web_context(context), html, dark, said)
                        .with_background_color(background(dark))
                        .build_as_child(&parent)
                })
                .ok()?;
                // X11 hands a key to the window under the pointer: the page then takes it, and
                // tells the app, only if WebKit is its window's focus.
                let _ = view.focus();
                gtk_loop::opened();
                view
            };
            Some(Self {
                view,
                height: None,
                shown: None,
                scrolled: None,
                original: false,
                wanted_original: false,
                hidden: true,
                dark,
                scale: 1.,
                html: html.to_string(),
                said: told,
            })
        }

        pub fn load(&mut self, html: &str, dark: bool) {
            #[cfg(not(target_os = "macos"))]
            let _ = self.view.set_background_color(background(dark));
            let _ = self.view.load_html(&themed(html, dark));
            self.scrolled = None;
            self.dark = dark;
            self.html = html.to_string();
        }

        /// Loads the page again in the scheme the app is now in.
        pub fn rescheme(&mut self, dark: bool) {
            if dark != self.dark {
                let html = std::mem::take(&mut self.html);
                self.load(&html, dark);
            }
        }

        /// Shows the page over `visible`, the part of `bounds` on screen, scrolled so it lines
        /// up with where its top would be. `scale` is the window's, from its pixels to the
        /// screen's.
        pub fn place(&mut self, bounds: Bounds<Pixels>, visible: Option<Bounds<Pixels>>, scale: f32) {
            let Some(visible) = visible.filter(|visible| visible.size.height > px(1.) && visible.size.width > px(1.))
            else {
                if !self.hidden {
                    let _ = self.view.set_visible(false);
                    self.hidden = true;
                }
                return;
            };
            #[cfg(target_os = "linux")]
            if self.scale != scale {
                gtk_loop::zoom(&self.view, scale);
            }
            let moved = self.shown != Some(visible) || self.scale != scale;
            let physical = |value: Pixels| (f32::from(value) * scale).round() as i32;
            let size = PhysicalSize::new(physical(visible.size.width), physical(visible.size.height));
            if moved {
                let position = PhysicalPosition::new(physical(visible.origin.x), physical(visible.origin.y));
                let _ = self.view.set_bounds(Rect { position: position.into(), size: size.into() });
                self.shown = Some(visible);
            }
            self.scale = scale;
            let cut = f32::from(visible.origin.y - bounds.origin.y);
            if self.scrolled.is_none_or(|scrolled| (cut - scrolled).abs() > 0.5) {
                let _ = self.view.evaluate_script(&format!("window.cut = {cut}; window.scrollTo(0, {cut});"));
                self.scrolled = Some(cut);
            }
            let appears = self.hidden;
            if appears {
                let _ = self.view.set_visible(true);
                self.hidden = false;
            }
            #[cfg(target_os = "linux")]
            if moved || appears {
                gtk_loop::fit(&self.view, size);
                gtk_loop::turn();
            }
        }

        /// After a click in the page: the keys go to the app's window again. On X11 they
        /// follow the pointer, and the page passes on what it gets.
        pub fn give_back_keys(&self) {
            #[cfg(not(target_os = "linux"))]
            let _ = self.view.focus_parent();
        }

        /// Pictures what the page shows now, which comes back as `Said::Shot`. False while it
        /// isn't on the screen.
        pub fn snapshot(&self) -> bool {
            let (Some(top), false) = (self.scrolled, self.hidden) else { return false };
            #[cfg(target_os = "linux")]
            gtk_loop::snapshot(&self.view, top, self.scale, self.said.clone());
            #[cfg(target_os = "windows")]
            preview::snapshot(&self.view, top, self.scale, self.said.clone());
            #[cfg(target_os = "macos")]
            let _ = (top, &self.said);
            true
        }

        pub fn apply_original(&mut self) {
            let original = self.wanted_original;
            if original == self.original {
                return;
            }
            self.original = original;
            let script = format!(
                "(() => {{ const body = document.body; if (!body) return; body.classList.toggle('dark', {}); if (body.classList.contains('paper')) body.classList.toggle('light', {}); window.wonnetDrawn(); }})();",
                !original, original
            );
            let _ = self.view.evaluate_script(&script);
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for Page {
        fn drop(&mut self) {
            gtk_loop::closed();
        }
    }

    /// The page, its script, and what it may open: links go to the browser.
    fn described<'a>(
        builder: WebViewBuilder<'a>,
        html: &str,
        dark: bool,
        said: async_channel::Sender<Said>,
    ) -> WebViewBuilder<'a> {
        builder
            .with_html(themed(html, dark))
            .with_visible(false)
            .with_initialization_script(SCRIPT)
            .with_ipc_handler(move |request| {
                let body = request.body();
                let message = if let Some(height) = body.strip_prefix("height:") {
                    height.parse().ok().map(Said::Height)
                } else if let Some(delta) = body.strip_prefix("wheel:") {
                    delta.parse().ok().map(Said::Wheel)
                } else if let Some(pressed) = body.strip_prefix("key:") {
                    keystroke(pressed).map(Said::Key)
                } else {
                    match body.as_str() {
                        "done" => Some(Said::Done),
                        "drawn" => Some(Said::Drawn),
                        _ => None,
                    }
                };
                if let Some(message) = message {
                    let _ = said.try_send(message);
                }
            })
            .with_navigation_handler(|url| {
                if url.starts_with("about:") || url.starts_with("data:") {
                    return true;
                }
                let _ = open::that_detached(&url);
                false
            })
            .with_new_window_req_handler(|url, _| {
                let _ = open::that_detached(&url);
                wry::NewWindowResponse::Deny
            })
    }

    /// Turns the thread's scroll by the wheel turned over a page.
    pub fn wheel(scroll: &ScrollHandle, delta: f32) {
        let offset = scroll.offset();
        let max = scroll.max_offset().y;
        let next = (offset.y - px(delta)).min(px(0.)).max(-max);
        scroll.set_offset(point(offset.x, next));
    }

    /// The page in the app's scheme, whatever the system's: the core's CSS asks for
    /// `prefers-color-scheme`, which is answered here.
    fn themed(html: &str, dark: bool) -> String {
        let answer = if dark { "all" } else { "not all" };
        html.replace("@media (prefers-color-scheme: dark)", &format!("@media {answer}"))
    }

    /// The card's colour, under the page: it hides the picture of the page drawn under it.
    #[cfg(not(target_os = "macos"))]
    fn background(dark: bool) -> wry::RGBA {
        let color = gpui::Rgba::from(CARD_FILL.hsla(dark));
        let channel = |value: f32| (value * 255.).round() as u8;
        (channel(color.r), channel(color.g), channel(color.b), 255)
    }

    #[cfg(target_os = "windows")]
    mod preview {
        //! WebView2's picture of what a page shows.

        use webview2_com::CapturePreviewCompletedHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
        use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
        use windows::Win32::System::Com::{IStream, STREAM_SEEK_CUR, STREAM_SEEK_SET};
        use wry::{WebView, WebViewExtWindows as _};

        use super::{Capture, Said, Shot};

        /// Pictures the part of the page on the screen, which `said` carries back.
        pub fn snapshot(view: &WebView, top: f32, scale: f32, said: async_channel::Sender<Said>) {
            let Ok(core) = (unsafe { view.controller().CoreWebView2() }) else { return };
            let Ok(stream) = (unsafe { CreateStreamOnHGlobal(Default::default(), true) }) else { return };
            let written = stream.clone();
            let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                result?;
                let capture = Capture::Png(read(&written)?);
                let _ = said.try_send(Said::Shot(Shot { capture, top, scale }));
                Ok(())
            }));
            let _ = unsafe { core.CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, &stream, &handler) };
        }

        fn read(stream: &IStream) -> windows::core::Result<Vec<u8>> {
            let mut size = 0u64;
            unsafe { stream.Seek(0, STREAM_SEEK_CUR, Some(&mut size))? };
            unsafe { stream.Seek(0, STREAM_SEEK_SET, None)? };
            let mut bytes = vec![0u8; size as usize];
            let mut read = 0u32;
            unsafe { stream.Read(bytes.as_mut_ptr().cast(), bytes.len() as u32, Some(&mut read)).ok()? };
            bytes.truncate(read as usize);
            Ok(bytes)
        }
    }

    #[cfg(target_os = "linux")]
    mod gtk_loop {
        //! WebKitGTK on the app's X11 window: GTK runs on the window's thread, its loop turned
        //! from the app's, and each page is an X11 child window of the app's.

        use std::cell::{Cell, RefCell};
        use std::time::Duration;

        use gtk::prelude::*;
        use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};
        use wry::dpi::PhysicalSize;
        use wry::{WebContext, WebView, WebViewExtUnix as _};

        use super::{Capture, Said, Shot};

        thread_local! {
            static STARTED: Cell<Option<bool>> = const { Cell::new(None) };
            /// The pages open, while which GTK's loop turns at every frame.
            static PAGES: Cell<usize> = const { Cell::new(0) };
            /// One WebKit for every page, so that they share its network process.
            static CONTEXT: RefCell<WebContext> = RefCell::new(WebContext::new(None));
        }

        pub fn with_context<R>(make: impl FnOnce(&mut WebContext) -> R) -> R {
            CONTEXT.with_borrow_mut(make)
        }

        /// Starts GTK once, on X11 as the app's window is. False when it can't start.
        pub fn start(cx: &mut gpui::App) -> bool {
            if let Some(started) = STARTED.get() {
                return started;
            }
            gtk::gdk::set_allowed_backends("x11");
            let started = gtk::init().is_ok();
            STARTED.set(Some(started));
            if started {
                cx.spawn(async move |cx| {
                    loop {
                        turn();
                        let wait = if PAGES.get() > 0 { 16 } else { 500 };
                        cx.background_executor().timer(Duration::from_millis(wait)).await;
                    }
                })
                .detach();
            }
            started
        }

        /// Runs what GTK has waiting: a page moved now moves on the screen now.
        pub fn turn() {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }

        pub fn opened() {
            PAGES.set(PAGES.get() + 1);
        }

        pub fn closed() {
            PAGES.set(PAGES.get().saturating_sub(1));
        }

        /// The app's window as Xlib names it, which wry asks for: GPUI hands out its XCB id.
        pub struct Parent(std::ffi::c_ulong);

        pub fn parent(window: &gpui::Window) -> Option<Parent> {
            match HasWindowHandle::window_handle(window).ok()?.as_raw() {
                RawWindowHandle::Xcb(handle) => Some(Parent(handle.window.get().into())),
                RawWindowHandle::Xlib(handle) => Some(Parent(handle.window)),
                _ => None,
            }
        }

        impl HasWindowHandle for Parent {
            fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
                let raw = RawWindowHandle::Xlib(XlibWindowHandle::new(self.0));
                // SAFETY: the window outlives its pages, which the window's views own.
                Ok(unsafe { WindowHandle::borrow_raw(raw) })
            }
        }

        /// Pictures the part of the page on the screen, which `said` carries back.
        pub fn snapshot(view: &WebView, top: f32, scale: f32, said: async_channel::Sender<Said>) {
            use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt as _};
            let region = SnapshotRegion::Visible;
            view.webview().snapshot(region, SnapshotOptions::NONE, None::<&gtk::gio::Cancellable>, move |result| {
                let Ok(surface) = result else { return };
                let Ok(image) = gtk::cairo::ImageSurface::try_from(surface) else { return };
                let Some(capture) = pixels(&image) else { return };
                let _ = said.try_send(Said::Shot(Shot { capture, top, scale }));
            });
        }

        fn pixels(image: &gtk::cairo::ImageSurface) -> Option<Capture> {
            let (width, height) = (image.width() as usize, image.height() as usize);
            let stride = image.stride() as usize;
            let mut data = Vec::with_capacity(width * height * 4);
            image
                .with_data(|bytes| {
                    for row in bytes.chunks(stride).take(height) {
                        data.extend_from_slice(&row[..width * 4]);
                    }
                })
                .ok()?;
            if image.format() == gtk::cairo::Format::Rgb24 {
                data.as_chunks_mut::<4>().0.iter_mut().for_each(|pixel| pixel[3] = 255);
            }
            Some(Capture::Bgra { width: width as u32, height: height as u32, data })
        }

        /// Draws the page at the window's scale: GTK knows only whole scales, often none.
        pub fn zoom(view: &WebView, scale: f32) {
            let gtk_scale = view.webview().scale_factor().max(1) as f64;
            let _ = view.zoom(scale as f64 / gtk_scale);
        }

        /// Holds the page's GTK window and WebKit to their place's size: GTK keeps its own idea
        /// of the size of a window it didn't make, and lays a page out at that when it shows.
        pub fn fit(view: &WebView, size: PhysicalSize<i32>) {
            let widget = view.webview();
            let gtk_scale = widget.scale_factor().max(1);
            let (width, height) = ((size.width / gtk_scale).max(1), (size.height / gtk_scale).max(1));
            if let Some(window) = widget.toplevel().and_then(|top| top.downcast::<gtk::Window>().ok()) {
                window.resize(width, height);
            }
            widget.size_allocate(&gtk::Allocation::new(0, 0, width, height));
        }
    }
}
