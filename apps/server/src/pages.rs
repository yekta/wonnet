//! The markup of the browser pages (`web.rs`), in the apps' theme tokens. Nothing here runs
//! script: the choices show and hide with CSS alone.

use chrono::{DateTime, Utc};
use mail_protocol::Provider;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use uuid::Uuid;

use crate::access::{Access, Privilege, Role};
use crate::oauth::{AuthorizeRequest, Client, GrantRow, host_of, on_device};

const TOKENS: &str = include_str!("../../../packages/theme/tokens.css");
const MARK: &str = include_str!("../../marketing/public/favicon.svg");

const GLOBE: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#;
const ARROW_RIGHT: &str = r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#;
const MONITOR: &str = r#"<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>"#;
const ALERT: &str = r#"<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>"#;
const SHIELD_CHECK: &str = r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>"#;
const BOX: &str = r#"<path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/>"#;
const INBOX: &str = r#"<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>"#;
const LIST_FILTER: &str = r#"<path d="M2 5h20"/><path d="M6 12h12"/><path d="M9 19h6"/>"#;
const EYE: &str = r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"#;
const ARCHIVE: &str = r#"<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>"#;
const FILE_TEXT: &str = r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>"#;
const MAIL_X: &str = r#"<path d="M22 12.532V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h8.792"/><path d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7"/><path d="m16.5 16.5 5 5"/><path d="m21.5 16.5-5 5"/>"#;
const CHECK: &str = r#"<path d="M20 6 9 17l-5-5"/>"#;
const UNPLUG: &str = r#"<path d="m19 5 3-3"/><path d="m2 22 3-3"/><path d="M6.3 20.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6-2.3 2.3a2.4 2.4 0 0 0 0 3.4Z"/><path d="M7.5 13.5 10 11"/><path d="M10.5 16.5 13 14"/><path d="m12 6 6 6 2.3-2.3a2.4 2.4 0 0 0 0-3.4l-2.6-2.6a2.4 2.4 0 0 0-3.4 0Z"/>"#;
const GOOGLE: &str = r#"<path fill="var(--google-blue)" d="M45.12 24.5c0-1.56-.14-3.06-.4-4.5H24v8.51h11.84c-.51 2.75-2.06 5.08-4.39 6.64v5.52h7.11c4.16-3.83 6.56-9.47 6.56-16.17z"/><path fill="var(--google-green)" d="M24 46c5.94 0 10.92-1.97 14.56-5.33l-7.11-5.52c-1.97 1.32-4.49 2.1-7.45 2.1-5.73 0-10.58-3.87-12.31-9.07H4.34v5.7C7.96 41.07 15.4 46 24 46z"/><path fill="var(--google-yellow)" d="M11.69 28.18C11.25 26.86 11 25.45 11 24s.25-2.86.69-4.18v-5.7H4.34C2.85 17.09 2 20.45 2 24c0 3.55.85 6.91 2.34 9.88l7.35-5.7z"/><path fill="var(--google-red)" d="M24 10.75c3.23 0 6.13 1.11 8.41 3.29l6.31-6.31C34.91 4.18 29.93 2 24 2 15.4 2 7.96 6.93 4.34 13.12l7.35 5.7c1.73-5.2 6.58-9.07 12.31-9.07z"/>"#;
const MICROSOFT: &str = r#"<path fill="var(--microsoft-red)" d="M1 1h10v10h-10z"/><path fill="var(--microsoft-green)" d="M12 1h10v10h-10z"/><path fill="var(--microsoft-blue)" d="M1 12h10v10h-10z"/><path fill="var(--microsoft-yellow)" d="M12 12h10v10h-10z"/>"#;

