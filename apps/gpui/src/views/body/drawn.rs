//! A message's body on Linux: WPE WebKit paints the page off the screen and the body draws it
//! like a picture, so what the app shows over it shows, and a change redraws only its band.
//! WebKit holds the part of the page around what's on the screen, and is moved along once the
//! thread scrolls out of it.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    Bounds, ClipboardItem, Context, Corners, CursorStyle, DispatchPhase, Entity, Global, Hitbox, HitboxBehavior,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, ScrollHandle,
    Subscription, WeakEntity, Window, div, point, px, size,
};

use super::{SCRIPT, themed};
use crate::models::MessageItem;
use crate::store::Store;
use crate::theme::*;
use crate::ui;
use crate::wpe::{self, BAND, Page, Pointer, Said};

/// WebKit's way of posting what the shared script says, and where the page is scrolled to, which
/// tells what each picture shows: a new page is said to be at its top before it's painted.
const LINUX_SCRIPT: &str = r#"
window.ipc = { postMessage: function (message) { window.webkit.messageHandlers.ipc.postMessage(message); } };
window.ipc.postMessage('scrolled:' + window.scrollY);
window.addEventListener('scroll', function () { window.ipc.postMessage('scrolled:' + window.scrollY); });
"#;

/// The question whose answer is the page's selection, for the clipboard.
const COPY: u64 = 1;

/// How much of the page WebKit holds above and below what's on the screen, at the least.
const MARGIN: f32 = 200.;

/// The body whose page was pressed last, which alone keeps a selection.
#[derive(Default)]
struct Selecting(Option<WeakEntity<BodyView>>);

impl Global for Selecting {}

/// The part of the page WebKit lays out: `top` down from the page's top, at the window's scale.
#[derive(Clone, Copy, PartialEq)]
struct Span {
    top: f32,
    width: i32,
    height: i32,
    scale: f32,
}

/// What WebKit painted, `top` down from the page's top, in bands of `BAND` rows.
struct Drawn {
    top: f32,
    width: u32,
    height: u32,
    bands: Vec<Arc<RenderImage>>,
}

pub struct BodyView {
    store: Entity<Store>,
    message: MessageItem,
    page: Option<Page>,
    making: bool,
    failed: bool,
    height: Option<f32>,
    span: Option<Span>,
    drawn: Option<Drawn>,
    cursor: CursorStyle,
    /// The button that went down in the page and is still held: the page has the pointer until
    /// it's up.
    held: Option<MouseButton>,
    hovered: bool,
    /// The scheme and the colours the page was loaded in.
    dark: bool,
    original: bool,
    _keys: Subscription,
}

