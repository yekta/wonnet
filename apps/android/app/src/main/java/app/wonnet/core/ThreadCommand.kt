package app.wonnet.core

import app.wonnet.theme.Symbol
import app.wonnet.theme.ThemeColor
import app.wonnet.theme.Tokens

/** What can be done to threads, alike from the keys, the bars, the menus and the palette. */
enum class ThreadCommand(val id: String, val symbol: Symbol) {
    Archive("archive", Symbol.Archive),
    Inbox("inbox", Symbol.ArchiveRestore),
    Trash("trash", Symbol.Trash),
    DeleteForever("deleteForever", Symbol.Trash),
    Spam("spam", Symbol.ShieldAlert),
    Star("star", Symbol.Star),
    Read("read", Symbol.MailOpen),
    MarkRead("markRead", Symbol.CheckCheck),
    Snooze("snooze", Symbol.Clock),
    Label("label", Symbol.Tag),
    Move("move", Symbol.FolderInput),
    Mute("mute", Symbol.BellOff),
    Unsubscribe("unsubscribe", Symbol.MailMinus),
    Block("block", Symbol.Ban),
    Print("print", Symbol.Printer),
    Reply("reply", Symbol.Reply),
    ReplyAll("replyAll", Symbol.ReplyAll),
    Forward("forward", Symbol.Forward),
    ExpandAll("expandAll", Symbol.ChevronsUpDown),
    ;

    /** Its name for these threads: "Unstar" when they all are starred. */
    fun title(store: MailStore, threads: List<String>): String = when (this) {
        Archive -> "Archive"
        Inbox -> "Move to Inbox"
        Trash -> "Trash"
        DeleteForever -> "Delete forever"
        Spam -> "Spam"
        Star -> if (store.allStarred(threads)) "Unstar" else "Star"
        Read -> if (store.anyUnread(threads)) "Mark read" else "Mark unread"
        MarkRead -> "Mark read"
        Snooze -> if (store.remindsInsteadOfSnoozing) "Remind me" else "Snooze"
        Label -> "Label"
        Move -> "Move to"
        Mute -> if (store.isMuted(threads)) "Unmute" else "Mute"
        Unsubscribe -> "Unsubscribe"
        Block -> "Block sender"
        Print -> "Print"
        Reply -> "Reply"
        ReplyAll -> "Reply all"
        Forward -> "Forward"
        ExpandAll -> if (store.anyMessageFolded) "Expand all messages" else "Collapse all messages"
    }

    /** Its icon for these threads: what it would make of them. */
    fun symbol(store: MailStore, threads: List<String>): Symbol = when (this) {
        Read -> if (store.anyUnread(threads)) Symbol.MailOpen else Symbol.Mail
        Star -> if (store.allStarred(threads)) Symbol.StarFilled else Symbol.Star
        else -> symbol
    }

    /** Its name where it is picked for a swipe, which does it to any thread. */
    val swipeName: String
        get() = when (this) {
            Read -> "Read or unread"
            Star -> "Star or unstar"
            Snooze -> "Snooze"
            Move -> "Move to"
            Mute -> "Mute"
            else -> name.replaceFirstChar { it.uppercase() }
        }

    /** Its colour on a swipe, and its icon's on that colour. */
    val swipeTint: Pair<ThemeColor, ThemeColor>
        get() = when (this) {
            Read -> Tokens.primary to Tokens.primaryForeground
            Snooze -> Tokens.warning to Tokens.warningForeground
            Archive -> Tokens.success to Tokens.successForeground
            Trash, Spam -> Tokens.destructive to Tokens.destructiveForeground
            Star -> Tokens.star to Tokens.starForeground
            Mute -> Tokens.pending to Tokens.pendingForeground
            else -> Tokens.process to Tokens.processForeground
        }

    /** Its icon on a swipe over this row: what it would make of it. */
    fun swipeSymbol(row: ThreadRow): Symbol = if (this == Read) (if (row.unread) Symbol.MailOpen else Symbol.Mail) else symbol

    /** Whether doing it takes the thread out of the list at once, so its row slides away. */
    val takesRowAway: Boolean get() = this in listOf(Archive, Trash, Spam, Mute)

    companion object {
        fun of(id: String): ThreadCommand? = entries.firstOrNull { it.id == id }

        /** What doesn't fit on a bar: in its "more" menu. */
        val more = listOf(Star, Inbox, Move, Mute, Unsubscribe, Spam, Block, DeleteForever, Print, ExpandAll)

        /** What a bar over selected threads offers, and what its "more" menu does. */
        val selection = listOf(Archive, Trash, Snooze, Label, Read)
        val selectionMore = more.filter { it != Print && it != ExpandAll }

        /** What only the open thread can have done to it, one at a time. */
        val openOnly = listOf(Reply, ReplyAll, Forward, Print, ExpandAll)
    }
}
