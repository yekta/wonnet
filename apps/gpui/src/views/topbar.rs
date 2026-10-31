//! The bar on top: the mailboxes' button, the mailbox's name, its tabs, search and compose over
//! the list; back and the thread's actions over a thread; what can be done to a selection.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, MouseButton, SharedString, Window, WindowControlArea, div, px, svg};

use super::MailWindow;
use crate::store::{Store, ThreadCommand};
use crate::theme::*;
use crate::ui::{self, ActionButton, IconButton, MenuEntry, PillTab, Symbol, Variant};

/// Where the bar's own controls start: past the window's buttons on a Mac.
pub const LEADING: f32 = if cfg!(target_os = "macos") { 88. } else { space::S };

/// The window's bar: dragged to move the window, double-clicked to zoom it; on Windows and
/// Linux it ends with the window's buttons, on a Mac the window's buttons sit before it.
pub fn window_bar(children: Vec<AnyElement>, cx: &App) -> impl IntoElement + use<> {
    let drag = div()
        .id("window-drag")
        .absolute()
        .inset_0()
        .when(!cfg!(target_os = "windows"), |area| area.window_control_area(WindowControlArea::Drag))
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            if window.default_prevented() {
                return;
            }
            if event.click_count == 2 {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
                return;
            }
            #[cfg(target_os = "windows")]
            crate::platform::drag_window(window);
            #[cfg(not(target_os = "windows"))]
            window.start_window_move();
        })
        .when(cfg!(target_os = "linux"), |area| {
            area.on_mouse_down(MouseButton::Right, |event, window, _| window.show_window_menu(event.position))
        });
    div()
        .relative()
        .flex_none()
        .h(px(size::BAR))
        .w_full()
        .flex()
        .items_center()
        .bg(cx.color(BACKGROUND))
        .border_b_1()
        .border_color(cx.color(BORDER))
        .child(drag)
        .child(div().relative().flex().items_center().flex_1().min_w_0().h_full().pl(px(LEADING)).children(children))
        .when(!cfg!(target_os = "macos"), |bar| bar.child(window_controls(cx)))
}

/// Minimize, maximize and close, at the end of the bar where the system draws none.
fn window_controls(cx: &App) -> impl IntoElement + use<> {
    let control = |id: &'static str, icon: &'static str, area: WindowControlArea, close: bool| {
        let hover = if close { cx.color(DESTRUCTIVE) } else { cx.color(BACKGROUND_ACCENT) };
        let hover_text = if close { cx.color(DESTRUCTIVE_FOREGROUND) } else { cx.color(FOREGROUND) };
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .group(id)
            .hover(move |button| button.bg(hover))
            .window_control_area(area)
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, _| {
                if !cfg!(target_os = "linux") {
                    return;
                }
                match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    _ => window.remove_window(),
                }
            })
            .child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .flex_none()
                    .text_color(cx.color(MUTED_FOREGROUND))
                    .group_hover(id, move |glyph| glyph.text_color(hover_text)),
            )
    };
    div()
        .flex()
        .h_full()
        .child(control("window-min", "icons/window-minimize.svg", WindowControlArea::Min, false))
        .child(control("window-max", "icons/window-maximize.svg", WindowControlArea::Max, false))
        .child(control("window-close", "icons/window-close.svg", WindowControlArea::Close, true))
}

pub fn top_bar(
    window_view: &mut MailWindow,
    window: &mut Window,
    cx: &mut Context<MailWindow>,
) -> impl IntoElement + use<> {
    let store = window_view.store.clone();
    let state = store.read(cx);
    let open = state.conversation.clone();
    let selecting = open.is_none() && !state.selection.is_empty();
    let toggle = store.clone();
    let mut leading: Vec<AnyElement> = vec![
        IconButton::new("menu", Symbol::Menu, "Mailboxes")
            .plain()
            .on_click(move |_, _, cx| {
                toggle.update(cx, |store, cx| {
                    store.sidebar_open = !store.sidebar_open;
                    cx.notify();
                })
            })
            .into_any_element(),
    ];
    if open.is_some() && !state.sidebar_open {
        let back = store.clone();
        leading.push(
            IconButton::new("back", Symbol::ArrowLeft, "Back (Esc)")
                .on_click(move |_, _, cx| back.update(cx, |store, cx| store.close(cx)))
                .into_any_element(),
        );
    }
    let bar = if selecting {
        leading.push(selection_count(&store, cx).into_any_element());
        ui::header_bar(None, leading, None, vec![selection_actions(&store, window, cx).into_any_element()], cx)
    } else if let Some(open) = open {
        let compose = store.clone();
        let trailing = vec![compose_button(compose, state.key_help("Compose", "compose"), window).into_any_element()];
        ui::header_bar(None, leading, Some(thread_actions(&store, &open.id, cx).into_any_element()), trailing, cx)
    } else {
        let compose_help = state.key_help("Compose", "compose");
        leading.push(mailbox_title(&store, cx).into_any_element());
        let tabs = mailbox_tabs(&store, cx).into_any_element();
        let search = search_field(window_view, cx).into_any_element();
        let trailing = vec![
            div().w(px(160.)).child(search).into_any_element(),
            compose_button(store.clone(), compose_help, window).into_any_element(),
        ];
        ui::header_bar(None, leading, Some(tabs), trailing, cx)
    };
    window_bar(vec![bar.flex_1().px(px(0.)).pr(px(space::S)).into_any_element()], cx)
}

