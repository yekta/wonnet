//! What a connected app's access comes to, as the sheet that chooses it draws it: the sentence
//! that says it, the filter's chips, how much mail it reaches, and the menu that adds to the
//! filter. It is all counted on this device's copy of the mail, so it never waits.

use std::collections::{HashMap, HashSet};

use mail_protocol::Address;
use mail_protocol::access::{Access, Conditions, Part, Privilege, Role, age_days, person_matches};

use crate::api::{
    AccessSummary, FilterChip, FilterEdit, FilterKind, FilterLine, FilterOption, FilterOptions, FilterSection,
    FilterValue, SentencePart,
};
use crate::store::{LocalAccount, LocalLabel, ReachRow};

/// The mail the sheet counts: the messages of the accounts the access reaches, their labels and
/// the accounts themselves.
pub struct Mail {
    pub rows: std::sync::Arc<Vec<ReachRow>>,
    pub labels: Vec<LocalLabel>,
    pub accounts: Vec<LocalAccount>,
    /// When the counting is done, in Unix milliseconds: the ages are counted back from it.
    pub now: i64,
}

/// Finds the messages whose body has some words, as `search_id`s.
pub type BodySearch<'a> = &'a mut dyn FnMut(&str) -> HashSet<i64>;

const DAY: i64 = 86_400_000;

/// The mailboxes a label condition can name, in the order the menu shows them.
const MAILBOXES: [(&str, &str); 6] = [
    ("inbox", "Inbox"),
    ("sent", "Sent"),
    ("archive", "Archive"),
    ("drafts", "Drafts"),
    ("spam", "Spam"),
    ("trash", "Trash"),
];

const DATES: [&str; 5] = ["newer:30d", "newer:6m", "newer:1y", "older:1y", "older:3y"];

impl FilterKind {
    pub const ALL: [FilterKind; 7] = [
        FilterKind::From,
        FilterKind::To,
        FilterKind::Label,
        FilterKind::Subject,
        FilterKind::Body,
        FilterKind::Attachment,
        FilterKind::Date,
    ];

    fn title(self) -> &'static str {
        match self {
            FilterKind::From => "From",
            FilterKind::To => "To or Cc",
            FilterKind::Label => "Label",
            FilterKind::Subject => "Subject has",
            FilterKind::Body => "Body has",
            FilterKind::Attachment => "Attachment",
            FilterKind::Date => "Date",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            FilterKind::From => "A person or @domain",
            FilterKind::To => "A person or a list",
            FilterKind::Label => "Or a mailbox",
            FilterKind::Subject => "Exact text",
            FilterKind::Body => "Words",
            FilterKind::Attachment => "A file name or type",
            FilterKind::Date => "Older or newer than",
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            FilterKind::From => "user",
            FilterKind::To => "send",
            FilterKind::Label => "tag",
            FilterKind::Subject => "type",
            FilterKind::Body => "text",
            FilterKind::Attachment => "paperclip",
            FilterKind::Date => "calendar",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            FilterKind::From => "Search senders",
            FilterKind::To => "Search recipients",
            FilterKind::Label => "Search labels",
            FilterKind::Subject => "Text in the subject",
            FilterKind::Body => "Words in the body",
            FilterKind::Attachment => "A file name, or .pdf",
            FilterKind::Date => "Search dates",
        }
    }
}

/// Each condition as a kind and a value; an age is `newer:30d` or `older:1y`.
pub fn entries(conditions: &Conditions) -> Vec<FilterValue> {
    let mut entries = Vec::new();
    let mut push = |kind: FilterKind, list: &Vec<String>| {
        entries.extend(list.iter().map(|value| FilterValue { kind, value: value.clone() }));
    };
    push(FilterKind::From, &conditions.from);
    push(FilterKind::To, &conditions.to);
    push(FilterKind::Label, &conditions.labels);
    push(FilterKind::Subject, &conditions.subject);
    push(FilterKind::Body, &conditions.body);
    push(FilterKind::Attachment, &conditions.attachment);
    if let Some(age) = &conditions.newer_than {
        entries.push(FilterValue { kind: FilterKind::Date, value: format!("newer:{age}") });
    }
    if let Some(age) = &conditions.older_than {
        entries.push(FilterValue { kind: FilterKind::Date, value: format!("older:{age}") });
    }
    entries
}

fn list_of(conditions: &mut Conditions, kind: FilterKind) -> Option<&mut Vec<String>> {
    match kind {
        FilterKind::From => Some(&mut conditions.from),
        FilterKind::To => Some(&mut conditions.to),
        FilterKind::Label => Some(&mut conditions.labels),
        FilterKind::Subject => Some(&mut conditions.subject),
        FilterKind::Body => Some(&mut conditions.body),
        FilterKind::Attachment => Some(&mut conditions.attachment),
        FilterKind::Date => None,
    }
}

fn remove(conditions: &mut Conditions, value: &FilterValue) {
    if let Some(list) = list_of(conditions, value.kind) {
        list.retain(|entry| !entry.eq_ignore_ascii_case(&value.value));
        return;
    }
    match value.value.split_once(':') {
        Some(("newer", age)) if conditions.newer_than.as_deref() == Some(age) => conditions.newer_than = None,
        Some(("older", age)) if conditions.older_than.as_deref() == Some(age) => conditions.older_than = None,
        _ => {}
    }
}

