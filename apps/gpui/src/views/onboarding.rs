//! The first screen: add a mail account, which also signs in. Google and Microsoft open in the
//! browser, which comes back to the app with a `wonnet://` link; JMAP and the server to use are
//! pages of their own. In the settings, the same ways to add an account.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, SharedString, Window, div, px};
use gpui_component::input::{InputEvent, InputState};

use crate::store::Store;
use crate::theme::*;
use crate::ui::{self, ActionButton, Symbol, Variant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Start,
    Jmap,
    Server,
}

pub struct OnboardingView {
    store: Entity<Store>,
    /// The first screen, with the app's name over the choices; else the settings' sheet.
    welcome: bool,
    page: Page,
    url: Entity<InputState>,
    email: Entity<InputState>,
    password: Entity<InputState>,
    server: Entity<InputState>,
    pending: bool,
    error: Option<String>,
}

impl OnboardingView {
    pub fn new(store: Entity<Store>, welcome: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("https://api.fastmail.com"));
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("you@example.com"));
        let password = cx.new(|cx| InputState::new(window, cx).placeholder("App password").masked(true));
        let current = store.read(cx).server.clone();
        let server =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://mail.example.com").default_value(current));
        for field in [&url, &email, &password, &server] {
            cx.subscribe_in(field, window, |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.submit(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            })
            .detach();
        }
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self { store, welcome, page: Page::Start, url, email, password, server, pending: false, error: None }
    }

    fn show(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.error = None;
        match page {
            Page::Jmap => self.url.update(cx, |field, cx| field.focus(window, cx)),
            Page::Server => self.server.update(cx, |field, cx| field.focus(window, cx)),
            Page::Start => {}
        }
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let value = |field: &Entity<InputState>, cx: &Context<Self>| field.read(cx).value().trim().to_string();
        match self.page {
            Page::Jmap => {
                let (url, email, password) =
                    (value(&self.url, cx), value(&self.email, cx), self.password.read(cx).value().to_string());
                if url.is_empty() || email.is_empty() || password.is_empty() {
                    return;
                }
                self.pending = true;
                self.error = None;
                let this = cx.entity().downgrade();
                let password_field = self.password.clone();
                let window_handle = window.window_handle();
                self.store.update(cx, |store, cx| {
                    store.add_jmap(url, email, password, cx, move |_, result, cx| {
                        let added = result.is_ok();
                        let _ = this.update(cx, |this, cx| {
                            this.pending = false;
                            if let Err(error) = result {
                                this.error = Some(error);
                            }
                            cx.notify();
                        });
                        if added {
                            let _ = window_handle.update(cx, |_, window, cx| {
                                password_field.update(cx, |field, cx| field.set_value("", window, cx))
                            });
                        }
                    })
                });
            }
            Page::Server => {
                let url = value(&self.server, cx);
                if url.is_empty() {
                    return;
                }
                self.pending = true;
                self.error = None;
                let this = cx.entity().downgrade();
                self.store.update(cx, |store, cx| {
                    store.set_server(url, cx, move |_, result, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.pending = false;
                            match result {
                                Ok(()) => this.page = Page::Start,
                                Err(error) => this.error = Some(error),
                            }
                            cx.notify();
                        });
                    })
                });
            }
            Page::Start => {}
        }
        cx.notify();
    }

    fn choices(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let google = self.store.clone();
        let microsoft = self.store.clone();
        let signing_in = self.store.read(cx).signing_in.map(|(provider, _)| provider);
        let waiting = "Finish signing in in your browser";
        div()
            .flex()
            .flex_col()
            .gap(px(space::S))
            .w_full()
            .child(option(
                "google",
                "Continue with Google",
                if signing_in == Some("google") { waiting } else { "Gmail and Google Workspace" },
                ui::google_mark(20., cx).into_any_element(),
                Rc::new(move |_, _, cx| google.update(cx, |store, cx| store.sign_in_with_google(cx))),
                cx,
            ))
            .child(option(
                "microsoft",
                "Continue with Microsoft",
                if signing_in == Some("microsoft") { waiting } else { "Outlook, Hotmail and Microsoft 365" },
                ui::microsoft_mark(20., cx).into_any_element(),
                Rc::new(move |_, _, cx| microsoft.update(cx, |store, cx| store.sign_in_with_microsoft(cx))),
                cx,
            ))
            .child(option(
                "jmap",
                "Continue with JMAP",
                "Fastmail, Stalwart and others",
                ui::icon(Symbol::Server, 20.).text_color(cx.color(FOREGROUND)).into_any_element(),
                Rc::new(move |_, window, cx| this.update(cx, |this, cx| this.show(Page::Jmap, window, cx))),
                cx,
            ))
    }

    fn form(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let back = this.clone();
        let submit = this.clone();
        let (title, detail, button) = match self.page {
            Page::Jmap => ("JMAP server", "Any JMAP server, with an app password.".to_string(), "Add account"),
            _ => (
                "Your own server",
                "The app starts on Wonnet's server. Running your own, point this computer at it.".to_string(),
                "Use this server",
            ),
        };
        let fields: Vec<gpui::AnyElement> = match self.page {
            Page::Jmap => vec![
                ui::input_field("jmap-url", Some("Server"), &self.url, cx).into_any_element(),
                ui::input_field("jmap-email", Some("Email"), &self.email, cx).into_any_element(),
                ui::input_field("jmap-password", Some("Password"), &self.password, cx).into_any_element(),
            ],
            _ => vec![ui::input_field("server-url", Some("Server"), &self.server, cx).into_any_element()],
        };
        let disabled = match self.page {
            Page::Jmap => {
                [&self.url, &self.email, &self.password].iter().any(|field| field.read(cx).value().is_empty())
            }
            _ => self.server.read(cx).value().is_empty(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(space::M))
            .w_full()
            .child(
                div().ml(px(-(size::SMALL / 2. + size::BUTTON_GAP / 2.))).child(
                    ActionButton::new("onboarding-back", "Back")
                        .symbol(Symbol::ArrowLeft)
                        .variant(Variant::Ghost)
                        .size(ControlSize::Small)
                        .on_click(move |_, window, cx| back.update(cx, |this, cx| this.show(Page::Start, window, cx))),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::XL))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(space::XXS))
                            .child(div().text(TextStyle::Title, cx).child(title))
                            .child(ui::paragraph_in(detail, TextStyle::Body, MUTED_FOREGROUND, cx)),
                    )
                    .child(div().flex().flex_col().gap(px(space::M)).children(fields))
                    .when_some(self.error.clone(), |form, error| {
                        form.child(ui::notice(error, ui::Tone::Error, None, None, cx))
                    })
                    .child(
                        div().mx(px(-size::BUTTON_GAP / 2.)).flex().child(
                            ActionButton::new("onboarding-submit", button)
                                .variant(Variant::Primary)
                                .pending(self.pending)
                                .disabled(disabled)
                                .wide()
                                .on_click(move |_, window, cx| submit.update(cx, |this, cx| this.submit(window, cx))),
                        ),
                    ),
            )
    }
}

