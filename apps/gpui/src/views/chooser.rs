//! The sheets that are a field over a list: the command palette (Ctrl+K), the labels, the
//! snooze times, the times to send later or be reminded at, and the snippets. What is typed
//! narrows the list; the arrows move through it, Enter picks, Esc closes.

use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{App, Context, Entity, ScrollHandle, SharedString, Task, Window, div, px};
use gpui_component::input::{InputEvent, InputState};
use mail_core::api::Command;

use crate::models::{Action, TimeChoice, TimeChoices};
use crate::store::label_id;
use crate::store::{Labeling, PaletteScope, SettingsEntry, SettingsSection, Snippet, Store, ThreadCommand};
use crate::theme::*;
use crate::ui::{self, ChoiceRow, Lead, SheetSize, Symbol};

#[derive(Clone, Debug, PartialEq)]
pub enum ChooserKind {
    Palette(PaletteScope),
    Label(Labeling),
    Snooze(Vec<String>),
    SendLater,
    Remind,
    Snippet,
}

type Run = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
enum Pick {
    Run(Run),
    Time(TimeChoice),
    Snippet(Snippet),
}

#[derive(Clone)]
struct Item {
    id: String,
    title: String,
    detail: Option<String>,
    lead: Lead,
    keywords: String,
    pick: Pick,
}

impl Item {
    fn run(
        id: impl Into<String>,
        title: impl Into<String>,
        symbol: Symbol,
        detail: Option<String>,
        run: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail,
            lead: Lead::Symbol(symbol),
            keywords: String::new(),
            pick: Pick::Run(Rc::new(run)),
        }
    }
}

pub struct Chooser {
    store: Entity<Store>,
    kind: ChooserKind,
    query: Entity<InputState>,
    highlighted: usize,
    times: Vec<TimeChoice>,
    timing: Option<Task<()>>,
    scroll: ScrollHandle,
    /// Closes it, when someone other than the store opened it.
    pub closing: Option<Run>,
    pub picked_time: Option<Rc<dyn Fn(TimeChoice, &mut Window, &mut App)>>,
    pub picked_snippet: Option<Rc<dyn Fn(Snippet, &mut Window, &mut App)>>,
}

/// The chooser the store wants over the window, by a key that changes when it is another.
pub fn wanted(store: &Store) -> Option<(String, ChooserKind)> {
    if let Some(scope) = store.palette {
        return Some((format!("palette-{scope:?}"), ChooserKind::Palette(scope)));
    }
    if let Some(labeling) = &store.labeling {
        return Some((format!("label-{}", labeling.id), ChooserKind::Label(labeling.clone())));
    }
    store
        .snoozing
        .as_ref()
        .map(|threads| (format!("snooze-{}", threads.join(",")), ChooserKind::Snooze(threads.clone())))
}

pub fn close_all(store: &mut Store, cx: &mut Context<Store>) {
    store.palette = None;
    store.labeling = None;
    store.snoozing = None;
    store.contact = None;
    cx.notify();
}

