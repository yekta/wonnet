package app.wonnet.core

import android.os.SystemClock
import android.view.KeyEvent

/** Where a command's keys work: anywhere, over the list, or in an open thread. */
enum class KeyScope {
    Everywhere, List, Thread;

    fun overlaps(other: KeyScope) = this == Everywhere || other == Everywhere || this == other
}

/**
 * Something a hardware keyboard can do, with the keys it has until the user changes them on the
 * Mac, whose keybindings sync. A key is written as the settings store it: `e`, `shift+e`, `mod+k`
 * (Ctrl+K here), `g i` (G, then I). A sign is the key it is on, on a US keyboard: `shift+3` is #.
 */
data class KeyCommand(val id: String, val scope: KeyScope = KeyScope.Everywhere, val defaults: List<String>) {
    val preferenceKey get() = "keys:$id"

    companion object {
        private fun key(id: String, vararg defaults: String, scope: KeyScope = KeyScope.Everywhere) = KeyCommand(id, scope, defaults.toList())

        val all: List<KeyCommand> = listOf(
            key("next", "j", "arrowdown"),
            key("previous", "k", "arrowup"),
            key("open", "enter", "o", scope = KeyScope.List),
            key("top", "mod+arrowup", scope = KeyScope.List),
            key("bottom", "mod+arrowdown", scope = KeyScope.List),
            key("nextMessage", "n", scope = KeyScope.Thread),
            key("previousMessage", "p", scope = KeyScope.Thread),
            key("expandAll", "shift+o", scope = KeyScope.Thread),
            key("nextSplit", "tab", scope = KeyScope.List),
            key("previousSplit", "shift+tab", scope = KeyScope.List),
            key("search", "/"),
            key("palette", "mod+k"),
            key("select", "x", scope = KeyScope.List),
            key("selectNext", "shift+j", "shift+arrowdown", scope = KeyScope.List),
            key("selectPrevious", "shift+k", "shift+arrowup", scope = KeyScope.List),
            key("selectAll", "mod+a", scope = KeyScope.List),
            key("archive", "e"),
            key("inbox", "shift+e"),
            key("snooze", "h"),
            key("star", "s"),
            key("read", "u"),
            key("markRead", "shift+i"),
            key("trash", "shift+3", "backspace"),
            key("spam", "shift+1"),
            key("mute", "shift+m"),
            key("label", "l"),
            key("removeLabel", "y"),
            key("move", "v"),
            key("unsubscribe", "mod+u"),
            key("undo", "z", "mod+z"),
            key("print", "mod+p"),
            key("compose", "c", "mod+n"),
            key("reply", "r"),
            key("replyAll", "a"),
            key("replyAllInThread", "enter", scope = KeyScope.Thread),
            key("forward", "f"),
            key("go.inbox", "g i"),
            key("go.starred", "g s"),
            key("go.sent", "g t"),
            key("go.drafts", "g d"),
            key("go.archive", "g e"),
            key("go.snoozed", "g h"),
            key("go.spam", "g shift+1"),
            key("go.trash", "g shift+3"),
            key("go.label", "g l"),
            key("unreadOnly", "shift+u", scope = KeyScope.List),
            key("starredOnly", "shift+s", scope = KeyScope.List),
            key("go.account.0", "ctrl+0"),
        ) + (1..9).map { key("go.account.$it", "ctrl+$it") } + listOf(
            key("zero", scope = KeyScope.List),
            key("settings", "mod+,"),
        )
    }
}

/** The command's keys: the user's, or its defaults. */
fun MailStore.keys(command: KeyCommand): List<String> =
    (preferences[command.preferenceKey] as? kotlinx.serialization.json.JsonArray)?.mapNotNull { it.text } ?: command.defaults

/**
 * The keys of a hardware keyboard, as the Mac's keybindings set them, ignored while typing or with
 * a sheet open. Esc closes what is on top.
 */
class KeyHandler(private val store: MailStore, private val focusSearch: () -> Unit) {
    /** The first key of a longer one, as G of G I, and when it was pressed. */
    private var started: Pair<String, Long>? = null

    /** Whether the key was taken. */
    fun handle(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false
        val plain = !event.isCtrlPressed && !event.isMetaPressed && !event.isAltPressed && !event.isShiftPressed
        if (plain && event.keyCode == KeyEvent.KEYCODE_ESCAPE) return escape()
        if (store.sheetOpen || !store.signedIn) return false
        val stroke = stroke(event) ?: return false
        val scope = if (store.conversation == null) KeyScope.List else KeyScope.Thread
        started?.let { (first, at) ->
            started = null
            val command = command("$first $stroke", scope)
            if (SystemClock.elapsedRealtime() - at < 1500 && command != null) return perform(command.id)
        }
        command(stroke, scope)?.let { return perform(it.id) }
        if (!startsSequence(stroke, scope)) return false
        started = stroke to SystemClock.elapsedRealtime()
        return true
    }

    private fun command(key: String, scope: KeyScope) = KeyCommand.all.firstOrNull { it.scope.overlaps(scope) && key in store.keys(it) }

    private fun startsSequence(stroke: String, scope: KeyScope) =
        KeyCommand.all.any { command -> command.scope.overlaps(scope) && store.keys(command).any { it.startsWith("$stroke ") } }

