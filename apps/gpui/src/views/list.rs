//! The thread list: one line a thread, drawn on the page in the centred column, only the rows on
//! screen made. Hovering a row shows what can be done to it, as Newton did, and the circle in the
//! gutter before it that selects it. While rows are selected every row shows its circle and a
//! click selects instead of opening. Ctrl-click picks rows, Shift-click the rows up to one; a
//! right-click offers what can be done to the row, or to the selection it is in.

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, ClickEvent, Context, Entity, HighlightStyle, MouseButton, ScrollStrategy, SharedString, StyledText, Task,
    UniformListScrollHandle, Window, div, linear_color_stop, linear_gradient, point, px, uniform_list,
};

use crate::models::{Filter, ThreadRow};
use crate::store::{Store, ThreadCommand};
use crate::theme::*;
use crate::ui::{self, ActionButton, Chip, MenuEntry, Symbol, Variant, icon, icon_in};

pub struct ThreadList {
    store: Entity<Store>,
    scroll: UniformListScrollHandle,
    /// The row the keyboard was on when the list was last drawn, to scroll to it when it moves.
    shown_selected: Option<String>,
    /// The row last clicked: it is under the pointer, so the list doesn't move to show it.
    clicked: Option<String>,
    shown_mailbox: (String, Option<Filter>, bool),
    prefetch: Option<Task<()>>,
    restored: bool,
}

const LIST_INSET: f32 = space::L;

