package app.wonnet.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.wonnet.core.Action
import app.wonnet.core.Address
import app.wonnet.core.Labeling
import app.wonnet.core.MailStore
import app.wonnet.core.Mailbox
import app.wonnet.core.PaletteScope
import app.wonnet.core.Person
import app.wonnet.core.Snippet
import app.wonnet.core.TimeChoice
import app.wonnet.core.account
import app.wonnet.core.apply
import app.wonnet.core.closeSettings
import app.wonnet.core.createLabel
import app.wonnet.core.labels
import app.wonnet.core.parseTime
import app.wonnet.core.remindsInsteadOfSnoozing
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.Avatar
import app.wonnet.ui.ChoiceList
import app.wonnet.ui.ChoiceRow
import app.wonnet.ui.FormSection
import app.wonnet.ui.SearchField
import app.wonnet.ui.Sheet
import kotlinx.coroutines.delay

/** The sheets over everything, one at a time: the palette, the labels, the snooze times, the settings and the gallery. */
@Composable
fun Sheets(store: MailStore) {
    store.palette?.let { scope -> CommandPalette(store, scope) }
    store.labeling?.let { labeling -> androidx.compose.runtime.key(labeling.id) { LabelPicker(store, labeling) } }
    store.snoozing?.let { threads -> SnoozePicker(store, threads) }
    if (store.settings != null) SettingsSheet(store) { store.closeSettings() }
    if (store.galleryOpen) Gallery { store.galleryOpen = false }
}

/** A sheet's list and the field that narrows it, the field over the list. */
@Composable
fun ColumnScope.SearchedList(
    placeholder: String,
    query: String,
    onQuery: (String) -> Unit,
    symbol: Symbol = Symbol.Search,
    autofocus: Boolean = false,
    submit: () -> Unit,
    move: (Int) -> Unit,
    escape: () -> Unit,
    list: @Composable ColumnScope.() -> Unit,
) {
    SearchField(placeholder, query, onQuery, symbol = symbol, autofocus = autofocus, submit = submit, move = move, escape = escape)
    list()
}

/** The index `step` away, kept inside a list of `count`. */
fun Int.moved(step: Int, count: Int) = (this + step).coerceIn(0, (count - 1).coerceAtLeast(0))

/** When a snooze ends, or when a reminder comes: what the field says, read by the core as it is typed, or the usual times. */
@Composable
fun SnoozePicker(store: MailStore, threads: List<String>) {
    TimeSheet(
        store, if (store.remindsInsteadOfSnoozing) "Remind me if nobody answers" else "Snooze until", "Tomorrow 9am, in 2 hours, fri",
        "That isn't a time I know.", { store.snoozing = null },
    ) { choice ->
        store.snoozing = null
        store.act(Action.Snooze, threads, until = choice.until)
    }
}

/** When something should happen: the usual times, or what the user writes ("tomorrow 9am", "in 2 hours", "fri"), read by the core. */
@Composable
fun TimeSheet(store: MailStore, title: String, placeholder: String, empty: String, close: () -> Unit, pick: (TimeChoice) -> Unit) {
    var text by remember { mutableStateOf("") }
    var choices by remember { mutableStateOf<List<TimeChoice>>(emptyList()) }
    var highlighted by remember { mutableIntStateOf(0) }
    LaunchedEffect(text) {
        if (text.isNotEmpty()) delay(120)
        choices = store.parseTime(text)
        highlighted = 0
    }
    Sheet(title, close) {
        SearchedList(
            placeholder, text, { text = it }, Symbol.Clock, submit = { choices.getOrNull(highlighted)?.let(pick) },
            move = { highlighted = highlighted.moved(it, choices.size) }, escape = close,
        ) {
            ChoiceList(choices, highlighted, Modifier.weight(1f), if (text.isEmpty()) null else empty, key = { it.id }) { index, choice ->
                ChoiceRow(choice.name, detail = choice.label, highlighted = index == highlighted) { pick(choice) }
            }
        }
    }
}

/** The snippets to put in, narrowed by name as the user types. */
@Composable
fun SnippetPicker(snippets: List<Snippet>, close: () -> Unit, pick: (Snippet) -> Unit) {
    var query by remember { mutableStateOf("") }
    var highlighted by remember { mutableIntStateOf(0) }
    val matches = if (query.isEmpty()) snippets else snippets.filter { it.name.contains(query, true) || it.text.contains(query, true) }
    val empty = if (snippets.isEmpty()) "Add snippets in Settings, then type ; and a name to put one in." else "No snippet by that name."
    Sheet("Snippets", close) {
        SearchedList(
            "Find a snippet", query, { query = it; highlighted = 0 }, Symbol.Zap, submit = { matches.getOrNull(highlighted)?.let(pick) },
            move = { highlighted = highlighted.moved(it, matches.size) }, escape = close,
        ) {
            ChoiceList(matches, highlighted, Modifier.weight(1f), empty, key = { it.id }) { index, snippet ->
                ChoiceRow(snippet.name, detail = snippet.text.replace('\n', ' '), highlighted = index == highlighted) { pick(snippet) }
            }
        }
    }
}

/** A label, or a new one by the name typed. */
private sealed interface LabelChoice {
    val id: String

    data class Existing(val mailbox: Mailbox) : LabelChoice {
        override val id get() = mailbox.id
    }