fn add(conditions: &mut Conditions, value: &FilterValue) {
    let text = value.value.trim().to_string();
    if text.is_empty() {
        return;
    }
    if let Some(list) = list_of(conditions, value.kind) {
        if !list.iter().any(|entry| entry.eq_ignore_ascii_case(&text)) {
            list.push(text);
        }
        return;
    }
    match text.split_once(':') {
        Some(("newer", age)) if age_days(age).is_some() => conditions.newer_than = Some(age.into()),
        Some(("older", age)) if age_days(age).is_some() => conditions.older_than = Some(age.into()),
        _ => {}
    }
}

/// The access after an edit of its filter, tidied: label conditions of accounts it no longer
/// reaches are dropped, so the server takes it.
pub fn edited(mut access: Access, edit: Option<&FilterEdit>, labels: &[LocalLabel]) -> Access {
    if let Some(edit) = edit {
        let conditions = match edit.line {
            FilterLine::Only => &mut access.only,
            FilterLine::Never => &mut access.never,
        };
        if let Some(value) = &edit.remove {
            remove(conditions, value);
        }
        if let Some(value) = &edit.add {
            add(conditions, value);
        }
    }
    let known = |label: &String| {
        MAILBOXES.iter().any(|(mailbox, _)| mailbox == label)
            || labels.iter().any(|known| known.id == *label && access.reaches(&known.account_id))
    };
    let only: Vec<String> = access.only.labels.iter().filter(|label| known(label)).cloned().collect();
    let never: Vec<String> = access.never.labels.iter().filter(|label| known(label)).cloned().collect();
    access.only.labels = only;
    access.never.labels = never;
    access.only = access.only.clone().normalized().unwrap_or(access.only);
    access.never = access.never.clone().normalized().unwrap_or(access.never);
    access
}

/// The words of each body condition, searched once.
fn body_hits(values: &[FilterValue], body: BodySearch) -> HashMap<String, HashSet<i64>> {
    values
        .iter()
        .filter(|value| value.kind == FilterKind::Body)
        .map(|value| (value.value.clone(), body(&value.value)))
        .collect()
}

fn holds(row: &ReachRow, value: &FilterValue, hits: &HashMap<String, HashSet<i64>>, now: i64) -> bool {
    let text = value.value.to_lowercase();
    match value.kind {
        FilterKind::From => person_matches(&text, &row.from_email, row.from_name.as_deref()),
        FilterKind::To => {
            row.recipients.iter().any(|address| person_matches(&text, &address.email, address.name.as_deref()))
        }
        FilterKind::Label if value.value == "archive" => {
            !row.labels.iter().any(|label| ["inbox", "trash", "spam"].contains(&label.as_str()))
        }
        FilterKind::Label => row.labels.contains(&value.value),
        FilterKind::Subject => row.subject.to_lowercase().contains(&text),
        FilterKind::Body => hits.get(&value.value).is_some_and(|hits| hits.contains(&row.search_id)),
        FilterKind::Attachment => row.attachments.iter().any(|name| name.to_lowercase().contains(&text)),
        FilterKind::Date => match text.split_once(':') {
            Some(("newer", age)) => age_days(age).is_some_and(|days| row.date >= now - days * DAY),
            Some(("older", age)) => age_days(age).is_some_and(|days| row.date < now - days * DAY),
            _ => false,
        },
    }
}

/// How many messages the access reaches: those of threads where one message meets every kind
/// of `only` and none meets an entry of `never`.
pub fn reached(mail: &Mail, access: &Access, body: BodySearch) -> usize {
    let only = entries(&access.only);
    let never = entries(&access.never);
    if only.is_empty() && never.is_empty() {
        return mail.rows.len();
    }
    let all: Vec<FilterValue> = only.iter().chain(&never).cloned().collect();
    let hits = body_hits(&all, body);
    let kinds: Vec<Vec<&FilterValue>> = FilterKind::ALL
        .iter()
        .map(|kind| only.iter().filter(|value| value.kind == *kind).collect::<Vec<_>>())
        .filter(|values| !values.is_empty())
        .collect();
    let mut threads: HashMap<&str, (bool, bool, usize)> = HashMap::new();
    for row in mail.rows.iter() {
        let meets = kinds.iter().all(|values| values.iter().any(|value| holds(row, value, &hits, mail.now)));
        let kept_out = never.iter().any(|value| holds(row, value, &hits, mail.now));
        let thread = threads.entry(row.thread.as_str()).or_insert((false, false, 0));
        thread.0 |= meets;
        thread.1 |= kept_out;
        thread.2 += 1;
    }
    threads.values().filter(|(meets, kept_out, _)| *meets && !kept_out).map(|(.., count)| count).sum()
}

/// How many messages meet one condition.
fn matching(mail: &Mail, value: &FilterValue, hits: &HashMap<String, HashSet<i64>>) -> usize {
    mail.rows.iter().filter(|row| holds(row, value, hits, mail.now)).count()
}