/// Compose: its words when there is room, its icon alone when there is not.
fn compose_button(store: Entity<Store>, help: String, window: &Window) -> ActionButton {
    let narrow = window.bounds().size.width < px(1060.);
    ActionButton::new("compose", "Compose")
        .symbol(Symbol::SquarePen)
        .variant(Variant::Outline)
        .icon_only(narrow)
        .tooltip(help)
        .on_click(move |_, _, cx| store.update(cx, |store, cx| store.new_message(cx)))
}

/// "Inbox · Work", the dot fainter, with a spinner after it while its mail is still coming.
fn mailbox_title(store: &Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let (name, account) = state.base_mailbox_title();
    let syncing = state.mailbox_syncing();
    div()
        .flex()
        .items_center()
        .gap(px(space::XS))
        .min_w_0()
        .ml(px(-space::XXS))
        .child(
            div()
                .text(TextStyle::Subheading, cx)
                .truncate()
                .child(name)
                .when_some(account, |title, account| {
                    title.child(div().text_color(cx.color(MUTED_FOREGROUND)).child(" · ")).child(account)
                })
                .flex(),
        )
        .child(match syncing {
            true => ui::spinner(14., cx.color(MUTED_STRONGER_FOREGROUND)).into_any_element(),
            false => div().size(px(14.)).into_any_element(),
        })
}

/// The mailboxes kept as tabs over the list: dragged to move, right-clicked to take off.
fn mailbox_tabs(store: &Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let selected = state.search_rows.is_none().then(|| state.base_mailbox());
    let shown_in = state.list_account();
    let tabs = state
        .tabs()
        .into_iter()
        .map(|tab| {
            let pick = store.clone();
            let menu = store.clone();
            let id = tab.id.clone();
            let saved = state.saved_tab(&tab.id);
            PillTab {
                id: SharedString::from(format!("tab-{}", tab.id)),
                symbol: Symbol::named(&tab.symbol),
                title: state.tab_name(&tab, shown_in.as_deref()).into(),
                count: tab.unread,
                chosen: selected.as_deref() == Some(tab.id.as_str()),
                on_click: Rc::new(move |_, _, cx| pick.update(cx, |store, cx| store.select_mailbox(&id, cx))),
                menu: Rc::new(move |_, _| {
                    let menu = menu.clone();
                    let saved = saved.clone();
                    vec![MenuEntry::item("Remove from Tabs", move |_, cx| {
                        menu.update(cx, |store, cx| store.toggle_tab(&saved, cx))
                    })]
                }),
            }
        })
        .collect();
    let order = store.clone();
    let moved =
        Rc::new(move |dragged: &SharedString, target: &SharedString, after: bool, _: &mut Window, cx: &mut App| {
            let dragged = dragged.trim_start_matches("tab-").to_string();
            let target = target.trim_start_matches("tab-").to_string();
            order.update(cx, |store, cx| {
                let ids = ui::reordered(store.tabs().into_iter().map(|tab| tab.id).collect(), &dragged, &target, after);
                let saved = ids.iter().map(|id| store.saved_tab(id)).collect();
                store.set_tabs(saved, cx);
            });
        });
    ui::pill_tabs(tabs, moved, cx)
}

/// The search field of the top bar; its words search as they are typed.
fn search_field(window_view: &MailWindow, cx: &mut Context<MailWindow>) -> impl IntoElement + use<> {
    let this = cx.entity();
    let clear = this.clone();
    let keys: ui::FieldKeys = Rc::new(move |key, window, cx| {
        if key != "escape" {
            return false;
        }
        this.update(cx, |this, cx| this.clear_search(window, cx));
        true
    });
    ui::search_field(
        "search",
        &window_view.search,
        ui::SearchStyle::Bar,
        Symbol::Search,
        keys,
        Rc::new(move |window, cx| clear.update(cx, |this, cx| this.clear_search(window, cx))),
        cx,
    )
}