    data class Create(val name: String) : LabelChoice {
        override val id get() = "create/$name"
    }
}

/** The labels of the threads' account, to add or take off, or to move the threads to; what is typed narrows them, or names a new one. */
@Composable
fun LabelPicker(store: MailStore, labeling: Labeling) {
    var query by remember { mutableStateOf("") }
    var highlighted by remember { mutableIntStateOf(0) }
    val account = labeling.threads.firstOrNull()?.let(store::account)
    val conversation = store.conversation
    val applied = if (!labeling.move && conversation != null && labeling.threads == listOf(conversation.id)) conversation.labels.map { it.id }.toSet() else emptySet()
    val labels = account?.let(store::labels) ?: emptyList()
    val typed = query.trim()
    val choices = buildList<LabelChoice> {
        addAll((if (typed.isEmpty()) labels else labels.filter { it.name.contains(typed, true) }).map { LabelChoice.Existing(it) })
        if (typed.isNotEmpty() && labels.none { it.name.equals(typed, true) }) add(LabelChoice.Create(typed))
    }
    fun pick(index: Int) {
        val target = account ?: return
        when (val choice = choices.getOrNull(index) ?: return) {
            is LabelChoice.Existing -> {
                val label = MailStore.labelID(choice.mailbox.id) ?: return
                store.apply(label, target, labeling, remove = label in applied)
            }
            is LabelChoice.Create -> store.createLabel(choice.name, target, labeling)
        }
    }
    Sheet(if (labeling.move) "Move to" else "Label", { store.labeling = null }) {
        SearchedList(
            "Find or create a label", query, { query = it; highlighted = 0 }, Symbol.Tag, submit = { pick(highlighted) },
            move = { highlighted = highlighted.moved(it, choices.size) }, escape = { store.labeling = null },
        ) {
            ChoiceList(choices, highlighted, Modifier.weight(1f), "No labels yet. Type a name to make one.", key = { it.id }) { index, choice ->
                when (choice) {
                    is LabelChoice.Existing -> {
                        val on = MailStore.labelID(choice.mailbox.id) in applied
                        ChoiceRow(choice.mailbox.name, detail = if (on) "Remove" else null, symbol = if (on) Symbol.Check else Symbol.Tag, highlighted = index == highlighted) { pick(index) }
                    }
                    is LabelChoice.Create -> ChoiceRow("Create label “${choice.name}”", symbol = Symbol.Plus, highlighted = index == highlighted) { pick(index) }
                }
            }
        }
    }
}

/**
 * Someone the thread is with: who they are, a way to write to them, and the latest threads with
 * them, which open on a tap. In a sheet, `closeSheet` closes it before compose opens.
 */
@Composable
fun PersonView(store: MailStore, email: String, closeSheet: () -> Unit, openThread: (String) -> Unit) {
    var person by remember { mutableStateOf<Person?>(null) }
    LaunchedEffect(email) { person = store.person(email) }
    val name = person?.name?.takeIf { it.isNotEmpty() } ?: email
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(Space.xl), verticalArrangement = Arrangement.spacedBy(Space.l)) {
        Avatar(person?.initials ?: email.take(1).uppercase(), email, 52f)
        Column(verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
            Text(name, TextStyle.Heading)
            SelectionContainer { Text(email, TextStyle.Label, color = Tokens.mutedStrongerForeground.color) }
        }
        ActionButton("New message", symbol = Symbol.SquarePen) {
            closeSheet()
            store.write(Address(person?.name?.takeIf { it.isNotEmpty() }, email))
        }
        val threads = person?.threads ?: emptyList()
        if (threads.isNotEmpty()) {
            Column(Modifier.padding(top = Space.s)) {
                FormSection("Recent") {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        for (row in threads) ChoiceRow(row.subject.ifEmpty { "(no subject)" }, detail = row.date) { openThread(row.id) }
                    }
                }
            }
        }
    }
}

/** What the palette opens with: everything, or only the labels to go to. */
@Composable
private fun CommandPalette(store: MailStore, scope: PaletteScope) {
    var query by remember { mutableStateOf("") }
    var highlighted by remember { mutableIntStateOf(0) }
    var snoozeChoices by remember { mutableStateOf<List<TimeChoice>>(emptyList()) }
    LaunchedEffect(Unit) {
        if (scope == PaletteScope.Everything && store.targets.isNotEmpty()) snoozeChoices = store.parseTime("")
    }
    val found = Palette.filter(Palette.items(store, scope, snoozeChoices), query)
    fun run(index: Int) {
        val item = found.getOrNull(index) ?: return
        store.palette = null
        item.perform()
    }
    Sheet(if (scope == PaletteScope.Labels) "Go to a label" else "Commands", { store.palette = null }) {
        SearchedList(
            if (scope == PaletteScope.Labels) "Go to a label" else "Type a command or a place to go", query, { query = it; highlighted = 0 },
            autofocus = true, submit = { run(highlighted) }, move = { highlighted = highlighted.moved(it, found.size) }, escape = { store.palette = null },
        ) {
            ChoiceList(found, highlighted, Modifier.weight(1f), "Nothing by that name.", key = { it.id }) { index, item ->
                ChoiceRow(item.title, detail = item.shortcut, symbol = item.symbol, highlighted = index == highlighted) { run(index) }
            }
        }
    }
}
