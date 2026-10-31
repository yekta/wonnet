//! Selecting threads, running commands on them, labels, and the questions before what can't
//! be taken back.

use std::rc::Rc;

use gpui::{AppContext as _, Context};
use mail_core::api::Command;

use super::prefs::label_id;
use super::{Confirmation, Labeling, PaletteScope, Store, ThreadCommand, is_draft};
use crate::models::*;

impl Store {
    // MARK: Selection

    pub fn toggle_selection(&mut self, thread: &str, cx: &mut Context<Self>) {
        if !self.selection.remove(thread) {
            self.selection.insert(thread.to_string());
        }
        self.selection_anchor = Some(thread.to_string());
        self.selected = Some(thread.to_string());
        cx.notify();
    }

    /// Selects the rows from the anchor to this one, as a Shift-click does.
    pub fn extend_selection_to(&mut self, thread: &str, cx: &mut Context<Self>) {
        let list = self.visible_rows();
        let anchor = self.selection_anchor.clone().or_else(|| self.selected.clone());
        let from = anchor.and_then(|anchor| list.iter().position(|row| row.id == anchor));
        let to = list.iter().position(|row| row.id == thread);
        let (Some(from), Some(to)) = (from, to) else {
            return self.toggle_selection(thread, cx);
        };
        let range: Vec<String> = list[from.min(to)..=from.max(to)].iter().map(|row| row.id.clone()).collect();
        self.selection.extend(range);
        self.selected = Some(thread.to_string());
        cx.notify();
    }