/// What can be done to the open thread: archive, trash, snooze, label, mark unread, and more.
fn thread_actions(store: &Entity<Store>, thread: &str, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let threads = vec![thread.to_string()];
    let buttons = [
        ThreadCommand::Archive,
        ThreadCommand::Trash,
        ThreadCommand::Snooze,
        ThreadCommand::Label,
        ThreadCommand::Read,
    ]
    .into_iter()
    .map(|command| {
        let run = store.clone();
        let threads = threads.clone();
        let help = state.key_help(&command.title(state, &threads), command.id());
        IconButton::new(SharedString::from(format!("thread-{}", command.id())), command.symbol(), help)
            .on_click(move |_, _, cx| run.update(cx, |store, cx| store.run(command, Some(threads.clone()), cx)))
            .into_any_element()
    });
    let more = {
        let store = store.clone();
        let thread = thread.to_string();
        IconButton::new("thread-more", Symbol::Ellipsis, "More")
            .menu(move |_, cx| command_entries(&store, &ThreadCommand::MORE, std::slice::from_ref(&thread), cx))
    };
    div().flex().items_center().children(buttons).child(more)
}

/// Commands as the entries of a menu.
pub fn command_entries(
    store: &Entity<Store>,
    commands: &[ThreadCommand],
    threads: &[String],
    cx: &App,
) -> Vec<MenuEntry> {
    let state = store.read(cx);
    state
        .applicable(commands, threads)
        .into_iter()
        .map(|command| {
            let run = store.clone();
            let threads = threads.to_vec();
            let destructive = command == ThreadCommand::DeleteForever;
            let entry = MenuEntry::item(command.title(state, &threads), move |_, cx| {
                run.update(cx, |store, cx| store.run(command, Some(threads.clone()), cx))
            })
            .symbol(command.symbol());
            if destructive { entry.destructive() } else { entry }
        })
        .collect()
}

/// How many threads are selected, after the circle that selects all of them or none.
fn selection_count(store: &Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let all = state.selection.len() == state.visible_rows().len();
    let help = if all { "Select none".to_string() } else { state.key_help("Select all", "selectAll") };
    let toggle = store.clone();
    div()
        .flex()
        .items_center()
        .child(ui::SelectCircle::new("select-all", all, help, move |_, _, cx| {
            toggle.update(cx, |store, cx| if all { store.clear_selection(cx) } else { store.select_all(cx) })
        }))
        .child(
            div()
                .text(TextStyle::LabelStrong, cx)
                .whitespace_nowrap()
                .child(format!("{} selected", state.selection.len())),
        )
}

/// What can be done to all the selected threads, named while there is room, and the way out.
fn selection_actions(store: &Entity<Store>, window: &Window, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let threads = state.targets();
    let named = window.bounds().size.width > px(1000.);
    let buttons = state.applicable(&ThreadCommand::SELECTION, &threads).into_iter().map(|command| {
        let run = store.clone();
        let threads = threads.clone();
        let title = command.title(state, &threads);
        let help = state.key_help(&title, command.id());
        let id = SharedString::from(format!("selection-{}", command.id()));
        let action = move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            run.update(cx, |store, cx| store.run(command, Some(threads.clone()), cx))
        };
        if named {
            ActionButton::new(id, title)
                .symbol(command.symbol())
                .variant(Variant::Ghost)
                .tooltip(help)
                .on_click(action)
                .into_any_element()
        } else {
            IconButton::new(id, command.symbol(), help).plain().on_click(action).into_any_element()
        }
    });
    let more_commands = state.applicable(&ThreadCommand::selection_more(), &threads);
    let more = (!more_commands.is_empty()).then(|| {
        let store = store.clone();
        let threads = threads.clone();
        IconButton::new("selection-more", Symbol::Ellipsis, "More")
            .plain()
            .menu(move |_, cx| command_entries(&store, &more_commands, &threads, cx))
    });
    let clear = store.clone();
    div().flex().items_center().children(buttons).children(more).child(
        IconButton::new("selection-clear", Symbol::X, "Clear the selection (Esc)")
            .plain()
            .on_click(move |_, _, cx| clear.update(cx, |store, cx| store.clear_selection(cx))),
    )
}
