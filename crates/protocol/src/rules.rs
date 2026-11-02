//! The user's rules, which the server applies to new mail and the apps to the mail already
//! there: each is a `rule:<id>` preference. A rule matches mail that meets every condition it
//! has, any one of a condition's entries; one without conditions matches nothing.

use serde::{Deserialize, Serialize};

pub const RULE_KEY: &str = "rule:";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// Senders: an address, or `@domain` for everyone there.
    pub from: Vec<String>,
    /// Recipients, To or Cc, as `from`.
    pub to: Vec<String>,
    /// Words the subject contains, case aside.
    pub subject: Vec<String>,
    /// A label's id: the rule then holds for its account only.
    pub label: Option<String>,
    pub skip_inbox: bool,
    pub mark_read: bool,
    /// Where it is listed. The apps write it as any number.
    #[serde(deserialize_with = "whole")]
    pub order: i64,
}

impl Rule {
    pub fn matches(&self, from: &str, recipients: &[&str], subject: &str) -> bool {
        let conditions = [&self.from, &self.to, &self.subject];
        if conditions.iter().all(|entries| entries.iter().all(|entry| entry.trim().is_empty())) {
            return false;
        }
        let subject = subject.to_lowercase();
        holds(&self.from, |pattern| address_matches(pattern, from))
            && holds(&self.to, |pattern| recipients.iter().any(|recipient| address_matches(pattern, recipient)))
            && holds(&self.subject, |words| subject.contains(words))
    }

    pub fn does_something(&self) -> bool {
        self.label.is_some() || self.skip_inbox || self.mark_read
    }
}

fn whole<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(f64::deserialize(deserializer)? as i64)
}

/// A condition without entries holds for every message.
fn holds(entries: &[String], test: impl Fn(&str) -> bool) -> bool {
    let entries: Vec<String> =
        entries.iter().map(|entry| entry.trim().to_lowercase()).filter(|entry| !entry.is_empty()).collect();
    entries.is_empty() || entries.iter().any(|entry| test(entry))
}

/// `pattern` is lower case already.
fn address_matches(pattern: &str, address: &str) -> bool {
    let address = address.trim().to_lowercase();
    match pattern.starts_with('@') {
        true => address.ends_with(pattern),
        false => address == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule() -> Rule {
        Rule { mark_read: true, ..Default::default() }
    }

    #[test]
    fn matches_mail_that_meets_every_condition() {
        let github =
            Rule { from: vec!["@GitHub.com".into()], to: vec!["dux-shell@noreply.github.com".into()], ..rule() };
        let to = ["me@example.com", "Dux-Shell@noreply.github.com"];
        assert!(github.matches("notifications@github.com", &to, "Re: [duxlabs-dev/dux-shell] Fix"));
        assert!(!github.matches("notifications@github.com", &["me@example.com"], "Fix"));
        assert!(!github.matches("someone@example.com", &to, "Fix"));
    }

    #[test]
    fn matches_any_entry_of_a_condition() {
        let words = Rule { subject: vec!["[dux-shell]".into(), "invoice".into()], ..rule() };
        assert!(words.matches("a@b.c", &[], "Your Invoice for May"));
        assert!(words.matches("a@b.c", &[], "[dux-shell] PR"));
        assert!(!words.matches("a@b.c", &[], "Hello"));
    }

    #[test]
    fn a_domain_is_not_part_of_an_address() {
        let domain = Rule { from: vec!["@hub.com".into()], ..rule() };
        assert!(domain.matches("x@hub.com", &[], ""));
        assert!(!domain.matches("x@github.com", &[], ""));
        let address = Rule { from: vec!["x@hub.com".into()], ..rule() };
        assert!(!address.matches("ax@hub.com", &[], ""));
    }

    #[test]
    fn reads_an_order_written_as_any_number() {
        let rule: Rule = serde_json::from_str(r#"{"from": ["@x.com"], "mark_read": true, "order": 2.0}"#).unwrap();
        assert_eq!(rule.order, 2);
    }

    #[test]
    fn a_rule_without_conditions_matches_nothing() {
        assert!(!Rule { from: vec![" ".into()], ..rule() }.matches("a@b.c", &[], "hi"));
    }
}
