//! The markup of the pages a browser sees while an application asks to reach the user's mail
//! (`connect.rs`), in the apps' theme tokens. The waiting page runs one script, which asks every
//! second whether the app has answered; the others run none.

use maud::{DOCTYPE, Markup, PreEscaped, html};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use uuid::Uuid;

const TOKENS: &str = include_str!("../../../packages/theme/tokens.css");
const MARK: &str = include_str!("../../marketing/public/favicon.svg");
const GET_THE_APP: &str = "https://wonnet.app";

const GLOBE: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#;
const ARROW_RIGHT: &str = r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#;
const MONITOR: &str = r#"<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>"#;
const ALERT: &str = r#"<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>"#;
const SHIELD_CHECK: &str = r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>"#;
const BOX: &str = r#"<path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/>"#;
const CHECK: &str = r#"<path d="M20 6 9 17l-5-5"/>"#;
const X: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
const LOADER: &str = r#"<path d="M21 12a9 9 0 1 1-6.219-8.56"/>"#;
const SQUARE_ARROW: &str =
    r#"<path d="M21 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6"/><path d="m21 3-9 9"/><path d="M15 3h6v6"/>"#;

const STYLE: &str = r#"
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--background);color:var(--foreground);font:15px/1.4 "Avenir Next","Avenir","Nunito Sans",-apple-system,system-ui,sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:440px;margin:0 auto;padding:40px 16px 48px;display:flex;flex-direction:column;align-items:stretch}
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
.body{padding:16px;display:flex;flex-direction:column;gap:12px}
h2{margin:0;font-size:17px;font-weight:600;color:var(--emphasized-foreground);line-height:1.2}
.caption{margin:-6px 0 0;color:var(--muted-foreground);font-size:14px}
.button{display:inline-flex;align-items:center;justify-content:center;gap:6px;height:36px;padding:0 16px;border:0;border-radius:var(--radius-md);font:inherit;font-weight:600;cursor:pointer;text-decoration:none;white-space:nowrap}
.button .icon{width:15px;height:15px}
.button.secondary{background:var(--card-accent);color:var(--foreground)}
.button.secondary:hover{background:var(--card-larger-accent-stronger)}
.button.primary{background:var(--primary);color:var(--primary-foreground)}
.button.primary:hover{opacity:var(--opacity-lit)}
.button.wide{width:100%;height:44px}
.button:focus-visible{outline:2px solid var(--ring);outline-offset:2px}
.or{display:flex;align-items:center;gap:10px;color:var(--muted-stronger-foreground);font-size:13px}
.or::before,.or::after{content:"";flex:1;height:1px;background:var(--border-card)}
.phone{display:flex;flex-direction:column;align-items:center;gap:10px}
.qr{border-radius:var(--radius-md);overflow:hidden;line-height:0;border:1px solid var(--border-card)}
.qr svg{width:172px;height:172px;shape-rendering:crispEdges}
.phone p{margin:0;font-size:13px;color:var(--muted-foreground)}
@media (pointer:coarse){.phone{display:none}}
.status{display:flex;align-items:center;gap:8px;padding:12px 16px;border-top:1px solid var(--border-card);font-size:14px;color:var(--muted-foreground)}
.status .icon{width:16px;height:16px}
.status>span{display:none;align-items:center;gap:8px}
.spin{animation:spin 1s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
@media (prefers-reduced-motion:reduce){.spin{animation:none}}
body[data-state=waiting] .waiting,body[data-state=approved] .approved,body[data-state=denied] .denied,body[data-state=expired] .expired{display:flex}
.approved{color:var(--success)}
.expired{color:var(--destructive)}
body:not([data-state=waiting]) .actions{display:none}
body:not([data-state=waiting]) .status{border-top:0}
.foot{margin:16px 0 0;text-align:center;font-size:14px;color:var(--muted-foreground)}
.foot a{color:var(--foreground)}
.notice{padding:10px 12px;border-radius:var(--radius-md);font-size:14px;color:var(--destructive);background:color-mix(in srgb,var(--destructive) calc(var(--opacity-color-tint) * 100%),transparent)}
"#;

/// Asks every second whether the app has answered, then goes on, or says the request is over.
const POLL: &str = r#"
const id = document.body.dataset.request;
async function poll() {
  try {
    const response = await fetch(`/oauth/requests/${id}`, { cache: "no-store" });
    const { state } = await response.json();
    if (state !== "waiting") {
      document.body.dataset.state = state;
      if (state !== "expired") location.replace(`/oauth/requests/${id}/finish`);
      return;
    }
  } catch {}
  setTimeout(poll, 1000);
}
setTimeout(poll, 1000);
"#;

fn icon(paths: &str) -> Markup {
    PreEscaped(format!(r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">{paths}</svg>"#))
}

fn spinning(paths: &str) -> Markup {
    PreEscaped(format!(r#"<svg class="icon spin" viewBox="0 0 24 24" aria-hidden="true">{paths}</svg>"#))
}

/// The address as a QR code, dark on light with its light margin whatever the theme, since that is
/// what cameras read.
fn qr(address: &str) -> Markup {
    let Ok(code) = QrCode::with_error_correction_level(address, EcLevel::M) else { return html! {} };
    let image = code.render::<svg::Color>().quiet_zone(true).build();
    let image = image.find("<svg").map_or(image.as_str(), |start| &image[start..]);
    PreEscaped(image.to_string())
}

fn shell(title: &str, subtitle: &str, state: Option<(&str, Uuid)>, content: Markup) -> Markup {
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
            @match state {
                Some((state, request)) => body data-state=(state) data-request=(request) { main { (header(subtitle)) (content) } },
                None => body { main { (header(subtitle)) (content) } },
            }
        }
    }
}

fn header(subtitle: &str) -> Markup {
    html! {
        div.brand { (PreEscaped(MARK)) "Wonnet" }
        p.subtitle { (subtitle) }
    }
}

fn get_the_app() -> Markup {
    html! { p.foot { "Don't have Wonnet? Get it at " a href=(GET_THE_APP) target="_blank" rel="noopener" { "wonnet.app" } } }
}

pub fn error(message: &str) -> Markup {
    shell(
        "Something went wrong",
        "This can't go on",
        None,
        html! { div.card { div.body { div.notice { (message) } } } },
    )
}

/// The application asking, as the pages show it.
pub struct Asking {
    pub client: String,
    pub verified: bool,
    pub publisher: Option<String>,
    pub redirect_host: String,
    pub on_device: bool,
}

fn asking_line(asking: &Asking) -> String {
    format!("{} is asking to reach your mail", asking.client)
}

fn client_summary(asking: &Asking, warn: bool) -> Markup {
    html! {
        div.card.client {
            div.client-name {
                @if asking.verified { (icon(SHIELD_CHECK)) } @else { (icon(BOX)) }
                span { (asking.client) }
                @if asking.verified {
                    span.chip.success { (icon(SHIELD_CHECK)) "Verified" }
                } @else {
                    span.chip.warning { (icon(ALERT)) "Unverified" }
                }
            }
            div.facts {
                p.fact {
                    (icon(GLOBE))
                    @match &asking.publisher {
                        Some(host) => span { "Published by " b { (host) } },
                        None => span { "Registered itself, publisher unknown" },
                    }
                }
                p.fact { (icon(ARROW_RIGHT)) span { "Sends you back to " b { (asking.redirect_host) } } }
                @if asking.on_device {
                    p.fact.warning { (icon(MONITOR)) span { "Sends you back to an application on this device" } }
                }
                @if warn {
                    p.fact.warning { (icon(ALERT)) span { "Only go on if you started this yourself" } }
                }
            }
        }
    }
}

pub struct Waiting<'a> {
    pub asking: &'a Asking,
    pub id: Uuid,
    /// The `wonnet://connect` link that opens the app on this device.
    pub open: &'a str,
    /// The page the QR code leads a phone to.
    pub phone: &'a str,
    pub nonce: &'a str,
}

/// What the browser the application opened shows: who is asking, the button that opens the app
/// and the QR code for a phone, and how the answer is coming along.
pub fn waiting(page: &Waiting) -> Markup {
    let client = &page.asking.client;
    let content = html! {
        (client_summary(page.asking, false))
        div.card {
            div.body.actions {
                h2 { "Choose in Wonnet" }
                p.caption { "Pick which mail " (client) " reaches and what it can do with it." }
                a.button.primary.wide href=(page.open) { (icon(SQUARE_ARROW)) "Open in Wonnet" }
                div.phone {
                    div.or style="align-self:stretch" { "or on your phone" }
                    div.qr { (qr(page.phone)) }
                    p { "Scan it with your phone's camera." }
                }
            }
            div.status role="status" aria-live="polite" {
                span.waiting { (spinning(LOADER)) "Waiting for your answer in Wonnet" }
                span.approved { (icon(CHECK)) "Approved. Taking you back to " (client) }
                span.denied { (icon(X)) "Denied. Taking you back to " (client) }
                span.expired { (icon(ALERT)) "This request has expired. Close this window and connect again from " (client) "." }
            }
        }
        (get_the_app())
        script nonce=(page.nonce) { (PreEscaped(POLL)) }
    };
    shell("Connect", &asking_line(page.asking), Some(("waiting", page.id)), content)
}

/// Where a browser without scripts lands when it goes on before the app has answered.
pub fn not_yet(id: Uuid) -> Markup {
    let content = html! {
        div.card {
            div.body {
                h2 { "Not answered yet" }
                p.caption { "Approve or deny in Wonnet, then continue." }
                a.button.primary.wide href=(format!("/oauth/requests/{id}/finish")) { "Continue" }
            }
        }
    };
    shell("Connect", "Waiting for your answer in Wonnet", None, content)
}

/// The page a phone opens from the QR code: the button that opens the app on the request.
pub fn open_on_phone(asking: &Asking, open: &str) -> Markup {
    let content = html! {
        (client_summary(asking, true))
        div.card {
            div.body {
                a.button.primary.wide href=(open) { (icon(SQUARE_ARROW)) "Open in Wonnet" }
                p.caption style="margin:0" { "Pick there which mail " (asking.client) " reaches. The other screen goes on by itself." }
            }
        }
        (get_the_app())
    };
    shell("Connect", &asking_line(asking), None, content)
}
