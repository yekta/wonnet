//! Connected apps as Settings lists them, and an application's request as the app asks about it.

use chrono::{DateTime, TimeZone, Utc};
use mail_protocol::access::{Access, Privilege, Role};
use mail_protocol::api::{ConnectRequest, ConnectedApp};

use crate::api::{AccessChoice, ConnectRequestView, ConnectedAppView};
use crate::render::dates;
use crate::store::LocalAccount;

fn role_title(role: Role) -> &'static str {
    match role {
        Role::Reader => "Reader",
        Role::Organizer => "Organizer",
    }
}

fn privilege_title(privilege: Privilege) -> &'static str {
    match privilege {
        Privilege::ReadBodies => "Reads whole messages",
        Privilege::Unsubscribe => "Unsubscribes from lists",
    }
}

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

/// "Organizer · Every account · Reads whole messages": what the app may do, the accounts named as
/// the sidebar names them.
pub fn access(access: &Access, accounts: &[LocalAccount]) -> String {
    let mut parts = vec![role_title(access.role).to_string()];
    match access.all_accounts {
        true => parts.push("Every account".into()),
        false => {
            let reached: Vec<&str> = accounts.iter().filter(|account| access.reaches(&account.id)).map(title).collect();
            parts.push(if reached.is_empty() { "No accounts left".into() } else { reached.join(", ") });
        }
    }
    parts.extend(access.privileges.iter().map(|privilege| privilege_title(*privilege).to_string()));
    parts.join(" · ")
}

pub fn apps<Tz: TimeZone>(
    apps: Vec<ConnectedApp>,
    accounts: &[LocalAccount],
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
            ConnectedAppView {
                access: access(&app.access, accounts),
                used: format!("Approved {} · {used}", ago(app.approved_at, now)),
                id: app.id,
                name: app.client,
                verified: app.verified,
            }
        })
        .collect()
}

fn choice(id: &str, title: &str, detail: &str, symbol: &str) -> AccessChoice {
    AccessChoice { id: id.into(), title: title.into(), detail: detail.into(), symbol: Some(symbol.into()), color: None }
}

pub fn request(key: String, request: ConnectRequest, accounts: &[LocalAccount]) -> ConnectRequestView {
    let accounts = accounts
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
        .collect();
    ConnectRequestView {
        key,
        publisher: match &request.publisher {
            Some(host) => format!("Published by {host}"),
            None => "Registered itself, publisher unknown".into(),
        },
        back_to: format!("Sends you back to {}", request.redirect_host),
        on_device: request.on_device,
        verified: request.verified,
        accounts,
        roles: vec![
            choice(
                "reader",
                "Reader",
                "Can search your mail and see who sent it, its subject and its first lines.",
                "eye",
            ),
            choice(
                "organizer",
                "Organizer",
                "Can also archive, label, mark read, snooze, trash, block senders and add rules.",
                "archive",
            ),
        ],
        privileges: vec![
            choice(
                "read_bodies",
                "Read whole messages",
                "Without it, only the first lines of each message.",
                "file-text",
            ),
            choice(
                "unsubscribe",
                "Unsubscribe from lists",
                "Asks a list's sender to stop, by its link or by email.",
                "mail-x",
            ),
        ],
        client: request.client,
    }
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
    fn names_what_an_app_may_do() {
        let accounts = [account("a", "ann@gmail.com", ""), account("b", "ann@work.com", "Work")];
        let every = Access { role: Role::Reader, all_accounts: true, accounts: vec![], privileges: vec![] };
        assert_eq!(access(&every, &accounts), "Reader · Every account");
        let some = Access {
            role: Role::Organizer,
            all_accounts: false,
            accounts: vec!["b".into(), "gone".into()],
            privileges: vec![Privilege::ReadBodies],
        };
        assert_eq!(access(&some, &accounts), "Organizer · Work · Reads whole messages");
        let none = Access { accounts: vec!["gone".into()], ..some };
        assert_eq!(access(&none, &accounts), "Organizer · No accounts left · Reads whole messages");
    }
}
