//! Connected apps as Settings lists them, and an application's request as the app asks about it.

use chrono::{DateTime, TimeZone, Utc};
use mail_protocol::access::Access;
use mail_protocol::api::{ConnectRequest, ConnectedApp};

use crate::api::{AccessChoice, ConnectRequestView, ConnectedAppView};
use crate::render::{access, dates};
use crate::store::{LocalAccount, LocalLabel};

fn provider_name(provider: &str) -> &'static str {
    match provider {
        "gmail" => "Gmail",
        "outlook" => "Outlook",
        _ => "JMAP",
    }
}

fn title(account: &LocalAccount) -> &str {
    if account.name.is_empty() { &account.address } else { &account.name }
}

/// "just now", "5 minutes ago", "3 hours ago", "yesterday", else the day.
pub fn ago<Tz: TimeZone>(ms: i64, now: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let then = Utc.timestamp_millis_opt(ms).single().unwrap_or_default();
    let minutes = (now.with_timezone(&Utc) - then).num_minutes();
    match minutes {
        ..1 => "just now".into(),
        1 => "a minute ago".into(),
        2..60 => format!("{minutes} minutes ago"),
        60..120 => "an hour ago".into(),
        120..1440 => format!("{} hours ago", minutes / 60),
        _ => match dates::short(ms, now).as_str() {
            "Yesterday" => "yesterday".into(),
            day => day.into(),
        },
    }
}

pub fn apps<Tz: TimeZone>(
    apps: Vec<ConnectedApp>,
    accounts: &[LocalAccount],
    labels: &[LocalLabel],
    now: &DateTime<Tz>,
) -> Vec<ConnectedAppView>
where
    Tz::Offset: std::fmt::Display,
{
    apps.into_iter()
        .map(|app| {
            let used = match app.used_at {
                Some(used) => format!("Used {}", ago(used, now)),
                None => "Not used yet".into(),
            };
            let sentence = access::plain(&access::sentence(&app.client, &app.access, accounts, labels));
            ConnectedAppView {
                access: settled(&sentence, &app.client),
                used: format!("Approved {} · {used}", ago(app.approved_at, now)),
                id: app.id,
                name: app.client,
                verified: app.verified,
            }
        })
        .collect()
}

/// The sentence of an app already approved, under its name: "Reads the subject of mail in…".
fn settled(sentence: &str, client: &str) -> String {
    let rest = sentence.strip_prefix(&format!("{client} will ")).unwrap_or(sentence);
    [("read ", "Reads "), ("see ", "Sees "), ("reach ", "Reaches ")]
        .iter()
        .find_map(|(will, does)| rest.strip_prefix(will).map(|rest| format!("{does}{rest}")))
        .unwrap_or_else(|| rest.to_string())
}

fn choice(id: &str, title: &str, detail: &str, symbol: &str) -> AccessChoice {
    AccessChoice { id: id.into(), title: title.into(), detail: detail.into(), symbol: Some(symbol.into()), color: None }
}

fn account_choices(accounts: &[LocalAccount]) -> Vec<AccessChoice> {
    accounts
        .iter()
        .map(|account| AccessChoice {
            id: account.id.clone(),
            title: title(account).to_string(),
            detail: match account.name.is_empty() {
                true => provider_name(&account.provider).to_string(),
                false => format!("{} · {}", account.address, provider_name(&account.provider)),
            },
            symbol: None,
            color: Some(account.color.clone()),
        })
        .collect()
}

/// Who the sheet is about: an application asking, or one connected already.
struct Asker<'a> {
    key: String,
    app: Option<String>,
    client: String,
    verified: bool,
    publisher: Option<&'a str>,
    back_to: String,
    on_device: bool,
}