impl ThreadList {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let selected = store.read(cx).selected.clone();
        Self {
            store,
            scroll: UniformListScrollHandle::new(),
            shown_selected: selected,
            clicked: None,
            shown_mailbox: (String::new(), None, false),
            prefetch: None,
            restored: false,
        }
    }

    /// Asks for the bodies of the rows on screen once the list stops moving.
    fn schedule_prefetch(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let store = self.store.clone();
        self.prefetch = Some(cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            store.update(cx, |store, cx| {
                let rows = store.visible_rows();
                let range = range.start.min(rows.len())..range.end.min(rows.len());
                let ids = rows[range.clone()].iter().map(|row| row.id.clone()).collect();
                store.prefetch(ids);
                if range.end + 30 > store.rows.len() && store.search_rows.is_none() {
                    store.load_more(cx);
                }
            });
        }));
    }

    fn follow_selection(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let mailbox = (store.mailbox.clone(), store.filter, store.list_ready);
        if mailbox != self.shown_mailbox {
            let switched = self.shown_mailbox.0 != mailbox.0 || self.shown_mailbox.1 != mailbox.1;
            let became_ready = mailbox.2 && !self.shown_mailbox.2;
            if self.restored && (switched || became_ready) && mailbox.2 {
                self.scroll.0.borrow().base_handle.set_offset(point(px(0.), px(0.)));
            }
            self.shown_mailbox = mailbox;
        }
        if !self.restored && store.booted {
            self.restored = true;
            let offset = store.list_offset as f32;
            self.scroll.0.borrow().base_handle.set_offset(point(px(0.), px(-offset)));
        }
        let selected = store.selected.clone();
        if selected == self.shown_selected {
            return;
        }
        if selected != self.clicked
            && let Some(index) =
                selected.as_ref().and_then(|id| store.visible_rows().iter().position(|row| &row.id == id))
        {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        self.clicked = None;
        self.shown_selected = selected;
    }

    fn clicked_row(&mut self, row: &ThreadRow, hit: Hit, event: &ClickEvent, cx: &mut Context<Self>) {
        let modifiers = event.modifiers();
        let id = row.id.clone();
        let command_click = if cfg!(target_os = "macos") { modifiers.platform } else { modifiers.control };
        self.store.update(cx, |store, cx| {
            if modifiers.shift {
                return store.extend_selection_to(&id, cx);
            }
            let selecting = !store.selection.is_empty();
            if hit == Hit::Select || command_click || (selecting && hit != Hit::Star) {
                return store.toggle_selection(&id, cx);
            }
            let threads = store.targets_for(&id);
            let command = match hit {
                Hit::Star => Some(ThreadCommand::Star),
                Hit::Archive => Some(ThreadCommand::Archive),
                Hit::Trash => Some(ThreadCommand::Trash),
                Hit::Snooze => Some(ThreadCommand::Snooze),
                Hit::Read => Some(ThreadCommand::Read),
                Hit::Select | Hit::Open => None,
            };
            match command {
                Some(command) => store.run(command, Some(threads), cx),
                None => {
                    store.clear_selection(cx);
                    store.open(&id, cx);
                }
            }
        });
        if hit == Hit::Open {
            self.clicked = Some(id);
        }
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        self.schedule_prefetch(range.clone(), cx);
        let store = self.store.read(cx);
        let rows = store.visible_rows();
        let selecting = !store.selection.is_empty();
        let circles = store.select_circles();
        let loading = store.loading_more && store.search_rows.is_none();
        let width: f32 = window.bounds().size.width.into();
        let column = (width - 48. - 2. * size::SELECT_GUTTER).clamp(0., size::CARD_WIDTH);
        let select_help = store.key_help("Select", "select");
        let shown: Vec<Option<(ThreadRow, RowState)>> = range
            .clone()
            .map(|index| {
                rows.get(index).map(|row| {
                    let state = RowState {
                        current: store.selected.as_ref() == Some(&row.id),
                        checked: store.selection.contains(&row.id),
                        selecting,
                        circles,
                        column,
                    };
                    (row.clone(), state)
                })
            })
            .collect();
        let mut items = Vec::new();
        for (index, row) in range.zip(shown) {
            match row {
                Some((row, state)) => {
                    items.push(self.row(index, row, state, select_help.clone(), cx).into_any_element())
                }
                None if loading => items.push(loading_more(cx).into_any_element()),
                None => {}
            }
        }
        items
    }

    fn row(
        &self,
        index: usize,
        row: ThreadRow,
        state: RowState,
        select_help: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let group = SharedString::from(format!("row-{index}"));
        let unread = row.unread;
        let strong = if unread { FOREGROUND } else { MUTED_FOREGROUND };
        let draft = row.draft_id.is_some();
        let starred = row.starred && !draft;
        let date_weight = if unread { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL };
        let date_color = if unread { strong } else { MUTED_STRONGER_FOREGROUND };
        let show_circle = state.checked || state.selecting;
        let hover_actions = !state.selecting;
        let entity = cx.entity();

        let face_fill = if state.checked { Some(cx.tint(PRIMARY, opacity::COLOR_TINT)) } else { None };
        let subject = if row.subject.is_empty() { "(no subject)".to_string() } else { row.subject.clone() };
        let snippet = row.snippet.replace('\n', " ");
        let subject_end = subject.len();
        let words = if snippet.is_empty() { subject } else { format!("{subject}  –  {snippet}") };
        let subject_style = HighlightStyle {
            color: Some(cx.color(strong)),
            font_weight: Some(if unread { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL }),
            ..HighlightStyle::default()
        };
        let line = div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(13.))
            .text_color(cx.color(MUTED_STRONGER_FOREGROUND))
            .child(StyledText::new(words).with_highlights([(0..subject_end, subject_style)]));
        let actions = actions(&row, &group, index, &entity, cx);
        let fade_from = cx.color(BACKGROUND_ACCENT_LARGER).opacity(0.);
        let fade_to = cx.color(BACKGROUND_ACCENT_LARGER);
        let open_row = row.clone();
        let menu_row = row.clone();
        let menu_entity = entity.clone();
        let face = div()
            .relative()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .h(px(size::ROW_HEIGHT - size::ROW_GAP))
            .flex()
            .items_center()
            .rounded(px(radius::SM))
            .when_some(face_fill, |face, fill| face.bg(fill))
            .when(!state.checked, |face| {
                face.group_hover(group.clone(), |face| face.bg(cx.color(BACKGROUND_ACCENT_LARGER)))
            })
            .when(state.current, |face| {
                face.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(radius::SM))
                        .border_1()
                        .border_dashed()
                        .border_color(cx.color(BACKGROUND_ACCENT_STRONGER)),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px(3.))
                    .top(px(5.))
                    .bottom(px(5.))
                    .w(px(size::ACCOUNT_BAR))
                    .rounded_full()
                    .bg(cx.color(account_color(&row.color))),
            )
            .when(unread, |face| {
                face.child(div().absolute().left(px(18.)).size(px(7.)).rounded_full().bg(cx.color(PRIMARY)))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .size_full()
                    .min_w_0()
                    .pl(px(36.))
                    .pr(px(space::L))
                    .font_family(FONT)
                    .child(
                        div()
                            .w(px(190.))
                            .flex_none()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(if unread { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                            .text_color(cx.color(strong))
                            .child(row.senders.clone()),
                    )
                    .child(div().w(px(38.)).flex_none().pl(px(14.)).when(row.attachment, |slot| {
                        slot.child(icon(Symbol::Paperclip, 14.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)))
                    }))
                    .child(line)
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(space::M))
                            .pl(px(space::L))
                            .when(hover_actions, |end| end.group_hover(group.clone(), |end| end.invisible()))
                            .when(starred, |end| end.child(icon_in(Symbol::StarFilled, 14., cx.color(STAR))))
                            .child(
                                div()
                                    .min_w(px(58.))
                                    .flex()
                                    .justify_end()
                                    .whitespace_nowrap()
                                    .text_size(px(12.))
                                    .font_weight(date_weight)
                                    .text_color(cx.color(date_color))
                                    .child(row.date.clone()),
                            ),
                    ),
            )
            .when(hover_actions, |face| {
                face.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(space::L - 6.))
                        .flex()
                        .items_center()
                        .invisible()
                        .group_hover(group.clone(), |actions| actions.visible())
                        .child(div().w(px(space::XXL + space::XXS)).h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(fade_from, 0.),
                            linear_color_stop(fade_to, 1.),
                        )))
                        .child(div().h_full().flex().items_center().pr(px(0.)).bg(fade_to).children(actions)),
                )
            });
        let gutter = div()
            .id(SharedString::from(format!("select-{index}")))
            .w(px(size::SELECT_GUTTER))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .when(!show_circle && state.circles, |gutter| {
                gutter.invisible().group_hover(group.clone(), |gutter| gutter.visible())
            })
            .when(!show_circle && !state.circles, |gutter| gutter.invisible())
            .tooltip(move |_, cx| {
                ui::Tooltip::view(if state.checked { "Deselect".to_string() } else { select_help.clone() }, cx)
            })
            .child(ui::select_circle(state.checked, Some(SharedString::from(format!("select-{index}-hover"))), cx))
            .group(SharedString::from(format!("select-{index}-hover")))
            .on_click({
                let entity = entity.clone();
                let row = row.clone();
                move |event, _, cx| {
                    cx.stop_propagation();
                    entity.update(cx, |list, cx| list.clicked_row(&row, Hit::Select, event, cx));
                }
            });
        div()
            .id(SharedString::from(format!("thread-{}", row.id)))
            .h(px(size::ROW_HEIGHT))
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .id(group.clone())
                    .group(group.clone())
                    .w(px(state.column + 2. * size::SELECT_GUTTER))
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(gutter)
                    .child(face)
                    .child(div().w(px(size::SELECT_GUTTER)))
                    .on_click(move |event, _, cx| {
                        entity.update(cx, |list, cx| list.clicked_row(&open_row, Hit::Open, event, cx))
                    })
                    .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                        let entries = menu_entity.read(cx).menu(&menu_row, cx);
                        ui::open_menu(format!("row-menu-{}", menu_row.id), entries, Some(event.position), window, cx);
                    }),
            )
    }

    /// What a right-click on the row offers: for the selection when the row is in it.
    fn menu(&self, row: &ThreadRow, cx: &App) -> Vec<MenuEntry> {
        let store = self.store.read(cx);
        let threads = store.targets_for(&row.id);
        let many = threads.len() > 1;
        let mut entries = Vec::new();
        if !many && row.draft_id.is_none() {
            let open = self.store.clone();
            let id = row.id.clone();
            entries.push(MenuEntry::item("Open", move |_, cx| open.update(cx, |store, cx| store.open(&id, cx))));
            entries.push(MenuEntry::Separator);
        }
        let groups: [&[ThreadCommand]; 3] = [
            if many { &[] } else { &[ThreadCommand::Reply, ThreadCommand::Forward] },
            &[
                ThreadCommand::Archive,
                ThreadCommand::Trash,
                ThreadCommand::Snooze,
                ThreadCommand::Read,
                ThreadCommand::Star,
            ],
            &[ThreadCommand::Label, ThreadCommand::Move],
        ];
        for group in groups {
            let commands = super::topbar::command_entries(&self.store, group, &threads, cx);
            if commands.is_empty() {
                continue;
            }
            entries.extend(commands);
            entries.push(MenuEntry::Separator);
        }
        let toggle = self.store.clone();
        if many {
            entries.push(
                MenuEntry::item("Clear the selection", move |_, cx| {
                    toggle.update(cx, |store, cx| store.clear_selection(cx))
                })
                .symbol(Symbol::X),
            );
        } else {
            let id = row.id.clone();
            let title = if store.selection.contains(&row.id) { "Deselect" } else { "Select" };
            entries.push(
                MenuEntry::item(title, move |_, cx| toggle.update(cx, |store, cx| store.toggle_selection(&id, cx)))
                    .symbol(Symbol::CircleCheck),
            );
        }
        entries
    }
}