pub fn thousands(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn messages(count: usize) -> String {
    match count {
        1 => "1 message".into(),
        _ => format!("{} messages", thousands(count)),
    }
}

/// "30 days", "2 weeks", "6 months", "a year".
fn age_words(age: &str) -> String {
    let Some(unit) = age.chars().last() else { return age.into() };
    let count: u32 = age[..age.len() - unit.len_utf8()].parse().unwrap_or(0);
    let word = match unit {
        'd' => "day",
        'w' => "week",
        'm' => "month",
        _ => "year",
    };
    match count {
        1 if word == "day" || word == "week" || word == "month" => format!("a {word}"),
        1 => "a year".into(),
        _ => format!("{count} {word}s"),
    }
}

fn date_words(value: &str) -> String {
    match value.split_once(':') {
        Some(("newer", age)) => format!("newer than {}", age_words(age)),
        Some(("older", age)) => format!("older than {}", age_words(age)),
        _ => value.into(),
    }
}

fn mailbox_name(value: &str) -> Option<&'static str> {
    MAILBOXES.iter().find(|(mailbox, _)| *mailbox == value).map(|(_, name)| *name)
}

fn label_name(labels: &[LocalLabel], value: &str) -> String {
    if let Some(name) = mailbox_name(value) {
        return name.into();
    }
    labels.iter().find(|label| label.id == value).map(|label| label.name.clone()).unwrap_or_else(|| "a label".into())
}

/// What a chip says: the word for its kind, and its value.
fn chip_words(value: &FilterValue, labels: &[LocalLabel]) -> (String, String) {
    match value.kind {
        FilterKind::From => ("from".into(), person_words(&value.value)),
        FilterKind::To => ("to".into(), person_words(&value.value)),
        FilterKind::Label if mailbox_name(&value.value).is_some() => ("in".into(), label_name(labels, &value.value)),
        FilterKind::Label => ("labelled".into(), label_name(labels, &value.value)),
        FilterKind::Subject => ("subject has".into(), format!("“{}”", value.value)),
        FilterKind::Body => ("body has".into(), format!("“{}”", value.value)),
        FilterKind::Attachment => ("file".into(), value.value.clone()),
        FilterKind::Date => (String::new(), date_words(&value.value)),
    }
}

/// An address or a domain as it is; other text in quotes, since it is found in names too.
fn person_words(value: &str) -> String {
    match value.contains('@') {
        true => value.into(),
        false => format!("“{value}”"),
    }
}

/// "Work", "Work and Personal", "every account".
pub fn accounts_words(access: &Access, accounts: &[LocalAccount]) -> String {
    if access.all_accounts {
        return "every account".into();
    }
    let names: Vec<String> = accounts
        .iter()
        .filter(|account| access.reaches(&account.id))
        .map(|account| if account.name.is_empty() { account.address.clone() } else { account.name.clone() })
        .collect();
    match names.as_slice() {
        [] => "no account".into(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn joined(words: &[String], last: &str) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., end] => format!("{} {last} {end}", rest.join(", ")),
    }
}

struct Sentence(Vec<SentencePart>);

impl Sentence {
    fn plain(&mut self, text: &str) {
        self.push(text, false);
    }

    fn strong(&mut self, text: &str) {
        self.push(text, true);
    }

    fn push(&mut self, text: &str, strong: bool) {
        match self.0.last_mut() {
            Some(last) if last.strong == strong => last.text.push_str(text),
            _ => self.0.push(SentencePart { text: text.into(), strong }),
        }
    }

    /// One kind's values: "from **a** or **b**", "with **“x”** in the subject".
    fn kind(&mut self, kind: FilterKind, values: &[&FilterValue], labels: &[LocalLabel]) {
        let words: Vec<String> = values.iter().map(|value| chip_words(value, labels).1).collect();
        let or = |sentence: &mut Sentence, words: &[String]| {
            for (index, word) in words.iter().enumerate() {
                if index > 0 {
                    sentence.plain(" or ");
                }
                sentence.strong(word);
            }
        };
        match kind {
            FilterKind::From => {
                self.plain("from ");
                or(self, &words);
            }
            FilterKind::To => {
                self.plain("sent to ");
                or(self, &words);
            }
            FilterKind::Label => {
                let (mailboxes, named): (Vec<&&FilterValue>, Vec<&&FilterValue>) =
                    values.iter().partition(|value| mailbox_name(&value.value).is_some());
                let words = |list: &[&&FilterValue]| -> Vec<String> {
                    list.iter().map(|value| label_name(labels, &value.value)).collect()
                };
                if !named.is_empty() {
                    self.plain("labelled ");
                    or(self, &words(&named));
                }
                if !mailboxes.is_empty() {
                    self.plain(if named.is_empty() { "in " } else { " or in " });
                    or(self, &words(&mailboxes));
                }
            }
            FilterKind::Subject => {
                self.plain("with ");
                or(self, &words);
                self.plain(" in the subject");
            }
            FilterKind::Body => {
                self.plain("with ");
                or(self, &words);
                self.plain(" in the body");
            }
            FilterKind::Attachment => {
                self.plain("with a file named ");
                or(self, &words);
            }
            FilterKind::Date => or(self, &words),
        }
    }
}

