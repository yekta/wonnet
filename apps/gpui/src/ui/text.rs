//! Text that can be selected and copied, as a thread's subject. Built on GPUI Base's window-wide
//! selection, which `Root` draws and copies.

use std::ops::Range;

use gpui::{
    App, Bounds, Corners, Edges, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId,
    IntoElement, LayoutId, PaintQuad, Pixels, Point, SharedString, StyledText, Window, transparent_black,
};
use gpui_base::{TextSelection, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun};

use crate::theme::*;

/// A run of selectable text.
pub struct RichText {
    id: ElementId,
    text: SharedString,
    styled: StyledText,
}

impl RichText {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        let text = text.into();
        Self { id: id.into(), styled: StyledText::new(text.clone()), text }
    }
}

impl IntoElement for RichText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichText {
    type RequestLayoutState = TextSelectionHandle;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let id = global_id.expect("rich text has an id");
        let handle = window.with_element_state(id, |kept: Option<TextSelectionHandle>, _| {
            let handle = kept.unwrap_or_else(|| TextSelectionHandle::new(self.text.clone(), cx));
            (handle.clone(), handle)
        });
        let (layout_id, ()) = self.styled.request_layout(global_id, inspector_id, window, cx);
        (layout_id, handle)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        handle: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled.prepaint(global_id, inspector_id, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let registration = TextSelectionRegistration::new(hitbox.clone(), bounds)
            .with_text_bounds(vec![bounds])
            .with_rendered_element(handle, window, cx);
        handle.register(registration, window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        handle: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled.layout().clone();
        let before = TextSelection::selected_text(window, cx);
        let run = TextSelectionRun::new(self.text.clone(), layout.clone(), bounds);
        let projection = handle.update_runs(&[run], cx);
        if before != TextSelection::selected_text(window, cx) {
            window.refresh();
        }
        let color = cx.tint(PRIMARY, opacity::COLOR_TINT_STRONGER);
        for range in projection.ranges().iter().flatten().cloned() {
            paint_selection(&layout, range, color, window);
        }
        self.styled.paint(global_id, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}

fn paint_selection(layout: &gpui::TextLayout, range: Range<usize>, color: Hsla, window: &mut Window) {
    let (Some(start), Some(end)) = (layout.position_for_index(range.start), layout.position_for_index(range.end))
    else {
        return;
    };
    let bounds = layout.bounds();
    let height = layout.line_height();
    for quad in selection_quads(start, end, bounds, height) {
        window.paint_quad(PaintQuad {
            bounds: quad,
            background: color.into(),
            corner_radii: Corners::default(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: Default::default(),
        });
    }
}

/// The boxes a selection covers from `start` to `end` over wrapped lines.
fn selection_quads(
    start: Point<Pixels>,
    end: Point<Pixels>,
    bounds: Bounds<Pixels>,
    height: Pixels,
) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(start, Point::new(end.x, end.y + height))];
    }
    let mut quads = vec![Bounds::from_corners(start, Point::new(bounds.right(), start.y + height))];
    if end.y > start.y + height {
        quads
            .push(Bounds::from_corners(Point::new(bounds.left(), start.y + height), Point::new(bounds.right(), end.y)));
    }
    quads.push(Bounds::from_corners(Point::new(bounds.left(), end.y), Point::new(end.x, end.y + height)));
    quads
}