    /** Does the command, or says it doesn't apply here, so the key goes on. */
    private fun perform(id: String): Boolean {
        val inThread = store.conversation != null
        ThreadCommand.of(id)?.takeIf { it != ThreadCommand.ExpandAll }?.let {
            store.run(it)
            return true
        }
        if (id.startsWith("go.account.")) {
            val number = id.removePrefix("go.account.").toIntOrNull() ?: 0
            if (number != 0 && number > store.accounts.size) return false
            store.select(if (number == 0) "inbox" else "${store.accounts[number - 1].id}/inbox")
            return true
        }
        if (id == "go.label") {
            store.showPalette(PaletteScope.Labels)
            return true
        }
        if (id.startsWith("go.")) {
            store.go(id.removePrefix("go."))
            return true
        }
        when (id) {
            "next" -> store.move(1)
            "previous" -> store.move(-1)
            "open" -> {
                val current = store.selected
                if (inThread || current == null) return false
                store.open(current)
            }
            "top" -> store.move(-store.visibleRows.size)
            "bottom" -> store.move(store.visibleRows.size)
            "nextMessage", "previousMessage" -> {
                if (!inThread) return false
                store.moveMessage(if (id == "nextMessage") 1 else -1)
            }
            "expandAll" -> {
                if (!inThread) return false
                store.toggleAllMessages()
            }
            "nextSplit", "previousSplit" -> {
                if (store.splits.isEmpty() || inThread) return false
                store.moveSplit(if (id == "nextSplit") 1 else -1)
            }
            "search" -> focusSearch()
            "palette" -> store.showPalette()
            "select" -> {
                if (inThread) return false
                if (store.selected == null) store.move(1)
                store.selected?.let { store.toggleSelection(it) }
            }
            "selectNext" -> store.extendSelection(1)
            "selectPrevious" -> store.extendSelection(-1)
            "selectAll" -> {
                if (inThread || store.visibleRows.isEmpty()) return false
                store.selectAll()
            }
            "removeLabel" -> store.removeShownLabel()
            "undo" -> store.undo()
            "compose" -> store.newMessage()
            "replyAllInThread" -> {
                if (!inThread) return false
                store.reply(ReplyKind.ReplyAll)
            }
            "unreadOnly" -> store.toggleFilter(Filter.Unread)
            "starredOnly" -> store.toggleFilter(Filter.Starred)
            "zero" -> store.archiveAll()
            "settings" -> store.openSettings()
            else -> return false
        }
        return true
    }

    private fun escape(): Boolean {
        when {
            store.confirmation != null -> store.confirmation = null
            store.settings != null -> store.closeSettings()
            store.palette != null -> store.palette = null
            store.labeling != null -> store.labeling = null
            store.snoozing != null -> store.snoozing = null
            store.conversation != null -> store.close()
            store.selection.isNotEmpty() -> store.clearSelection()
            store.searchRows != null -> store.endSearch()
            store.filter != null -> store.narrow(null)
            else -> return false
        }
        return true
    }

    companion object {
        private val named = mapOf(
            KeyEvent.KEYCODE_ENTER to "enter", KeyEvent.KEYCODE_NUMPAD_ENTER to "enter", KeyEvent.KEYCODE_TAB to "tab",
            KeyEvent.KEYCODE_SPACE to "space", KeyEvent.KEYCODE_DEL to "backspace", KeyEvent.KEYCODE_FORWARD_DEL to "delete",
            KeyEvent.KEYCODE_DPAD_LEFT to "arrowleft", KeyEvent.KEYCODE_DPAD_RIGHT to "arrowright", KeyEvent.KEYCODE_DPAD_DOWN to "arrowdown",
            KeyEvent.KEYCODE_DPAD_UP to "arrowup", KeyEvent.KEYCODE_MOVE_HOME to "home", KeyEvent.KEYCODE_MOVE_END to "end",
            KeyEvent.KEYCODE_PAGE_UP to "pageup", KeyEvent.KEYCODE_PAGE_DOWN to "pagedown",
            KeyEvent.KEYCODE_MINUS to "-", KeyEvent.KEYCODE_EQUALS to "=", KeyEvent.KEYCODE_LEFT_BRACKET to "[",
            KeyEvent.KEYCODE_RIGHT_BRACKET to "]", KeyEvent.KEYCODE_BACKSLASH to "\\", KeyEvent.KEYCODE_SEMICOLON to ";",
            KeyEvent.KEYCODE_APOSTROPHE to "'", KeyEvent.KEYCODE_COMMA to ",", KeyEvent.KEYCODE_PERIOD to ".",
            KeyEvent.KEYCODE_SLASH to "/", KeyEvent.KEYCODE_GRAVE to "`",
        )

        /** The key with the modifiers held, as in `mod+shift+k`. Ctrl is the command key here; Meta stands for the Mac's Ctrl. */
        fun stroke(event: KeyEvent): String? {
            val code = event.keyCode
            val name = named[code] ?: when (code) {
                in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> ('a' + (code - KeyEvent.KEYCODE_A)).toString()
                in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> ('0' + (code - KeyEvent.KEYCODE_0)).toString()
                else -> return null
            }
            val held = listOf(event.isCtrlPressed to "mod", event.isMetaPressed to "ctrl", event.isAltPressed to "alt", event.isShiftPressed to "shift")
            return (held.filter { it.first }.map { it.second } + name).joinToString("+")
        }
    }
}