impl Chooser {
    pub fn new(store: Entity<Store>, kind: ChooserKind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = match &kind {
            ChooserKind::Palette(PaletteScope::Labels) => "Go to a label",
            ChooserKind::Palette(_) => "Type a command or a place to go",
            ChooserKind::Label(_) => "Find or create a label",
            ChooserKind::Snooze(_) => "Tomorrow 9am, in 2 hours, fri",
            ChooserKind::SendLater | ChooserKind::Remind => "Tomorrow 9am, in 2 hours, Friday",
            ChooserKind::Snippet => "Find a snippet",
        };
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        query.update(cx, |query, cx| query.focus(window, cx));
        cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.highlighted = 0;
                this.ask_times(cx);
                cx.notify();
            }
        })
        .detach();
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let mut chooser = Self {
            store,
            kind,
            query,
            highlighted: 0,
            times: Vec::new(),
            timing: None,
            scroll: ScrollHandle::new(),
            closing: None,
            picked_time: None,
            picked_snippet: None,
        };
        chooser.ask_times(cx);
        chooser
    }

    /// The times what is typed says, or the usual ones, read by the core as it is typed.
    fn ask_times(&mut self, cx: &mut Context<Self>) {
        let wants = match &self.kind {
            ChooserKind::Snooze(_) | ChooserKind::SendLater | ChooserKind::Remind => true,
            ChooserKind::Palette(PaletteScope::Everything) => !self.store.read(cx).targets().is_empty(),
            _ => false,
        };
        if !wants {
            return;
        }
        let text = match &self.kind {
            ChooserKind::Palette(_) => String::new(),
            _ => self.query.read(cx).value().to_string(),
        };
        let wait = !text.is_empty();
        let answer = self.store.read(cx).core.call::<TimeChoices>(Command::ParseTime { text });
        self.timing = Some(cx.spawn(async move |this, cx| {
            if wait {
                cx.background_executor().timer(Duration::from_millis(120)).await;
            }
            let choices = answer.await.map(|found| found.choices).unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                this.times = choices;
                this.highlighted = 0;
                cx.notify();
            });
        }));
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        match &self.closing {
            Some(closing) => closing(window, cx),
            None => self.store.update(cx, close_all),
        }
    }

    fn title(&self, cx: &App) -> &'static str {
        match &self.kind {
            ChooserKind::Palette(PaletteScope::Labels) => "Go to a label",
            ChooserKind::Palette(_) => "Commands",
            ChooserKind::Label(labeling) if labeling.moving => "Move to",
            ChooserKind::Label(_) => "Label",
            ChooserKind::Snooze(_) if self.store.read(cx).reminds_instead_of_snoozing() => {
                "Remind me if nobody answers"
            }
            ChooserKind::Snooze(_) => "Snooze until",
            ChooserKind::SendLater => "Send later",
            ChooserKind::Remind => "Remind me if nobody replies",
            ChooserKind::Snippet => "Snippets",
        }
    }

    fn empty(&self, cx: &App) -> Option<&'static str> {
        let typed = !self.query.read(cx).value().is_empty();
        match &self.kind {
            ChooserKind::Palette(_) => Some("Nothing by that name."),
            ChooserKind::Label(_) => Some("No labels yet. Type a name to make one."),
            ChooserKind::Snooze(_) => typed.then_some("That isn't a time I know."),
            ChooserKind::SendLater | ChooserKind::Remind => typed.then_some("That isn't a time I can read."),
            ChooserKind::Snippet if self.store.read(cx).snippets().is_empty() => {
                Some("Add snippets in Settings, then type ; and a name to put one in.")
            }
            ChooserKind::Snippet => Some("No snippet by that name."),
        }
    }

    fn items(&self, cx: &App) -> Vec<Item> {
        let query = self.query.read(cx).value().to_string();
        match &self.kind {
            ChooserKind::Palette(scope) => filter(palette(&self.store, *scope, &self.times, cx), &query),
            ChooserKind::Label(labeling) => self.label_items(labeling, &query, cx),
            ChooserKind::Snooze(_) | ChooserKind::SendLater | ChooserKind::Remind => self
                .times
                .iter()
                .map(|time| Item {
                    id: time.id.clone(),
                    title: time.name.clone(),
                    detail: Some(time.label.clone()),
                    lead: Lead::None,
                    keywords: String::new(),
                    pick: Pick::Time(time.clone()),
                })
                .collect(),
            ChooserKind::Snippet => {
                let query = query.to_lowercase();
                self.store
                    .read(cx)
                    .snippets()
                    .into_iter()
                    .filter(|snippet| {
                        query.is_empty()
                            || snippet.name.to_lowercase().contains(&query)
                            || snippet.text.to_lowercase().contains(&query)
                    })
                    .map(|snippet| Item {
                        id: snippet.id.clone(),
                        title: snippet.name.clone(),
                        detail: Some(snippet.text.replace('\n', " ")),
                        lead: Lead::None,
                        keywords: String::new(),
                        pick: Pick::Snippet(snippet),
                    })
                    .collect()
            }
        }
    }

    /// The labels of the threads' account, and making one by what is typed.
    fn label_items(&self, labeling: &Labeling, query: &str, cx: &App) -> Vec<Item> {
        let store = self.store.read(cx);
        let Some(account) = labeling.threads.first().and_then(|thread| store.account_of(thread)) else {
            return Vec::new();
        };
        let labels = store.labels_of(&account);
        let applied: Vec<String> = match (&store.conversation, labeling.moving) {
            (Some(open), false) if labeling.threads == [open.id.clone()] => {
                open.labels.iter().map(|label| label.id.clone()).collect()
            }
            _ => Vec::new(),
        };
        let typed = query.trim().to_string();
        let mut items: Vec<Item> = labels
            .iter()
            .filter(|label| typed.is_empty() || label.name.to_lowercase().contains(&typed.to_lowercase()))
            .filter_map(|label| {
                let id = label_id(&label.id)?;
                let on = applied.contains(&id);
                let store = self.store.clone();
                let labeling = labeling.clone();
                let account = account.clone();
                Some(Item {
                    id: label.id.clone(),
                    title: label.name.clone(),
                    detail: on.then(|| "Remove".to_string()),
                    lead: Lead::Symbol(if on { Symbol::Check } else { Symbol::Tag }),
                    keywords: String::new(),
                    pick: Pick::Run(Rc::new(move |_, cx| {
                        store.update(cx, |store, cx| store.apply_label(id.clone(), &account, &labeling, on, cx))
                    })),
                })
            })
            .collect();
        if !typed.is_empty() && !labels.iter().any(|label| label.name.eq_ignore_ascii_case(&typed)) {
            let store = self.store.clone();
            let labeling = labeling.clone();
            let name = typed.clone();
            items.push(Item::run(
                format!("create/{typed}"),
                format!("Create label “{typed}”"),
                Symbol::Plus,
                None,
                move |_, cx| {
                    store
                        .update(cx, |store, cx| store.create_label(name.clone(), account.clone(), labeling.clone(), cx))
                },
            ));
        }
        items
    }

    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.items(cx);
        let Some(item) = items.get(index).cloned() else { return };
        match item.pick {
            Pick::Run(run) => {
                self.close(window, cx);
                run(window, cx);
            }
            Pick::Time(time) => match (&self.kind, self.picked_time.clone()) {
                (ChooserKind::Snooze(threads), _) => {
                    let threads = threads.clone();
                    self.close(window, cx);
                    self.store.update(cx, |store, cx| store.act(Action::Snooze, &threads, Some(time.until), None, cx));
                }
                (_, Some(picked)) => picked(time, window, cx),
                _ => self.close(window, cx),
            },
            Pick::Snippet(snippet) => match self.picked_snippet.clone() {
                Some(picked) => picked(snippet, window, cx),
                None => self.close(window, cx),
            },
        }
    }

    fn key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let count = self.items(cx).len();
        match key {
            "down" => self.highlighted = (self.highlighted + 1).min(count.saturating_sub(1)),
            "up" => self.highlighted = self.highlighted.saturating_sub(1),
            "enter" => {
                let index = self.highlighted;
                self.pick(index, window, cx);
                return true;
            }
            "escape" => {
                self.close(window, cx);
                return true;
            }
            _ => return false,
        }
        self.scroll.scroll_to_item(self.highlighted);
        cx.notify();
        true
    }
}

