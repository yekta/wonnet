//! What happens to mail new to the server as it is synced: mail from a blocked address or
//! `@domain` goes to the trash, the user's rules label it, take it out of the inbox or mark it
//! read, mail in a muted thread leaves the inbox, and mail from someone else ends the snooze of
//! its thread (which is what makes "remind me if no reply" work). The provider learns all but
//! the snooze, which is the server's own, through the usual ops.

use std::collections::{BTreeMap, HashSet};

use mail_protocol::rules::RULE_KEY;
use mail_protocol::{MessageState, Op, Rule, role};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

use crate::db::{AccountRow, UserTx, Written};

/// Applies the rules to messages just inserted. True when ops were queued for the provider.
pub async fn apply(tx: &mut UserTx, account: &AccountRow, new: &[Written]) -> sqlx::Result<bool> {
    if new.is_empty() {
        return Ok(false);
    }
    let own = account.own_addresses();
    let wanted: Vec<String> = new.iter().flat_map(|message| keys(account, message)).collect();
    let set: HashSet<String> = sqlx::query_scalar(
        "SELECT key FROM preferences WHERE user_id = $1 AND key = ANY($2) AND NOT deleted AND value <> 'false'",
    )
    .bind(account.user_id)
    .bind(&wanted)
    .fetch_all(&mut *tx.tx)
    .await?
    .into_iter()
    .collect();
    let rules = rules_for(tx, account).await?;

    // A message's ops are queued in the order they are applied to it: its state before each
    // is what the one before it left.
    let mut trash: Vec<(Uuid, MessageState)> = Vec::new();
    let mut labelled: BTreeMap<&str, Vec<(Uuid, MessageState)>> = BTreeMap::new();
    let mut archive: Vec<(Uuid, MessageState)> = Vec::new();
    let mut read: Vec<(Uuid, MessageState)> = Vec::new();
    let mut answered: Vec<&str> = Vec::new();
    for message in new {
        let mut state = message.state();
        let from_user = own.contains(&message.from_email);
        let [muted, address, domain] = keys(account, message);
        let blocked = !from_user && (set.contains(&address) || set.contains(&domain));
        if blocked && ![role::TRASH, role::SPAM, role::SENT].iter().any(|role| state.has(role)) {
            trash.push((message.id, state));
            continue;
        }
        let matched = if from_user || ruled_out(&state) { Vec::new() } else { matching(&rules, message) };
        for label in matched.iter().filter_map(|rule| rule.label.as_deref()) {
            if state.has(label) {
                continue;
            }
            labelled.entry(label).or_default().push((message.id, state.clone()));
            Op::AddLabel { ids: Vec::new(), label: label.into() }.apply(&mut state);
        }
        let leaves = set.contains(&muted) || matched.iter().any(|rule| rule.skip_inbox);
        if leaves && state.has(role::INBOX) {
            archive.push((message.id, state.clone()));
            Op::Archive { ids: Vec::new() }.apply(&mut state);
        }
        if state.unread && matched.iter().any(|rule| rule.mark_read) {
            read.push((message.id, state.clone()));
            Op::SetUnread { ids: Vec::new(), unread: false }.apply(&mut state);
        }
        if !from_user && state.has(role::INBOX) {
            answered.push(&message.thread_id);
        }
    }

    crate::db::apply_op(tx, &trash, &Op::Trash { ids: Vec::new() }).await?;
    for (label, messages) in &labelled {
        crate::db::apply_op(tx, messages, &Op::AddLabel { ids: Vec::new(), label: (*label).into() }).await?;
    }
    crate::db::apply_op(tx, &archive, &Op::Archive { ids: Vec::new() }).await?;
    crate::db::apply_op(tx, &read, &Op::SetUnread { ids: Vec::new(), unread: false }).await?;
    if !answered.is_empty() {
        sqlx::query(
            "UPDATE messages SET snoozed_until = NULL, rev = nextval('revs')
             WHERE account_id = $1 AND thread_id = ANY($2) AND snoozed_until IS NOT NULL AND NOT deleted",
        )
        .bind(account.id)
        .bind(&answered)
        .execute(&mut *tx.tx)
        .await?;
    }
    Ok(!trash.is_empty() || !labelled.is_empty() || !archive.is_empty() || !read.is_empty())
}

/// The user's rules that do something to this account's mail: a rule with a label holds for
/// the label's account only.
async fn rules_for(tx: &mut UserTx, account: &AccountRow) -> sqlx::Result<Vec<Rule>> {
    let values: Vec<Json<Value>> =
        sqlx::query_scalar("SELECT value FROM preferences WHERE user_id = $1 AND starts_with(key, $2) AND NOT deleted")
            .bind(account.user_id)
            .bind(RULE_KEY)
            .fetch_all(&mut *tx.tx)
            .await?;
    // A rule an app wrote wrong is left out rather than stopping the sync.
    let rules: Vec<Rule> = values
        .into_iter()
        .filter_map(|Json(value)| serde_json::from_value::<Rule>(value).ok())
        .filter(Rule::does_something)
        .collect();
    let labels: Vec<Uuid> =
        rules.iter().filter_map(|rule| rule.label.as_deref().and_then(|label| label.parse().ok())).collect();
    let own: HashSet<String> =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM labels WHERE account_id = $1 AND id = ANY($2) AND NOT deleted")
            .bind(account.id)
            .bind(&labels)
            .fetch_all(&mut *tx.tx)
            .await?
            .into_iter()
            .map(|id| id.to_string())
            .collect();
    Ok(rules.into_iter().filter(|rule| rule.label.as_ref().is_none_or(|label| own.contains(label))).collect())
}

/// Mail a rule leaves alone: in Sent, Drafts, Spam or Trash.
fn ruled_out(state: &MessageState) -> bool {
    [role::SENT, role::DRAFTS, role::SPAM, role::TRASH].iter().any(|role| state.has(role))
}

fn matching<'a>(rules: &'a [Rule], message: &Written) -> Vec<&'a Rule> {
    let recipients = &message.recipients.0;
    let to: Vec<&str> = recipients.to.iter().chain(&recipients.cc).map(|address| address.email.as_str()).collect();
    rules.iter().filter(|rule| rule.matches(&message.from_email, &to, &message.subject)).collect()
}

/// The preference keys that touch a message: its thread muted, its sender or their domain blocked.
fn keys(account: &AccountRow, message: &Written) -> [String; 3] {
    let domain = message.from_email.rsplit_once('@').map(|(_, domain)| domain).unwrap_or_default();
    [
        format!("muted:{}:{}", account.id, message.thread_id),
        format!("blocked:{}", message.from_email),
        format!("blocked:@{domain}"),
    ]
}
