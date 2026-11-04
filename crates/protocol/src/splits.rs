//! The user's custom splits of the inbox, shown while Split Inbox is on (`split_inbox`): each is
//! a `split:<id>` preference. A thread is in the first split one of its messages matches.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::rules::address_matches;

pub const SPLIT_KEY: &str = "split:";
pub const SPLIT_INBOX_KEY: &str = "split_inbox";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Split {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    /// Senders: an address, or `@domain` for everyone there and at its subdomains.
    #[serde(default)]
    pub from: Vec<String>,
    /// A label's id: mail that has it is in the split.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub order: i64,
}

impl Split {
    /// Whether a message (its sender and labels) puts its thread in this split. The user's own
    /// mail is no sender of it.
    pub fn matches(&self, sender: &str, labels: &[String], me: &HashSet<String>) -> bool {
        let from = !me.contains(sender)
            && self
                .from
                .iter()
                .map(|pattern| pattern.trim().to_lowercase())
                .any(|pattern| address_matches(&pattern, sender));
        from || self.label.as_ref().is_some_and(|label| labels.contains(label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_mail_by_sender_or_label() {
        let me = HashSet::from(["me@shop.example".to_string()]);
        let split = Split { from: vec!["@Shop.example".into()], label: Some("orders".into()), ..Default::default() };
        assert!(split.matches("news@mail.shop.example", &[], &me));
        assert!(split.matches("a@b.c", &["orders".into()], &me));
        assert!(!split.matches("me@shop.example", &[], &me));
        assert!(!split.matches("a@b.c", &[], &me));
    }
}