impl Render for Chooser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.items(cx);
        let this = cx.entity();
        let empty = items.is_empty().then(|| self.empty(cx)).flatten();
        let rows: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let pick = this.clone();
                let mut row = ChoiceRow::new(
                    SharedString::from(format!("choice-{}", item.id)),
                    item.title.clone(),
                    move |_, window, cx| pick.update(cx, |this, cx| this.pick(index, window, cx)),
                )
                .lead(item.lead.clone())
                .highlighted(index == self.highlighted);
                if let Some(detail) = item.detail {
                    row = row.detail(detail);
                }
                row.into_any_element()
            })
            .collect();
        let keys_target = this.clone();
        let keys: ui::FieldKeys =
            Rc::new(move |key, window, cx| keys_target.update(cx, |this, cx| this.key(key, window, cx)));
        let symbol = match &self.kind {
            ChooserKind::Label(_) => Symbol::Tag,
            ChooserKind::Snooze(_) | ChooserKind::SendLater | ChooserKind::Remind => Symbol::Clock,
            ChooserKind::Snippet => Symbol::Zap,
            ChooserKind::Palette(_) => Symbol::Search,
        };
        let clear = self.query.clone();
        let field = ui::search_field(
            "chooser-search",
            &self.query,
            ui::SearchStyle::Sheet,
            symbol,
            keys,
            Rc::new(move |window, cx| clear.update(cx, |query, cx| query.set_value("", window, cx))),
            cx,
        );
        let list = div()
            .id("chooser-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(px(space::S))
            .children(rows)
            .when_some(empty, |list, empty| {
                list.child(
                    div().p(px(space::S + 2.)).text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx).child(empty),
                )
            });
        let size = if matches!(self.kind, ChooserKind::Palette(_)) { SheetSize::Medium } else { SheetSize::Small };
        let close = this.clone();
        ui::sheet(
            "chooser",
            self.title(cx),
            size,
            BACKGROUND,
            Vec::new(),
            div().size_full().flex().flex_col().child(field).child(list),
            move |window, cx| close.update(cx, |this, cx| this.close(window, cx)),
            cx,
        )
    }
}

