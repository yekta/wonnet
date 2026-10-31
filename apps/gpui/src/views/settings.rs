//! The settings over the whole window: their sections on the left, behind a line that drags
//! their width, and the open one's page on the right with its title over it. Back and Esc go
//! back to the mail.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, Context, DragMoveEvent, Entity, MouseButton, Pixels, ScrollHandle, SharedString, Window,
    canvas, div, point, px,
};
use gpui_component::input::{InputEvent, InputState, TextareaState};

use super::Recording;
use super::onboarding::OnboardingView;
use crate::keys::{self, KeyCommand};
use crate::models::AccountView;
use crate::store::{Appearance, CustomSplit, LocalSettings, SettingsEntry, SettingsSection, Snippet, Store};
use crate::theme::*;
use crate::ui::{self, ActionButton, IconButton, Lead, MenuEntry, NavRow, SheetSize, Symbol, Variant};
use crate::updater::{self, UpdateState, Updater};

/// A sheet of the settings: adding an account, a split, a snippet.
enum Sheet {
    AddAccount(Entity<OnboardingView>),
    Split { split: CustomSplit, name: Entity<InputState>, from: Entity<InputState>, new: bool },
    Snippet { snippet: Snippet, name: Entity<InputState>, text: Entity<TextareaState>, new: bool },
}

pub struct SettingsView {
    store: Entity<Store>,
    updater: Entity<Updater>,
    search: Entity<InputState>,
    keys_search: Entity<InputState>,
    names: HashMap<String, Entity<InputState>>,
    /// The account whose signature is being written, and the box it is written in.
    signature: Option<(String, Entity<TextareaState>)>,
    sheet: Option<Sheet>,
    scroll: ScrollHandle,
    groups: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    shown: Option<SettingsSection>,
    /// How many accounts there were, to close Add account once one is added.
    accounts_seen: usize,
}

/// What is dragged to make the sections wider or narrower.
#[derive(Clone)]
struct Divider;

impl Render for Divider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl SettingsView {
    pub fn new(store: Entity<Store>, updater: Entity<Updater>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = store.read(cx).settings_query.clone();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings").default_value(query));
        let keys_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search keybindings"));
        cx.subscribe(&search, |this, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                let query = field.read(cx).value().to_string();
                this.store.update(cx, |store, cx| {
                    store.settings_query = query;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.subscribe(&keys_search, |_, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                cx.notify();
            }
        })
        .detach();
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        Self {
            store,
            updater,
            search,
            keys_search,
            names: HashMap::new(),
            signature: None,
            sheet: None,
            scroll: ScrollHandle::new(),
            groups: Rc::default(),
            shown: None,
            accounts_seen: usize::MAX,
        }
    }

    /// Keeps the account names typed but not yet left with Return or a click elsewhere.
    pub fn save_names(&self, cx: &mut App) {
        for account in self.store.read(cx).accounts() {
            let Some(field) = self.names.get(&account.id) else { continue };
            let text: String = field.read(cx).value().chars().take(AccountView::NAME_MAX).collect();
            if text != account.name {
                self.store.update(cx, |store, cx| store.set_account_name(&account, &text, cx));
            }
        }
    }

    fn close(&self, cx: &mut App) {
        self.store.update(cx, |store, cx| store.close_settings(cx));
    }

    /// The group a search picked, scrolled to once the page is drawn.
    fn scroll_to_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let section = self.store.read(cx).settings;
        if section != self.shown {
            self.shown = section;
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.groups.borrow_mut().clear();
        }
        let Some(target) = self.store.read(cx).settings_target.clone() else { return };
        let target = if target == "signatures" {
            self.store.read(cx).accounts().first().map(|account| account.id.clone()).unwrap_or(target)
        } else {
            target
        };
        let Some(bounds) = self.groups.borrow().get(&target).copied() else {
            window.request_animation_frame();
            return;
        };
        let viewport = self.scroll.bounds();
        let top = bounds.origin.y - viewport.origin.y - self.scroll.offset().y;
        let wanted = (top - px(space::L)).max(px(0.)).min(self.scroll.max_offset().y);
        self.scroll.set_offset(point(px(0.), -wanted));
        self.store.update(cx, |store, _| store.settings_target = None);
    }

