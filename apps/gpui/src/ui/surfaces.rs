//! What things rest on: the bar across a page, a sheet over the window, cards, popovers, the
//! banner and the toast at the bottom, notices and the empty state.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId,
    MouseButton, Path, PathBuilder, Pixels, Point, SharedString, Size, Window, canvas, div, point, px,
};

use super::{ActionButton, IconButton, Surface, Symbol, Variant, icon, line, paragraph_in, rule, spinner};
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

/// Every sheet's shell over the window: its header with its buttons and the close at its end, its
/// content, at its size.
/// `background` is the page's colour unless it is a card's.
pub fn sheet<E: IntoElement, F: Fn(&mut Window, &mut App) + 'static>(
    id: &str,
    title: impl Into<SharedString>,
    size: SheetSize,
    background: ThemeColor,
    mut trailing: Vec<AnyElement>,
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
    trailing.push(close_button.into_any_element());
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
        .child(
            header_bar(Some(title.into()), Vec::new(), None, trailing, cx).px(px(size::BAR_BUTTON_INSET)).flex_none(),
        )
        .child(rule(rule_color, cx))
        .child(div().flex_1().min_h_0().flex().flex_col().child(content))
        .into_any_element()
}

/// A bar across the bottom of a sheet, under its content: its buttons at its end, the main one
/// last.
pub fn footer_bar(buttons: Vec<AnyElement>, cx: &App) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_end()
        .px(px(space::XL - size::BUTTON_GAP / 2.))
        .py(px(space::M))
        .border_t_1()
        .border_color(cx.color(BORDER))
        .children(buttons)
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

/// Draws `popup` under `face`, left-aligned with it and in from the window's edges, over
/// everything and clipped by nothing it is in; above it when there's no room below. `faces`
/// gathers where every face is, so a click on one is left to it rather than to the popup's
/// click outside.
pub struct PopupUnder {
    face: AnyElement,
    popup: Option<AnyElement>,
    faces: Option<Rc<RefCell<Vec<Bounds<Pixels>>>>>,
    /// The popup as it is drawn over its face, and the most it grows to, by which it goes under
    /// or over once and for all: it doesn't flip as it grows and shrinks.
    over: Option<(AnyElement, Pixels)>,
}

pub fn popup_under(face: impl IntoElement, popup: Option<AnyElement>) -> PopupUnder {
    PopupUnder { face: face.into_any_element(), popup, faces: None, over: None }
}

impl PopupUnder {
    /// The popup laid out for over its face (what is nearest the face at its bottom), used when
    /// one `tallest` doesn't fit under it.
    pub fn over(mut self, over: Option<AnyElement>, tallest: f32) -> Self {
        self.over = over.map(|over| (over, px(tallest)));
        self
    }

    pub fn faces(mut self, faces: Rc<RefCell<Vec<Bounds<Pixels>>>>) -> Self {
        self.faces = Some(faces);
        self
    }
}

/// Between a popup and its face, and the least between a popup and the window's edges.
const POPUP_GAP: f32 = space::XS;
const POPUP_MARGIN: f32 = space::S;

/// Where a popup of `popup` goes under (or above) `face`, in a window of `viewport`.
pub fn popup_origin(face: Bounds<Pixels>, popup: Size<Pixels>, viewport: Size<Pixels>) -> Point<Pixels> {
    let (gap, margin) = (px(POPUP_GAP), px(POPUP_MARGIN));
    let x = face.left().min(viewport.width - popup.width - margin).max(margin);
    let below = face.bottom() + gap;
    let above = face.top() - gap - popup.height;
    let room_below = viewport.height - margin - below;
    let room_above = face.top() - gap - margin;
    let y = if popup.height > room_below && room_above > room_below { above.max(margin) } else { below };
    point(x, y)
}

