//! Someone the thread is with: who they are, a way to write to them, and the latest threads with
//! them. Beside the thread on a wide window; a sheet on a narrow one.

use gpui::prelude::*;
use gpui::{Context, Entity, SharedString, Window, div, px};
use mail_core::api::Command;

use crate::models::{Address, Person};
use crate::store::Store;
use crate::theme::*;
use crate::ui::{self, ActionButton, Avatar, NavRow, Symbol, Variant};

pub struct PersonView {
    store: Entity<Store>,
    pub email: String,
    person: Option<Person>,
}

impl PersonView {
    pub fn new(store: Entity<Store>, email: String, cx: &mut Context<Self>) -> Self {
        let answer = store.read(cx).core.call::<Person>(Command::Person { email: email.clone() });
        cx.spawn(async move |this, cx| {
            let person = answer.await.ok();
            let _ = this.update(cx, |this, cx| {
                this.person = person;
                cx.notify();
            });
        })
        .detach();
        Self { store, email, person: None }
    }

    fn name(&self) -> String {
        match &self.person {
            Some(person) if !person.name.is_empty() => person.name.clone(),
            _ => self.email.clone(),
        }
    }
}

impl Render for PersonView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let initials = self.person.as_ref().map(|person| person.initials.clone()).unwrap_or_else(|| {
            self.email.chars().next().map(|first| first.to_uppercase().collect()).unwrap_or_default()
        });
        let write = self.store.clone();
        let address = Address {
            name: self.person.as_ref().map(|person| person.name.clone()).filter(|name| !name.is_empty()),
            email: self.email.clone(),
        };
        let threads = self.person.as_ref().map(|person| person.threads.clone()).unwrap_or_default();
        let rows = threads.into_iter().map(|row| {
            let open = self.store.clone();
            let id = row.id.clone();
            let subject = if row.subject.is_empty() { "(no subject)".to_string() } else { row.subject.clone() };
            NavRow::new(SharedString::from(format!("person-{}", row.id)), subject)
                .inset(space::XL)
                .detail(row.date.clone())
                .on_card()
                .on_click(move |_, _, cx| {
                    open.update(cx, |store, cx| {
                        store.contact = None;
                        store.open(&id, cx);
                    })
                })
        });
        let rows: Vec<_> = rows.collect();
        let header = div()
            .flex()
            .flex_col()
            .gap(px(space::L))
            .px(px(space::XL))
            .pt(px(space::XL))
            .child(Avatar::new(&initials, &self.email, 52.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::XXS))
                    .child(div().text(TextStyle::Heading, cx).child(self.name()))
                    .child(div().text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx).child(self.email.clone())),
            )
            .child(
                div().ml(px(-size::BUTTON_GAP / 2.)).child(
                    ActionButton::new("person-write", "New message")
                        .symbol(Symbol::SquarePen)
                        .variant(Variant::Outline)
                        .on_click(move |_, _, cx| {
                            let address = address.clone();
                            write.update(cx, |store, cx| {
                                store.contact = None;
                                store.write_to(address, cx);
                            })
                        }),
                ),
            );
        let recent = (!rows.is_empty()).then(|| {
            div()
                .pt(px(space::L + space::S))
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(space::XL))
                        .pb(px(space::M - size::ROW_GAP / 2.))
                        .child(ui::section_heading("Recent", None, cx)),
                )
                .children(rows)
        });
        div()
            .id("person")
            .size_full()
            .overflow_y_scroll()
            .bg(cx.color(CARD))
            .child(div().flex().flex_col().pb(px(space::XL)).child(header).children(recent))
    }
}