/// A wide button for one of a few ways to do something: a mark, what it is, a line about it.
fn option(
    id: &str,
    title: &str,
    detail: &str,
    mark: gpui::AnyElement,
    on_click: ui::OnClick,
    cx: &gpui::App,
) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .items_center()
        .gap(px(space::M))
        .w_full()
        .min_h(px(size::OPTION))
        .px(px(space::L))
        .rounded(px(radius::MD))
        .bg(cx.color(CARD))
        .border_1()
        .border_color(cx.color(BORDER))
        .cursor_pointer()
        .hover(|button| button.bg(cx.color(CARD_ACCENT)))
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .child(div().size(px(24.)).flex().items_center().justify_center().child(mark))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(ui::line(title.to_string(), TextStyle::BodyStrong, cx))
                .child(ui::line(detail.to_string(), TextStyle::Caption, cx)),
        )
        .child(ui::icon(Symbol::ChevronRight, 14.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)))
}

impl Render for OnboardingView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.page == Page::Start {
            if self.welcome {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(space::XXL + space::S))
                    .w_full()
                    .child(
                        div().flex().flex_col().items_center().gap(px(space::L)).child(ui::app_mark(64., cx)).child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(space::XXS))
                                .child(div().text(TextStyle::Display, cx).child("Wonnet"))
                                .child(
                                    div()
                                        .text_in(TextStyle::Body, MUTED_FOREGROUND, cx)
                                        .child("Email that feels instant."),
                                ),
                        ),
                    )
                    .child(self.choices(cx))
                    .into_any_element()
            } else {
                self.choices(cx).into_any_element()
            }
        } else {
            self.form(cx).into_any_element()
        };
        let column = div()
            .w_full()
            .max_w(px(size::FORM_WIDTH + space::XL * 2.))
            .px(px(space::XL))
            .py(px(space::XXL))
            .child(content);
        if !self.welcome {
            return div()
                .id("add-account")
                .size_full()
                .overflow_y_scroll()
                .flex()
                .justify_center()
                .child(column)
                .into_any_element();
        }
        let this = cx.entity();
        let lift = f32::from(window.bounds().size.height) * 0.08;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.color(BACKGROUND))
            .child(super::topbar::window_bar(Vec::new(), cx))
            .child(
                div()
                    .id("welcome")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .pb(px(lift))
                    .child(column),
            )
            .when(self.page == Page::Start, |page| {
                page.child(
                    div().flex().justify_center().p(px(space::L)).child(
                        ActionButton::new("own-server", "Use your own server")
                            .symbol(Symbol::Server)
                            .variant(Variant::Ghost)
                            .size(ControlSize::Small)
                            .on_click(move |_, window, cx| {
                                this.update(cx, |this, cx| this.show(Page::Server, window, cx))
                            }),
                    ),
                )
            })
            .into_any_element()
    }
}