    /// Marks where a group is, for a search to scroll to it.
    fn mark(&self, id: &str, element: gpui::Div) -> gpui::Div {
        let groups = self.groups.clone();
        let id = id.to_string();
        element.relative().child(
            canvas(
                move |bounds, _, _| {
                    groups.borrow_mut().insert(id.clone(), bounds);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = self.store.read(cx);
        let section = store.settings.unwrap_or(SettingsSection::General);
        let query = store.settings_query.clone();
        let rows: Vec<AnyElement> = if query.trim().is_empty() {
            SettingsSection::ALL
                .into_iter()
                .map(|page| {
                    let open = self.store.clone();
                    NavRow::new(SharedString::from(format!("section-{page:?}")), page.title())
                        .symbol(page.symbol())
                        .selected(page == section)
                        .vertical_padding(size::ROW_GAP / 2.)
                        .on_click(move |_, _, cx| open.update(cx, |store, cx| store.open_settings(page, None, cx)))
                        .into_any_element()
                })
                .collect()
        } else {
            let found = SettingsEntry::matching(&query);
            if found.is_empty() {
                vec![
                    div()
                        .px(px(space::L + 2.))
                        .py(px(space::S))
                        .text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)
                        .child("No settings found")
                        .into_any_element(),
                ]
            } else {
                found
                    .into_iter()
                    .map(|entry| {
                        let open = self.store.clone();
                        let page = entry.section;
                        let target = entry.id.clone();
                        NavRow::new(SharedString::from(format!("found-{}", entry.id)), entry.title.clone())
                            .symbol(page.symbol())
                            .vertical_padding(size::ROW_GAP / 2.)
                            .trailing(ui::line(page.title(), TextStyle::Caption, cx))
                            .on_click(move |_, _, cx| {
                                open.update(cx, |store, cx| store.open_settings(page, Some(target.clone()), cx))
                            })
                            .into_any_element()
                    })
                    .collect()
            }
        };
        let this = cx.entity();
        let clear = self.search.clone();
        let keys: ui::FieldKeys = Rc::new(move |key, _, cx| {
            if key != "escape" {
                return false;
            }
            this.update(cx, |this, cx| {
                if this.store.read(cx).settings_query.is_empty() {
                    this.close(cx);
                }
            });
            true
        });
        let back = self.store.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().px(px(space::M)).py(px(space::S)).child(ui::search_field(
                "settings-search",
                &self.search,
                ui::SearchStyle::Bar,
                Symbol::Search,
                keys,
                Rc::new(move |window, cx| clear.update(cx, |field, cx| field.set_value("", window, cx))),
                cx,
            )))
            .child(
                div().id("settings-sections").flex_1().min_h_0().overflow_y_scroll().py(px(space::XXS)).children(rows),
            )
            .child(
                div().border_t_1().border_color(cx.color(BORDER)).child(
                    NavRow::new("settings-back", "Back")
                        .symbol(Symbol::ArrowLeft)
                        .vertical_padding(2.)
                        .on_click(move |_, _, cx| back.update(cx, |store, cx| store.close_settings(cx))),
                ),
            )
    }

    // MARK: General

    fn general(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let appearance = store.local.appearance;
        let theme_store = self.store.clone();
        let theme = ActionButton::new("theme", appearance.name()).chevron().menu(move |_, _| {
            Appearance::ALL
                .into_iter()
                .map(|choice| {
                    let store = theme_store.clone();
                    MenuEntry::item(choice.name(), move |window, cx| {
                        store.update(cx, |store, cx| {
                            store.local.appearance = choice;
                            store.local.save();
                            cx.notify();
                        });
                        let _ = window;
                        cx.refresh_windows();
                    })
                    .checked(choice == appearance)
                })
                .collect()
        });
        let server_symbol = if store.connection == "offline" { Symbol::WifiOff } else { Symbol::Server };
        vec![
            self.mark(
                "appearance",
                ui::settings_group(
                    "Appearance",
                    None,
                    vec![
                        ui::settings_row(
                            ui::settings_label("Theme", None, Lead::None, false, cx),
                            vec![theme.into_any_element()],
                        )
                        .into_any_element(),
                    ],
                    cx,
                ),
            )
            .into_any_element(),
            self.mark(
                "updates",
                ui::settings_group(
                    "Updates",
                    Some("New versions are downloaded and installed on their own; a restart finishes them."),
                    vec![self.version_row(cx).into_any_element()],
                    cx,
                ),
            )
            .into_any_element(),
            self.mark(
                "server",
                ui::settings_group(
                    "Server",
                    Some("Wonnet's server syncs your accounts and keeps this computer's copy of the mail current."),
                    vec![
                        ui::settings_row(
                            ui::settings_label(
                                store.server.clone(),
                                Some(store.connection_text().into()),
                                Lead::Symbol(server_symbol),
                                false,
                                cx,
                            ),
                            Vec::new(),
                        )
                        .into_any_element(),
                    ],
                    cx,
                ),
            )
            .into_any_element(),
        ]
    }

    /// The version this is, the offer of a new one, the update as it goes, and the restart.
    fn version_row(&self, cx: &App) -> impl IntoElement + use<> {
        let updater = self.updater.read(cx);
        let status: Option<String> = match &updater.state {
            UpdateState::Idle | UpdateState::Checking => None,
            UpdateState::UpToDate => Some("Up to date.".into()),
            UpdateState::Available(version) => Some(format!("{} is available.", updater::label(version))),
            UpdateState::Downloading(version, fraction) => {
                Some(format!("Downloading {}, {}%", updater::label(version), (fraction * 100.) as u32))
            }
            UpdateState::Installing(version) => Some(format!("Installing {}", updater::label(version))),
            UpdateState::Ready(version) => {
                Some(format!("{} is installed. Restart to use it.", updater::label(version)))
            }
            UpdateState::Failed(message) => Some(message.clone()),
        };
        let handle = self.updater.clone();
        let button = match &updater.state {
            UpdateState::Available(_) => ActionButton::new("settings-update", "Update")
                .variant(Variant::Primary)
                .on_click(move |_, _, cx| handle.update(cx, |updater, cx| updater.install(cx))),
            UpdateState::Ready(_) => ActionButton::new("settings-restart", "Restart")
                .variant(Variant::Success)
                .on_click(move |_, _, cx| handle.update(cx, |updater, cx| updater.relaunch(cx))),
            UpdateState::Failed(_) => ActionButton::new("settings-retry", "Try again")
                .symbol(Symbol::RefreshCw)
                .on_click(move |_, _, cx| handle.update(cx, |updater, cx| updater.retry(cx))),
            state => ActionButton::new("settings-check", "Check for updates")
                .symbol(Symbol::RefreshCw)
                .pending(*state == UpdateState::Checking)
                .on_click(move |_, _, cx| handle.update(cx, |updater, cx| updater.check(true, cx))),
        };
        ui::settings_row(
            ui::settings_label(updater::label(updater::VERSION), status.map(SharedString::from), Lead::None, false, cx),
            vec![button.into_any_element()],
        )
    }

    // MARK: Accounts

    fn accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let accounts = self.store.read(cx).accounts();
        let this = cx.entity();
        let mut items = vec![
            self.mark(
                "accounts",
                ui::section_heading(
                    "Accounts",
                    Some("Each account's colour marks its mail in the list and its mailboxes in the sidebar.".into()),
                    cx,
                )
                .px(px(space::XXS)),
            )
            .into_any_element(),
            div()
                .ml(px(-size::BUTTON_GAP / 2.))
                .child(ActionButton::new("add-account", "Add account").symbol(Symbol::Plus).on_click(
                    move |_, window, cx| {
                        this.update(cx, |this, cx| {
                            let store = this.store.clone();
                            this.sheet =
                                Some(Sheet::AddAccount(cx.new(|cx| OnboardingView::new(store, false, window, cx))));
                            cx.notify();
                        })
                    },
                ))
                .into_any_element(),
        ];
        for account in &accounts {
            if let Some(field) = self.names.get(&account.id) {
                let editing = gpui::Focusable::focus_handle(field.read(cx), cx).is_focused(window);
                if !editing && field.read(cx).value() != account.name {
                    let name = account.name.clone();
                    field.update(cx, |field, cx| field.set_value(name, window, cx));
                }
            } else {
                let name = account.name.clone();
                let field = cx.new(|cx| InputState::new(window, cx).placeholder("Work, Personal").default_value(name));
                let store = self.store.clone();
                let id = account.id.clone();
                cx.subscribe(&field, move |_, field, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        let text: String = field.read(cx).value().chars().take(AccountView::NAME_MAX).collect();
                        let account = store.read(cx).accounts().into_iter().find(|account| account.id == id);
                        if let Some(account) = account {
                            store.update(cx, |store, cx| store.set_account_name(&account, &text, cx));
                        }
                    }
                })
                .detach();
                self.names.insert(account.id.clone(), field);
            }
            items.push(self.mark(&account.id, self.account_card(account, cx)).into_any_element());
        }
        let sign_out = self.store.clone();
        items.push(
            self.mark(
                "sign-out",
                ui::settings_group(
                    "Wonnet",
                    None,
                    vec![
                        ui::settings_row(
                            ui::settings_label(
                                "Sign out",
                                Some("Every account leaves this computer. Signing in again brings them back.".into()),
                                Lead::None,
                                false,
                                cx,
                            ),
                            vec![
                                ActionButton::new("sign-out", "Sign out")
                                    .symbol(Symbol::LogOut)
                                    .variant(Variant::Destructive)
                                    .on_click(move |_, _, cx| {
                                        sign_out.update(cx, |store, cx| store.confirm_signing_out(cx))
                                    })
                                    .into_any_element(),
                            ],
                        )
                        .into_any_element(),
                    ],
                    cx,
                )
                .pt(px(space::S)),
            )
            .into_any_element(),
        );
        items
    }

    fn account_card(&self, account: &AccountView, cx: &mut Context<Self>) -> gpui::Div {
        let store = self.store.read(cx);
        let remove = self.store.clone();
        let removed = account.clone();
        let more =
            IconButton::new(SharedString::from(format!("account-more-{}", account.id)), Symbol::Ellipsis, "More")
                .plain()
                .menu(move |_, _| {
                    let remove = remove.clone();
                    let account = removed.clone();
                    vec![
                        MenuEntry::item("Remove Account", move |_, cx| {
                            remove.update(cx, |store, cx| store.confirm_removing(&account, cx))
                        })
                        .destructive(),
                    ]
                });
        let colour = self.store.clone();
        let id = account.id.clone();
        let swatches = ui::swatch_row(
            &format!("swatch-{}", account.id),
            &account.color,
            Rc::new(move |name, _, cx| colour.update(cx, |store, cx| store.set_account_color(&id, &name, cx))),
            cx,
        );
        let notify = self.store.clone();
        let key = format!("notify:{}", account.id);
        let on = store.flag(&key, true);
        let name_field = self.names.get(&account.id).map(|field| {
            div()
                .w(px(size::SETTINGS_FIELD_WIDTH))
                .child(ui::input_field(&format!("name-{}", account.id), None, field, cx))
                .into_any_element()
        });
        ui::settings_card(
            vec![
                ui::settings_row(
                    ui::settings_label(
                        account.title.clone(),
                        Some(store.subtitle_of(account).into()),
                        Lead::Dot(account_color(&account.color)),
                        true,
                        cx,
                    ),
                    vec![more.into_any_element()],
                )
                .into_any_element(),
                ui::settings_divider(cx).into_any_element(),
                ui::settings_row(
                    ui::settings_label(
                        "Name",
                        Some("Shown in the sidebar in place of the address.".into()),
                        Lead::None,
                        false,
                        cx,
                    ),
                    name_field.into_iter().collect(),
                )
                .into_any_element(),
                ui::settings_divider(cx).into_any_element(),
                ui::settings_row(
                    ui::settings_label("Colour", None, Lead::None, false, cx),
                    vec![swatches.into_any_element()],
                )
                .into_any_element(),
                ui::settings_divider(cx).into_any_element(),
                ui::settings_row(
                    ui::settings_label(
                        "Notifications",
                        Some("New mail makes a notification.".into()),
                        Lead::None,
                        false,
                        cx,
                    ),
                    vec![
                        ui::switch(
                            SharedString::from(format!("notify-{}", account.id)),
                            on,
                            move |on, _, cx| notify.update(cx, |store, cx| store.set_flag(&key, on, true, cx)),
                            cx,
                        )
                        .into_any_element(),
                    ],
                )
                .into_any_element(),
                ui::settings_divider(cx).into_any_element(),
                self.signature_row(account, cx).into_any_element(),
            ],
            cx,
        )
    }

    /// The signature put under what the account sends, and Edit, which opens it in a box.
    fn signature_row(&self, account: &AccountView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = self.store.read(cx);
        let own = store.own_signature(account);
        let provided = store.provided_signature(account);
        let shown = if own.is_empty() { provided.clone() } else { Some(own.clone()) };
        let editing = self.signature.as_ref().filter(|(id, _)| *id == account.id).map(|(_, field)| field.clone());
        let summary: Option<SharedString> = match (&own, &provided, &editing) {
            (_, _, Some(_)) => None,
            (own, _, _) if !own.is_empty() => None,
            (_, None, _) => Some("None. It goes under what you write from this account.".into()),
            (_, Some(_), _) => {
                Some(format!("Kept by {}, until you write your own.", store.provider_name(account)).into())
            }
        };
        let this = cx.entity();
        let start = shown.clone().unwrap_or_default();
        let id = account.id.clone();
        let edit = editing.is_none().then(|| {
            let this = this.clone();
            ActionButton::new(
                SharedString::from(format!("signature-edit-{id}")),
                if shown.is_none() { "Add" } else { "Edit" },
            )
            .symbol(if shown.is_none() { Symbol::Plus } else { Symbol::Pencil })
            .variant(Variant::Ghost)
            .on_click(move |_, window, cx| {
                let start = start.clone();
                let id = id.clone();
                this.update(cx, |this, cx| {
                    let field = cx.new(|cx| TextareaState::new(window, cx).default_value(start));
                    field.update(cx, |field, cx| field.focus(window, cx));
                    this.signature = Some((id, field));
                    cx.notify();
                })
            })
            .into_any_element()
        });
        let below = match editing {
            Some(field) => {
                let save = this.clone();
                let cancel = this.clone();
                let account = account.clone();
                let start = shown.clone().unwrap_or_default();
                let unchanged = field.read(cx).value().as_ref() == start;
                let reading = field.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S))
                    .px(px(space::L))
                    .pb(px(space::L))
                    .child(ui::text_box("signature", None, &field, 5, cx))
                    .when(provided.is_some(), |column| {
                        column.child(ui::notice(
                            format!("Left empty, the one kept by {} is used.", store.provider_name(&account)),
                            ui::Tone::Muted,
                            None,
                            None,
                            cx,
                        ))
                    })
                    .child(div().flex().justify_end().mr(px(-size::BUTTON_GAP / 2.)).child(ui::form_buttons(
                        "signature",
                        "Save",
                        unchanged,
                        Rc::new(move |_, _, cx| {
                            let text = reading.read(cx).value().trim().to_string();
                            save.update(cx, |this, cx| {
                                this.store.update(cx, |store, cx| store.set_signature(&text, &account, cx));
                                this.signature = None;
                                cx.notify();
                            })
                        }),
                        Rc::new(move |_, _, cx| {
                            cancel.update(cx, |this, cx| {
                                this.signature = None;
                                cx.notify();
                            })
                        }),
                    )))
                    .into_any_element()
            }
            None => match shown {
                Some(text) => div()
                    .mx(px(space::L))
                    .mb(px(space::L))
                    .p(px(space::M))
                    .rounded(px(radius::SM))
                    .bg(cx.color(BACKGROUND_SECONDARY))
                    .border_1()
                    .border_color(cx.color(BORDER_CARD))
                    .child(ui::paragraph_in(text, TextStyle::Label, MUTED_FOREGROUND, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
        };
        div()
            .flex()
            .flex_col()
            .child(ui::settings_row(
                ui::settings_label("Signature", summary, Lead::None, false, cx),
                edit.into_iter().collect(),
            ))
            .child(below)
    }

    // MARK: Inbox

    fn inbox(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let splits = store.custom_splits();
        let this = cx.entity();
        let split_on = store.flag("split_inbox", false);
        let toggle = self.store.clone();
        let mut split_rows = vec![
            ui::settings_row(
                ui::settings_label(
                    "Split Inbox",
                    Some("Important mail first, then your splits, then Other: newsletters and notifications.".into()),
                    Lead::None,
                    false,
                    cx,
                ),
                vec![
                    ui::switch(
                        "split-inbox",
                        split_on,
                        move |on, _, cx| toggle.update(cx, |store, cx| store.set_flag("split_inbox", on, false, cx)),
                        cx,
                    )
                    .into_any_element(),
                ],
            )
            .into_any_element(),
            ui::settings_divider(cx).into_any_element(),
        ];
        for split in &splits {
            let edit = this.clone();
            let menu = self.store.clone();
            let menu_edit = this.clone();
            let chosen = split.clone();
            let menu_split = split.clone();
            let order = self.store.clone();
            let target = split.clone();
            split_rows.push(
                movable_row(
                    SharedString::from(format!("split-{}", split.id)),
                    "splits",
                    ui::settings_label(
                        split.name.clone(),
                        Some(store.summary_of(split).into()),
                        Lead::Symbol(Symbol::ListFilter),
                        false,
                        cx,
                    )
                    .into_any_element(),
                    Some(
                        ui::icon(Symbol::ChevronRight, 12.)
                            .text_color(cx.color(MUTED_STRONGER_FOREGROUND))
                            .into_any_element(),
                    ),
                    Rc::new(move |window, cx| {
                        edit.update(cx, |this, cx| this.edit_split(chosen.clone(), false, window, cx))
                    }),
                    Rc::new(move |_, _| {
                        let remove = menu.clone();
                        let edit = menu_edit.clone();
                        let split = menu_split.clone();
                        let edited = split.clone();
                        vec![
                            MenuEntry::item("Edit", move |window, cx| {
                                edit.update(cx, |this, cx| this.edit_split(edited.clone(), false, window, cx))
                            }),
                            MenuEntry::item("Remove", move |_, cx| {
                                remove.update(cx, |store, cx| store.remove_split(&split, cx))
                            })
                            .destructive(),
                        ]
                    }),
                    Rc::new(move |dragged, after, _, cx| {
                        let dragged = dragged.trim_start_matches("split-").to_string();
                        order.update(cx, |store, cx| {
                            let all = store.custom_splits();
                            let Some(moved) = all.iter().find(|split| split.id == dragged).cloned() else { return };
                            let ids = all.iter().map(|split| split.id.clone()).collect();
                            let ids = ui::reordered(ids, &dragged, &target.id, after);
                            let index = ids.iter().position(|id| *id == dragged).unwrap_or(0);
                            store.move_custom_split(&moved, index, cx);
                        });
                    }),
                    cx,
                )
                .into_any_element(),
            );
        }
        let add = this.clone();
        split_rows.push(
            ui::settings_row(
                div().ml(px(-space::L + space::S - size::BUTTON_GAP / 2.)).child(
                    ActionButton::new("add-split", "Add split").symbol(Symbol::Plus).on_click(move |_, window, cx| {
                        add.update(cx, |this, cx| {
                            let split = this.store.read(cx).new_split();
                            this.edit_split(split, true, window, cx);
                        })
                    }),
                ),
                Vec::new(),
            )
            .into_any_element(),
        );
        let circles = store.flag("select_circles", true);
        let circle_toggle = self.store.clone();
        vec![
            self.mark("split-inbox", ui::settings_group("Split Inbox", Some("Your splits come after Important. Drag one to move it."), split_rows, cx)).into_any_element(),
            self.mark("tabs", ui::settings_group("Tabs", Some("The mailboxes kept over the list. Drag a tab to move it; right-click it to remove it."), self.tab_rows(cx), cx)).into_any_element(),
            self.mark(
                "selecting",
                ui::settings_group(
                    "Selecting",
                    None,
                    vec![ui::settings_row(
                        ui::settings_label(
                            "Circles on hover",
                            Some(format!("A circle beside the row under the pointer selects it. Without them, {}-click and Shift-click still do.", if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }).into()),
                            Lead::None,
                            false,
                            cx,
                        ),
                        vec![ui::switch("select-circles", circles, move |on, _, cx| circle_toggle.update(cx, |store, cx| store.set_flag("select_circles", on, true, cx)), cx).into_any_element()],
                    )
                    .into_any_element()],
                    cx,
                ),
            )
            .into_any_element(),
        ]
    }

    fn tab_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let tabs = store.saved_tabs();
        let mut rows = Vec::new();
        if tabs.is_empty() {
            rows.push(ui::settings_note("No tabs. Mailboxes are opened from the sidebar.", cx).into_any_element());
            rows.push(ui::settings_divider(cx).into_any_element());
        }
        for tab in &tabs {
            let remove = self.store.clone();
            let id = tab.id.clone();
            let order = self.store.clone();
            let target = tab.id.clone();
            rows.push(
                movable_row(
                    SharedString::from(format!("saved-tab-{}", tab.id)),
                    "saved-tabs",
                    ui::settings_label(
                        store.tab_name(tab, None),
                        None,
                        Lead::Symbol(Symbol::named(&tab.symbol)),
                        false,
                        cx,
                    )
                    .into_any_element(),
                    None,
                    Rc::new(|_, _| {}),
                    Rc::new(move |_, _| {
                        let remove = remove.clone();
                        let id = id.clone();
                        vec![
                            MenuEntry::item("Remove Tab", move |_, cx| {
                                remove.update(cx, |store, cx| store.toggle_tab(&id, cx))
                            })
                            .destructive(),
                        ]
                    }),
                    Rc::new(move |dragged, after, _, cx| {
                        let dragged = dragged.trim_start_matches("saved-tab-").to_string();
                        order.update(cx, |store, cx| {
                            let ids = store.saved_tabs().into_iter().map(|tab| tab.id).collect();
                            let ids = ui::reordered(ids, &dragged, &target, after);
                            let index = ids.iter().position(|id| *id == dragged).unwrap_or(0);
                            store.move_tab(&dragged, index, cx);
                        });
                    }),
                    cx,
                )
                .into_any_element(),
            );
        }
        let (unified, accounts) = store.mailboxes_for_tabs();
        let adding = self.store.clone();
        let add = ActionButton::new("add-tab", "Add tab").symbol(Symbol::Plus).menu(move |_, _| {
            let item = |mailbox: &crate::models::Mailbox| {
                let store = adding.clone();
                let id = mailbox.id.clone();
                MenuEntry::item(mailbox.name.clone(), move |_, cx| {
                    store.update(cx, |store, cx| store.toggle_tab(&id, cx))
                })
                .symbol(Symbol::named(&mailbox.symbol))
            };
            let mut entries: Vec<MenuEntry> = unified.iter().map(item).collect();
            for (account, mailboxes) in &accounts {
                entries.push(MenuEntry::Heading(account.title.clone().into()));
                entries.extend(mailboxes.iter().map(item));
            }
            entries
        });
        rows.push(
            ui::settings_row(div().ml(px(-space::L + space::S - size::BUTTON_GAP / 2.)).child(add), Vec::new())
                .into_any_element(),
        );
        rows
    }

    fn edit_split(&mut self, split: CustomSplit, new: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Team").default_value(split.name.clone()));
        let from = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ann@example.com, @example.com")
                .default_value(split.from.join(", "))
        });
        name.update(cx, |field, cx| field.focus(window, cx));
        for field in [&name, &from] {
            cx.subscribe(field, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.save_sheet(cx),
                _ => {}
            })
            .detach();
        }
        self.sheet = Some(Sheet::Split { split, name, from, new });
        cx.notify();
    }

    /// Saves the split or the snippet being written, when it has a name, and closes its sheet.
    fn save_sheet(&mut self, cx: &mut Context<Self>) {
        match &self.sheet {
            Some(Sheet::Split { split, name, from, .. }) => {
                let mut split = split.clone();
                split.name = name.read(cx).value().trim().to_string();
                split.from = CustomSplit::senders(from.read(cx).value().as_ref());
                if split.name.is_empty() {
                    return;
                }
                self.store.update(cx, |store, cx| store.save_split(&split, cx));
            }
            Some(Sheet::Snippet { snippet, name, text, .. }) => {
                let mut snippet = snippet.clone();
                snippet.name = name.read(cx).value().to_string();
                snippet.text = text.read(cx).value().to_string();
                if snippet.name.trim().is_empty() {
                    return;
                }
                self.store.update(cx, |store, cx| store.save_snippet(&snippet, cx));
            }
            _ => return,
        }
        self.sheet = None;
        cx.notify();
    }

    // MARK: Reading

    fn reading(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let dark = store.flag("dark_mail", true);
        let images = store.flag("remote_images", true);
        let (dark_toggle, images_toggle) = (self.store.clone(), self.store.clone());
        let blocked = store.blocked_senders();
        let mut blocked_rows: Vec<AnyElement> = Vec::new();
        if blocked.is_empty() {
            blocked_rows.push(ui::settings_note("No one is blocked.", cx).into_any_element());
        }
        for (index, address) in blocked.iter().enumerate() {
            let unblock = self.store.clone();
            let who = address.clone();
            blocked_rows.push(
                ui::settings_row(
                    ui::settings_label(address.clone(), None, Lead::Symbol(Symbol::Ban), false, cx),
                    vec![
                        ActionButton::new(SharedString::from(format!("unblock-{address}")), "Unblock")
                            .variant(Variant::Ghost)
                            .on_click(move |_, _, cx| unblock.update(cx, |store, cx| store.unblock(&who, cx)))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            );
            if index + 1 < blocked.len() {
                blocked_rows.push(ui::settings_divider(cx).into_any_element());
            }
        }
        vec![
            self.mark(
                "mail",
                ui::settings_group(
                    "Mail",
                    None,
                    vec![
                        ui::settings_row(
                            ui::settings_label("Dark mode for mail", Some("In dark mode, newsletters and other designed mail are drawn in dark colours. Each message can show the original.".into()), Lead::None, false, cx),
                            vec![ui::switch("dark-mail", dark, move |on, _, cx| dark_toggle.update(cx, |store, cx| store.set_flag("dark_mail", on, true, cx)), cx).into_any_element()],
                        )
                        .into_any_element(),
                        ui::settings_divider(cx).into_any_element(),
                        ui::settings_row(
                            ui::settings_label("Load remote images", Some("Off, pictures hosted by the sender wait until you ask for them, so nobody learns when you read their mail.".into()), Lead::None, false, cx),
                            vec![ui::switch("remote-images", images, move |on, _, cx| images_toggle.update(cx, |store, cx| store.set_flag("remote_images", on, true, cx)), cx).into_any_element()],
                        )
                        .into_any_element(),
                    ],
                    cx,
                ),
            )
            .into_any_element(),
            self.mark("blocked", ui::settings_group("Blocked senders", Some("Their new mail goes to the trash. Block someone from a message of theirs."), blocked_rows, cx)).into_any_element(),
        ]
    }

    // MARK: Writing

    fn writing(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let delay = store.local.undo_delay;
        let delay_store = self.store.clone();
        let undo =
            ActionButton::new("undo-delay", LocalSettings::undo_delay_name(delay)).chevron().menu(move |_, _| {
                LocalSettings::UNDO_DELAYS
                    .into_iter()
                    .map(|seconds| {
                        let store = delay_store.clone();
                        MenuEntry::item(LocalSettings::undo_delay_name(seconds), move |_, cx| {
                            store.update(cx, |store, cx| {
                                store.local.undo_delay = seconds;
                                store.local.save();
                                cx.notify();
                            })
                        })
                        .checked(seconds == delay)
                    })
                    .collect()
            });
        let this = cx.entity();
        let mut snippet_rows: Vec<AnyElement> = Vec::new();
        for snippet in store.snippets() {
            let edit = this.clone();
            let remove = self.store.clone();
            let chosen = snippet.clone();
            let key = snippet.key();
            snippet_rows.push(
                ui::settings_row(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(ui::line(snippet.name.clone(), TextStyle::Body, cx))
                        .child(ui::line(snippet.text.replace('\n', " "), TextStyle::Caption, cx)),
                    vec![
                        IconButton::new(
                            SharedString::from(format!("snippet-more-{}", snippet.id)),
                            Symbol::Ellipsis,
                            "More",
                        )
                        .plain()
                        .menu(move |_, _| {
                            let edit = edit.clone();
                            let remove = remove.clone();
                            let chosen = chosen.clone();
                            let key = key.clone();
                            vec![
                                MenuEntry::item("Edit", move |window, cx| {
                                    edit.update(cx, |this, cx| this.edit_snippet(chosen.clone(), false, window, cx))
                                }),
                                MenuEntry::item("Remove", move |_, cx| {
                                    remove.update(cx, |store, cx| store.set_preference(&key, None, cx))
                                })
                                .destructive(),
                            ]
                        })
                        .into_any_element(),
                    ],
                )
                .into_any_element(),
            );
            snippet_rows.push(ui::settings_divider(cx).into_any_element());
        }
        let add = this.clone();
        snippet_rows.push(
            ui::settings_row(
                div().ml(px(-space::L + space::S - size::BUTTON_GAP / 2.)).child(
                    ActionButton::new("add-snippet", "Add snippet").symbol(Symbol::Plus).on_click(
                        move |_, window, cx| {
                            add.update(cx, |this, cx| {
                                this.edit_snippet(
                                    Snippet { id: crate::store::uuid(), name: String::new(), text: String::new() },
                                    true,
                                    window,
                                    cx,
                                )
                            })
                        },
                    ),
                ),
                Vec::new(),
            )
            .into_any_element(),
        );
        let hint = format!(
            "Type ; and a name while writing to put one in, or press {}. {{first_name}} becomes the first recipient's first name.",
            keys::label("mod+;")
        );
        vec![
            self.mark(
                "sending",
                ui::settings_group(
                    "Sending",
                    None,
                    vec![
                        ui::settings_row(
                            ui::settings_label(
                                "Undo send",
                                Some("How long a message waits after Send, to be taken back.".into()),
                                Lead::None,
                                false,
                                cx,
                            ),
                            vec![undo.into_any_element()],
                        )
                        .into_any_element(),
                    ],
                    cx,
                ),
            )
            .into_any_element(),
            self.mark("snippets", ui::settings_group("Snippets", Some(&hint), snippet_rows, cx)).into_any_element(),
        ]
    }

    fn edit_snippet(&mut self, snippet: Snippet, new: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("thanks").default_value(snippet.name.clone()));
        let text = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder("Thanks {first_name}!").default_value(snippet.text.clone())
        });
        name.update(cx, |field, cx| field.focus(window, cx));
        cx.subscribe(&name, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.save_sheet(cx),
            _ => {}
        })
        .detach();
        self.sheet = Some(Sheet::Snippet { snippet, name, text, new });
        cx.notify();
    }

    // MARK: Keybindings

    fn keybindings(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let all = keys::all();
        let changed = all.iter().any(|command| store.keys_changed(command));
        let query = self.keys_search.read(cx).value().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let found: Vec<KeyCommand> = all
            .into_iter()
            .filter(|command| {
                let text = format!(
                    "{} {} {}",
                    command.title,
                    command.group,
                    store.keys(command).iter().map(|key| keys::label(key)).collect::<Vec<_>>().join(" ")
                )
                .to_lowercase();
                words.iter().all(|word| text.contains(word))
            })
            .collect();
        let reset = self.store.clone();
        let clear = self.keys_search.clone();
        let this = cx.entity();
        let field_keys: ui::FieldKeys = Rc::new(move |key, _, cx| {
            if key != "escape" {
                return false;
            }
            this.update(cx, |this, cx| {
                if this.keys_search.read(cx).value().is_empty() {
                    this.close(cx);
                }
            });
            true
        });
        let mut items = vec![
            self.mark(
                "keybindings",
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S))
                    .mr(px(-size::BUTTON_GAP / 2.))
                    .child(div().flex_1().child(ui::search_field("keys-search", &self.keys_search, ui::SearchStyle::Bar, Symbol::Search, field_keys, Rc::new(move |window, cx| clear.update(cx, |field, cx| field.set_value("", window, cx))), cx)))
                    .child(ActionButton::new("reset-keys", "Reset all").symbol(Symbol::Undo).variant(Variant::Ghost).disabled(!changed).on_click(move |_, _, cx| reset.update(cx, |store, cx| store.reset_all_keys(cx)))),
            )
            .into_any_element(),
            div()
                .px(px(space::XXS))
                .mt(px(-space::L))
                .child(ui::notice("Click a key to record another in its place, or + to add one. A key given to one command leaves the command that had it.", ui::Tone::Muted, None, None, cx))
                .into_any_element(),
        ];
        for group in keys::GROUPS {
            let rows: Vec<&KeyCommand> = found.iter().filter(|command| command.group == group).collect();
            if rows.is_empty() {
                continue;
            }
            let mut children = Vec::new();
            for (index, command) in rows.iter().enumerate() {
                children.push(
                    self.mark(&format!("key-{}", command.id), div().child(self.key_row(command, cx)))
                        .into_any_element(),
                );
                if index + 1 < rows.len() {
                    children.push(ui::settings_divider(cx).into_any_element());
                }
            }
            items.push(ui::settings_group(group, None, children, cx).into_any_element());
        }
        if found.is_empty() {
            items.push(
                ui::settings_card(vec![ui::settings_note("No keybindings found.", cx).into_any_element()], cx)
                    .into_any_element(),
            );
        }
        items
    }

    fn key_row(&self, command: &KeyCommand, cx: &App) -> impl IntoElement + use<> {
        let store = self.store.read(cx);
        let keys_now = store.keys(command);
        let changed = store.keys_changed(command);
        let mut trailing: Vec<AnyElement> = keys_now
            .iter()
            .map(|key| key_recorder(&self.store, command, Some(key.clone()), cx).into_any_element())
            .collect();
        trailing.push(key_recorder(&self.store, command, None, cx).into_any_element());
        let menu_store = self.store.clone();
        let menu_command = command.clone();
        let menu_keys = keys_now.clone();
        trailing.push(
            IconButton::new(SharedString::from(format!("key-more-{}", command.id)), Symbol::Ellipsis, "More")
                .size(ControlSize::Small)
                .plain()
                .menu(move |_, _| {
                    let reset = menu_store.clone();
                    let command = menu_command.clone();
                    let mut entries = vec![
                        MenuEntry::item("Reset to Default", move |_, cx| {
                            reset.update(cx, |store, cx| store.reset_keys(&command, cx))
                        })
                        .disabled(!changed),
                    ];
                    if !menu_keys.is_empty() {
                        entries.push(MenuEntry::Separator);
                        for key in &menu_keys {
                            let remove = menu_store.clone();
                            let command = menu_command.clone();
                            let key = key.clone();
                            entries.push(
                                MenuEntry::item(format!("Remove {}", keys::label(&key)), move |_, cx| {
                                    remove.update(cx, |store, cx| store.remove_key(&key, &command, cx))
                                })
                                .destructive(),
                            );
                        }
                    }
                    entries
                })
                .into_any_element(),
        );
        ui::settings_row(
            div()
                .flex()
                .items_center()
                .gap(px(space::S))
                .child(ui::line(command.title.clone(), TextStyle::Body, cx))
                .when(changed, |label| label.child(div().text(TextStyle::Footnote, cx).child("Changed"))),
            trailing,
        )
    }
}

