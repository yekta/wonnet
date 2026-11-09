//! The mail a connected app reaches within its accounts, as SQL on `messages`: a thread is
//! reached when one of its messages meets every kind of the access's `only`, and no message of
//! it meets an entry of its `never`. The tests of a message are here too, for `tools.rs`.

use chrono::{DateTime, Duration, Utc};
use mail_protocol::access::{Access, Conditions, age_days};
use sqlx::{Postgres, QueryBuilder};

use crate::search;

pub const RECIPIENTS: &str = "jsonb_array_elements(coalesce(recipients->'to', '[]') \
    || coalesce(recipients->'cc', '[]') || coalesce(recipients->'bcc', '[]'))";
pub const CC: &str = "jsonb_array_elements(coalesce(recipients->'cc', '[]'))";

/// Whether the address in `email`, its name in `name`, is `entry`: an address, an @domain with
/// its subdomains, or text in the name or the address. `entry` is lower case already.
pub fn address_is(sql: &mut QueryBuilder<Postgres>, email: &str, name: &str, entry: &str) {
    if let Some(domain) = entry.strip_prefix('@') {
        let host = format!("split_part(lower({email}), '@', 2)");
        sql.push(format!("({host} = ")).push_bind(domain.to_string());
        sql.push(format!(" OR right({host}, length(")).push_bind(domain.to_string()).push(") + 1) = ");
        sql.push_bind(format!(".{domain}")).push(")");
        return;
    }
    if entry.contains('@') {
        sql.push(format!("lower({email}) = ")).push_bind(entry.to_string());
        return;
    }
    sql.push(format!("strpos(lower(coalesce({name}, '') || ' ' || {email}), "))
        .push_bind(entry.to_string())
        .push(") > 0");
}

pub fn recipient_is(sql: &mut QueryBuilder<Postgres>, among: &str, entry: &str) {
    sql.push(format!("EXISTS (SELECT 1 FROM {among} AS recipient WHERE "));
    address_is(sql, "(recipient->>'email')", "(recipient->>'name')", entry);
    sql.push(")");
}

pub fn subject_has(sql: &mut QueryBuilder<Postgres>, text: &str) {
    sql.push("strpos(lower(subject), ").push_bind(text.to_lowercase()).push(") > 0");
}

/// `words` as `search::lexemes` makes them: the body's weight, each word after the one before.
pub fn body_has(sql: &mut QueryBuilder<Postgres>, words: &str) {
    sql.push("search @@ to_tsquery('simple', ").push_bind(words.to_string()).push(")");
}

pub fn attachment_named(sql: &mut QueryBuilder<Postgres>, text: &str) {
    sql.push(
        "EXISTS (SELECT 1 FROM jsonb_array_elements(attachments) AS attachment \
         WHERE strpos(lower(attachment->>'name'), ",
    );
    sql.push_bind(text.to_lowercase()).push(") > 0)");
}

/// The body's words of an entry, for `body_has`; none when it has no words.
pub fn body_words(entry: &str) -> Option<String> {
    search::lexemes(entry, ":D", " <-> ")
}

/// One entry of `Conditions`, as a test of a message.
enum Test<'a> {
    Sender(&'a str),
    Recipient(&'a str),
    Label(&'a str),
    Subject(&'a str),
    Body(&'a str),
    Attachment(&'a str),
    Newer(DateTime<Utc>),
    Older(DateTime<Utc>),
}

/// The tests of each kind that has entries.
fn kinds(conditions: &Conditions, now: DateTime<Utc>) -> Vec<Vec<Test<'_>>> {
    let ago = |age: &Option<String>| age.as_deref().and_then(age_days).map(|days| now - Duration::days(days));
    let kinds: Vec<Vec<Test>> = vec![
        conditions.from.iter().map(|entry| Test::Sender(entry)).collect(),
        conditions.to.iter().map(|entry| Test::Recipient(entry)).collect(),
        conditions.labels.iter().map(|entry| Test::Label(entry)).collect(),
        conditions.subject.iter().map(|entry| Test::Subject(entry)).collect(),
        conditions.body.iter().map(|entry| Test::Body(entry)).collect(),
        conditions.attachment.iter().map(|entry| Test::Attachment(entry)).collect(),
        ago(&conditions.newer_than).map(Test::Newer).into_iter().collect(),
        ago(&conditions.older_than).map(Test::Older).into_iter().collect(),
    ];
    kinds.into_iter().filter(|tests| !tests.is_empty()).collect()
}

fn push_test(sql: &mut QueryBuilder<Postgres>, test: &Test) {
    match test {
        Test::Sender(entry) => address_is(sql, "from_email", "from_name", &entry.to_lowercase()),
        Test::Recipient(entry) => recipient_is(sql, RECIPIENTS, &entry.to_lowercase()),
        Test::Label("archive") => {
            sql.push("NOT labels && ARRAY['inbox', 'trash', 'spam']");
        }
        Test::Label(label) => {
            sql.push_bind(label.to_string()).push(" = ANY(labels)");
        }
        Test::Subject(text) => subject_has(sql, text),
        Test::Body(entry) => match body_words(entry) {
            Some(words) => body_has(sql, &words),
            None => {
                sql.push("false");
            }
        },
        Test::Attachment(text) => attachment_named(sql, text),
        Test::Newer(date) => {
            sql.push("date >= ").push_bind(*date);
        }
        Test::Older(date) => {
            sql.push("date < ").push_bind(*date);
        }
    }
}

fn push_any(sql: &mut QueryBuilder<Postgres>, tests: &[Test]) {
    sql.push("(");
    for (index, test) in tests.iter().enumerate() {
        if index > 0 {
            sql.push(" OR ");
        }
        push_test(sql, test);
    }
    sql.push(")");
}

/// Adds ` AND` the thread of each row of `table` (a query's name for `messages`) is reached.
pub fn push(sql: &mut QueryBuilder<Postgres>, access: &Access, table: &str) {
    let now = Utc::now();
    let thread = format!(
        "SELECT 1 FROM messages reach WHERE reach.account_id = {table}.account_id \
         AND reach.thread_id = {table}.thread_id AND NOT reach.deleted"
    );
    let only = kinds(&access.only, now);
    if !only.is_empty() {
        sql.push(format!(" AND EXISTS ({thread}"));
        for tests in &only {
            sql.push(" AND ");
            push_any(sql, tests);
        }
        sql.push(")");
    }
    let never: Vec<Test> = kinds(&access.never, now).into_iter().flatten().collect();
    if !never.is_empty() {
        sql.push(format!(" AND NOT EXISTS ({thread} AND "));
        push_any(sql, &never);
        sql.push(")");
    }
}
