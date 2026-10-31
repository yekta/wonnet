//! What can be done to threads, alike from the keys, the toolbars, the menus and the palette.

use super::Store;
use crate::ui::Symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreadCommand {
    Archive,
    Inbox,
    Trash,
    DeleteForever,
    Spam,
    Star,
    Read,
    MarkRead,
    Snooze,
    Label,
    Move,
    Mute,
    Unsubscribe,
    Block,
    Print,
    Reply,
    ReplyAll,
    Forward,
    ExpandAll,
}

impl ThreadCommand {
    pub const ALL: [ThreadCommand; 19] = [
        Self::Archive,
        Self::Inbox,
        Self::Trash,
        Self::DeleteForever,
        Self::Spam,
        Self::Star,
        Self::Read,
        Self::MarkRead,
        Self::Snooze,
        Self::Label,
        Self::Move,
        Self::Mute,
        Self::Unsubscribe,
        Self::Block,
        Self::Print,
        Self::Reply,
        Self::ReplyAll,
        Self::Forward,
        Self::ExpandAll,
    ];

    /// What doesn't fit on a toolbar: in its "more" menu.
    pub const MORE: [ThreadCommand; 10] = [
        Self::Star,
        Self::Inbox,
        Self::Move,
        Self::Mute,
        Self::Unsubscribe,
        Self::Spam,
        Self::Block,
        Self::DeleteForever,
        Self::Print,
        Self::ExpandAll,
    ];

    /// What the bar over selected threads offers.
    pub const SELECTION: [ThreadCommand; 5] = [Self::Archive, Self::Trash, Self::Snooze, Self::Label, Self::Read];

    /// What only the open thread can have done to it.
    pub const OPEN_ONLY: [ThreadCommand; 5] =
        [Self::Reply, Self::ReplyAll, Self::Forward, Self::Print, Self::ExpandAll];

    /// The "more" menu over a selection.
    pub fn selection_more() -> Vec<ThreadCommand> {
        Self::MORE.into_iter().filter(|command| !matches!(command, Self::Print | Self::ExpandAll)).collect()
    }

    /// The id its keybinding has.
    pub fn id(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Inbox => "inbox",
            Self::Trash => "trash",
            Self::DeleteForever => "deleteForever",
            Self::Spam => "spam",
            Self::Star => "star",
            Self::Read => "read",
            Self::MarkRead => "markRead",
            Self::Snooze => "snooze",
            Self::Label => "label",
            Self::Move => "move",
            Self::Mute => "mute",
            Self::Unsubscribe => "unsubscribe",
            Self::Block => "block",
            Self::Print => "print",
            Self::Reply => "reply",
            Self::ReplyAll => "replyAll",
            Self::Forward => "forward",
            Self::ExpandAll => "expandAll",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    /// Its name for these threads: "Unstar" when they all are starred.
    pub fn title(self, store: &Store, threads: &[String]) -> String {
        match self {
            Self::Archive => "Archive",
            Self::Inbox => "Move to Inbox",
            Self::Trash => "Trash",
            Self::DeleteForever => "Delete forever",
            Self::Spam => "Spam",
            Self::Star if store.all_starred(threads) => "Unstar",
            Self::Star => "Star",
            Self::Read if store.any_unread(threads) => "Mark read",
            Self::Read => "Mark unread",
            Self::MarkRead => "Mark read",
            Self::Snooze if store.reminds_instead_of_snoozing() => "Remind me",
            Self::Snooze => "Snooze",
            Self::Label => "Label",
            Self::Move => "Move to",
            Self::Mute if store.is_muted(threads) => "Unmute",
            Self::Mute => "Mute",
            Self::Unsubscribe => "Unsubscribe",
            Self::Block => "Block sender",
            Self::Print => "Print",
            Self::Reply => "Reply",
            Self::ReplyAll => "Reply all",
            Self::Forward => "Forward",
            Self::ExpandAll if store.any_message_folded() => "Expand all messages",
            Self::ExpandAll => "Collapse all messages",
        }
        .to_string()
    }

    pub fn symbol(self) -> Symbol {
        match self {
            Self::Archive => Symbol::Archive,
            Self::Inbox => Symbol::ArchiveRestore,
            Self::Trash | Self::DeleteForever => Symbol::Trash,
            Self::Spam => Symbol::ShieldAlert,
            Self::Star => Symbol::Star,
            Self::Read => Symbol::MailOpen,
            Self::MarkRead => Symbol::CheckCheck,
            Self::Snooze => Symbol::Clock,
            Self::Label => Symbol::Tag,
            Self::Move => Symbol::FolderInput,
            Self::Mute => Symbol::BellOff,
            Self::Unsubscribe => Symbol::MailMinus,
            Self::Block => Symbol::Ban,
            Self::Print => Symbol::Printer,
            Self::Reply => Symbol::Reply,
            Self::ReplyAll => Symbol::ReplyAll,
            Self::Forward => Symbol::Forward,
            Self::ExpandAll => Symbol::ChevronsUpDown,
        }
    }
}
