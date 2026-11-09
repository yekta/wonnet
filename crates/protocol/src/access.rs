//! What a connected app may do with the user's mail, as they chose when they approved it: which
//! accounts it reaches and which of their mail (`only`, `never`), what it reads of each message,
//! a role, and privileges the role doesn't cover.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Reader,
    Organizer,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Reader, Role::Organizer];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Organizer => "organizer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    /// What an access approved before `reads` kept: it reads as `Subject` and `Body`.
    ReadBodies,
    /// Leaving a list reaches the list's sender, outside the mailbox.
    Unsubscribe,
}

impl Privilege {
    pub const ALL: [Privilege; 1] = [Privilege::Unsubscribe];

    pub fn as_str(self) -> &'static str {
        match self {
            Privilege::ReadBodies => "read_bodies",
            Privilege::Unsubscribe => "unsubscribe",
        }
    }
}

/// A part of a message an app may read. Who sent it, to whom, when and its labels it always
/// sees; a snippet is the start of the body, so it is the body's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Subject,
    Body,
    /// Their names, kinds and sizes.
    Attachments,
}

impl Part {
    pub const ALL: [Part; 3] = [Part::Subject, Part::Body, Part::Attachments];

    pub fn as_str(self) -> &'static str {
        match self {
            Part::Subject => "subject",
            Part::Body => "body",
            Part::Attachments => "attachments",
        }
    }
}

/// What mail must be like, kind by kind. A kind's entries are alternatives. As `only`, a thread
/// is reached when one of its messages meets every kind given; as `never`, a thread is out when
/// one of its messages meets any entry of any kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Conditions {
    /// Senders: an address, `@domain` for everyone there and at its subdomains, or other text
    /// the sender's name or address contains.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<String>,
    /// Recipients in To, Cc or Bcc, as `from`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<String>,
    /// Label ids, or a mailbox: `inbox`, `sent`, `drafts`, `trash`, `spam`, or `archive` for mail
    /// in none of inbox, trash and spam.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Text the subject contains, case aside.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject: Vec<String>,
    /// Words the body has, each entry a phrase.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<String>,
    /// Text an attachment's file name contains, case aside: `.pdf`, `invoice`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachment: Vec<String>,
    /// An age (`30d`, `2w`, `6m`, `1y`): mail newer than that.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newer_than: Option<String>,
    /// An age: mail older than that.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub older_than: Option<String>,
}

/// The mailboxes a `labels` entry may name instead of a label's id.
pub const MAILBOXES: [&str; 6] = ["inbox", "sent", "drafts", "trash", "spam", "archive"];

/// The most entries a kind of `Conditions` takes, and the longest an entry may be.
const MOST_ENTRIES: usize = 50;
const LONGEST_ENTRY: usize = 200;

impl Conditions {
    pub fn is_empty(&self) -> bool {
        *self == Conditions::default()
    }

    /// The lists of text entries, with what each is called.
    pub fn lists(&self) -> [(&'static str, &Vec<String>); 6] {
        [
            ("from", &self.from),
            ("to", &self.to),
            ("labels", &self.labels),
            ("subject", &self.subject),
            ("body", &self.body),
            ("attachment", &self.attachment),
        ]
    }

    /// Trimmed, lower case where case doesn't count, without repeats or empty entries; an error
    /// says what doesn't fit.
    pub fn normalized(mut self) -> Result<Self, String> {
        for (lower, list) in [
            (true, &mut self.from),
            (true, &mut self.to),
            (false, &mut self.labels),
            (true, &mut self.subject),
            (true, &mut self.body),
            (true, &mut self.attachment),
        ] {
            let mut seen = Vec::new();
            for entry in list.drain(..) {
                let entry = entry.trim();
                let entry = if lower { entry.to_lowercase() } else { entry.to_string() };
                if !entry.is_empty() && !seen.contains(&entry) {
                    seen.push(entry);
                }
            }
            if seen.len() > MOST_ENTRIES || seen.iter().any(|entry| entry.chars().count() > LONGEST_ENTRY) {
                return Err("That filter is too long.".into());
            }
            *list = seen;
        }
        for age in [&mut self.newer_than, &mut self.older_than] {
            if let Some(text) = age.take().map(|text| text.trim().to_lowercase()).filter(|text| !text.is_empty()) {
                age_days(&text).ok_or_else(|| format!("{text:?} isn't an age like 30d, 6m or 1y."))?;
                *age = Some(text);
            }
        }
        Ok(self)
    }
}

/// How many days an age is: `30d`, `2w`, `6m` (30 days each) or `1y` (365).
pub fn age_days(text: &str) -> Option<i64> {
    let unit = text.chars().last()?;
    let count: i64 = text[..text.len() - unit.len_utf8()].parse().ok().filter(|count| *count > 0)?;
    let days = match unit {
        'd' => 1,
        'w' => 7,
        'm' => 30,
        'y' => 365,
        _ => return None,
    };
    count.checked_mul(days).filter(|days| *days <= 365 * 100)
}

/// Whether an address (and its name) is what a `from` or `to` entry says: an address, an
/// `@domain` with its subdomains, or text in the name or the address. `entry` is lower case.
pub fn person_matches(entry: &str, email: &str, name: Option<&str>) -> bool {
    if entry.starts_with('@') || entry.contains('@') {
        return crate::rules::address_matches(entry, email);
    }
    let haystack = format!("{} {}", name.unwrap_or_default(), email).to_lowercase();
    haystack.contains(entry)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "Stored")]