/// A key of a keybinding on a button that records another: clicked, it takes the next keys
/// pressed with their modifiers, or two in a row. Esc or a click elsewhere stops it. Without a
/// key it is a plus, to add one.
fn key_recorder(
    store: &Entity<Store>,
    command: &KeyCommand,
    key: Option<String>,
    cx: &App,
) -> impl IntoElement + use<> {
    let id = SharedString::from(format!("record-{}-{}", command.id, key.clone().unwrap_or_else(|| "add".into())));
    let recording =
        cx.try_global::<Recording>().filter(|recording| recording.id == id).map(|recording| recording.strokes.clone());
    let face = match (&recording, &key) {
        (Some(strokes), _) => {
            let text = if strokes.is_empty() {
                "Press keys".to_string()
            } else {
                strokes.iter().map(|stroke| keys::label(stroke)).collect::<Vec<_>>().join(" ")
            };
            div().text_in(TextStyle::Label, PRIMARY, cx).child(text).into_any_element()
        }
        (None, Some(key)) => ui::key_caps(keys::caps(key), cx).into_any_element(),
        (None, None) => ui::icon(Symbol::Plus, 13.).into_any_element(),
    };
    let help = match (&recording, &key) {
        (Some(_), _) => "Press the keys, or Esc to stop".to_string(),
        (None, Some(key)) => format!("Record a key in place of {}", keys::label(key)),
        (None, None) => "Add a key".to_string(),
    };
    let store = store.clone();
    let command = command.clone();
    let start_id = id.clone();
    let active = recording.is_some();
    div()
        .id(id.clone())
        .px(px(size::BUTTON_GAP / 2.))
        .cursor_pointer()
        .tooltip(move |_, cx| ui::Tooltip::view(help.clone(), cx))
        .on_click(move |_, window, cx| {
            if active {
                return;
            }
            let store = store.clone();
            let command = command.clone();
            let replacing = key.clone();
            cx.set_global(Recording {
                strokes: Vec::new(),
                id: start_id.clone(),
                done: Rc::new(move |pressed, _, cx| {
                    let replacing = replacing.clone();
                    store.update(cx, |store, cx| store.set_key(&pressed, &command, replacing.as_deref(), cx))
                }),
            });
            window.refresh();
        })
        .on_mouse_down_out(move |_, window, cx| {
            if cx.try_global::<Recording>().is_some_and(|recording| recording.id == id) {
                cx.remove_global::<Recording>();
                window.refresh();
            }
        })
        .child(
            div()
                .min_w(px(size::SMALL))
                .min_h(px(size::SMALL))
                .px(px(space::XXS))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::SM))
                .text_color(cx.color(MUTED_FOREGROUND))
                .when(active, |face| face.bg(cx.color(BACKGROUND_ACCENT_STRONGER)))
                .when(!active, |face| {
                    face.hover(|face| face.bg(cx.color(BACKGROUND_ACCENT)).text_color(cx.color(FOREGROUND)))
                })
                .child(face),
        )
}