impl IntoElement for PopupUnder {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PopupUnder {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        (self.face.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.face.prepaint(window, cx);
        if let Some(faces) = &self.faces {
            faces.borrow_mut().push(bounds);
        }
        let Some(mut popup) = self.popup.take() else { return };
        let viewport = window.viewport_size();
        let space = gpui::size(AvailableSpace::Definite(viewport.width), AvailableSpace::Definite(viewport.height));
        if let Some((mut over, tallest)) = self.over.take() {
            let (gap, margin) = (px(POPUP_GAP), px(POPUP_MARGIN));
            let room_below = viewport.height - margin - (bounds.bottom() + gap);
            let room_above = bounds.top() - gap - margin;
            let goes_over = tallest > room_below && room_above > room_below;
            let chosen = if goes_over { &mut over } else { &mut popup };
            // It takes no more than the room on its side; what it holds scrolls inside that.
            let room = if goes_over { room_above } else { room_below };
            let space = gpui::size(AvailableSpace::Definite(viewport.width), AvailableSpace::Definite(room));
            let size = chosen.layout_as_root(space, window, cx);
            let x = bounds.left().min(viewport.width - size.width - margin).max(margin);
            let y = if goes_over { (bounds.top() - gap - size.height).max(margin) } else { bounds.bottom() + gap };
            let chosen = if goes_over { over } else { popup };
            window.defer_draw(chosen, point(x, y), 10, None);
            return;
        }
        let size = popup.layout_as_root(space, window, cx);
        let origin = popup_origin(bounds, size, viewport);
        window.defer_draw(popup, origin, 10, None);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.face.paint(window, cx);
    }
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

/// A list's name, muted so it doesn't look like one of its rows.
pub fn muted_section_heading(title: impl Into<SharedString>, cx: &App) -> Div {
    div().text_in(TextStyle::Subheading, MUTED_FOREGROUND, cx).child(title.into())
}

/// The tone of a notice or a chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Error,
    Warning,
    Success,
}

impl Tone {
    pub fn color(self) -> ThemeColor {
        match self {
            Self::Muted => MUTED_STRONGER_FOREGROUND,
            Self::Error => DESTRUCTIVE,
            Self::Warning => WARNING,
            Self::Success => SUCCESS,
        }
    }
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
    let color = tone.color();
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
            div()
                .flex()
                .items_center()
                .children(banner.action.map(|action| action.surface(Surface::Popover)))
                .children(banner.close.map(|close| close.on_popover())),
        )
}

/// What a toast about a message being sent shows before its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToastMark {
    /// The seconds left of the undo delay, which started at `shown`.
    Countdown { shown: Instant, duration: Duration },
    /// That it is going.
    Sending,
    /// That it went.
    Sent,
}

/// The note at the bottom: a line and maybe an action, gone after a while. A toast about a
/// message being sent starts with its mark: a ring that fills as the undo delay runs out, then a
/// spinner in `PROCESS` while it goes, then a check in `SUCCESS`. The mark and the action are
/// capsules inset from the toast's edge by as much on every side, so their ends are concentric
/// with the toast's.
pub fn toast(
    message: SharedString,
    action: Option<(SharedString, super::OnClick)>,
    mark: Option<ToastMark>,
    cx: &App,
) -> impl IntoElement + use<> {
    let has_action = action.is_some();
    div()
        .flex()
        .items_center()
        .p(px(TOAST_INSET))
        .h(px(size::LARGE))
        .rounded_full()
        .bg(cx.color(POPOVER))
        .border_1()
        .border_color(cx.color(BORDER_POPOVER))
        .shadow(cx.shadow(ShadowSize::Md, opacity::SHADOW))
        .occlude()
        .when_some(mark, |toast, mark| toast.child(toast_mark(mark, cx)))
        .child(
            div()
                .text_in(TextStyle::Label, FOREGROUND, cx)
                .whitespace_nowrap()
                .pl(px(if mark.is_some() { space::S + 2. } else { space::M }))
                .pr(px(if has_action { space::XL } else { space::M }))
                .child(message),
        )
        .when_some(action, |toast, (title, on_click)| {
            toast.child(
                div()
                    .id("toast-action")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .h(px(TOAST_INNER))
                    .min_w(px(TOAST_INNER * 1.6))
                    .px(px(space::L))
                    .rounded_full()
                    .border_1()
                    .border_color(cx.tint(PRIMARY, opacity::COLOR_BORDER))
                    .hover(|face| face.bg(cx.tint(PRIMARY, opacity::COLOR_TINT)))
                    .cursor_pointer()
                    .text_in(TextStyle::LabelStrong, PRIMARY, cx)
                    .whitespace_nowrap()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        on_click(event, window, cx);
                    })
                    .child(title),
            )
        })
}

const TOAST_INSET: f32 = space::XS;
/// The height of what is in a toast: the ring and the action.
const TOAST_INNER: f32 = size::LARGE - TOAST_INSET * 2.;
const COUNTDOWN_LINE: f32 = 2.5;

fn toast_mark(mark: ToastMark, cx: &App) -> AnyElement {
    let badge = |tint: ThemeColor| {
        div()
            .flex_none()
            .size(px(TOAST_INNER))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(cx.tint(tint, opacity::COLOR_TINT))
    };
    match mark {
        ToastMark::Countdown { shown, duration } => toast_countdown(shown, duration, cx).into_any_element(),
        ToastMark::Sending => badge(PROCESS).child(spinner(16., cx.color(PROCESS))).into_any_element(),
        ToastMark::Sent => {
            badge(SUCCESS).child(icon(Symbol::Check, 16.).text_color(cx.color(SUCCESS))).into_any_element()
        }
    }
}