const STYLE: &str = r#"
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--background);color:var(--foreground);font:15px/1.4 "Avenir Next","Avenir","Nunito Sans",-apple-system,system-ui,sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:560px;margin:0 auto;padding:48px 16px 64px;display:flex;flex-direction:column;align-items:stretch}
a{color:inherit}
.brand{display:flex;align-items:center;gap:10px;align-self:center;font-size:20px;font-weight:600;color:var(--emphasized-foreground)}
.brand svg{width:32px;height:32px;border-radius:var(--radius-md);box-shadow:0 1px 3px color-mix(in srgb,var(--shadow) 16%,transparent)}
.subtitle{margin:10px 0 0;text-align:center;color:var(--muted-foreground)}
.icon{width:16px;height:16px;flex:none;fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round}
.card{margin-top:16px;background:var(--card);border:1px solid var(--border-card);border-radius:var(--radius-lg)}
.client{padding:16px;display:flex;flex-direction:column;gap:12px}
.client-name{display:flex;flex-wrap:wrap;align-items:center;gap:6px 8px;font-size:17px;font-weight:600;color:var(--emphasized-foreground);line-height:1.2}
.client-name .icon{width:18px;height:18px}
.facts{display:flex;flex-direction:column;gap:6px;color:var(--muted-foreground);font-size:14px}
.fact{margin:0;display:flex;align-items:flex-start;gap:8px;line-height:1.3}
.fact .icon{width:14px;height:14px;margin-top:2px}
.fact b{font-weight:500;color:var(--foreground)}
.warning{color:var(--warning)}
.chip{display:inline-flex;align-items:center;gap:4px;height:22px;padding:0 8px;border-radius:999px;font-size:12px;font-weight:600;line-height:1}
.chip .icon{width:12px;height:12px}
.chip.success{color:var(--success);background:color-mix(in srgb,var(--success) calc(var(--opacity-color-tint) * 100%),transparent)}
.chip.warning{color:var(--warning);background:color-mix(in srgb,var(--warning) calc(var(--opacity-color-tint) * 100%),transparent)}
.chip.plain{color:var(--foreground);background:var(--background-secondary);font-weight:500}
.dot{width:8px;height:8px;border-radius:50%;flex:none}
.body{padding:12px 16px 16px}
h2{margin:20px 0 0;font-size:17px;font-weight:600;color:var(--emphasized-foreground);line-height:1.2}
h2:first-child{margin-top:4px}
.caption{margin:4px 0 0;color:var(--muted-foreground);font-size:14px}
.choices{margin-top:10px;display:flex;flex-direction:column;gap:2px;padding:2px;background:var(--input);border:1px solid var(--border-input);border-radius:var(--radius-lg)}
.choice{position:relative;display:flex;align-items:flex-start;gap:10px;padding:9px 10px;border-radius:var(--radius-md);cursor:pointer;min-height:36px}
.choice:hover{background:var(--background-accent-larger)}
.choice input{position:absolute;opacity:0;pointer-events:none}
.choice:has(input:focus-visible){outline:2px solid var(--ring);outline-offset:-2px}
.mark{flex:none;width:18px;height:18px;margin-top:1px;display:grid;place-items:center;border:1.5px solid var(--muted-stronger-foreground);background:var(--card);color:var(--primary-foreground)}
.mark.round{border-radius:50%}
.mark.square{border-radius:var(--radius-xs)}
.mark .icon{width:12px;height:12px;stroke-width:3;opacity:0}
.choice:has(input:checked) .mark{background:var(--primary);border-color:var(--primary)}
.choice:has(input:checked) .mark .icon{opacity:1}
.choice-icon{flex:none;margin-top:2px;color:var(--foreground)}
.choice-text{display:flex;flex-direction:column;gap:2px;min-width:0}
.choice-title{font-weight:600;color:var(--emphasized-foreground);line-height:1.25}
.choice-detail{font-size:14px;color:var(--muted-foreground);line-height:1.3}
.account-line{display:flex;align-items:center;gap:8px;min-width:0}
.account-line span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.accounts{display:none;margin-top:6px}
form:has(input[name=access][value=some]:checked) .accounts{display:flex}
.footer{display:flex;justify-content:flex-end;align-items:center;gap:6px;padding:10px;border-top:1px solid var(--border-card)}
.footer .spacer{flex:1}
.button{display:inline-flex;align-items:center;justify-content:center;gap:6px;height:36px;padding:0 16px;border:0;border-radius:var(--radius-md);font:inherit;font-weight:600;cursor:pointer;text-decoration:none;white-space:nowrap}
.button .icon{width:15px;height:15px}
.button.secondary{background:var(--background-secondary);color:var(--foreground)}
.button.secondary:hover{background:var(--background-secondary-accent)}
.card .button.secondary{background:var(--card-accent)}
.card .button.secondary:hover{background:var(--card-larger-accent-stronger)}
.button.primary{background:var(--primary);color:var(--primary-foreground)}
.button.primary:hover{opacity:var(--opacity-lit)}
.button.wide{width:100%;height:44px;justify-content:center}
.button.wide svg{width:18px;height:18px}
.signed-in{margin:16px 0 0;text-align:center;font-size:14px;color:var(--muted-foreground)}
.signed-in b{font-weight:600;color:var(--foreground)}
.link{background:none;border:0;padding:0;font:inherit;color:var(--foreground);text-decoration:underline;cursor:pointer}
.notice{margin-top:12px;padding:10px 12px;border-radius:var(--radius-md);font-size:14px;color:var(--destructive);background:color-mix(in srgb,var(--destructive) calc(var(--opacity-color-tint) * 100%),transparent)}
.stack{display:flex;flex-direction:column;gap:8px;padding:16px}
.field{display:flex;flex-direction:column;gap:4px;font-size:14px;color:var(--muted-foreground)}
.field input{height:36px;padding:0 10px;border:1px solid var(--border-input);border-radius:var(--radius-md);background:var(--input);color:var(--emphasized-foreground);font:inherit}
.field input:focus{outline:2px solid var(--ring);outline-offset:-1px}
details summary{cursor:pointer;list-style:none;padding:12px 16px;font-weight:600;color:var(--emphasized-foreground)}
details summary::-webkit-details-marker{display:none}
details[open] summary{border-bottom:1px solid var(--border-card)}
.or{display:flex;align-items:center;gap:10px;color:var(--muted-stronger-foreground);font-size:13px}
.or::before,.or::after{content:"";flex:1;height:1px;background:var(--border-card)}
.app{padding:14px 16px;display:flex;flex-direction:column;gap:10px}
.app + .app{border-top:1px solid var(--border-card)}
.app-head{display:flex;align-items:center;gap:8px}
.app-head .client-name{flex:1;font-size:16px}
.chips{display:flex;flex-wrap:wrap;gap:6px}
.meta{margin:0;font-size:13px;color:var(--muted-foreground)}
.empty{margin:0;padding:28px 16px;text-align:center;color:var(--muted-foreground)}
.copy{display:flex;flex-direction:column;gap:8px;padding:16px}
.copy p{margin:0;font-size:14px;color:var(--muted-foreground)}
.copy input,.copy code{display:block;width:100%;height:36px;padding:0 10px;border:1px solid var(--border-input);border-radius:var(--radius-md);background:var(--input);color:var(--emphasized-foreground);font:13px ui-monospace,SFMono-Regular,Menlo,monospace;line-height:34px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
.copy code{height:auto;white-space:normal;word-break:break-all;line-height:1.5;padding:8px 10px}
.copy-title{font-weight:600;color:var(--emphasized-foreground)}
"#;

fn icon(paths: &str) -> Markup {
    PreEscaped(format!(r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">{paths}</svg>"#))
}

fn logo(paths: &str, view_box: &str) -> Markup {
    PreEscaped(format!(r#"<svg viewBox="{view_box}" aria-hidden="true">{paths}</svg>"#))
}

fn shell(title: &str, subtitle: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                meta name="referrer" content="no-referrer";
                title { (title) " · Wonnet" }
                style { (PreEscaped(TOKENS)) (PreEscaped(STYLE)) }
            }
            body {
                main {
                    div.brand { (PreEscaped(MARK)) "Wonnet" }
                    p.subtitle { (subtitle) }
                    (content)
                }
            }
        }
    }
}

pub fn error(message: &str) -> Markup {
    shell("Something went wrong", "This can't go on", html! { div.notice { (message) } })
}

/// How a page that needs a signed-in browser asks for it. `next` is where it goes after.
pub struct SignIn<'a> {
    pub next: &'a str,
    pub google: bool,
    pub microsoft: bool,
    pub error: Option<&'a str>,
}

pub fn sign_in(sign_in: &SignIn) -> Markup {
    let start = |provider: &str| {
        let next: String = url::form_urlencoded::byte_serialize(sign_in.next.as_bytes()).collect();
        format!("/auth/{provider}/start?next={next}")
    };
    let content = html! {
        @if let Some(error) = sign_in.error { div.notice { (error) } }
        div.card {
            div.stack {
                @if sign_in.google {
                    a.button.secondary.wide href=(start("google")) { (logo(GOOGLE, "0 0 48 48")) "Continue with Google" }
                }
                @if sign_in.microsoft {
                    a.button.secondary.wide href=(start("microsoft")) { (logo(MICROSOFT, "0 0 23 23")) "Continue with Microsoft" }
                }
            }
        }
        div.card {
            details {
                summary { "Sign in with a JMAP server" }
                form.stack method="post" action="/web/jmap" {
                    input type="hidden" name="next" value=(sign_in.next);
                    label.field { "Server" input name="url" type="url" placeholder="https://api.fastmail.com/jmap/session" required; }
                    label.field { "Email" input name="username" type="email" autocomplete="username" required; }
                    label.field { "Password" input name="password" type="password" autocomplete="current-password" required; }
                    button.button.primary type="submit" { "Sign In" }
                }
            }
        }
        p.signed-in { "Use the address you started Wonnet with." }
    };
    shell("Sign in", "Sign in to Wonnet to continue", content)
}

/// An account as the pages list it.
pub struct AccountChoice {
    pub id: Uuid,
    pub address: String,
    pub name: String,
    pub provider: Provider,
    pub color: String,
}

impl AccountChoice {
    fn provider_name(&self) -> &'static str {
        match self.provider {
            Provider::Gmail => "Gmail",
            Provider::Outlook => "Outlook",
            Provider::Jmap => "JMAP",
        }
    }
}