/// A row a settings card's list moves: it hovers, opens on a click, offers a menu on a
/// right-click, and is dragged onto another to take its place.
#[allow(clippy::too_many_arguments)]
fn movable_row(
    id: SharedString,
    group: &'static str,
    leading: AnyElement,
    trailing: Option<AnyElement>,
    clicked: Rc<dyn Fn(&mut Window, &mut App)>,
    menu: Rc<dyn Fn(&mut Window, &mut App) -> Vec<MenuEntry>>,
    dropped: ui::Dropped,
    cx: &App,
) -> impl IntoElement + use<> {
    let menu_id = id.clone();
    let dragged = ui::Dragged { id: id.clone(), title: id.clone() };
    let row = div()
        .id(id)
        .border_b_1()
        .border_color(cx.color(BORDER_CARD))
        .cursor_pointer()
        .hover(|row| row.bg(cx.color(BACKGROUND_ACCENT_LARGER)))
        .on_click(move |_, window, cx| clicked(window, cx))
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            let entries = menu(window, cx);
            ui::open_menu(menu_id.clone(), entries, Some(event.position), window, cx);
        })
        .child(ui::settings_row(leading, trailing.into_iter().collect()));
    ui::movable(row, group.into(), dragged, false, dropped, cx)
}

impl SettingsView {
    fn sheet_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sheet = self.sheet.as_ref()?;
        let this = cx.entity();
        let close = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| {
                this.update(cx, |this, cx| {
                    this.sheet = None;
                    cx.notify();
                })
            }
        };
        let content = match sheet {
            Sheet::AddAccount(view) => ui::sheet(
                "add-account-sheet",
                "Add account",
                SheetSize::Medium,
                BACKGROUND,
                Vec::new(),
                view.clone(),
                close.clone(),
                cx,
            )
            .into_any_element(),
            Sheet::Split { split, name, from, new } => {
                let labels = self.store.read(cx).split_labels();
                let chosen = split.label.clone();
                let chosen_name = chosen
                    .as_ref()
                    .and_then(|id| labels.iter().find(|(label, _)| label == id))
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| "None".into());
                let pick_label = this.clone();
                let label_menu = (!labels.is_empty()).then(|| {
                    let labels = labels.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::XXS))
                        .child(
                            div()
                                .text(TextStyle::Footnote, cx)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("With the label"),
                        )
                        .child(div().ml(px(-size::BUTTON_GAP / 2.)).child(
                            ActionButton::new("split-label", chosen_name).chevron().menu(move |_, _| {
                                let set = |label: Option<String>| {
                                    let this = pick_label.clone();
                                    move |_: &mut Window, cx: &mut App| {
                                        this.update(cx, |this, cx| {
                                            if let Some(Sheet::Split { split, .. }) = &mut this.sheet {
                                                split.label = label.clone();
                                            }
                                            cx.notify();
                                        })
                                    }
                                };
                                let mut entries = vec![MenuEntry::item("None", set(None)).checked(chosen.is_none())];
                                entries.extend(labels.iter().map(|(id, name)| {
                                    MenuEntry::item(name.clone(), set(Some(id.clone())))
                                        .checked(chosen.as_ref() == Some(id))
                                }));
                                entries
                            }),
                        ))
                });
                let save = this.clone();
                let disabled = name.read(cx).value().trim().is_empty();
                let title = if *new { "New split" } else { "Edit split" };
                let save_button = ActionButton::new("split-save", "Save")
                    .variant(Variant::Primary)
                    .disabled(disabled)
                    .on_click(move |_, _, cx| save.update(cx, |this, cx| this.save_sheet(cx)));
                ui::sheet(
                    "split-sheet",
                    title,
                    SheetSize::Small,
                    BACKGROUND,
                    vec![save_button.into_any_element()],
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::M))
                        .p(px(space::XL))
                        .child(ui::input_field("split-name", Some("Name"), name, cx))
                        .child(ui::input_field("split-from", Some("From"), from, cx))
                        .child(ui::notice(
                            "Addresses, or @ and a domain for everyone there.",
                            ui::Tone::Muted,
                            None,
                            None,
                            cx,
                        ))
                        .children(label_menu),
                    close.clone(),
                    cx,
                )
                .into_any_element()
            }
            Sheet::Snippet { name, text, new, .. } => {
                let save = this.clone();
                let disabled = name.read(cx).value().trim().is_empty();
                let title = if *new { "New snippet" } else { "Edit snippet" };
                let save_button = ActionButton::new("snippet-save", "Save")
                    .variant(Variant::Primary)
                    .disabled(disabled)
                    .on_click(move |_, _, cx| save.update(cx, |this, cx| this.save_sheet(cx)));
                ui::sheet(
                    "snippet-sheet",
                    title,
                    SheetSize::Small,
                    BACKGROUND,
                    vec![save_button.into_any_element()],
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::M))
                        .p(px(space::XL))
                        .child(ui::input_field("snippet-name", Some("Name"), name, cx))
                        .child(ui::text_box("snippet-text", Some("Text"), text, 8, cx)),
                    close.clone(),
                    cx,
                )
                .into_any_element()
            }
        };
        Some(ui::scrim(content, close, cx).into_any_element())
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts_now = self.store.read(cx).accounts().len();
        if accounts_now > self.accounts_seen && matches!(self.sheet, Some(Sheet::AddAccount(_))) {
            self.sheet = None;
        }
        self.accounts_seen = accounts_now;
        self.scroll_to_target(window, cx);
        let section = self.store.read(cx).settings.unwrap_or(SettingsSection::General);
        let groups = match section {
            SettingsSection::General => self.general(cx),
            SettingsSection::Accounts => self.accounts(window, cx),
            SettingsSection::Inbox => self.inbox(cx),
            SettingsSection::Reading => self.reading(cx),
            SettingsSection::Writing => self.writing(cx),
            SettingsSection::Keybindings => self.keybindings(cx),
        };
        let width =
            self.store.read(cx).local.settings_sidebar.clamp(size::SETTINGS_SIDEBAR_MIN, size::SETTINGS_SIDEBAR_MAX);
        let back = self.store.clone();
        let resize = self.store.clone();
        let sheet = self.sheet_view(cx);
        let save = self.store.clone();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.color(BACKGROUND))
            .on_drop(move |_: &Divider, _, cx| save.read(cx).local.save())
            .child(super::topbar::window_bar(
                vec![
                    div()
                        .w(px(width - super::topbar::LEADING))
                        .flex_none()
                        .child(
                            ActionButton::new("settings-back-bar", "Back")
                                .symbol(Symbol::ArrowLeft)
                                .variant(Variant::Ghost)
                                .tooltip("Back to the mail (Esc)")
                                .on_click(move |_, _, cx| back.update(cx, |store, cx| store.close_settings(cx))),
                        )
                        .into_any_element(),
                    div()
                        .flex_1()
                        .flex()
                        .justify_center()
                        .child(div().text_in(TextStyle::LabelStrong, MUTED_FOREGROUND, cx).child(section.title()))
                        .into_any_element(),
                    div().w(px(width - super::topbar::LEADING)).flex_none().into_any_element(),
                ],
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .on_drag_move::<Divider>(move |event: &DragMoveEvent<Divider>, _, cx| {
                        let wanted = f32::from(event.event.position.x)
                            .clamp(size::SETTINGS_SIDEBAR_MIN, size::SETTINGS_SIDEBAR_MAX);
                        resize.update(cx, |store, cx| {
                            store.local.settings_sidebar = wanted;
                            cx.notify();
                        });
                    })
                    .child(div().w(px(width)).flex_none().h_full().child(self.sidebar(cx)))
                    .child(
                        div().relative().w(px(size::HAIRLINE)).h_full().flex_none().bg(cx.color(BORDER)).child(
                            div()
                                .id("settings-divider")
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(px((size::HAIRLINE - size::DIVIDER_GRAB) / 2.))
                                .w(px(size::DIVIDER_GRAB))
                                .cursor_col_resize()
                                .on_drag(Divider, |_, _, _, cx| cx.new(|_| Divider)),
                        ),
                    )
                    .child(
                        div()
                            .id("settings-page")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div().w_full().flex().justify_center().px(px(space::XL)).child(
                                    div()
                                        .w_full()
                                        .max_w(px(size::SETTINGS_PAGE))
                                        .pt(px(space::XL))
                                        .pb(px(space::XXL * 2.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(if section == SettingsSection::Accounts {
                                            space::XL
                                        } else {
                                            space::XXL
                                        }))
                                        .children(groups),
                                ),
                            ),
                    ),
            )
            .children(sheet)
    }
}