fn by_kind(values: &[FilterValue]) -> Vec<(FilterKind, Vec<&FilterValue>)> {
    FilterKind::ALL
        .iter()
        .map(|kind| (*kind, values.iter().filter(|value| value.kind == *kind).collect::<Vec<_>>()))
        .filter(|(_, values)| !values.is_empty())
        .collect()
}

/// "Claude Code will **read the subject and body** of mail in **Work**, **except** mail from
/// **@chase.com**, and can **archive, label, snooze and trash** it."
pub fn sentence(client: &str, access: &Access, accounts: &[LocalAccount], labels: &[LocalLabel]) -> Vec<SentencePart> {
    let mut sentence = Sentence(Vec::new());
    sentence.plain(&format!("{client} will "));
    if !access.all_accounts && !accounts.iter().any(|account| access.reaches(&account.id)) {
        sentence.strong("reach no mail");
        sentence.plain(" until you pick an account.");
        return sentence.0;
    }
    let parts: Vec<String> =
        Part::ALL.iter().filter(|part| access.reads(**part)).map(|part| part.as_str().to_string()).collect();
    match parts.is_empty() {
        true => {
            sentence.strong("see who sent and received");
            sentence.plain(" mail in ");
        }
        false => {
            sentence.strong(&format!("read the {}", joined(&parts, "and")));
            sentence.plain(" of mail in ");
        }
    }
    sentence.strong(&accounts_words(access, accounts));
    let only_entries = entries(&access.only);
    for (index, (kind, values)) in by_kind(&only_entries).iter().enumerate() {
        sentence.plain(if index == 0 { " " } else { " and " });
        sentence.kind(*kind, values, labels);
    }
    let never_entries = entries(&access.never);
    let never = by_kind(&never_entries);
    if !never.is_empty() {
        sentence.plain(", ");
        sentence.strong("except");
        sentence.plain(" mail ");
        for (index, (kind, values)) in never.iter().enumerate() {
            if index > 0 {
                sentence.plain(" or ");
            }
            sentence.kind(*kind, values, labels);
        }
    }
    let organizes = access.role == Role::Organizer;
    let unsubscribes = access.has(Privilege::Unsubscribe);
    match (organizes, unsubscribes) {
        (false, false) => sentence.plain(", and only looks"),
        (true, false) => {
            sentence.plain(", and can ");
            sentence.strong("archive, label, snooze and trash");
            sentence.plain(" it");
        }
        (false, true) => {
            sentence.plain(", and can ");
            sentence.strong("unsubscribe");
            sentence.plain(" from its lists");
        }
        (true, true) => {
            sentence.plain(", and can ");
            sentence.strong("archive, label, snooze and trash");
            sentence.plain(" it and ");
            sentence.strong("unsubscribe");
            sentence.plain(" from its lists");
        }
    }
    sentence.plain(".");
    sentence.0
}

pub fn plain(parts: &[SentencePart]) -> String {
    parts.iter().map(|part| part.text.as_str()).collect()
}

fn chips(
    mail: &Mail,
    values: &[FilterValue],
    line: FilterLine,
    hits: &HashMap<String, HashSet<i64>>,
) -> Vec<FilterChip> {
    let mut ordered: Vec<&FilterValue> = values.iter().collect();
    if line == FilterLine::Only {
        ordered.sort_by_key(|value| FilterKind::ALL.iter().position(|kind| *kind == value.kind));
    }
    let mut chips: Vec<FilterChip> = Vec::new();
    for (index, value) in ordered.iter().enumerate() {
        let joiner = match (index, line) {
            (0, _) => None,
            (_, FilterLine::Never) => Some("or".to_string()),
            (_, FilterLine::Only) if ordered[index - 1].kind == value.kind => Some("or".into()),
            (_, FilterLine::Only) => Some("and".into()),
        };
        let (label, text) = chip_words(value, &mail.labels);
        let count = matching(mail, value, hits);
        chips.push(FilterChip {
            kind: value.kind,
            value: value.value.clone(),
            label,
            text,
            joiner,
            matches: count,
            help: messages(count),
        });
    }
    chips
}