pub struct Access {
    pub role: Role,
    /// Every account of the user's, the ones added later too. Otherwise only `accounts`.
    pub all_accounts: bool,
    pub accounts: Vec<String>,
    pub reads: Vec<Part>,
    pub privileges: Vec<Privilege>,
    #[serde(skip_serializing_if = "Conditions::is_empty")]
    pub only: Conditions,
    #[serde(skip_serializing_if = "Conditions::is_empty")]
    pub never: Conditions,
}

/// An access as it is written, by this version or one from before `reads`, which read the
/// subject always and the body with `read_bodies`.
#[derive(Deserialize)]
struct Stored {
    role: Role,
    all_accounts: bool,
    #[serde(default)]
    accounts: Vec<String>,
    #[serde(default)]
    reads: Option<Vec<Part>>,
    #[serde(default)]
    privileges: Vec<Privilege>,
    #[serde(default)]
    only: Conditions,
    #[serde(default)]
    never: Conditions,
}

impl From<Stored> for Access {
    fn from(stored: Stored) -> Self {
        let reads = stored.reads.unwrap_or_else(|| match stored.privileges.contains(&Privilege::ReadBodies) {
            true => vec![Part::Subject, Part::Body],
            false => vec![Part::Subject],
        });
        let privileges =
            stored.privileges.into_iter().filter(|privilege| *privilege != Privilege::ReadBodies).collect();
        Access {
            role: stored.role,
            all_accounts: stored.all_accounts,
            accounts: stored.accounts,
            reads,
            privileges,
            only: stored.only,
            never: stored.never,
        }
    }
}

impl Default for Access {
    /// What a request starts with: every account, all of it, the subject, and only looking.
    fn default() -> Self {
        Access {
            role: Role::Reader,
            all_accounts: true,
            accounts: Vec::new(),
            reads: vec![Part::Subject],
            privileges: Vec::new(),
            only: Conditions::default(),
            never: Conditions::default(),
        }
    }
}

impl Access {
    pub fn reaches(&self, account_id: &str) -> bool {
        self.all_accounts || self.accounts.iter().any(|id| id == account_id)
    }

    pub fn organizes(&self) -> bool {
        self.role == Role::Organizer
    }

    pub fn has(&self, privilege: Privilege) -> bool {
        self.privileges.contains(&privilege)
    }

    pub fn reads(&self, part: Part) -> bool {
        self.reads.contains(&part)
    }

    /// Only some of the accounts' mail: `only` or `never` says which.
    pub fn filtered(&self) -> bool {
        !self.only.is_empty() || !self.never.is_empty()
    }

    /// All of the user's mail: what settings that hold for all mail, as rules, need.
    pub fn whole(&self) -> bool {
        self.all_accounts && !self.filtered()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_access_from_before_parts() {
        let old: Access =
            serde_json::from_str(r#"{"role":"reader","all_accounts":true,"privileges":["read_bodies","unsubscribe"]}"#)
                .unwrap();
        assert_eq!(old.reads, [Part::Subject, Part::Body]);
        assert_eq!(old.privileges, [Privilege::Unsubscribe]);
        let older: Access = serde_json::from_str(r#"{"role":"reader","all_accounts":true}"#).unwrap();
        assert_eq!(older.reads, [Part::Subject]);
        let new: Access = serde_json::from_str(r#"{"role":"reader","all_accounts":true,"reads":[]}"#).unwrap();
        assert!(new.reads.is_empty());
    }

    #[test]
    fn writes_only_the_conditions_it_has() {
        let access = Access {
            never: Conditions { from: vec!["@chase.com".into()], ..Default::default() },
            ..Default::default()
        };
        let written = serde_json::to_value(&access).unwrap();
        assert_eq!(written["never"], serde_json::json!({ "from": ["@chase.com"] }));
        assert!(written.get("only").is_none());
        assert_eq!(serde_json::from_value::<Access>(written).unwrap(), access);
    }

    #[test]
    fn normalizes_conditions() {
        let conditions = Conditions {
            from: vec![" @Chase.com".into(), "@chase.com".into(), " ".into()],
            labels: vec!["Label-Id".into()],
            newer_than: Some(" 30D ".into()),
            ..Default::default()
        };
        let normalized = conditions.normalized().unwrap();
        assert_eq!(normalized.from, ["@chase.com"]);
        assert_eq!(normalized.labels, ["Label-Id"]);
        assert_eq!(normalized.newer_than.as_deref(), Some("30d"));
        let wrong = Conditions { older_than: Some("soon".into()), ..Default::default() };
        assert!(wrong.normalized().is_err());
    }

    #[test]
    fn reads_ages() {
        assert_eq!(age_days("30d"), Some(30));
        assert_eq!(age_days("2w"), Some(14));
        assert_eq!(age_days("6m"), Some(180));
        assert_eq!(age_days("1y"), Some(365));
        assert_eq!(age_days("0d"), None);
        assert_eq!(age_days("1x"), None);
        assert_eq!(age_days("999999999y"), None);
    }

    #[test]
    fn matches_people_by_address_domain_or_name() {
        assert!(person_matches("@chase.com", "alerts@mail.chase.com", None));
        assert!(!person_matches("@chase.com", "x@chasen.studio", None));
        assert!(person_matches("ann@x.com", "Ann@X.com", None));
        assert!(person_matches("twombly", "cy@example.com", Some("Cy Twombly")));
        assert!(!person_matches("zebra", "cy@example.com", Some("Cy")));
    }
}
