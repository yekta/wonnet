package app.wonnet.screens

import app.wonnet.core.Action
import app.wonnet.core.Filter
import app.wonnet.core.KeyCommand
import app.wonnet.core.MailStore
import app.wonnet.core.PaletteScope
import app.wonnet.core.SettingsEntry
import app.wonnet.core.SettingsSection
import app.wonnet.core.ThreadCommand
import app.wonnet.core.TimeChoice
import app.wonnet.core.applicable
import app.wonnet.core.archiveAll
import app.wonnet.core.canEmpty
import app.wonnet.core.clearSelection
import app.wonnet.core.emptyMailbox
import app.wonnet.core.keys
import app.wonnet.core.labels
import app.wonnet.core.openSettings
import app.wonnet.core.remindsInsteadOfSnoozing
import app.wonnet.core.run
import app.wonnet.core.selectAll
import app.wonnet.theme.Symbol
import kotlinx.coroutines.launch

/** One thing the palette can do. `keywords` are more it is found by, beyond its title. */
data class PaletteItem(
    val id: String,
    val title: String,
    val symbol: Symbol,
    val shortcut: String? = null,
    val keywords: String = "",
    val perform: () -> Unit,
)

/**
 * The command palette, Ctrl+K on a hardware keyboard: everything that can be done to the threads at
 * hand, and everywhere to go, found by typing a few of the letters of its name.
 */
object Palette {
    /** The command's first key, as the palette shows it. */
    private fun key(store: MailStore, id: String): String? {
        val command = KeyCommand.all.firstOrNull { it.id == id } ?: return null
        return store.keys(command).firstOrNull()?.let(::label)
    }

    /** What Shift types on each sign's key, on a US keyboard: `shift+3` reads #. */
    private val shifted = mapOf(
        "1" to "!", "2" to "@", "3" to "#", "4" to "$", "5" to "%", "6" to "^", "7" to "&", "8" to "*", "9" to "(", "0" to ")",
        "-" to "_", "=" to "+", "[" to "{", "]" to "}", "\\" to "|", ";" to ":", "'" to "\"", "," to "<", "." to ">", "/" to "?", "`" to "~",
    )

    /** The key as a line of text says it: "Ctrl+K", "G I", "#". */
    fun label(key: String): String = key.split(" ").joinToString(" ") { stroke ->
        val parts = stroke.split("+")
        val sign = shifted[parts.last()]
        if (parts.size == 2 && parts[0] == "shift" && sign != null) return@joinToString sign
        parts.joinToString("+") { part ->
            when (part) {
                "mod" -> "Ctrl"
                "ctrl" -> "Meta"
                "alt" -> "Alt"
                "shift" -> "Shift"
                "arrowup" -> "↑"
                "arrowdown" -> "↓"
                "enter" -> "Enter"
                "backspace" -> "Backspace"
                else -> part.uppercase()
            }
        }
    }

    fun items(store: MailStore, scope: PaletteScope, snoozeChoices: List<TimeChoice>): List<PaletteItem> {
        if (scope == PaletteScope.Labels) return labelItems(store)
        return threadItems(store, snoozeChoices) + generalItems(store) + mailboxItems(store) + labelItems(store) + settingsItems(store)
    }

    private fun threadItems(store: MailStore, snoozeChoices: List<TimeChoice>): List<PaletteItem> {
        val threads = store.targets
        if (threads.isEmpty()) return emptyList()
        val many = threads.size > 1
        val commands = store.applicable(
            ThreadCommand.entries.filter { command ->
                if (command == ThreadCommand.ExpandAll) !many && store.conversation?.id == threads.first() else !many || command !in ThreadCommand.openOnly
            },
            threads,
        )
        val suffix = if (many) " ${threads.size} threads" else ""
        val items = commands.map { command ->
            PaletteItem("command/${command.id}", command.title(store, threads) + suffix, command.symbol, key(store, command.id)) { store.run(command, threads) }
        }
        if (ThreadCommand.Snooze !in commands) return items
        val verb = if (store.remindsInsteadOfSnoozing) "Remind me" else "Snooze"
        return items + snoozeChoices.map { choice ->
            PaletteItem("snooze/${choice.id}", "$verb: ${choice.name} · ${choice.label}", Symbol.Clock) { store.act(Action.Snooze, threads, until = choice.until) }
        }
    }