/// Everything the sheet draws about an access: the sentence, the chips, how much it reaches.
pub fn summary(client: &str, mail: &Mail, access: Access, body: BodySearch) -> AccessSummary {
    let only = entries(&access.only);
    let never = entries(&access.never);
    let all: Vec<FilterValue> = only.iter().chain(&never).cloned().collect();
    let hits = body_hits(&all, body);
    let where_ = accounts_words(&access, &mail.accounts);
    let total = mail.rows.len();
    let reached = reached(mail, &access, &mut |words: &str| hits.get(words).cloned().unwrap_or_default());
    let reaches_no_account = !access.all_accounts && !mail.accounts.iter().any(|account| access.reaches(&account.id));
    let only_chips = chips(mail, &only, FilterLine::Only, &hits);
    let never_chips = chips(mail, &never, FilterLine::Never, &hits);
    let warning = only_chips
        .iter()
        .chain(&never_chips)
        .find(|chip| chip.matches == 0)
        .map(|chip| format!("{} matches no mail in {where_}, so it changes nothing.", chip.text));
    AccessSummary {
        sentence: sentence(client, &access, &mail.accounts, &mail.labels),
        reach: match (reaches_no_account, total) {
            (true, _) => "Pick an account first".into(),
            (false, 0) => format!("No mail in {where_} yet"),
            (false, _) => format!("Reaches {} of {} in {where_}", thousands(reached), messages(total)),
        },
        fraction: if total == 0 { 0.0 } else { reached as f64 / total as f64 },
        only: only_chips,
        never: never_chips,
        only_empty: format!("All mail in {where_}"),
        never_empty: "Nothing kept out".into(),
        warning,
        problem: reaches_no_account.then(|| "Pick at least one account.".to_string()),
        access,
    }
}

/// What the rows say about the people of one side of the mail: their addresses with how much
/// mail each has, the most recent name, and their domains.
struct People {
    addresses: Vec<(String, Option<String>, usize)>,
    domains: Vec<(String, usize)>,
}

fn people(mail: &Mail, kind: FilterKind) -> People {
    let mut addresses: HashMap<String, (Option<String>, usize, i64)> = HashMap::new();
    let mut count = |email: &str, name: &Option<String>, date: i64| {
        if email.is_empty() {
            return;
        }
        let entry = addresses.entry(email.to_lowercase()).or_insert((None, 0, 0));
        entry.1 += 1;
        if date >= entry.2 && name.as_deref().is_some_and(|name| !name.trim().is_empty()) {
            entry.0 = name.clone();
            entry.2 = date;
        }
    };
    for row in mail.rows.iter() {
        match kind {
            FilterKind::From => count(&row.from_email, &row.from_name, row.date),
            _ => row.recipients.iter().for_each(|address| count(&address.email, &address.name, row.date)),
        }
    }
    let mut domains: HashMap<String, usize> = HashMap::new();
    for (email, (_, count, _)) in &addresses {
        if let Some((_, domain)) = email.rsplit_once('@') {
            *domains.entry(format!("@{domain}")).or_default() += count;
        }
    }
    let mut addresses: Vec<(String, Option<String>, usize)> =
        addresses.into_iter().map(|(email, (name, count, _))| (email, name, count)).collect();
    addresses.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut domains: Vec<(String, usize)> = domains.into_iter().collect();
    domains.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    People { addresses, domains }
}

struct Menu {
    sections: Vec<FilterSection>,
    editing: Option<FilterValue>,
}

impl Menu {
    fn section(&mut self, title: Option<&str>, options: Vec<FilterOption>) {
        if options.is_empty() {
            return;
        }
        let checked = |option: &FilterOption| {
            self.editing.as_ref().is_some_and(|editing| {
                option.kind == editing.kind
                    && option.value.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(&editing.value))
            })
        };
        let options = options.into_iter().map(|option| FilterOption { checked: checked(&option), ..option }).collect();
        self.sections.push(FilterSection { title: title.map(str::to_string), options });
    }
}

fn option(kind: FilterKind, value: &str, title: String, count: usize) -> FilterOption {
    FilterOption {
        id: format!("{}:{value}", kind.key()),
        kind,
        value: Some(value.into()),
        title,
        detail: None,
        symbol: Some(kind.symbol().into()),
        avatar: None,
        count: Some(thousands(count)),
        checked: false,
    }
}

impl FilterKind {
    fn key(self) -> &'static str {
        match self {
            FilterKind::From => "from",
            FilterKind::To => "to",
            FilterKind::Label => "label",
            FilterKind::Subject => "subject",
            FilterKind::Body => "body",
            FilterKind::Attachment => "attachment",
            FilterKind::Date => "date",
        }
    }
}

fn found(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(query)
}

fn people_options(mail: &Mail, kind: FilterKind, query: &str, scoped: bool, menu: &mut Menu) {
    let found_people = people(mail, kind);
    let domains: Vec<FilterOption> = found_people
        .domains
        .iter()
        .filter(|(domain, _)| found(domain, query))
        .take(3)
        .map(|(domain, count)| FilterOption {
            detail: Some("Everyone there".into()),
            symbol: Some("users".into()),
            ..option(kind, domain, domain.clone(), *count)
        })
        .collect();
    let take =
        if scoped { if domains.is_empty() { 6 } else { 4 } } else { 3usize.saturating_sub(domains.len()).max(1) };
    let persons: Vec<FilterOption> = found_people
        .addresses
        .iter()
        .filter(|(email, name, _)| found(email, query) || name.as_deref().is_some_and(|name| found(name, query)))
        .take(take)
        .map(|(email, name, count)| FilterOption {
            detail: name.as_ref().map(|_| email.clone()),
            symbol: None,
            avatar: Some(Address { name: name.clone(), email: email.clone() }),
            ..option(kind, email, name.clone().unwrap_or_else(|| email.clone()), *count)
        })
        .collect();
    match scoped {
        true => {
            menu.section(Some("Whole domains"), domains);
            menu.section(Some("People"), persons);
            let exact = found_people.addresses.iter().any(|(email, ..)| *email == query)
                || found_people.domains.iter().any(|(domain, _)| *domain == query);
            if !query.is_empty() && !exact {
                let value = FilterValue { kind, value: query.into() };
                let count = matching(mail, &value, &HashMap::new());
                let title = match query.contains('@') {
                    true => query.to_string(),
                    false => format!("Anyone whose name or address has “{query}”"),
                };
                menu.section(
                    None,
                    vec![FilterOption { symbol: Some("search".into()), ..option(kind, query, title, count) }],
                );
            }
        }
        false => {
            let title = if kind == FilterKind::From { "Senders" } else { "Recipients" };
            menu.section(Some(title), domains.into_iter().chain(persons).collect());
        }
    }
}