impl BodyView {
    pub fn new(store: Entity<Store>, message: MessageItem, _scroll: ScrollHandle, cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, cx| {
            for band in this.drawn.take().into_iter().flat_map(|drawn| drawn.bands) {
                cx.drop_image(band, None);
            }
        })
        .detach();
        let keys = cx.observe_keystrokes(|this, event, _, cx| {
            let keystroke = &event.keystroke;
            let modifiers = keystroke.modifiers;
            let copy = keystroke.key == "c" && modifiers.control && !modifiers.alt && !modifiers.shift;
            if !copy || event.action.is_some() {
                return;
            }
            let selecting = cx.try_global::<Selecting>().and_then(|selecting| selecting.0.as_ref());
            if selecting.is_none_or(|selecting| selecting.entity_id() != cx.entity_id()) {
                return;
            }
            let Some(page) = &this.page else { return };
            page.run("String(window.getSelection())", COPY);
        });
        Self {
            store,
            message,
            page: None,
            making: false,
            failed: false,
            height: None,
            span: None,
            drawn: None,
            cursor: CursorStyle::Arrow,
            held: None,
            hovered: false,
            dark: false,
            original: false,
            _keys: keys,
        }
    }

    pub fn set_message(&mut self, message: MessageItem, cx: &mut Context<Self>) {
        if message == self.message {
            return;
        }
        let changed = message.html != self.message.html;
        self.message = message;
        if changed {
            self.load(self.dark);
        }
        cx.notify();
    }

    /// Lets go of the pointer while the thread isn't drawn.
    pub fn hide(&mut self) {
        self.held = None;
        self.hovered = false;
    }

    fn load(&mut self, dark: bool) {
        let Some(page) = &self.page else { return };
        self.dark = dark;
        let html = themed(self.message.html.as_deref().unwrap_or_default(), dark);
        page.load(&html, gpui::Rgba::from(CARD_FILL.hsla(dark)));
    }

    fn make_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page.is_some() {
            return;
        }
        let (said, heard) = async_channel::unbounded();
        let width = self.store.read(cx).body_width.round().max(1.) as i32;
        self.page = Some(Page::new(&format!("{LINUX_SCRIPT}{SCRIPT}"), width, 1, window.scale_factor(), said));
        self.load(cx.dark());
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(said) = heard.recv().await {
                let done = this.update_in(cx, |this, window, cx| match said {
                    Said::Height(height) => {
                        this.height = Some(height.max(20.));
                        this.settle();
                        cx.notify();
                    }
                    Said::Frame(frame) => this.take_frame(frame, window, cx),
                    Said::Cursor(cursor) => {
                        this.cursor = cursor;
                        cx.notify();
                    }
                    Said::Answer(COPY, text) if !text.is_empty() => {
                        cx.write_to_clipboard(ClipboardItem::new_string(text))
                    }
                    Said::Answer(..) => {}
                    Said::Failed => {
                        this.failed = true;
                        cx.notify();
                    }
                });
                if done.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn take_frame(&mut self, frame: wpe::Frame, window: &mut Window, cx: &mut Context<Self>) {
        let resized =
            self.drawn.as_ref().is_none_or(|drawn| (drawn.width, drawn.height) != (frame.width, frame.height));
        if resized {
            if frame.bands.len() != frame.height.div_ceil(BAND) as usize {
                return;
            }
            for band in self.drawn.take().into_iter().flat_map(|drawn| drawn.bands) {
                cx.drop_image(band, Some(window));
            }
            let bands = frame.bands.into_iter().map(|(_, band)| band).collect();
            self.drawn = Some(Drawn { top: frame.top, width: frame.width, height: frame.height, bands });
            cx.notify();
            return;
        }
        let Some(drawn) = &mut self.drawn else { return };
        drawn.top = frame.top;
        for (index, band) in frame.bands {
            let Some(slot) = drawn.bands.get_mut(index) else { continue };
            cx.drop_image(std::mem::replace(slot, band), Some(window));
        }
        cx.notify();
    }

    /// Has WebKit hold the part of the page around `visible`, the part of `bounds` on the screen.
    fn follow(&mut self, bounds: Bounds<Pixels>, visible: Bounds<Pixels>, scale: f32, cx: &mut Context<Self>) {
        let Some(page) = &self.page else { return };
        let width = f32::from(bounds.size.width).round() as i32;
        let shown = f32::from(visible.size.height);
        if width < 1 || shown < 1. {
            return;
        }
        let tall = self.height.unwrap_or(f32::from(bounds.size.height)).max(1.);
        let top = f32::from(visible.origin.y - bounds.origin.y);
        let bottom = (top + shown).min(tall);
        let holds = self.span.is_some_and(|span| {
            span.width == width && span.scale == scale && span.top <= top && span.top + span.height as f32 >= bottom
        });
        if !holds {
            let margin = (shown / 2.).max(MARGIN);
            let height = (shown + 2. * margin).min(tall).ceil();
            let start = (top - margin).clamp(0., (tall - height).max(0.)).round();
            let span = Span { top: start, width, height: height as i32, scale };
            if self.span.is_none_or(|old| (old.width, old.height, old.scale) != (width, span.height, scale)) {
                page.resize(width, span.height, scale);
            }
            if self.span.is_none_or(|old| old.top != start) {
                page.run(&scroll(start), 0);
            }
            self.span = Some(span);
        }
        let Some(height) = self.height else { return };
        let id = self.message.id.clone();
        self.store.update(cx, |store, _| store.note_body_height(&id, width as f64, height as f64));
    }

    /// Where `position` falls in the part of the page WebKit holds.
    fn at(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> (f32, f32) {
        let top = self.span.map_or(0., |span| span.top);
        (f32::from(position.x - bounds.origin.x), f32::from(position.y - bounds.origin.y) - top)
    }

    fn moved(&mut self, event: &MouseMoveEvent, bounds: Bounds<Pixels>, hovered: bool) {
        let Some(page) = &self.page else { return };
        let (x, y) = self.at(event.position, bounds);
        let held = if self.held == Some(MouseButton::Left) { wpe::MODIFIER_BUTTON1 } else { 0 };
        if hovered || self.held.is_some() {
            page.pointer(Pointer::Move, 0, x, y, modifiers(event.modifiers) | held, 0);
        } else if self.hovered {
            page.pointer(Pointer::Leave, 0, x, y, modifiers(event.modifiers), 0);
        }
        self.hovered = hovered;
    }

    fn pressed(&mut self, event: &MouseDownEvent, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let Some(page) = &self.page else { return };
        let Some(button) = button(event.button) else { return };
        let (x, y) = self.at(event.position, bounds);
        let held = if event.button == MouseButton::Left { wpe::MODIFIER_BUTTON1 } else { 0 };
        page.pointer(Pointer::Down, button, x, y, modifiers(event.modifiers) | held, event.click_count as u32);
        self.held = Some(event.button);
        let me = cx.entity().downgrade();
        let before = cx.default_global::<Selecting>().0.replace(me.clone());
        if let Some(before) = before.filter(|before| before.entity_id() != me.entity_id()) {
            let _ = before.update(cx, |body, _| {
                let Some(page) = &body.page else { return };
                page.run("window.getSelection().removeAllRanges();", 0);
            });
        }
    }

    fn released(&mut self, event: &MouseUpEvent, bounds: Bounds<Pixels>) {
        self.held = None;
        let Some(page) = &self.page else { return };
        let Some(button) = button(event.button) else { return };
        let (x, y) = self.at(event.position, bounds);
        page.pointer(Pointer::Up, button, x, y, modifiers(event.modifiers), 0);
    }

    /// Puts a page laid out again where the body wants it, a page just loaded too: scrolled to
    /// the span, in the message's own colours if they're shown.
    fn settle(&self) {
        let Some(page) = &self.page else { return };
        if let Some(span) = self.span {
            page.run(&scroll(span.top), 0);
        }
        if self.original {
            self.apply_original();
        }
    }

    /// Shows the page in the app's scheme, and the message in its own colours or the app's.
    fn restyle(&mut self, original: bool, dark: bool) {
        if dark != self.dark {
            self.load(dark);
        }
        if original != self.original {
            self.original = original;
            self.apply_original();
        }
    }

    fn apply_original(&self) {
        let Some(page) = &self.page else { return };
        let original = self.original;
        page.run(
            &format!(
                "(() => {{ const body = document.body; if (!body) return; body.classList.toggle('dark', {}); if (body.classList.contains('paper')) body.classList.toggle('light', {}); }})();",
                !original, original
            ),
            0,
        );
    }
}

impl Render for BodyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page.is_none() && !self.making {
            self.making = true;
            cx.defer_in(window, |this, window, cx| {
                this.making = false;
                this.make_page(window, cx);
            });
        }
        if self.failed {
            return ui::paragraph_in(
                "This message can't be shown here.",
                TextStyle::Body,
                MUTED_STRONGER_FOREGROUND,
                cx,
            )
            .into_any_element();
        }
        let store = self.store.read(cx);
        let original = store.original_messages.contains(&self.message.id);
        let kept = store.body_height(&self.message.id, store.body_width).map(|height| height as f32);
        self.restyle(original, cx.dark());
        let height = self.height.or(kept);
        let placeholder = height.is_none().then(|| ui::body_placeholder(self.message.shape.clone(), cx));
        let drawn = self.drawn.as_ref().map(|drawn| (drawn.top, drawn.width, drawn.bands.clone()));
        let cursor = self.cursor;
        let body = cx.entity();
        div()
            .relative()
            .w_full()
            .when_some(height, |frame, height| frame.h(px(height)))
            .children(placeholder)
            .child(
                gpui::canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |bounds, hitbox, window, cx| {
                        let scale = window.scale_factor();
                        let visible = bounds.intersect(&window.content_mask().bounds);
                        if let Some((top, width, bands)) = &drawn {
                            paint(bounds, visible, *top, *width, bands, scale, window);
                        }
                        window.set_cursor_style(cursor, &hitbox);
                        listen(&body, bounds, &hitbox, window);
                        let body = body.clone();
                        cx.defer(move |cx| body.update(cx, |body, cx| body.follow(bounds, visible, scale, cx)));
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }
}

/// Draws the bands that show in `visible`, each on whole pixels of the screen.
fn paint(
    bounds: Bounds<Pixels>,
    visible: Bounds<Pixels>,
    top: f32,
    width: u32,
    bands: &[Arc<RenderImage>],
    scale: f32,
    window: &mut Window,
) {
    let left = (f32::from(bounds.origin.x) * scale).round();
    let start = ((f32::from(bounds.origin.y) + top) * scale).round();
    let (shown_top, shown_bottom) = (f32::from(visible.top()) * scale, f32::from(visible.bottom()) * scale);
    for (index, band) in bands.iter().enumerate() {
        let y = start + (index as u32 * BAND) as f32;
        let rows = band.size(0).height.0 as f32;
        if y + rows < shown_top || y > shown_bottom {
            continue;
        }
        let at = Bounds {
            origin: point(px(left / scale), px(y / scale)),
            size: size(px(width as f32 / scale), px(rows / scale)),
        };
        let _ = window.paint_image(at, at, Corners::default(), band.clone(), 0, false);
    }
}

/// Hands the page the pointer over it, and the rest of a press that began there.
fn listen(body: &Entity<BodyView>, bounds: Bounds<Pixels>, hitbox: &Hitbox, window: &mut Window) {
    let (moving, hovering) = (body.clone(), hitbox.clone());
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let hovered = hovering.is_hovered(window);
        moving.update(cx, |body, _| body.moved(event, bounds, hovered));
    });
    let (pressing, hovering) = (body.clone(), hitbox.clone());
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || !hovering.is_hovered(window) {
            return;
        }
        pressing.update(cx, |body, cx| body.pressed(event, bounds, cx));
        cx.stop_propagation();
    });
    let releasing = body.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
        if phase != DispatchPhase::Bubble || releasing.read(cx).held != Some(event.button) {
            return;
        }
        releasing.update(cx, |body, _| body.released(event, bounds));
        cx.stop_propagation();
    });
}

fn scroll(top: f32) -> String {
    format!("window.cut = {top}; window.scrollTo(0, {top});")
}

fn button(button: MouseButton) -> Option<u32> {
    match button {
        MouseButton::Left => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Right => Some(3),
        _ => None,
    }
}

fn modifiers(modifiers: Modifiers) -> u32 {
    [
        (modifiers.control, wpe::MODIFIER_CONTROL),
        (modifiers.shift, wpe::MODIFIER_SHIFT),
        (modifiers.alt, wpe::MODIFIER_ALT),
        (modifiers.platform, wpe::MODIFIER_META),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(0, |bits, (_, bit)| bits | bit)
}