/// The seconds left, in a ring that fills as they run out, drawn again every frame.
fn toast_countdown(shown: Instant, duration: Duration, cx: &App) -> impl IntoElement + use<> {
    let left = duration.saturating_sub(shown.elapsed());
    let fraction = if duration.is_zero() { 1. } else { 1. - left.as_secs_f32() / duration.as_secs_f32() };
    let track = cx.color(POPOVER_ACCENT_STRONGER);
    let arc = cx.color(FOREGROUND);
    div()
        .relative()
        .flex_none()
        .size(px(TOAST_INNER))
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let center = bounds.center();
                    let radius = (TOAST_INNER - COUNTDOWN_LINE) / 2.;
                    if let Some(path) = ring_path(center, radius, 1.) {
                        window.paint_path(path, track);
                    }
                    if let Some(path) = ring_path(center, radius, fraction) {
                        window.paint_path(path, arc);
                    }
                    if fraction > 0. {
                        for end in [0., fraction] {
                            let point = ring_point(center, radius, end);
                            let cap = gpui::size(px(COUNTDOWN_LINE), px(COUNTDOWN_LINE));
                            let origin = point - gpui::point(px(COUNTDOWN_LINE / 2.), px(COUNTDOWN_LINE / 2.));
                            window.paint_quad(
                                gpui::fill(Bounds::new(origin, cap), arc).corner_radii(px(COUNTDOWN_LINE / 2.)),
                            );
                        }
                    }
                    if !left.is_zero() {
                        window.request_animation_frame();
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .text_in(TextStyle::LabelStrong, FOREGROUND, cx)
                .child(SharedString::from((left.as_secs_f32().ceil() as u64).to_string())),
        )
}

/// The point a `fraction` of the way round a ring, clockwise from the top.
fn ring_point(center: Point<Pixels>, radius: f32, fraction: f32) -> Point<Pixels> {
    let angle = std::f32::consts::TAU * fraction - std::f32::consts::FRAC_PI_2;
    center + gpui::point(px(radius * angle.cos()), px(radius * angle.sin()))
}

/// A line along a `fraction` of a ring, clockwise from the top.
fn ring_path(center: Point<Pixels>, radius: f32, fraction: f32) -> Option<Path<Pixels>> {
    if fraction <= 0. {
        return None;
    }
    let steps = (fraction * 96.).ceil() as usize;
    let mut path = PathBuilder::stroke(px(COUNTDOWN_LINE));
    path.move_to(ring_point(center, radius, 0.));
    for step in 1..=steps {
        path.line_to(ring_point(center, radius, fraction * step as f32 / steps as f32));
    }
    if fraction >= 1. {
        path.close();
    }
    path.build().ok()
}

/// Cancel and Save under a form, on `surface`.
pub fn form_buttons(
    id: &str,
    save: &str,
    save_disabled: bool,
    surface: Surface,
    on_save: super::OnClick,
    on_cancel: super::OnClick,
) -> Div {
    div()
        .flex()
        .items_center()
        .child(
            ActionButton::new(SharedString::from(format!("{id}-cancel")), "Cancel")
                .variant(Variant::Ghost)
                .surface(surface)
                .on_click(move |event, window, cx| on_cancel(event, window, cx)),
        )
        .child(
            ActionButton::new(SharedString::from(format!("{id}-save")), save.to_string())
                .variant(Variant::Primary)
                .disabled(save_disabled)
                .on_click(move |event, window, cx| on_save(event, window, cx)),
        )
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, point, px, size};

    use super::popup_origin;

    #[test]
    fn puts_a_popup_under_its_face_or_above_it_and_in_from_the_sides() {
        let viewport = size(px(800.), px(600.));
        let face = Bounds::new(point(px(100.), px(100.)), size(px(60.), px(28.)));
        assert_eq!(popup_origin(face, size(px(300.), px(200.)), viewport), point(px(100.), px(134.)));
        let right = Bounds::new(point(px(700.), px(100.)), size(px(60.), px(28.)));
        assert_eq!(popup_origin(right, size(px(300.), px(200.)), viewport).x, px(492.));
        let low = Bounds::new(point(px(100.), px(500.)), size(px(60.), px(28.)));
        assert_eq!(popup_origin(low, size(px(300.), px(200.)), viewport), point(px(100.), px(294.)));
    }
}