fn label_options(mail: &Mail, access: &Access, query: &str, scoped: bool, menu: &mut Menu) {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut archived = 0;
    for row in mail.rows.iter() {
        for label in &row.labels {
            *counts.entry(label.as_str()).or_default() += 1;
        }
        if !row.labels.iter().any(|label| ["inbox", "trash", "spam"].contains(&label.as_str())) {
            archived += 1;
        }
    }
    let many_accounts = mail.accounts.iter().filter(|account| access.reaches(&account.id)).count() > 1;
    let mut labels: Vec<FilterOption> = mail
        .labels
        .iter()
        .filter(|label| access.reaches(&label.account_id) && found(&label.name, query))
        .map(|label| {
            let account = mail.accounts.iter().find(|account| account.id == label.account_id);
            let detail = account
                .filter(|_| many_accounts)
                .map(|account| if account.name.is_empty() { account.address.clone() } else { account.name.clone() });
            FilterOption {
                detail,
                ..option(
                    FilterKind::Label,
                    &label.id,
                    label.name.clone(),
                    counts.get(label.id.as_str()).copied().unwrap_or(0),
                )
            }
        })
        .collect();
    labels.sort_by(|a, b| b.count_value().cmp(&a.count_value()).then_with(|| a.title.cmp(&b.title)));
    labels.truncate(if scoped { 8 } else { 3 });
    let mailboxes: Vec<FilterOption> = MAILBOXES
        .iter()
        .filter(|(_, name)| found(name, query))
        .map(|(mailbox, name)| {
            let count = if *mailbox == "archive" { archived } else { counts.get(mailbox).copied().unwrap_or(0) };
            FilterOption { symbol: Some("inbox".into()), ..option(FilterKind::Label, mailbox, name.to_string(), count) }
        })
        .take(if scoped { 6 } else { 2 })
        .collect();
    menu.section(Some("Labels"), labels);
    menu.section(Some("Mailboxes"), mailboxes);
}

impl FilterOption {
    fn count_value(&self) -> usize {
        self.count.as_deref().map(|count| count.replace(',', "")).and_then(|count| count.parse().ok()).unwrap_or(0)
    }
}

fn extensions(mail: &Mail) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for row in mail.rows.iter() {
        let mut seen = HashSet::new();
        for name in &row.attachments {
            if let Some((_, extension)) =
                name.rsplit_once('.').filter(|(stem, extension)| !stem.is_empty() && extension.len() <= 5)
            {
                let extension = format!(".{}", extension.to_lowercase());
                if seen.insert(extension.clone()) {
                    *counts.entry(extension).or_default() += 1;
                }
            }
        }
    }
    let mut counts: Vec<(String, usize)> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}