fn account_dot(color: &str) -> Markup {
    let color = match mail_protocol::ACCOUNT_COLORS.contains(&color) {
        true => color,
        false => "account-1",
    };
    html! { span.dot style=(format!("background:var(--{color})")) {} }
}

fn client_summary(client: &Client, redirect_uri: &str) -> Markup {
    html! {
        div.card.client {
            div.client-name {
                @if client.verified().is_some() { (icon(SHIELD_CHECK)) } @else { (icon(BOX)) }
                span { (client.name) }
                @if client.verified().is_some() {
                    span.chip.success { (icon(SHIELD_CHECK)) "Verified" }
                } @else {
                    span.chip.warning { (icon(ALERT)) "Unverified" }
                }
            }
            div.facts {
                p.fact {
                    (icon(GLOBE))
                    @match client.publisher() {
                        Some(host) => span { "Published by " b { (host) } },
                        None => span { "Registered itself, publisher unknown" },
                    }
                }
                p.fact { (icon(ARROW_RIGHT)) span { "Sends you back to " b { (host_of(redirect_uri)) } } }
                @if on_device(redirect_uri) {
                    p.fact.warning { (icon(MONITOR)) span { "Sends you back to an application on this device" } }
                }
                p.fact.warning { (icon(ALERT)) span { "Only approve if you started this yourself" } }
            }
        }
    }
}