/// The sheet's choices, the same for a request and for a connected app being changed.
fn sheet(asker: Asker, accounts: &[LocalAccount], access: Access) -> ConnectRequestView {
    ConnectRequestView {
        key: asker.key,
        app: asker.app,
        client: asker.client,
        verified: asker.verified,
        publisher: match asker.publisher {
            Some(host) => format!("Published by {host}"),
            None => "Registered itself, publisher unknown".into(),
        },
        back_to: asker.back_to,
        on_device: asker.on_device,
        accounts: account_choices(accounts),
        always: choice(
            "who",
            "Who and when",
            "Sender, recipients, date and labels. Always: it can’t sort mail without them.",
            "users",
        ),
        reads: vec![
            choice("subject", "Subject", "The line it was sent with.", "type"),
            choice("body", "Body", "The whole text of each message.", "file-text"),
            choice("attachments", "Attachments", "Their names, kinds and sizes.", "paperclip"),
        ],
        roles: vec![
            choice("reader", "Only look", "Search and read. Nothing changes.", "eye"),
            choice(
                "organizer",
                "Organize",
                "Also archive, label, mark read, snooze, trash and sort with rules.",
                "archive",
            ),
        ],
        privileges: vec![choice(
            "unsubscribe",
            "Unsubscribe from lists",
            "Writes to a list’s sender to stop. Reaches outside your mailbox.",
            "mail-x",
        )],
        access,
    }
}

pub fn request(key: String, request: ConnectRequest, accounts: &[LocalAccount]) -> ConnectRequestView {
    let asker = Asker {
        key,
        app: None,
        client: request.client,
        verified: request.verified,
        publisher: request.publisher.as_deref(),
        back_to: format!("Sends you back to {}", request.redirect_host),
        on_device: request.on_device,
    };
    sheet(asker, accounts, Access::default())
}

/// A connected app, to change what it may do.
pub fn editing(app: &ConnectedApp, accounts: &[LocalAccount]) -> ConnectRequestView {
    let asker = Asker {
        key: String::new(),
        app: Some(app.id.clone()),
        client: app.client.clone(),
        verified: app.verified,
        publisher: app.publisher.as_deref(),
        back_to: String::new(),
        on_device: false,
    };
    sheet(asker, accounts, app.access.clone())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Local};

    use super::*;

    fn account(id: &str, address: &str, name: &str) -> LocalAccount {
        LocalAccount {
            id: id.into(),
            provider: "gmail".into(),
            address: address.into(),
            name: name.into(),
            status: "ready".into(),
            color: "account-1".into(),
            identities: Vec::new(),
        }
    }

    #[test]
    fn says_how_long_ago() {
        let now = Local::now();
        let before = |minutes| (now - Duration::minutes(minutes)).timestamp_millis();
        assert_eq!(ago(before(0), &now), "just now");
        assert_eq!(ago(before(5), &now), "5 minutes ago");
        assert_eq!(ago(before(90), &now), "an hour ago");
        assert_eq!(ago(before(300), &now), "5 hours ago");
        assert!(!ago(before(60 * 24 * 40), &now).contains("ago"));
    }

    #[test]
    fn says_what_an_app_may_do_under_its_name() {
        use mail_protocol::access::{Conditions, Part, Role};
        let accounts = [account("a", "ann@gmail.com", ""), account("b", "ann@work.com", "Work")];
        let app = |access: Access| ConnectedApp {
            id: "x".into(),
            client: "Claude Code".into(),
            verified: true,
            publisher: None,
            access,
            approved_at: 0,
            used_at: None,
        };
        let some = Access {
            role: Role::Organizer,
            all_accounts: false,
            accounts: vec!["b".into(), "gone".into()],
            reads: vec![Part::Subject],
            never: Conditions { from: vec!["@chase.com".into()], ..Default::default() },
            ..Access::default()
        };
        let looking = Access { reads: vec![], ..Access::default() };
        let views = apps(vec![app(some), app(looking)], &accounts, &[], &Local::now());
        assert_eq!(
            views[0].access,
            "Reads the subject of mail in Work, except mail from @chase.com, and can archive, label, snooze and trash it."
        );
        assert_eq!(views[1].access, "Sees who sent and received mail in every account, and only looks.");
    }
}