    private fun generalItems(store: MailStore): List<PaletteItem> = buildList {
        add(PaletteItem("compose", "Compose", Symbol.SquarePen, key(store, "compose")) { store.newMessage() })
        add(PaletteItem("undo", "Undo", Symbol.Undo, key(store, "undo")) { store.undo() })
        add(PaletteItem("check-mail", "Check for new mail", Symbol.RefreshCw) { store.scope.launch { store.checkMail() } })
        add(PaletteItem("unread", if (store.filter == Filter.Unread) "Show everything" else "Unread only", Symbol.ListFilter, key(store, "unreadOnly")) { store.toggleFilter(Filter.Unread) })
        add(PaletteItem("starred", if (store.filter == Filter.Starred) "Show everything" else "Starred only", Symbol.Star, key(store, "starredOnly")) { store.toggleFilter(Filter.Starred) })
        if (store.searchRows == null && store.rows.isNotEmpty()) add(PaletteItem("zero", "Get Me To Zero: archive everything here", Symbol.Archive) { store.archiveAll() })
        if (store.canEmpty) add(PaletteItem("empty", "Empty ${store.mailboxName}", Symbol.Trash) { store.emptyMailbox() })
        if (store.conversation == null && store.visibleRows.isNotEmpty()) add(PaletteItem("select-all", "Select all", Symbol.SquareCheck, key(store, "selectAll")) { store.selectAll() })
        if (store.selection.isNotEmpty()) add(PaletteItem("select-none", "Clear the selection", Symbol.X, "Esc") { store.clearSelection() })
        if (store.splits.size > 1) {
            add(PaletteItem("split-next", "Next split", Symbol.ChevronRight, key(store, "nextSplit")) { store.moveSplit(1) })
            add(PaletteItem("split-previous", "Previous split", Symbol.ChevronLeft, key(store, "previousSplit")) { store.moveSplit(-1) })
        }
        add(PaletteItem("settings", "Settings", Symbol.Settings, key(store, "settings")) { store.openSettings() })
        add(PaletteItem("gallery", "Component gallery", Symbol.Monitor) { store.galleryOpen = true })
    }

    private fun mailboxItems(store: MailStore): List<PaletteItem> = buildList {
        for (mailbox in store.unified) {
            add(PaletteItem("go/${mailbox.id}", "Go to ${mailbox.name}", Symbol.named(mailbox.symbol), key(store, "go.${mailbox.id}")) { store.select(mailbox.id) })
        }
        for ((index, account) in store.accounts.withIndex()) {
            add(PaletteItem("go/${account.id}", "Go to ${account.title}", Symbol.Inbox, if (index < 9) key(store, "go.account.${index + 1}") else null) { store.select("${account.id}/inbox") })
            for (mailbox in account.mailboxes) {
                if (MailStore.labelID(mailbox.id) != null || mailbox.id.endsWith("/inbox")) continue
                add(PaletteItem("go/${mailbox.id}", "Go to ${mailbox.name} · ${account.title}", Symbol.named(mailbox.symbol)) { store.select(mailbox.id) })
            }
        }
    }

    private fun labelItems(store: MailStore): List<PaletteItem> {
        val several = store.accounts.size > 1
        return store.accounts.flatMap { account ->
            store.labels(account.id).map { label ->
                PaletteItem("go/${label.id}", if (several) "${label.name} · ${account.title}" else label.name, Symbol.Tag) { store.select(label.id) }
            }
        }
    }

    /** Every page of the settings, and every group on one, opened where it is. */
    private fun settingsItems(store: MailStore): List<PaletteItem> {
        val pages = SettingsSection.entries.filter { it != SettingsSection.General }.map { section ->
            PaletteItem("settings/${section.name}", "Settings: ${section.title}", section.symbol) { store.openSettings(section) }
        }
        val groups = SettingsEntry.all.filter { it.id != it.section.name.lowercase() }.map { entry ->
            PaletteItem("settings/${entry.section.name}/${entry.id}", "Settings: ${entry.section.title} › ${entry.title}", entry.section.symbol, keywords = entry.keywords) {
                store.openSettings(entry.section, entry.id)
            }
        }
        return pages + groups
    }

    /** The items whose titles have the letters typed, in order, best first, then those whose keywords have every word typed. */
    fun filter(items: List<PaletteItem>, query: String): List<PaletteItem> {
        val typed = query.lowercase().filter { !it.isWhitespace() }
        if (typed.isEmpty()) return items
        val words = query.split(Regex("\\s+")).filter { it.isNotEmpty() }
        return items.mapIndexedNotNull { index, item ->
            score(typed, item.title.lowercase())?.let { return@mapIndexedNotNull Triple(item, it, index) }
            if (item.keywords.isEmpty() || !words.all { item.keywords.contains(it, true) }) return@mapIndexedNotNull null
            Triple(item, 0, index)
        }.sortedWith(compareByDescending<Triple<PaletteItem, Int, Int>> { it.second }.thenBy { it.third }).map { it.first }
    }

    /** How well the letters match: more when they start words or follow each other. Null when they don't all appear in order. */
    fun score(typed: String, title: String): Int? {
        var score = 0
        var next = 0
        var previous = -2
        for ((index, character) in title.withIndex()) {
            if (next >= typed.length || character != typed[next]) continue
            val startsWord = index == 0 || !title[index - 1].isLetter()
            score += 1 + (if (startsWord) 3 else 0) + (if (index == previous + 1) 2 else 0)
            previous = index
            next++
        }
        return if (next == typed.length) score else null
    }
}

