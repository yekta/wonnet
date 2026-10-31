package app.wonnet.core

import kotlinx.coroutines.launch

/** What a notification's button asks for. */
sealed interface NotificationAnswer {
    data object Archive : NotificationAnswer
    data object Read : NotificationAnswer
    data class Reply(val text: String) : NotificationAnswer
}

/**
 * Does what a notification's button asked for, then waits until the server has it: the app may
 * only have been woken for this. The core catches up first, for mail that came while it slept.
 */
suspend fun MailStore.answer(answer: NotificationAnswer, thread: String) {
    runCatching { bridge.call("settle") }
    val read = mapOf("action" to Action.Read.id, "threads" to listOf(thread))
    when (answer) {
        NotificationAnswer.Archive -> runCatching { bridge.call("act", mapOf("action" to Action.Archive.id, "threads" to listOf(thread))) }
        NotificationAnswer.Read -> runCatching { bridge.call("act", read) }
        is NotificationAnswer.Reply -> {
            val reply = runCatching {
                bridge.call("reply_draft", mapOf("thread" to thread, "kind" to ReplyKind.Reply.id)) { DraftReply.read(it.objectOrEmpty) }
            }.getOrNull() ?: return
            runCatching { bridge.call("act", read) }
            runCatching { bridge.call("send", mapOf("draft" to reply.draft.copy(text = answer.text).toJson(), "delay" to 0)) }
        }
    }
    runCatching { bridge.call("settle") }
}

// Selection

fun MailStore.toggleSelection(thread: String) {
    selection = if (thread in selection) selection - thread else selection + thread
    selectionAnchor = thread
    selected = thread
}

/** Selects the current row and the one `step` away, as Shift+J/K do. */
fun MailStore.extendSelection(step: Int) {
    if (conversation != null) return
    selected?.let {
        selection = selection + it
        if (selectionAnchor == null) selectionAnchor = it
    }
    move(step)
    selected?.let { selection = selection + it }
}

fun MailStore.selectAll() {
    if (conversation != null) return
    selection = visibleRows.map { it.id }.toSet()
}

fun MailStore.clearSelection() {
    selection = emptySet()
    selectionAnchor = null
}

// Commands

/** Does a command to the threads given, or to the selection or the current thread. */
fun MailStore.run(command: ThreadCommand, given: List<String>? = null) {
    val chosen = given ?: targets
    if (command == ThreadCommand.Trash) deleteDrafts(chosen.filter(MailStore::isDraft))
    val threads = chosen.filter { !MailStore.isDraft(it) }
    val first = threads.firstOrNull() ?: return
    when (command) {
        ThreadCommand.Archive -> act(Action.Archive, threads)
        ThreadCommand.Inbox -> act(Action.Inbox, threads)
        ThreadCommand.Trash -> act(Action.Trash, threads)
        ThreadCommand.Spam -> act(Action.Spam, threads)
        ThreadCommand.Star -> toggleStar(threads)
        ThreadCommand.Read -> toggleRead(threads)
        ThreadCommand.MarkRead -> act(Action.Read, threads)
        ThreadCommand.Snooze -> snoozing = threads
        ThreadCommand.Label -> labeling = Labeling(threads, move = false)
        ThreadCommand.Move -> labeling = Labeling(threads, move = true)
        ThreadCommand.Mute -> act(if (isMuted(threads)) Action.Unmute else Action.Mute, threads)
        ThreadCommand.Unsubscribe -> act(Action.Unsubscribe, threads)
        ThreadCommand.Block -> confirmBlock(threads)
        ThreadCommand.DeleteForever -> confirmDelete(threads)
        ThreadCommand.Print -> printThread(first)
        ThreadCommand.Reply -> reply(ReplyKind.Reply, first)
        ThreadCommand.ReplyAll -> reply(ReplyKind.ReplyAll, first)
        ThreadCommand.Forward -> reply(ReplyKind.Forward, first)
        ThreadCommand.ExpandAll -> toggleAllMessages()
    }
}

/** The commands that apply to these threads: deleting alone when they are all drafts. */
fun MailStore.applicable(commands: List<ThreadCommand>, threads: List<String>): List<ThreadCommand> {
    if (threads.isEmpty() || !threads.all(MailStore::isDraft)) return commands
    return commands.filter { it == ThreadCommand.Trash }
}

private fun MailStore.deleteDrafts(rows: List<String>) {
    if (rows.isEmpty()) return
    val leaving = rows.toSet()
    val ids = visibleRows.filter { it.id in leaving }.mapNotNull { it.draftId }
    selection = selection - leaving
    if (selected in leaving) selected = null
    scope.launch {
        try {
            for (id in ids) bridge.call("delete_draft", mapOf("draft_id" to id))
            show(if (ids.size == 1) "Draft deleted." else "${ids.size} drafts deleted.")
        } catch (error: CoreError) {
            show(error.message)
        }
    }
}

fun MailStore.isMuted(threads: List<String>): Boolean {
    val open = conversation ?: return false
    return threads == listOf(open.id) && open.muted
}