#[derive(Clone, Copy)]
struct RowState {
    current: bool,
    checked: bool,
    selecting: bool,
    circles: bool,
    column: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Select,
    Star,
    Archive,
    Trash,
    Snooze,
    Read,
    Open,
}

/// The hovered row's actions, from the left: archive, trash, snooze, read, star.
fn actions(
    row: &ThreadRow,
    group: &SharedString,
    index: usize,
    entity: &Entity<ThreadList>,
    cx: &App,
) -> Vec<gpui::AnyElement> {
    let kinds: Vec<(Hit, Symbol, &str)> = if row.draft_id.is_some() {
        vec![(Hit::Trash, Symbol::Trash, "Delete draft")]
    } else {
        vec![
            (Hit::Archive, Symbol::Archive, "Archive"),
            (Hit::Trash, Symbol::Trash, "Trash"),
            (Hit::Snooze, Symbol::Clock, "Snooze"),
            (
                Hit::Read,
                if row.unread { Symbol::MailOpen } else { Symbol::Mail },
                if row.unread { "Mark read" } else { "Mark unread" },
            ),
            (
                Hit::Star,
                if row.starred { Symbol::StarFilled } else { Symbol::Star },
                if row.starred { "Unstar" } else { "Star" },
            ),
        ]
    };
    let _ = group;
    kinds
        .into_iter()
        .map(|(hit, symbol, help)| {
            let star = hit == Hit::Star;
            let rest = if star && row.starred { STAR } else { MUTED_FOREGROUND };
            let (hover_fill, hover_color) = if star {
                (cx.tint(STAR, opacity::COLOR_TINT), STAR)
            } else {
                (cx.color(BACKGROUND_ACCENT_LARGER_STRONGEST), FOREGROUND)
            };
            let entity = entity.clone();
            let row = row.clone();
            let glyph =
                if symbol == Symbol::StarFilled { icon_in(symbol, 16., cx.color(STAR)) } else { icon(symbol, 16.) };
            div()
                .id(SharedString::from(format!("action-{index}-{help}")))
                .w(px(30.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .tooltip(move |_, cx| ui::Tooltip::view(help, cx))
                .on_click(move |event, _, cx| {
                    cx.stop_propagation();
                    entity.update(cx, |list, cx| list.clicked_row(&row, hit, event, cx));
                })
                .child(
                    div()
                        .size(px(size::SMALL))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.color(rest))
                        .hover(move |circle| circle.bg(hover_fill).text_color(cx.color(hover_color)))
                        .child(glyph),
                )
                .into_any_element()
        })
        .collect()
}

fn loading_more(cx: &App) -> impl IntoElement + use<> {
    div()
        .h(px(size::ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(space::S))
        .child(ui::spinner(13., cx.color(MUTED_FOREGROUND)))
        .child(div().text(TextStyle::Caption, cx).child("Loading more"))
}

/// Over the list: the split inbox's tabs, the filter the list is narrowed to, emptying Trash or Spam.
fn list_header(store: &Entity<Store>, cx: &App) -> Option<impl IntoElement> {
    let state = store.read(cx);
    let shows =
        state.search_rows.is_none() && (!state.splits.is_empty() || state.filter.is_some() || state.can_empty());
    if !shows {
        return None;
    }
    let chosen = state
        .splits
        .iter()
        .find(|split| split.mailbox == state.mailbox)
        .or(state.splits.first())
        .map(|split| split.mailbox.clone());
    let tabs = state
        .splits
        .iter()
        .map(|split| {
            let pick = store.clone();
            let mailbox = split.mailbox.clone();
            let on_click: ui::OnClick =
                Rc::new(move |_, _, cx| pick.update(cx, |store, cx| store.select_mailbox(&mailbox, cx)));
            (
                SharedString::from(format!("split-{}", split.mailbox)),
                SharedString::from(split.name.clone()),
                split.unread,
                chosen.as_ref() == Some(&split.mailbox),
                on_click,
            )
        })
        .collect();
    let filter = state.filter.map(|filter| {
        let clear = store.clone();
        Chip::new("filter", if filter == Filter::Unread { "Unread" } else { "Starred" })
            .symbol(Symbol::ListFilter)
            .help("Show everything")
            .remove(move |_, _, cx| clear.update(cx, |store, cx| store.set_filter(None, cx)))
    });
    let empty = state.can_empty().then(|| {
        let empty = store.clone();
        ActionButton::new("empty-mailbox", format!("Empty {}", state.mailbox_name()))
            .variant(Variant::Ghost)
            .on_click(move |_, _, cx| empty.update(cx, |store, cx| store.empty_mailbox(cx)))
    });
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(space::M))
            .h(px(38.))
            .w_full()
            .max_w(px(size::CARD_WIDTH))
            .border_b_1()
            .border_color(cx.color(BORDER))
            .child(div().flex_1().min_w_0().px(px(space::L)).child(ui::tab_strip(tabs, cx)))
            .children(filter.map(|chip| div().pr(px(space::L)).child(chip)))
            .children(empty.map(|button| div().pr(px(space::L)).child(button))),
    )
}