// MARK: The palette

fn palette(store: &Entity<Store>, scope: PaletteScope, times: &[TimeChoice], cx: &App) -> Vec<Item> {
    let mut items = Vec::new();
    if scope == PaletteScope::Everything {
        items.extend(thread_items(store, times, cx));
        items.extend(general_items(store, cx));
        items.extend(mailbox_items(store, cx));
    }
    items.extend(label_items(store, cx));
    if scope == PaletteScope::Everything {
        items.extend(settings_items(store, cx));
    }
    items
}

fn thread_items(store: &Entity<Store>, times: &[TimeChoice], cx: &App) -> Vec<Item> {
    let state = store.read(cx);
    let threads = state.targets();
    if threads.is_empty() {
        return Vec::new();
    }
    let many = threads.len() > 1;
    let open = state.conversation.as_ref().map(|open| open.id.clone());
    let commands: Vec<ThreadCommand> = ThreadCommand::ALL
        .into_iter()
        .filter(|command| match command {
            ThreadCommand::ExpandAll => !many && open.as_ref() == threads.first(),
            _ => !many || !ThreadCommand::OPEN_ONLY.contains(command),
        })
        .collect();
    let commands = state.applicable(&commands, &threads);
    let suffix = if many { format!(" {} threads", threads.len()) } else { String::new() };
    let mut items: Vec<Item> = commands
        .iter()
        .map(|command| {
            let run = store.clone();
            let threads = threads.clone();
            let command = *command;
            Item::run(
                format!("command/{}", command.id()),
                command.title(state, &threads) + &suffix,
                command.symbol(),
                state.key_label(command.id()),
                move |_, cx| run.update(cx, |store, cx| store.run(command, Some(threads.clone()), cx)),
            )
        })
        .collect();
    if commands.contains(&ThreadCommand::Snooze) {
        let verb = if state.reminds_instead_of_snoozing() { "Remind me" } else { "Snooze" };
        items.extend(times.iter().map(|time| {
            let run = store.clone();
            let threads = threads.clone();
            let until = time.until;
            Item::run(
                format!("snooze/{}", time.id),
                format!("{verb}: {} · {}", time.name, time.label),
                Symbol::Clock,
                None,
                move |_, cx| run.update(cx, |store, cx| store.act(Action::Snooze, &threads, Some(until), None, cx)),
            )
        }));
    }
    items
}