    /// Selects the current row and the one `step` away, as Shift+J/K do.
    pub fn extend_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.conversation.is_some() {
            return;
        }
        if let Some(selected) = self.selected.clone() {
            self.selection.insert(selected.clone());
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(selected);
            }
        }
        self.move_by(step, cx);
        if let Some(selected) = self.selected.clone() {
            self.selection.insert(selected);
        }
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.conversation.is_some() {
            return;
        }
        self.selection = self.visible_rows().iter().map(|row| row.id.clone()).collect();
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selection_anchor = None;
        cx.notify();
    }

    // MARK: Commands

    /// Does a command to the threads given, or to the selection or the current thread.
    pub fn run(&mut self, command: ThreadCommand, given: Option<Vec<String>>, cx: &mut Context<Self>) {
        let chosen = given.unwrap_or_else(|| self.targets());
        if command == ThreadCommand::Trash {
            let drafts: Vec<String> = chosen.iter().filter(|thread| is_draft(thread)).cloned().collect();
            self.delete_drafts(drafts, cx);
        }
        let threads: Vec<String> = chosen.into_iter().filter(|thread| !is_draft(thread)).collect();
        let Some(first) = threads.first().cloned() else { return };
        match command {
            ThreadCommand::Archive => self.act(Action::Archive, &threads, None, None, cx),
            ThreadCommand::Inbox => self.act(Action::Inbox, &threads, None, None, cx),
            ThreadCommand::Trash => self.act(Action::Trash, &threads, None, None, cx),
            ThreadCommand::Spam => self.act(Action::Spam, &threads, None, None, cx),
            ThreadCommand::Star => self.toggle_star(&threads, cx),
            ThreadCommand::Read => self.toggle_read(&threads, cx),
            ThreadCommand::MarkRead => self.act(Action::Read, &threads, None, None, cx),
            ThreadCommand::Snooze => self.snoozing = Some(threads),
            ThreadCommand::Label => self.start_labeling(threads, false),
            ThreadCommand::Move => self.start_labeling(threads, true),
            ThreadCommand::Mute => {
                let action = if self.is_muted(&threads) { Action::Unmute } else { Action::Mute };
                self.act(action, &threads, None, None, cx);
            }
            ThreadCommand::Unsubscribe => self.act(Action::Unsubscribe, &threads, None, None, cx),
            ThreadCommand::Block => self.confirm_block(threads),
            ThreadCommand::DeleteForever => self.confirm_delete(threads),
            ThreadCommand::Print => self.print_thread(first, cx),
            ThreadCommand::Reply => self.reply(ReplyKind::Reply, Some(first), None, cx),
            ThreadCommand::ReplyAll => self.reply(ReplyKind::ReplyAll, Some(first), None, cx),
            ThreadCommand::Forward => self.reply(ReplyKind::Forward, Some(first), None, cx),
            ThreadCommand::ExpandAll => self.toggle_all_messages(cx),
        }
        cx.notify();
    }

    fn start_labeling(&mut self, threads: Vec<String>, moving: bool) {
        let id = self.next_id();
        self.labeling = Some(Labeling { id, threads, moving });
    }

    /// The commands that apply to these threads: deleting alone when they are all drafts.
    pub fn applicable(&self, commands: &[ThreadCommand], threads: &[String]) -> Vec<ThreadCommand> {
        if threads.is_empty() || !threads.iter().all(|thread| is_draft(thread)) {
            return commands.to_vec();
        }
        commands.iter().copied().filter(|command| *command == ThreadCommand::Trash).collect()
    }

    fn delete_drafts(&mut self, rows: Vec<String>, cx: &mut Context<Self>) {
        if rows.is_empty() {
            return;
        }
        let ids: Vec<String> = self
            .visible_rows()
            .iter()
            .filter(|row| rows.contains(&row.id))
            .filter_map(|row| row.draft_id.clone())
            .collect();
        for row in &rows {
            self.selection.remove(row);
        }
        if self.selected.as_ref().is_some_and(|selected| rows.contains(selected)) {
            self.selected = None;
        }
        let core = self.core.clone();
        let count = ids.len();
        cx.spawn(async move |this, cx| {
            let mut failed = None;
            for id in ids {
                if let Err(error) = core.call::<Empty>(Command::DeleteDraft { draft_id: id }).await {
                    failed = Some(error);
                    break;
                }
            }
            let _ = this.update(cx, |this, cx| match failed {
                Some(error) => this.show(error, cx),
                None if count == 1 => this.show("Draft deleted.", cx),
                None => this.show(format!("{count} drafts deleted."), cx),
            });
        })
        .detach();
    }

    pub fn is_muted(&self, threads: &[String]) -> bool {
        match (&self.conversation, threads) {
            (Some(open), [only]) if &open.id == only => open.muted,
            _ => false,
        }
    }

    /// Whether the list is of mail the user sent, where a snooze is a reminder.
    pub fn reminds_instead_of_snoozing(&self) -> bool {
        self.base_mailbox().rsplit('/').next() == Some("sent")
    }

    fn confirm_block(&mut self, threads: Vec<String>) {
        let senders = match threads.as_slice() {
            [only] => self
                .row(only)
                .map(|row| row.senders.clone())
                .or_else(|| self.conversation.as_ref().map(|open| open.from.clone())),
            _ => None,
        };
        let title = senders.map(|who| format!("Block {who}?")).unwrap_or_else(|| "Block these senders?".into());
        self.confirmation = Some(Confirmation {
            title,
            message: "New mail from them goes straight to the trash, and so does this.".into(),
            action: "Block".into(),
            cancel: "Cancel",
            safe: false,
            perform: Rc::new(move |this, cx| this.act(Action::Block, &threads, None, None, cx)),
        });
    }

    fn confirm_delete(&mut self, threads: Vec<String>) {
        let what = if threads.len() == 1 {
            "this conversation".to_string()
        } else {
            format!("these {} conversations", threads.len())
        };
        self.confirmation = Some(Confirmation {
            title: format!("Delete {what} forever?"),
            message: "It's deleted from your mail account too. This can't be undone.".into(),
            action: "Delete forever".into(),
            cancel: "Cancel",
            safe: false,
            perform: Rc::new(move |this, cx| this.act(Action::Delete, &threads, None, None, cx)),
        });
    }

    /// Whether the list is Trash or Spam, which can be emptied.
    pub fn can_empty(&self) -> bool {
        let base = self.base_mailbox();
        let last = base.rsplit('/').next().unwrap_or_default();
        self.search_rows.is_none() && !self.rows.is_empty() && (last == "trash" || last == "spam")
    }

    pub fn empty_mailbox(&mut self, cx: &mut Context<Self>) {
        if !self.can_empty() {
            return;
        }
        let mailbox = self.base_mailbox();
        self.confirmation = Some(Confirmation {
            title: format!("Empty {}?", self.mailbox_name()),
            message: "Everything in it is deleted from your mail account too. This can't be undone.".into(),
            action: "Empty".into(),
            cancel: "Cancel",
            safe: false,
            perform: Rc::new(move |this, cx| {
                this.clear_selection(cx);
                let command = Command::EmptyMailbox { mailbox: mailbox.clone() };
                this.ask_or_show(command, cx, |this, reply: ActReply, cx| this.report(reply, cx));
            }),
        });
        cx.notify();
    }

    /// Archives everything in the mailbox on screen, after asking: Get Me To Zero.
    pub fn archive_all(&mut self, cx: &mut Context<Self>) {
        if self.search_rows.is_some() {
            return;
        }
        let (mailbox, filter, keep) = (self.mailbox.clone(), self.filter, self.kept_rows().unwrap_or_default());
        let what = match self.filter {
            Some(Filter::Unread) => "every unread thread",
            Some(Filter::Starred) => "every starred thread",
            None => "everything",
        };
        self.confirmation = Some(Confirmation {
            title: format!("Archive {what} in {}?", self.shown_name()),
            message: "Every thread shown here goes to the archive.".into(),
            action: "Archive all".into(),
            cancel: "Cancel",
            safe: false,
            perform: Rc::new(move |this, cx| {
                this.clear_selection(cx);
                let command =
                    Command::ArchiveAll { mailbox: mailbox.clone(), before: None, filter, keep: keep.clone() };
                this.ask_or_show(command, cx, |this, reply: ActReply, cx| this.report(reply, cx));
            }),
        });
        cx.notify();
    }

    /// The list's name as shown: a split's own name, else the mailbox's.
    pub fn shown_name(&self) -> String {
        if self.splits.is_empty() {
            return self.mailbox_name();
        }
        self.splits.iter().find(|split| split.mailbox == self.mailbox).unwrap_or(&self.splits[0]).name.clone()
    }

    pub fn print_thread(&mut self, thread: String, cx: &mut Context<Self>) {
        self.ask_or_show(Command::PrintThread { thread }, cx, |_, reply: HtmlReply, cx| {
            let printed = cx.background_spawn(async move { crate::platform::print(&reply.html) });
            cx.spawn(async move |this, cx| {
                if let Err(error) = printed.await {
                    let _ = this.update(cx, |this, cx| this.show(error, cx));
                }
            })
            .detach();
        });
    }

    pub fn show_palette(&mut self, scope: PaletteScope, cx: &mut Context<Self>) {
        if self.sheet_open() {
            return;
        }
        self.palette = Some(scope);
        cx.notify();
    }

    // MARK: Labels

    pub fn account_of(&self, thread: &str) -> Option<String> {
        if let Some(open) = &self.conversation
            && open.id == thread
        {
            return Some(open.account_id.clone());
        }
        self.row(thread).map(|row| row.account_id.clone())
    }

    /// An account's labels, from its mailboxes.
    pub fn labels_of(&self, account: &str) -> Vec<Mailbox> {
        let accounts = self.accounts();
        let Some(found) = accounts.into_iter().find(|candidate| candidate.id == account) else { return Vec::new() };
        found.mailboxes.into_iter().filter(|mailbox| label_id(&mailbox.id).is_some()).collect()
    }

    /// Takes the label of the mailbox on screen off the threads at hand.
    pub fn remove_shown_label(&mut self, cx: &mut Context<Self>) {
        let Some(label) = label_id(&self.base_mailbox()) else { return };
        let targets = self.targets();
        if !targets.is_empty() {
            self.act(Action::RemoveLabel, &targets, None, Some(label), cx);
        }
    }

    /// Adds an account's label to its threads, takes it off, or moves them to it.
    pub fn apply_label(
        &mut self,
        label: String,
        account: &str,
        labeling: &Labeling,
        remove: bool,
        cx: &mut Context<Self>,
    ) {
        self.labeling = None;
        let threads = self.threads_in(&labeling.threads, account);
        let action = if labeling.moving {
            Action::Move
        } else if remove {
            Action::RemoveLabel
        } else {
            Action::AddLabel
        };
        self.act(action, &threads, None, Some(label), cx);
    }

    fn threads_in(&self, threads: &[String], account: &str) -> Vec<String> {
        threads.iter().filter(|thread| self.account_of(thread).as_deref() == Some(account)).cloned().collect()
    }

    pub fn create_label(&mut self, name: String, account: String, labeling: Labeling, cx: &mut Context<Self>) {
        self.labeling = None;
        let command = Command::CreateLabel { account: account.clone(), name };
        self.ask_or_show(command, cx, move |this, made: IdReply, cx| {
            this.apply_label(made.id, &account, &labeling, false, cx)
        });
    }
}
