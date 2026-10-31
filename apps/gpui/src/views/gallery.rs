//! Every component in its states, for working on the design: open the palette and pick
//! "Component gallery".

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, SharedString, Window, div, px};

use crate::models::{Address, ShapeLine};
use crate::store::Store;
use crate::theme::*;
use crate::ui::{self, ActionButton, Chip, ChoiceRow, IconButton, Lead, NavRow, SheetSize, Symbol, Variant};

pub struct Gallery {
    store: Entity<Store>,
    on: bool,
}

impl Gallery {
    pub fn new(store: Entity<Store>) -> Self {
        Self { store, on: true }
    }
}

fn section(title: &str, items: Vec<AnyElement>, cx: &App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::M))
        .child(ui::section_heading(title.to_string(), None, cx))
        .child(div().flex().flex_wrap().items_center().gap(px(space::S)).children(items))
        .into_any_element()
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let buttons = [Variant::Primary, Variant::Success, Variant::Outline, Variant::Ghost, Variant::Destructive]
            .into_iter()
            .enumerate()
            .map(|(index, variant)| {
                ActionButton::new(SharedString::from(format!("gallery-button-{index}")), format!("{variant:?}"))
                    .symbol(Symbol::Send)
                    .variant(variant)
                    .into_any_element()
            })
            .chain([
                ActionButton::new("gallery-small", "Small").size(ControlSize::Small).into_any_element(),
                ActionButton::new("gallery-large", "Large").size(ControlSize::Large).into_any_element(),
                ActionButton::new("gallery-pending", "Pending").pending(true).into_any_element(),
                ActionButton::new("gallery-disabled", "Disabled").disabled(true).into_any_element(),
                ActionButton::new("gallery-icon", "Icon only")
                    .symbol(Symbol::SquarePen)
                    .icon_only(true)
                    .into_any_element(),
                ActionButton::new("gallery-menu", "Choice")
                    .chevron()
                    .menu(|_, _| {
                        vec![ui::MenuEntry::item("One", |_, _| {}).checked(true), ui::MenuEntry::item("Two", |_, _| {})]
                    })
                    .into_any_element(),
            ])
            .collect();
        let icons = vec![
            IconButton::new("gallery-icon-1", Symbol::Archive, "Circled").into_any_element(),
            IconButton::new("gallery-icon-2", Symbol::Trash, "Plain").plain().into_any_element(),
            IconButton::new("gallery-icon-3", Symbol::AlarmClock, "Active").active(true).into_any_element(),
            IconButton::new("gallery-icon-4", Symbol::X, "Quiet").plain().quiet().into_any_element(),
            IconButton::new("gallery-icon-5", Symbol::Menu, "Strong").plain().strong().into_any_element(),
            IconButton::new("gallery-icon-6", Symbol::Send, "Pending").pending(true).into_any_element(),
            ui::StarButton::new("gallery-star", true, |_, _, _| {}).into_any_element(),
            ui::SelectCircle::new("gallery-select", true, "Selected", |_, _, _| {}).into_any_element(),
        ];
        let chips = vec![
            Chip::new("gallery-chip-1", "ann@example.com").remove(|_, _, _| {}).into_any_element(),
            Chip::new("gallery-chip-2", "not an address").invalid(true).remove(|_, _, _| {}).into_any_element(),
            Chip::new("gallery-chip-3", "Receipts").symbol(Symbol::Tag).into_any_element(),
            Chip::new("gallery-chip-4", "report.pdf").symbol(Symbol::Paperclip).detail("1.2 MB").into_any_element(),
        ];
        let toggle = this.clone();
        let misc = vec![
            ui::Avatar::new("AL", "ann@example.com", 34.).into_any_element(),
            ui::spinner(16., cx.color(MUTED_FOREGROUND)).into_any_element(),
            ui::key_caps(crate::keys::caps("mod+k"), cx).into_any_element(),
            ui::key_caps(crate::keys::caps("g i"), cx).into_any_element(),
            ui::switch(
                "gallery-switch",
                self.on,
                move |on, _, cx| {
                    toggle.update(cx, |this, cx| {
                        this.on = on;
                        cx.notify();
                    })
                },
                cx,
            )
            .into_any_element(),
            ui::swatch_row("gallery-swatch", "account-3", Rc::new(|_, _, _| {}), cx).into_any_element(),
        ];
        let rows = vec![
            div()
                .w(px(260.))
                .child(NavRow::new("gallery-nav-1", "Inbox").symbol(Symbol::Inbox).count(3).selected(true))
                .into_any_element(),
            div().w(px(260.)).child(NavRow::new("gallery-nav-2", "Work").dot(ACCOUNT_4)).into_any_element(),
            div()
                .w(px(320.))
                .child(
                    ChoiceRow::new("gallery-choice-1", "Tomorrow", |_, _, _| {})
                        .detail("Tue 8:00 AM")
                        .highlighted(true),
                )
                .into_any_element(),
            div()
                .w(px(320.))
                .child(
                    ChoiceRow::new("gallery-choice-2", "Ann Lee", |_, _, _| {})
                        .lead(Lead::Avatar(Address { name: Some("Ann Lee".into()), email: "ann@example.com".into() }))
                        .checked(true),
                )
                .into_any_element(),
        ];
        let surfaces = vec![
            ui::notice(
                "Images from the sender were left out.",
                ui::Tone::Muted,
                None,
                Some(ActionButton::new("gallery-notice", "Load images")),
                cx,
            )
            .into_any_element(),
            ui::notice("The server couldn't be reached.", ui::Tone::Error, Some(Symbol::CircleAlert), None, cx)
                .into_any_element(),
            ui::toast("Archived.".into(), Some(("Undo".into(), Rc::new(|_, _, _| {}))), cx).into_any_element(),
            ui::banner(
                ui::Banner {
                    symbol: Symbol::CircleArrowDown,
                    tint: PROCESS,
                    title: "Downloading v0.2.0".into(),
                    detail: "42%".into(),
                    progress: Some(0.42),
                    pending: false,
                    action: None,
                    close: None,
                },
                cx,
            )
            .into_any_element(),
            div()
                .w(px(400.))
                .child(ui::body_placeholder(vec![ShapeLine::Text(120), ShapeLine::Text(0), ShapeLine::Text(60)], cx))
                .into_any_element(),
        ];
        let close = self.store.clone();
        let content = div()
            .id("gallery-scroll")
            .size_full()
            .overflow_y_scroll()
            .p(px(space::XL))
            .flex()
            .flex_col()
            .gap(px(space::XXL))
            .child(section("Buttons", buttons, cx))
            .child(section("Icon buttons", icons, cx))
            .child(section("Chips", chips, cx))
            .child(section("Bits", misc, cx))
            .child(section("Rows", rows, cx))
            .child(section("Surfaces", surfaces, cx));
        ui::sheet(
            "gallery",
            "Component gallery",
            SheetSize::Large,
            BACKGROUND,
            Vec::new(),
            content,
            move |_, cx| {
                close.update(cx, |store, cx| {
                    store.gallery_open = false;
                    cx.notify();
                })
            },
            cx,
        )
    }
}