/// What the form shows as chosen: the defaults, or what was sent when it comes back with an error.
pub struct Choice {
    pub all_accounts: bool,
    pub accounts: Vec<Uuid>,
    pub role: Role,
    pub privileges: Vec<Privilege>,
}

impl Default for Choice {
    fn default() -> Self {
        Self { all_accounts: true, accounts: Vec::new(), role: Role::Reader, privileges: Vec::new() }
    }
}

fn role_text(role: Role) -> (&'static str, &'static str, &'static str) {
    match role {
        Role::Reader => ("Reader", "Can search your mail and see who sent it, its subject and its first lines.", EYE),
        Role::Organizer => {
            ("Organizer", "Can also archive, label, mark read, snooze, trash, block senders and add rules.", ARCHIVE)
        }
    }
}

fn privilege_text(privilege: Privilege) -> (&'static str, &'static str, &'static str) {
    match privilege {
        Privilege::ReadBodies => {
            ("Read whole messages", "Without it, the application sees the first lines only.", FILE_TEXT)
        }
        Privilege::Unsubscribe => {
            ("Unsubscribe from lists", "Asks a list's sender to stop, by its link or by email.", MAIL_X)
        }
    }
}

fn choice(
    kind: &str,
    name: &str,
    value: &str,
    checked: bool,
    icon_paths: Option<&str>,
    title: Markup,
    detail: Option<&str>,
) -> Markup {
    let round = kind == "radio";
    html! {
        label.choice {
            input type=(kind) name=(name) value=(value) checked[checked];
            span.mark.round[round].square[!round] { (icon(CHECK)) }
            @if let Some(paths) = icon_paths { span.choice-icon { (icon(paths)) } }
            span.choice-text {
                span.choice-title { (title) }
                @if let Some(detail) = detail { span.choice-detail { (detail) } }
            }
        }
    }
}