impl Render for ThreadList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_selection(cx);
        let store = self.store.read(cx);
        let count = store.visible_rows().len() + usize::from(store.loading_more && store.search_rows.is_none());
        let empty = store.list_empty();
        let message = empty_message(store);
        let searching = store.search_rows.is_some();
        let kept_offset = store.list_offset;
        let offset = self.scroll.0.borrow().base_handle.offset();
        let far_down = -offset.y > window.bounds().size.height;
        let scrolled: f64 = (-f32::from(offset.y)).into();
        if self.restored && (kept_offset - scrolled).abs() > 1. {
            self.store.update(cx, |store, _| store.note_scroll(scrolled));
        }
        let header = list_header(&self.store, cx);
        let has_header = header.is_some();
        let scroll = self.scroll.clone();
        let list =
            uniform_list("threads", count, cx.processor(|this, range, window, cx| this.render_rows(range, window, cx)))
                .track_scroll(&self.scroll)
                .size_full()
                .pt(px(if has_header { 0. } else { LIST_INSET }))
                .pb(px(LIST_INSET));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .when_some(header, |page, header| {
                page.child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .pt(px(space::L))
                        .px(px(space::XL + 4. + size::SELECT_GUTTER))
                        .child(header),
                )
            })
            .child(div().flex_1().min_h_0().relative().child(list).when(empty, |area| {
                area.child(div().absolute().inset_0().child(ui::empty_state(
                    if searching { Symbol::Search } else { Symbol::Inbox },
                    message,
                    cx,
                )))
            }))
            .when(far_down, |page| {
                page.child(
                    div().absolute().top(px(space::M)).left_0().right_0().flex().justify_center().child(
                        ui::FloatingButton::new("to-top", Symbol::ArrowUp, "Back to Top", move |_, _, _| {
                            scroll.0.borrow().base_handle.set_offset(point(px(0.), px(0.)));
                        })
                        .small(),
                    ),
                )
            })
    }
}

/// An empty mailbox's calm line.
fn empty_message(store: &Store) -> &'static str {
    if store.search_rows.is_some() {
        return "Nothing matches.";
    }
    if store.filter == Some(Filter::Unread) || store.in_unread() {
        return "Nothing unread.";
    }
    if store.filter == Some(Filter::Starred) {
        return "Nothing starred.";
    }
    if store.mailbox_syncing() {
        return "Fetching your mail";
    }
    if store.base_mailbox().ends_with("inbox") { "All done. Enjoy the quiet." } else { "Nothing here." }
}