/** Whether the list is of mail the user sent, where a snooze is a reminder. */
val MailStore.remindsInsteadOfSnoozing: Boolean get() = baseMailbox.split("/").last() == "sent"

private fun MailStore.confirmBlock(threads: List<String>) {
    val senders = if (threads.size == 1) row(threads[0])?.senders ?: conversation?.from else null
    val who = senders?.let { "Block $it?" } ?: "Block these senders?"
    confirmation = Confirmation(who, "New mail from them goes straight to the trash, and so does this.", "Block") {
        act(Action.Block, threads)
    }
}

private fun MailStore.confirmDelete(threads: List<String>) {
    val what = if (threads.size == 1) "this conversation" else "these ${threads.size} conversations"
    confirmation = Confirmation("Delete $what forever?", "It's deleted from your mail account too. This can't be undone.", "Delete forever") {
        act(Action.Delete, threads)
    }
}

/** Whether the list is Trash or Spam, which can be emptied. */
val MailStore.canEmpty: Boolean
    get() {
        if (searchRows != null || rows.isEmpty()) return false
        val last = baseMailbox.split("/").last()
        return last == "trash" || last == "spam"
    }

/** Deletes everything in Trash or Spam for good, after asking. */
fun MailStore.emptyMailbox() {
    if (!canEmpty) return
    val mailbox = baseMailbox
    confirmation = Confirmation(
        "Empty $mailboxName?", "Everything in it is deleted from your mail account too. This can't be undone.", "Empty",
    ) {
        clearSelection()
        scope.launch {
            try {
                report(bridge.call("empty_mailbox", mapOf("mailbox" to mailbox)) { ActReply.read(it.objectOrEmpty) })
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }
}

/** Archives everything in the mailbox on screen, after asking: Get Me To Zero. */
fun MailStore.archiveAll() {
    if (searchRows != null) return
    val fields = mapOf("mailbox" to mailbox, "filter" to filter?.id, "keep" to keptRows)
    val what = when (filter) {
        Filter.Unread -> "every unread thread"
        Filter.Starred -> "every starred thread"
        null -> "everything"
    }
    confirmation = Confirmation("Archive $what in $shownName?", "Every thread shown here goes to the archive.", "Archive all") {
        clearSelection()
        scope.launch {
            try {
                report(bridge.call("archive_all", fields) { ActReply.read(it.objectOrEmpty) })
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }
}

/** The list's name as shown: a split's own name ("Important"), else the mailbox's. */
val MailStore.shownName: String
    get() {
        if (splits.isEmpty()) return mailboxName
        return (splits.firstOrNull { it.mailbox == mailbox } ?: splits[0]).name
    }

fun MailStore.printThread(thread: String) {
    scope.launch {
        try {
            Printer.print(bridge.call("print_thread", mapOf("thread" to thread)).string("html"))
        } catch (error: CoreError) {
            show(error.message)
        }
    }
}

fun MailStore.showPalette(scope: PaletteScope = PaletteScope.Everything) {
    if (sheetOpen) return
    palette = scope
}

// Labels

fun MailStore.account(thread: String): String? {
    val open = conversation
    if (open != null && open.id == thread) return open.accountId
    return row(thread)?.accountId
}

/** An account's labels, from its mailboxes. */
fun MailStore.labels(account: String): List<Mailbox> =
    accounts.firstOrNull { it.id == account }?.mailboxes?.filter { MailStore.labelID(it.id) != null } ?: emptyList()

/** Takes the label of the mailbox on screen off the threads at hand. */
fun MailStore.removeShownLabel() {
    val label = MailStore.labelID(baseMailbox) ?: return
    if (targets.isEmpty()) return
    act(Action.RemoveLabel, targets, label = label)
}

/** Adds an account's label to its threads, takes it off, or moves them to it. */
fun MailStore.apply(label: String, account: String, labeling: Labeling, remove: Boolean = false) {
    this.labeling = null
    val wanted = labeling.threads.toSet()
    val found = visibleRows.filter { it.id in wanted && it.accountId == account }.map { it.id }.toMutableSet()
    conversation?.let { if (it.accountId == account) found += it.id }
    val threads = labeling.threads.filter { it in found }
    act(if (labeling.move) Action.Move else if (remove) Action.RemoveLabel else Action.AddLabel, threads, label = label)
}

fun MailStore.createLabel(name: String, account: String, labeling: Labeling) {
    this.labeling = null
    scope.launch {
        try {
            val made = bridge.call("create_label", mapOf("account" to account, "name" to name)).string("id")
            apply(made, account, labeling)
        } catch (error: CoreError) {
            show(error.message)
        }
    }
}

/** The times a snooze can be until: the usual ones, or what `text` says. */
suspend fun MailStore.parseTime(text: String): List<TimeChoice> = runCatching {
    bridge.call("parse_time", mapOf("text" to text)) { it.objectOrEmpty.objects("choices").map(TimeChoice::read) }
}.getOrDefault(emptyList())