pub struct Consent<'a> {
    pub client: &'a Client,
    pub request: &'a AuthorizeRequest,
    pub accounts: &'a [AccountChoice],
    pub choice: &'a Choice,
    pub csrf: &'a str,
    pub signed_in_as: &'a str,
    pub error: Option<&'a str>,
}

pub fn consent(consent: &Consent) -> Markup {
    let request = consent.request;
    let chosen = consent.choice;
    let hidden = [
        ("response_type", &request.response_type),
        ("client_id", &request.client_id),
        ("redirect_uri", &request.redirect_uri),
        ("state", &request.state),
        ("code_challenge", &request.code_challenge),
        ("code_challenge_method", &request.code_challenge_method),
        ("resource", &request.resource),
        ("scope", &request.scope),
    ];
    let content = html! {
        (client_summary(consent.client, &request.redirect_uri))
        form.card method="post" action="/oauth/authorize" {
            input type="hidden" name="csrf" value=(consent.csrf);
            @for (name, value) in hidden { input type="hidden" name=(name) value=(value); }
            div.body {
                h2 { "Access" }
                p.caption { "Which mail the application can reach, never more than you can." }
                div.choices {
                    (choice("radio", "access", "all", chosen.all_accounts, Some(INBOX), html! { "Every account" }, Some("Your accounts, the ones you add later too.")))
                    (choice("radio", "access", "some", !chosen.all_accounts, Some(LIST_FILTER), html! { "Only some accounts" }, Some("Only the accounts you pick.")))
                }
                div.choices.accounts {
                    @for account in consent.accounts {
                        @let title = html! {
                            span.account-line {
                                (account_dot(&account.color))
                                span { @if account.name.is_empty() { (account.address) } @else { (account.name) } }
                            }
                        };
                        @let detail = match account.name.is_empty() {
                            true => account.provider_name().to_string(),
                            false => format!("{} · {}", account.address, account.provider_name()),
                        };
                        (choice("checkbox", "accounts", &account.id.to_string(), chosen.accounts.contains(&account.id), None, title, Some(&detail)))
                    }
                }
                h2 { "Role" }
                p.caption { "The most the application can do with that mail." }
                div.choices {
                    @for role in Role::ALL {
                        @let (title, detail, paths) = role_text(role);
                        (choice("radio", "role", role.as_str(), chosen.role == role, Some(paths), html! { (title) }, Some(detail)))
                    }
                }
                h2 { "Privileges" }
                p.caption { "What the role doesn't cover." }
                div.choices {
                    @for privilege in Privilege::ALL {
                        @let (title, detail, paths) = privilege_text(privilege);
                        (choice("checkbox", "privileges", privilege.as_str(), chosen.privileges.contains(&privilege), Some(paths), html! { (title) }, Some(detail)))
                    }
                }
                @if let Some(error) = consent.error { div.notice { (error) } }
            }
            div.footer {
                button.button.secondary type="submit" name="decision" value="deny" formnovalidate { "Deny" }
                button.button.primary type="submit" name="decision" value="approve" { "Approve" }
            }
        }
        (signed_in(consent.signed_in_as, consent.csrf, "/connected-apps"))
    };
    shell("Connect an application", "An application is asking for access to your mail", content)
}

