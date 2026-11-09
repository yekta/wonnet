//! The sheet a `wonnet://connect` link opens over everything: who is asking, which mail it may
//! reach and what it may do with it, and Approve or Deny. Closed any other way, it denies.

use gpui::prelude::*;
use gpui::{AnyElement, App, ClickEvent, Div, Entity, SharedString, Window, div, px};

use crate::models::{AccessChoice, ConnectRequest};
use crate::store::{ConnectChoices, ConnectState, Store};
use crate::theme::*;
use crate::ui::{self, ActionButton, Chip, Lead, Mark, MarkRow, SheetSize, Symbol, Tone, Variant};

/// The sheet over its scrim, while a request is open.
pub fn connect_sheet(store: &Entity<Store>, cx: &App) -> Option<AnyElement> {
    let sheet = store.read(cx).connect.as_ref()?;
    let closing = store.clone();
    let close = move |_: &mut Window, cx: &mut App| closing.update(cx, |store, cx| store.close_connect(cx));
    let (title, content) = match &sheet.state {
        ConnectState::Loading => ("Connect an application".to_string(), loading(cx)),
        ConnectState::Failed(error) => ("Connect an application".to_string(), failed(error, store, cx)),
        ConnectState::Asking(choices) => (format!("Connect {}", choices.request.client), asking(choices, store, cx)),
        ConnectState::Approved(client) => (format!("Connect {client}"), approved(client, store, cx)),
    };
    let sheet = ui::sheet("connect", title, SheetSize::Large, BACKGROUND, Vec::new(), content, close.clone(), cx);
    Some(ui::scrim(sheet, close, cx))
}

fn close_button(id: &str, title: &str, variant: Variant, store: &Entity<Store>) -> ActionButton {
    let store = store.clone();
    ActionButton::new(SharedString::from(id.to_string()), title.to_string())
        .variant(variant)
        .on_click(move |_, _, cx| store.update(cx, |store, cx| store.close_connect(cx)))
}

fn approve_button(choices: &ConnectChoices, store: &Entity<Store>) -> ActionButton {
    let store = store.clone();
    ActionButton::new("connect-approve", "Approve")
        .symbol(Symbol::Check)
        .variant(Variant::Primary)
        .tooltip("Approve (Enter)")
        .pending(choices.answering)
        .disabled(!choices.ready())
        .on_click(move |_, _, cx| store.update(cx, |store, cx| store.approve_connect(cx)))
}

fn loading(cx: &App) -> Div {
    div().flex_1().flex().items_center().justify_center().child(ui::spinner(20., cx.color(MUTED_FOREGROUND)))
}

fn failed(error: &str, store: &Entity<Store>, cx: &App) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(div().flex_1().p(px(space::XL)).child(ui::notice(
            error.to_string(),
            Tone::Error,
            Some(Symbol::CircleAlert),
            None,
            cx,
        )))
        .child(ui::footer_bar(
            vec![close_button("connect-close", "Close", Variant::Outline, store).into_any_element()],
            cx,
        ))
}

fn approved(client: &str, store: &Entity<Store>, cx: &App) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::M))
        .p(px(space::XL))
        .child(ui::icon(Symbol::CircleCheck, 36.).text_color(cx.color(SUCCESS)))
        .child(div().text(TextStyle::Subheading, cx).child("Approved"))
        .child(
            ui::paragraph_in(
                format!("Your browser takes you back to {client}. You can revoke it in Settings → Connected apps."),
                TextStyle::Label,
                MUTED_FOREGROUND,
                cx,
            )
            .max_w(px(size::FORM_WIDTH))
            .text_center(),
        )
        .child(
            div()
                .pt(px(space::S))
                .child(close_button("connect-done", "Done", Variant::Primary, store).tooltip("Done (Enter)")),
        )
}

fn asking(choices: &ConnectChoices, store: &Entity<Store>, cx: &App) -> Div {
    let request = &choices.request;
    let deny = store.clone();
    let error = choices.error.clone().map(|error| ui::notice(error, Tone::Error, Some(Symbol::CircleAlert), None, cx));
    let deny_button = ActionButton::new("connect-deny", "Deny")
        .variant(Variant::Ghost)
        .disabled(choices.answering)
        .on_click(move |_, _, cx| deny.update(cx, |store, cx| store.deny_connect(cx)));
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div().id("connect-choices").flex_1().min_h_0().overflow_y_scroll().child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::XL))
                    .p(px(space::XL))
                    .child(asker(request, cx))
                    .child(ui::settings_group(
                        "Accounts",
                        Some("Which mail it can reach."),
                        vec![ui::mark_list(account_rows(choices, store)).into_any_element()],
                        cx,
                    ))
                    .child(ui::settings_group(
                        "Role",
                        Some("The most it can do with that mail."),
                        vec![ui::mark_list(role_rows(choices, store)).into_any_element()],
                        cx,
                    ))
                    .child(ui::settings_group(
                        "Privileges",
                        Some("What the role doesn't cover."),
                        vec![ui::mark_list(privilege_rows(choices, store)).into_any_element()],
                        cx,
                    ))
                    .children(error),
            ),
        )
        .child(ui::footer_bar(
            vec![deny_button.into_any_element(), approve_button(choices, store).into_any_element()],
            cx,
        ))
}