fn general_items(store: &Entity<Store>, cx: &App) -> Vec<Item> {
    let state = store.read(cx);
    let run = |id: &str, title: &str, symbol: Symbol, key: Option<String>, act: fn(&mut Store, &mut Context<Store>)| {
        let store = store.clone();
        Item::run(id, title, symbol, key, move |_, cx| store.update(cx, act))
    };
    let unread = state.filter == Some(crate::models::Filter::Unread);
    let starred = state.filter == Some(crate::models::Filter::Starred);
    let mut items = vec![
        run("compose", "Compose", Symbol::SquarePen, state.key_label("compose"), |store, cx| store.new_message(cx)),
        run("undo", "Undo", Symbol::Undo, state.key_label("undo"), |store, cx| store.undo(cx)),
        run("check-mail", "Check for new mail", Symbol::RefreshCw, None, |store, cx| store.check_mail(cx)),
        run(
            "unread",
            if unread { "Show everything" } else { "Unread only" },
            Symbol::ListFilter,
            state.key_label("unreadOnly"),
            |store, cx| store.toggle_filter(crate::models::Filter::Unread, cx),
        ),
        run(
            "starred",
            if starred { "Show everything" } else { "Starred only" },
            Symbol::Star,
            state.key_label("starredOnly"),
            |store, cx| store.toggle_filter(crate::models::Filter::Starred, cx),
        ),
    ];
    if state.search_rows.is_none() && !state.rows.is_empty() {
        items.push(run(
            "zero",
            "Get Me To Zero: archive everything here",
            Symbol::Archive,
            state.key_label("zero"),
            |store, cx| store.archive_all(cx),
        ));
    }
    if state.can_empty() {
        let title = format!("Empty {}", state.mailbox_name());
        let store = store.clone();
        items.push(Item::run("empty", title, Symbol::Trash, None, move |_, cx| {
            store.update(cx, |store, cx| store.empty_mailbox(cx))
        }));
    }
    if state.conversation.is_none() && !state.visible_rows().is_empty() {
        items.push(run("select-all", "Select all", Symbol::SquareCheck, state.key_label("selectAll"), |store, cx| {
            store.select_all(cx)
        }));
    }
    if !state.selection.is_empty() {
        items.push(run("select-none", "Clear the selection", Symbol::X, Some("Esc".into()), |store, cx| {
            store.clear_selection(cx)
        }));
    }
    if state.splits.len() > 1 {
        items.push(run("split-next", "Next split", Symbol::ChevronRight, state.key_label("nextSplit"), |store, cx| {
            store.move_split(1, cx)
        }));
        items.push(run(
            "split-previous",
            "Previous split",
            Symbol::ChevronLeft,
            state.key_label("previousSplit"),
            |store, cx| store.move_split(-1, cx),
        ));
    }
    items.push(run("settings", "Settings", Symbol::Settings, state.key_label("settings"), |store, cx| {
        store.open_settings(SettingsSection::General, None, cx)
    }));
    items.push(run("gallery", "Component gallery", Symbol::Monitor, None, |store, cx| {
        store.gallery_open = true;
        cx.notify();
    }));
    items
}

fn mailbox_items(store: &Entity<Store>, cx: &App) -> Vec<Item> {
    let state = store.read(cx);
    let go = |id: String, title: String, symbol: Symbol, key: Option<String>| {
        let store = store.clone();
        let mailbox = id.clone();
        Item::run(format!("go/{id}"), title, symbol, key, move |_, cx| {
            store.update(cx, |store, cx| store.select_mailbox(&mailbox, cx))
        })
    };
    let mut items: Vec<Item> = state
        .unified()
        .into_iter()
        .map(|mailbox| {
            go(
                mailbox.id.clone(),
                format!("Go to {}", mailbox.name),
                Symbol::named(&mailbox.symbol),
                state.key_label(&format!("go.{}", mailbox.id)),
            )
        })
        .collect();
    for (index, account) in state.accounts().into_iter().enumerate() {
        let key = (index < 9).then(|| state.key_label(&format!("go.account.{}", index + 1))).flatten();
        items.push(go(format!("{}/inbox", account.id), format!("Go to {}", account.title), Symbol::Inbox, key));
        for mailbox in &account.mailboxes {
            if label_id(&mailbox.id).is_some() || mailbox.id.ends_with("/inbox") {
                continue;
            }
            items.push(go(
                mailbox.id.clone(),
                format!("Go to {} · {}", mailbox.name, account.title),
                Symbol::named(&mailbox.symbol),
                None,
            ));
        }
    }
    items
}