fn signed_in(address: &str, csrf: &str, next: &str) -> Markup {
    html! {
        form.signed-in method="post" action="/web/sign-out" {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="next" value=(next);
            "Signed in as " b { (address) } " · "
            button.link type="submit" { "Sign out" }
        }
    }
}

pub struct ConnectedApps<'a> {
    pub grants: &'a [GrantRow],
    pub accounts: &'a [AccountChoice],
    pub mcp_url: &'a str,
    pub csrf: &'a str,
    pub signed_in_as: &'a str,
    pub now: DateTime<Utc>,
}

fn access_chips(access: &Access, accounts: &[AccountChoice]) -> Markup {
    let reached: Vec<&AccountChoice> = accounts.iter().filter(|account| access.reaches(account.id)).collect();
    html! {
        div.chips {
            @if access.all_accounts {
                span.chip.plain { (icon(INBOX)) "Every account" }
            } @else {
                @for account in &reached {
                    span.chip.plain { (account_dot(&account.color)) (account.address) }
                }
                @if reached.is_empty() { span.chip.plain { "No accounts left" } }
            }
            @let (role, _, paths) = role_text(access.role);
            span.chip.plain { (icon(paths)) (role) }
            @for privilege in &access.privileges {
                @let (title, _, paths) = privilege_text(*privilege);
                span.chip.plain { (icon(paths)) (title) }
            }
        }
    }
}

/// "just now", "5 minutes ago", "3 hours ago", else the day.
pub fn ago(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (now - then).num_minutes();
    match minutes {
        ..1 => "just now".into(),
        1 => "a minute ago".into(),
        2..60 => format!("{minutes} minutes ago"),
        60..120 => "an hour ago".into(),
        120..1440 => format!("{} hours ago", minutes / 60),
        _ => format!("on {}", then.format("%b %-d, %Y")),
    }
}

pub fn connected_apps(page: &ConnectedApps) -> Markup {
    let command = format!("claude mcp add --transport http wonnet {}", page.mcp_url);
    let content = html! {
        div.card.copy {
            span.copy-title { "Connect an application" }
            p { "Add this address as an MCP server in Claude, ChatGPT, Cursor or any application that speaks MCP. It sends you here to choose what it may do." }
            input type="text" readonly value=(page.mcp_url) aria-label="MCP server address";
            p { "In Claude Code:" }
            code { (command) }
        }
        div.card {
            @if page.grants.is_empty() {
                p.empty { "No applications are connected." }
            }
            @for grant in page.grants {
                @let client = grant.client();
                div.app {
                    div.app-head {
                        div.client-name {
                            @if client.verified().is_some() { (icon(SHIELD_CHECK)) } @else { (icon(BOX)) }
                            span { (grant.client_name) }
                            @if client.verified().is_some() {
                                span.chip.success { (icon(SHIELD_CHECK)) "Verified" }
                            } @else {
                                span.chip.warning { (icon(ALERT)) "Unverified" }
                            }
                        }
                        form method="post" action="/connected-apps/revoke" {
                            input type="hidden" name="csrf" value=(page.csrf);
                            input type="hidden" name="id" value=(grant.id.to_string());
                            button.button.secondary type="submit" { (icon(UNPLUG)) "Revoke" }
                        }
                    }
                    (access_chips(&grant.access.0, page.accounts))
                    p.meta {
                        "Approved " (ago(grant.created_at, page.now))
                        " · "
                        @match grant.last_used_at {
                            Some(used) => { "Last used " (ago(used, page.now)) },
                            None => "Never used",
                        }
                    }
                }
            }
        }
        (signed_in(page.signed_in_as, page.csrf, "/connected-apps"))
    };
    shell("Connected apps", "Applications that can reach your mail", content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_long_ago() {
        let now = DateTime::parse_from_rfc3339("2026-11-04T12:00:00Z").unwrap().to_utc();
        let before = |minutes| now - chrono::Duration::minutes(minutes);
        assert_eq!(ago(before(0), now), "just now");
        assert_eq!(ago(before(5), now), "5 minutes ago");
        assert_eq!(ago(before(90), now), "an hour ago");
        assert_eq!(ago(before(300), now), "5 hours ago");
        assert_eq!(ago(before(60 * 24 * 3), now), "on Nov 1, 2026");
    }
}