/// Who is asking, whether it is verified, where it sends the browser, and the warnings.
fn asker(request: &ConnectRequest, cx: &App) -> Div {
    let symbol = if request.verified { Symbol::ShieldCheck } else { Symbol::Box };
    let chip = match request.verified {
        true => Chip::new("connect-verified", "Verified").symbol(Symbol::ShieldCheck).tone(Tone::Success),
        false => Chip::new("connect-verified", "Unverified").symbol(Symbol::CircleAlert).tone(Tone::Warning),
    };
    let on_device = request.on_device.then(|| {
        ui::notice("Sends you back to an application on this device", Tone::Warning, Some(Symbol::Monitor), None, cx)
    });
    ui::settings_card(
        vec![
            div()
                .flex()
                .flex_col()
                .gap(px(space::M))
                .p(px(space::L))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S))
                        .child(ui::icon(symbol, 20.).text_color(cx.color(FOREGROUND)))
                        .child(ui::line(request.client.clone(), TextStyle::Heading, cx).flex_shrink(1.))
                        .child(chip),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::XS))
                        .child(ui::notice(request.publisher.clone(), Tone::Muted, Some(Symbol::Globe), None, cx))
                        .child(ui::notice(request.back_to.clone(), Tone::Muted, Some(Symbol::ArrowRight), None, cx))
                        .children(on_device)
                        .child(ui::notice(
                            "Only approve if you started this yourself",
                            Tone::Warning,
                            Some(Symbol::CircleAlert),
                            None,
                            cx,
                        )),
                )
                .into_any_element(),
        ],
        cx,
    )
}

/// What a click on a choice changes.
fn choosing(
    store: &Entity<Store>,
    change: impl Fn(&mut ConnectChoices) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let store = store.clone();
    move |_, _, cx| store.update(cx, |store, cx| store.choose(&change, cx))
}

fn account_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<AnyElement> {
    let mut rows = vec![
        MarkRow::new("connect-every-account", "Every account", Mark::Radio, choosing(store, |c| c.all_accounts = true))
            .detail("Including the ones you add later.")
            .lead(Lead::Symbol(Symbol::Inbox))
            .checked(choices.all_accounts)
            .into_any_element(),
        MarkRow::new(
            "connect-some-accounts",
            "Only some accounts",
            Mark::Radio,
            choosing(store, |c| c.all_accounts = false),
        )
        .detail("Only the ones you pick.")
        .lead(Lead::Symbol(Symbol::ListFilter))
        .checked(!choices.all_accounts)
        .into_any_element(),
    ];
    if choices.all_accounts {
        return rows;
    }
    rows.extend(choices.request.accounts.iter().map(|account| {
        let id = account.id.clone();
        let toggle = choosing(store, move |c| {
            if !c.accounts.remove(&id) {
                c.accounts.insert(id.clone());
            }
        });
        let color = account_color(account.color.as_deref().unwrap_or_default());
        MarkRow::new(
            SharedString::from(format!("connect-account-{}", account.id)),
            account.title.clone(),
            Mark::Check,
            toggle,
        )
        .detail(account.detail.clone())
        .lead(Lead::Dot(color))
        .checked(choices.accounts.contains(&account.id))
        .indent(size::MARK + space::M)
        .into_any_element()
    }));
    rows
}

fn role_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<AnyElement> {
    choices
        .request
        .roles
        .iter()
        .map(|role| {
            let id = role.id.clone();
            choice_row("connect-role", role, Mark::Radio, choosing(store, move |c| c.role = id.clone()))
                .checked(choices.role == role.id)
                .into_any_element()
        })
        .collect()
}

fn privilege_rows(choices: &ConnectChoices, store: &Entity<Store>) -> Vec<AnyElement> {
    choices
        .request
        .privileges
        .iter()
        .map(|privilege| {
            let id = privilege.id.clone();
            let toggle = choosing(store, move |c| {
                if !c.privileges.remove(&id) {
                    c.privileges.insert(id.clone());
                }
            });
            choice_row("connect-privilege", privilege, Mark::Check, toggle)
                .checked(choices.privileges.contains(&privilege.id))
                .into_any_element()
        })
        .collect()
}

fn choice_row(
    group: &str,
    choice: &AccessChoice,
    mark: Mark,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> MarkRow {
    let lead = choice.symbol.as_deref().map_or(Lead::None, |name| Lead::Symbol(Symbol::named(name)));
    MarkRow::new(SharedString::from(format!("{group}-{}", choice.id)), choice.title.clone(), mark, on_click)
        .detail(choice.detail.clone())
        .lead(lead)
}