fn label_items(store: &Entity<Store>, cx: &App) -> Vec<Item> {
    let state = store.read(cx);
    let accounts = state.accounts();
    let several = accounts.len() > 1;
    accounts
        .iter()
        .flat_map(|account| {
            state.labels_of(&account.id).into_iter().map(move |label| {
                let store = store.clone();
                let title = if several { format!("{} · {}", label.name, account.title) } else { label.name.clone() };
                let mailbox = label.id.clone();
                Item::run(format!("go/{}", label.id), title, Symbol::Tag, None, move |_, cx| {
                    store.update(cx, |store, cx| store.select_mailbox(&mailbox, cx))
                })
            })
        })
        .collect()
}

fn settings_items(store: &Entity<Store>, cx: &App) -> Vec<Item> {
    let state = store.read(cx);
    let pages =
        SettingsSection::ALL.into_iter().filter(|section| *section != SettingsSection::General).map(|section| {
            let store = store.clone();
            let key = (section == SettingsSection::Keybindings).then(|| state.key_label("keybindings")).flatten();
            Item::run(
                format!("settings/{section:?}"),
                format!("Settings: {}", section.title()),
                section.symbol(),
                key,
                move |_, cx| store.update(cx, |store, cx| store.open_settings(section, None, cx)),
            )
        });
    let groups = SettingsEntry::all()
        .into_iter()
        .filter(|entry| entry.id != format!("{:?}", entry.section).to_lowercase())
        .map(|entry| {
            let store = store.clone();
            let section = entry.section;
            let target = entry.id.clone();
            let mut item = Item::run(
                format!("settings/{section:?}/{}", entry.id),
                format!("Settings: {} › {}", section.title(), entry.title),
                section.symbol(),
                None,
                move |_, cx| store.update(cx, |store, cx| store.open_settings(section, Some(target.clone()), cx)),
            );
            item.keywords = entry.keywords.clone();
            item
        });
    pages.chain(groups).collect()
}

/// The items whose titles have the letters typed, in order, best first; then those whose
/// keywords have every word typed.
fn filter(items: Vec<Item>, query: &str) -> Vec<Item> {
    let typed: Vec<char> = query.to_lowercase().chars().filter(|character| !character.is_whitespace()).collect();
    if typed.is_empty() {
        return items;
    }
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut scored: Vec<(usize, i32, Item)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if let Some(score) = score(&typed, &item.title.to_lowercase().chars().collect::<Vec<_>>()) {
                return Some((index, score, item));
            }
            let keywords = item.keywords.to_lowercase();
            (!keywords.is_empty() && words.iter().all(|word| keywords.contains(word))).then_some((index, 0, item))
        })
        .collect();
    scored.sort_by(|one, other| other.1.cmp(&one.1).then(one.0.cmp(&other.0)));
    scored.into_iter().map(|(_, _, item)| item).collect()
}

/// How well the letters match: more when they start words or follow each other. None when they
/// don't all appear in order.
fn score(typed: &[char], title: &[char]) -> Option<i32> {
    let mut score = 0;
    let mut next = 0;
    let mut previous: isize = -2;
    for (index, character) in title.iter().enumerate() {
        if next >= typed.len() || *character != typed[next] {
            continue;
        }
        let starts_word = index == 0 || !title[index - 1].is_alphabetic();
        score += 1 + if starts_word { 3 } else { 0 } + if index as isize == previous + 1 { 2 } else { 0 };
        previous = index as isize;
        next += 1;
    }
    (next == typed.len()).then_some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_letters_that_start_words_higher() {
        let chars = |text: &str| text.chars().collect::<Vec<_>>();
        assert!(score(&chars("ar"), &chars("archive")) > score(&chars("ar"), &chars("clear")));
        assert_eq!(score(&chars("xyz"), &chars("archive")), None);
    }
}