/// The menu of what to add to a line of the filter, or to change a chip to (`editing`): with no
/// kind and nothing typed, the kinds; typed, what in the mail matches, of every kind or of the
/// one picked. Each value has how many messages it matches.
pub fn options(
    mail: &Mail,
    access: &Access,
    line: FilterLine,
    kind: Option<FilterKind>,
    query: &str,
    editing: Option<FilterValue>,
    body: BodySearch,
) -> FilterOptions {
    let typed = query.trim().to_string();
    let query = typed.to_lowercase();
    let mut menu = Menu { sections: Vec::new(), editing };
    let placeholder = match (kind, line) {
        (Some(kind), _) => kind.placeholder(),
        (None, FilterLine::Only) => "Search your mail, or pick a kind",
        (None, FilterLine::Never) => "What to keep out",
    };
    if kind.is_none() && query.is_empty() {
        let kinds = FilterKind::ALL
            .iter()
            .map(|kind| FilterOption {
                id: format!("kind:{}", kind.key()),
                kind: *kind,
                value: None,
                title: kind.title().into(),
                detail: Some(kind.hint().into()),
                symbol: Some(kind.symbol().into()),
                avatar: None,
                count: None,
                checked: false,
            })
            .collect();
        menu.section(None, kinds);
        return FilterOptions { placeholder: placeholder.into(), scope: None, sections: menu.sections, note: None };
    }
    let scoped = kind.is_some();
    let kinds: Vec<FilterKind> = kind.map(|kind| vec![kind]).unwrap_or_else(|| FilterKind::ALL.to_vec());
    let mut words: Vec<FilterOption> = Vec::new();
    let mut note = None;
    for kind in kinds {
        match kind {
            FilterKind::From | FilterKind::To => people_options(mail, kind, &query, scoped, &mut menu),
            FilterKind::Label => label_options(mail, access, &query, scoped, &mut menu),
            FilterKind::Subject | FilterKind::Body if query.is_empty() => {
                note = Some("Type the words. Capitals don’t matter.".to_string());
            }
            FilterKind::Subject | FilterKind::Body => {
                let value = FilterValue { kind, value: typed.clone() };
                let hits = body_hits(std::slice::from_ref(&value), body);
                let count = matching(mail, &value, &hits);
                words.push(option(kind, &typed, format!("{} “{typed}”", kind.title()), count));
            }
            FilterKind::Attachment => {
                let mut files: Vec<FilterOption> = extensions(mail)
                    .into_iter()
                    .filter(|(extension, _)| found(extension, &query))
                    .take(if scoped { 6 } else { 2 })
                    .map(|(extension, count)| option(kind, &extension, extension.clone(), count))
                    .collect();
                if scoped
                    && !query.is_empty()
                    && !files.iter().any(|file| file.value.as_deref() == Some(query.as_str()))
                {
                    let value = FilterValue { kind, value: query.clone() };
                    let count = matching(mail, &value, &HashMap::new());
                    files.push(FilterOption {
                        symbol: Some("search".into()),
                        ..option(kind, &query, format!("Any file named “{typed}”"), count)
                    });
                }
                menu.section(Some("Files"), files);
            }
            FilterKind::Date => {
                let dates: Vec<FilterOption> = DATES
                    .iter()
                    .map(|value| (value, capitalized(&date_words(value))))
                    .filter(|(_, title)| found(title, &query))
                    .map(|(value, title)| {
                        let count = matching(mail, &FilterValue { kind, value: value.to_string() }, &HashMap::new());
                        option(kind, value, title, count)
                    })
                    .collect();
                if scoped || !query.is_empty() {
                    menu.section(Some("Dates"), dates);
                }
            }
        }
    }
    menu.section(if scoped { None } else { Some("Words") }, words);
    if menu.sections.is_empty() && note.is_none() {
        note = Some(format!("Nothing in your mail matches “{typed}”."));
    }
    FilterOptions {
        placeholder: placeholder.into(),
        scope: kind.map(|kind| kind.title().to_string()),
        sections: menu.sections,
        note,
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(thread: &str, from: &str, subject: &str, labels: &[&str], date: i64) -> ReachRow {
        ReachRow {
            thread: thread.into(),
            from_email: from.into(),
            from_name: None,
            recipients: vec![Address::new(None, "ann@acme.com")],
            cc: Vec::new(),
            subject: subject.into(),
            labels: labels.iter().map(|label| label.to_string()).collect(),
            attachments: Vec::new(),
            date,
            search_id: date,
        }
    }

    const NOW: i64 = 1_800_000_000_000;

    fn mail() -> Mail {
        Mail {
            rows: std::sync::Arc::new(vec![
                row("bank", "bob@example.com", "About the loan", &["inbox"], NOW - DAY),
                row("bank", "alerts@mail.chase.com", "Fwd: Your statement", &["inbox"], NOW - DAY),
                row("lunch", "cy@example.com", "Lunch plans", &["inbox", "clients"], NOW - 2 * DAY),
                row("old", "cy@example.com", "Old news", &[], NOW - 400 * DAY),
            ]),
            labels: vec![LocalLabel { id: "clients".into(), account_id: "work".into(), name: "Clients".into() }],
            accounts: vec![LocalAccount {
                id: "work".into(),
                name: "Work".into(),
                address: "ann@acme.com".into(),
                ..Default::default()
            }],
            now: NOW,
        }
    }

    fn none(_: &str) -> HashSet<i64> {
        HashSet::new()
    }

    fn with(only: Conditions, never: Conditions) -> Access {
        Access { only, never, ..Access::default() }
    }

    #[test]
    fn a_never_keeps_a_whole_thread_out() {
        let access = with(Conditions::default(), Conditions { from: vec!["@chase.com".into()], ..Default::default() });
        assert_eq!(reached(&mail(), &access, &mut none), 2);
    }

    #[test]
    fn only_takes_threads_where_a_message_meets_every_kind() {
        let both =
            Conditions { from: vec!["cy@example.com".into()], labels: vec!["clients".into()], ..Default::default() };
        assert_eq!(reached(&mail(), &with(both, Conditions::default()), &mut none), 1);
        let either = Conditions { from: vec!["cy@example.com".into(), "@chase.com".into()], ..Default::default() };
        assert_eq!(reached(&mail(), &with(either, Conditions::default()), &mut none), 4);
        let recent = Conditions { newer_than: Some("30d".into()), ..Default::default() };
        assert_eq!(reached(&mail(), &with(recent, Conditions::default()), &mut none), 3);
        let archived = Conditions { labels: vec!["archive".into()], ..Default::default() };
        assert_eq!(reached(&mail(), &with(archived, Conditions::default()), &mut none), 1);
    }

    #[test]
    fn edits_add_remove_and_replace() {
        let add = FilterEdit {
            line: FilterLine::Never,
            remove: None,
            add: Some(FilterValue { kind: FilterKind::From, value: " @Chase.com".into() }),
        };
        let access = edited(Access::default(), Some(&add), &mail().labels);
        assert_eq!(access.never.from, ["@chase.com"]);
        let age = FilterEdit {
            line: FilterLine::Only,
            remove: None,
            add: Some(FilterValue { kind: FilterKind::Date, value: "newer:6m".into() }),
        };
        let access = edited(access, Some(&age), &mail().labels);
        assert_eq!(access.only.newer_than.as_deref(), Some("6m"));
        let replace = FilterEdit {
            line: FilterLine::Only,
            remove: Some(FilterValue { kind: FilterKind::Date, value: "newer:6m".into() }),
            add: Some(FilterValue { kind: FilterKind::Date, value: "older:1y".into() }),
        };
        let access = edited(access, Some(&replace), &mail().labels);
        assert_eq!((access.only.newer_than.as_deref(), access.only.older_than.as_deref()), (None, Some("1y")));
        let gone = with(
            Conditions { labels: vec!["clients".into(), "elsewhere".into(), "inbox".into()], ..Default::default() },
            Conditions::default(),
        );
        assert_eq!(edited(gone, None, &mail().labels).only.labels, ["clients", "inbox"]);
    }

    #[test]
    fn says_it_in_one_sentence() {
        let access = Access {
            role: Role::Organizer,
            all_accounts: false,
            accounts: vec!["work".into()],
            reads: vec![Part::Subject, Part::Body],
            never: Conditions { from: vec!["@chase.com".into()], labels: vec!["clients".into()], ..Default::default() },
            ..Access::default()
        };
        let parts = sentence("Claude Code", &access, &mail().accounts, &mail().labels);
        assert_eq!(
            plain(&parts),
            "Claude Code will read the subject and body of mail in Work, except mail from @chase.com or labelled Clients, and can archive, label, snooze and trash it."
        );
        assert!(parts.iter().any(|part| part.strong && part.text == "@chase.com"));
        let nothing = Access { all_accounts: false, accounts: vec![], ..Access::default() };
        assert_eq!(
            plain(&sentence("Claude", &nothing, &mail().accounts, &[])),
            "Claude will reach no mail until you pick an account."
        );
        let looking = Access { reads: vec![], ..Access::default() };
        assert_eq!(
            plain(&sentence("Raycast", &looking, &[], &[])),
            "Raycast will see who sent and received mail in every account, and only looks."
        );
    }

    #[test]
    fn chips_read_as_a_sentence_and_warn_when_they_match_nothing() {
        let only = Conditions {
            from: vec!["zebra".into(), "cy@example.com".into()],
            labels: vec!["clients".into()],
            ..Default::default()
        };
        let summary = summary("Claude", &mail(), with(only, Conditions::default()), &mut none);
        let words: Vec<(Option<&str>, &str, &str)> =
            summary.only.iter().map(|chip| (chip.joiner.as_deref(), chip.label.as_str(), chip.text.as_str())).collect();
        assert_eq!(
            words,
            [(None, "from", "“zebra”"), (Some("or"), "from", "cy@example.com"), (Some("and"), "labelled", "Clients")]
        );
        assert_eq!(summary.only[0].matches, 0);
        assert_eq!(
            summary.warning.as_deref(),
            Some("“zebra” matches no mail in every account, so it changes nothing.")
        );
        assert_eq!(summary.reach, "Reaches 1 of 4 messages in every account");
    }

    #[test]
    fn the_menu_offers_kinds_then_what_matches() {
        let access = Access::default();
        let kinds = options(&mail(), &access, FilterLine::Only, None, "", None, &mut none);
        assert_eq!(kinds.sections[0].options.len(), 7);
        assert!(kinds.sections[0].options.iter().all(|option| option.value.is_none()));
        let typed = options(&mail(), &access, FilterLine::Only, None, "exam", None, &mut none);
        let titles: Vec<&str> = typed
            .sections
            .iter()
            .flat_map(|section| section.options.iter().map(|option| option.title.as_str()))
            .collect();
        assert_eq!(titles[0], "@example.com");
        assert!(titles.contains(&"Subject has “exam”"));
        let editing = FilterValue { kind: FilterKind::Label, value: "clients".into() };
        let labels = options(&mail(), &access, FilterLine::Only, Some(FilterKind::Label), "", Some(editing), &mut none);
        let checked: Vec<&str> = labels
            .sections
            .iter()
            .flat_map(|section| &section.options)
            .filter(|option| option.checked)
            .map(|option| option.title.as_str())
            .collect();
        assert_eq!(checked, ["Clients"]);
        let nothing = options(&mail(), &access, FilterLine::Only, Some(FilterKind::From), "zebra", None, &mut none);
        assert_eq!(nothing.sections[0].options[0].title, "Anyone whose name or address has “zebra”");
        assert_eq!(nothing.sections[0].options[0].count.as_deref(), Some("0"));
    }
}
