//! The mailboxes of every account together, then each account with its colour, then settings,
//! sliding over the page from the left. The mailboxes are dragged into the order wanted, and so
//! are the accounts; both orders are synced.

use gpui::prelude::*;
use gpui::{App, Entity, MouseButton, SharedString, Window, div, px};

use crate::models::Mailbox;
use crate::store::{SettingsSection, Store};
use crate::theme::*;
use crate::ui::{self, MenuEntry, NavRow, Symbol};

pub fn sidebar(store: Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let close = store.clone();
    div()
        .absolute()
        .inset_0()
        .child(div().id("sidebar-scrim").absolute().inset_0().bg(cx.tint(OVERLAY, opacity::OVERLAY)).on_mouse_down(
            MouseButton::Left,
            move |_, _, cx| {
                close.update(cx, |store, cx| {
                    store.sidebar_open = false;
                    cx.notify();
                })
            },
        ))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(size::SIDEBAR_WIDTH))
                .flex()
                .flex_col()
                .bg(cx.color(BACKGROUND))
                .border_r_1()
                .border_color(cx.color(BORDER))
                .occlude()
                .child(super::topbar::window_bar(vec![close_button(&store).into_any_element()], cx))
                .child(rows(&store, cx))
                .child(footer(&store, cx)),
        )
}

fn close_button(store: &Entity<Store>) -> impl IntoElement + use<> {
    let store = store.clone();
    ui::IconButton::new("sidebar-menu", Symbol::Menu, "Mailboxes").plain().on_click(move |_, _, cx| {
        store.update(cx, |store, cx| {
            store.sidebar_open = false;
            cx.notify();
        })
    })
}

fn pick(store: &Entity<Store>, mailbox: &str) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let store = store.clone();
    let mailbox = mailbox.to_string();
    move |_, _, cx| {
        store.update(cx, |store, cx| {
            store.select_mailbox(&mailbox, cx);
            store.sidebar_open = false;
        })
    }
}

fn tab_menu(store: &Entity<Store>, mailbox: &str) -> impl Fn(&mut Window, &mut App) -> Vec<MenuEntry> + 'static {
    let store = store.clone();
    let mailbox = mailbox.to_string();
    move |_, cx| {
        let title = if store.read(cx).is_tab(&mailbox) { "Remove from Tabs" } else { "Add to Tabs" };
        let store = store.clone();
        let mailbox = mailbox.clone();
        vec![MenuEntry::item(title, move |_, cx| store.update(cx, |store, cx| store.toggle_tab(&mailbox, cx)))]
    }
}

fn mailbox_row(store: &Entity<Store>, mailbox: &Mailbox, indent: f32, cx: &App) -> NavRow {
    let state = store.read(cx);
    let selected = state.base_mailbox() == mailbox.id && state.search_rows.is_none();
    NavRow::new(SharedString::from(format!("side-{}", mailbox.id)), mailbox.name.clone())
        .symbol(Symbol::named(&mailbox.symbol))
        .count(mailbox.unread)
        .selected(selected)
        .indent(indent)
        .on_click(pick(store, &mailbox.id))
        .menu(tab_menu(store, &mailbox.id))
}

fn rows(store: &Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let state = store.read(cx);
    let unified = state.unified();
    let accounts = state.accounts();
    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    for mailbox in &unified {
        let order = store.clone();
        let target = mailbox.id.clone();
        rows.push(
            mailbox_row(store, mailbox, 0., cx)
                .movable("mailboxes", move |dragged, after, _, cx| {
                    let dragged = dragged.trim_start_matches("side-").to_string();
                    order.update(cx, |store, cx| {
                        let ids = ui::reordered(
                            store.unified().into_iter().map(|mailbox| mailbox.id).collect(),
                            &dragged,
                            &target,
                            after,
                        );
                        store.set_sidebar_order_mailboxes(ids, cx);
                    });
                })
                .into_any_element(),
        );
    }
    if !accounts.is_empty() {
        rows.push(
            div()
                .px(px(18.))
                .pt(px(space::XL + 2.))
                .pb(px(space::XS))
                .child(ui::section_heading("Accounts", None, cx))
                .into_any_element(),
        );
    }
    for account in &accounts {
        let expanded = state.expanded.contains(&account.id);
        let toggle = store.clone();
        let id = account.id.clone();
        let order = store.clone();
        let target = account.id.clone();
        let status = match account.status.as_str() {
            "syncing" => Some(ui::spinner(13., cx.color(MUTED_STRONGER_FOREGROUND)).into_any_element()),
            "reauth" | "error" => {
                Some(ui::icon(Symbol::CircleAlert, 13.).text_color(cx.color(DESTRUCTIVE)).into_any_element())
            }
            _ => None,
        };
        let mut row = NavRow::new(SharedString::from(format!("side-account-{}", account.id)), account.title.clone())
            .dot(account_color(&account.color))
            .on_click(move |_, _, cx| {
                toggle.update(cx, |store, cx| {
                    if !store.expanded.remove(&id) {
                        store.expanded.insert(id.clone());
                    }
                    cx.notify();
                })
            })
            .movable("accounts", move |dragged, after, _, cx| {
                let dragged = dragged.trim_start_matches("side-account-").to_string();
                order.update(cx, |store, cx| {
                    let ids = ui::reordered(
                        store.accounts().into_iter().map(|account| account.id).collect(),
                        &dragged,
                        &target,
                        after,
                    );
                    store.set_sidebar_order_accounts(ids, cx);
                });
            });
        if let Some(status) = status {
            row = row.trailing(status);
        }
        let chevron = if expanded { Symbol::ChevronDown } else { Symbol::ChevronRight };
        rows.push(
            row.trailing(ui::icon(chevron, 12.).text_color(cx.color(MUTED_STRONGEST_FOREGROUND))).into_any_element(),
        );
        if expanded {
            for mailbox in &account.mailboxes {
                rows.push(mailbox_row(store, mailbox, space::L, cx).into_any_element());
            }
        }
    }
    div()
        .id("sidebar-rows")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .py(px(space::S))
        .pb(px(size::SIDEBAR_END))
        .children(rows)
}

fn footer(store: &Entity<Store>, cx: &App) -> impl IntoElement + use<> {
    let offline = store.read(cx).connection == "offline";
    let open = store.clone();
    let row = NavRow::new("side-settings", "Settings")
        .symbol(Symbol::Settings)
        .vertical_padding(2.)
        .on_click(move |_, _, cx| open.update(cx, |store, cx| store.open_settings(SettingsSection::General, None, cx)));
    let row = if offline {
        row.trailing(
            div()
                .id("offline")
                .tooltip(|_, cx| ui::Tooltip::view("Offline", cx))
                .child(ui::icon(Symbol::WifiOff, 14.).text_color(cx.color(MUTED_STRONGER_FOREGROUND))),
        )
    } else {
        row
    };
    div().border_t_1().border_color(cx.color(BORDER)).child(row)
}
