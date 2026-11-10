package app.wonnet.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.wonnet.core.Access
import app.wonnet.core.AccessChoice
import app.wonnet.core.AccessSummary
import app.wonnet.core.ConnectLink
import app.wonnet.core.ConnectRequest
import app.wonnet.core.CoreError
import app.wonnet.core.FilterChip
import app.wonnet.core.FilterEdit
import app.wonnet.core.FilterLine
import app.wonnet.core.FilterOption
import app.wonnet.core.FilterOptions
import app.wonnet.core.FilterValue
import app.wonnet.core.MailStore
import app.wonnet.core.SentencePart
import app.wonnet.core.accessSummary
import app.wonnet.core.approveConnectRequest
import app.wonnet.core.connectRequest
import app.wonnet.core.denyConnectRequest
import app.wonnet.core.filterOptions
import app.wonnet.core.setConnectedAppAccess
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.SCALE
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.Avatar
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Card
import app.wonnet.ui.Chip
import app.wonnet.ui.Notice
import app.wonnet.ui.Rule
import app.wonnet.ui.SearchField
import app.wonnet.ui.SearchStyle
import app.wonnet.ui.SettingsDot
import app.wonnet.ui.SettingsGroup
import app.wonnet.ui.SettingsRow
import app.wonnet.ui.SettingsRule
import app.wonnet.ui.SettingsTile
import app.wonnet.ui.SettingsToggleRow
import app.wonnet.ui.Sheet
import app.wonnet.ui.Spinner
import app.wonnet.ui.SwipeToRemove
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** How far an account under Only some accounts is set in: its dot under the choice's words. */
private val accountIndent = 28.dp + Space.m

/**
 * What a `wonnet://connect` link opens: who is asking, what it may reach, read and do, and
 * Approve or Deny. Closed any other way before it is answered, it denies quietly, so the browser
 * hears at once. Opened for a connected app, the same choices change what it may do, with Save.
 */
@Composable
fun ConnectSheet(store: MailStore, link: ConnectLink) {
    var request by remember { mutableStateOf<ConnectRequest?>(null) }
    var summary by remember { mutableStateOf<AccessSummary?>(null) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var submitting by remember { mutableStateOf(false) }
    var approved by remember { mutableStateOf(false) }
    var answerError by remember { mutableStateOf<String?>(null) }
    var line by remember { mutableStateOf<FilterLine?>(null) }
    val scope = rememberCoroutineScope()
    val changing = link.app != null
    LaunchedEffect(link.id) {
        try {
            val loaded = store.connectRequest(link)
            summary = store.accessSummary(loaded.client, loaded.access, null)
            request = loaded
        } catch (error: CoreError) {
            loadError = error.message
        }
    }
    val close = { store.connecting = null }
    val dismiss = {
        val key = request?.key?.takeIf { it.isNotEmpty() } ?: link.key
        if (!changing && key != null && loadError == null && !submitting && !approved) store.denyConnectRequest(key)
        close()
    }
    val loaded = request
    val first = summary
    val title = when {
        line != null -> if (line == FilterLine.Only) "Only" else "Never"
        loaded == null -> if (changing) "Change an application" else "Connect an application"
        changing -> "Change ${loaded.client}"
        else -> "Connect ${loaded.client}"
    }
    Sheet(title, dismiss, back = if (line != null) ({ line = null }) else null) {
        val error = loadError
        when {
            error != null -> LoadFailed(error, close)
            loaded == null || first == null -> Box(Modifier.fillMaxSize().semantics { contentDescription = "Loading" }, contentAlignment = Alignment.Center) { Spinner(20f) }
            approved -> Approved(loaded.client, close)
            else -> ConnectForm(
                store, loaded, first, line, { line = it }, submitting, answerError,
                submit = { access ->
                    submitting = true
                    answerError = null
                    scope.launch {
                        try {
                            val app = loaded.app
                            if (app != null) {
                                store.setConnectedAppAccess(app, access)
                                store.connectedAppsRevision += 1
                                close()
                                store.show("Saved.")
                            } else {
                                store.approveConnectRequest(loaded.key, access)
                                approved = true
                            }
                        } catch (failure: CoreError) {
                            answerError = failure.message
                        }
                        submitting = false
                    }
                },
                cancel = {
                    if (changing) return@ConnectForm close()
                    store.denyConnectRequest(loaded.key)
                    close()
                    store.show("Denied ${loaded.client}.")
                },
            )
        }
    }
}

@Composable
private fun LoadFailed(message: String, close: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.l)) {
        Notice(message, error = true, symbol = Symbol.CircleAlert)
        ActionButton("Close", Modifier.offset(x = -Dims.buttonGap / 2), onClick = close)
    }
}

/** A line of the filter whose picker is open, and the chip it changes, if it changes one. */
private data class Picking(val line: FilterLine, val editing: FilterChip?)

/**
 * The choices as the system's grouped list, over Approve and Deny (Save and Cancel for a connected
 * app). The access is kept whole: a change is made to it at once and the core asked what it comes
 * to; a filter's edit goes to the core, which answers the access to keep. Only the answer to the
 * latest ask is kept. `line` is the filter's line whose page is open in place of the list.
 */
@Composable
private fun ColumnScope.ConnectForm(
    store: MailStore,
    request: ConnectRequest,
    first: AccessSummary,
    line: FilterLine?,
    open: (FilterLine?) -> Unit,
    submitting: Boolean,
    error: String?,
    submit: (Access) -> Unit,
    cancel: () -> Unit,
) {
    var access by remember { mutableStateOf(first.access) }
    var summary by remember { mutableStateOf(first) }
    var edits by remember { mutableStateOf(emptyList<FilterEdit>()) }
    var summaryError by remember { mutableStateOf<String?>(null) }
    var picking by remember { mutableStateOf<Picking?>(null) }
    var wantsSubmit by remember { mutableStateOf(false) }
    val scroll = rememberScrollState()
    val settled = edits.isEmpty() && summary.access == access
    LaunchedEffect(access, edits) {
        val edit = edits.firstOrNull()
        if (edit == null && summary.access == access) return@LaunchedEffect
        try {
            val answer = store.accessSummary(request.client, access, edit)
            summary = answer
            access = answer.access
            summaryError = null
        } catch (failure: CoreError) {
            summaryError = failure.message
            if (edit == null) summary = summary.copy(access = access)
        }
        if (edit != null) edits = edits.drop(1)
    }
    // Approve waits for the last change to be counted, so what is approved is what the sheet says.
    LaunchedEffect(wantsSubmit, settled) {
        if (!wantsSubmit || !settled) return@LaunchedEffect
        wantsSubmit = false
        if (summary.problem == null) submit(access)
    }
    val edit = { change: FilterEdit -> edits = edits + change }
    if (line != null) {
        LinePage(request.client, line, summary, edits, edit) { picking = Picking(line, it) }
    } else {
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(scroll)) {
            Column(
                // Room after the last group, so it doesn't end against the bar under it.
                Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp)
                    .padding(start = Space.l, top = Space.l, end = Space.l, bottom = Space.xxl + Space.xl),
                verticalArrangement = Arrangement.spacedBy(Space.xl),
            ) {
                WhoIsAsking(request)
                Choices(request, access) { access = it }
                FilterGroup(summary, open)
                Reads(request, access) { access = it }
                Abilities(request, access) { access = it }
                summaryError?.let { Notice(it, error = true, symbol = Symbol.CircleAlert) }
            }
        }
        Rule()
        Column(
            Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp).fillMaxWidth().padding(horizontal = Space.l, vertical = Space.s),
            verticalArrangement = Arrangement.spacedBy(Space.xs),
        ) {
            // What it all comes to, over the buttons that answer: down here it grows into the bar,
            // so the rows don't move under a finger when it changes.
            Sentence(summary.sentence, Modifier.padding(top = Space.xs))
            error?.let { Notice(it, Modifier.padding(vertical = Space.xs), error = true, symbol = Symbol.CircleAlert) }
            Row(Modifier.fillMaxWidth().offset(x = Dims.buttonGap / 2), horizontalArrangement = Arrangement.End) {
                val changing = request.app != null
                ActionButton(if (changing) "Cancel" else "Deny", variant = ButtonVariant.Ghost, enabled = !submitting, onClick = cancel)
                ActionButton(
                    if (changing) "Save" else "Approve", variant = ButtonVariant.Primary, pending = submitting || wantsSubmit,
                    enabled = summary.problem == null,
                ) { wantsSubmit = true }
            }
        }
    }
    picking?.let { shown ->
        key(shown) {
            FilterPicker(store, access, shown.line, shown.editing, apply = edit) { picking = null }
        }
    }
}

/** Who is asking: verified or not, who published it, where it sends the browser back to, and what to beware of. */
@Composable
private fun WhoIsAsking(request: ConnectRequest) {
    Card(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            Icon(if (request.verified) Symbol.ShieldCheck else Symbol.Box, 20f / SCALE, Tokens.foreground.color)
            Text(request.client, TextStyle.Heading, Modifier.weight(1f, fill = false).semantics { heading() })
            if (request.verified) {
                Chip("Verified", symbol = Symbol.ShieldCheck, tint = Tokens.success)
            } else {
                Chip("Unverified", symbol = Symbol.CircleAlert, tint = Tokens.warning)
            }
        }
        Column(Modifier.padding(top = Space.l), verticalArrangement = Arrangement.spacedBy(Space.s)) {
            Notice(request.publisher, symbol = Symbol.Globe)
            if (request.backTo.isNotEmpty()) Notice(request.backTo, symbol = Symbol.ArrowRight)
            if (request.onDevice) Notice("Sends you back to an application on this device", warning = true, symbol = Symbol.Monitor)
            if (request.app == null) Notice("Only approve if you started this yourself", warning = true, symbol = Symbol.CircleAlert)
        }
    }
}

/** What the choices come to, in one sentence, its strong runs in bold. */
@Composable
private fun Sentence(parts: List<SentencePart>, modifier: Modifier = Modifier) {
    val strong = Tokens.emphasizedForeground.color
    val sentence = buildAnnotatedString {
        for (part in parts) {
            if (part.strong) withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = strong)) { append(part.text) } else append(part.text)
        }
    }
    Text(sentence, TextStyle.Body, modifier.fillMaxWidth())
}

/** A choice's picture: its symbol, or an account's dot. */
@Composable
private fun ChoiceLead(choice: AccessChoice) {
    val color = choice.color
    if (color != null) return SettingsDot(Dims.accountColor(color).color)
    SettingsTile(choice.symbol?.let(Symbol::named) ?: Symbol.Tag)
}

/** Whose mail it reaches: every account, or the ones picked under Only some accounts. */
@Composable
private fun Choices(request: ConnectRequest, access: Access, change: (Access) -> Unit) {
    SettingsGroup("Accounts", "Whose mail it can reach.", warning = if (access.allAccounts) null else problemOf(request, access)) {
        SettingsRow(
            "Every account", detail = "Including the ones you add later.", leading = { SettingsTile(Symbol.Inbox) }, wrap = true, checked = access.allAccounts,
        ) { change(access.withAllAccounts(true)) }
        SettingsRule()
        SettingsRow(
            "Only some accounts", detail = "Only the ones you pick.", leading = { SettingsTile(Symbol.ListFilter) }, wrap = true, checked = !access.allAccounts,
        ) { change(access.withAllAccounts(false)) }
        if (access.allAccounts) return@SettingsGroup
        for (account in request.accounts) {
            key(account.id) {
                SettingsRule(accountIndent)
                SettingsRow(account.title, detail = account.detail, leading = { ChoiceLead(account) }, indent = accountIndent, checked = account.id in access.accounts) {
                    change(access.toggling(account.id))
                }
            }
        }
    }
}

/** Why the accounts picked reach nothing, said at once rather than after the core counts. */
private fun problemOf(request: ConnectRequest, access: Access): String? =
    if (request.accounts.any { it.id in access.accounts }) null else "Pick at least one account."

/** The filter's two lines, each opening its page, and how much mail that leaves. */
@Composable
private fun FilterGroup(summary: AccessSummary, open: (FilterLine?) -> Unit) {
    SettingsGroup("Filter", summary.reach, warning = summary.warning) {
        SettingsRow(
            "Only", value = if (summary.only.isEmpty()) "All mail" else "${summary.only.size}", leading = { SettingsTile(Symbol.Filter) }, chevron = true,
        ) { open(FilterLine.Only) }
        SettingsRule()
        SettingsRow(
            "Never", value = if (summary.never.isEmpty()) "Nothing" else "${summary.never.size}", leading = { SettingsTile(Symbol.Ban) }, chevron = true,
        ) { open(FilterLine.Never) }
    }
}

/** What it reads of each message, the parts as switches. */
@Composable
private fun Reads(request: ConnectRequest, access: Access, change: (Access) -> Unit) {
    SettingsGroup("What it reads", "It always sees the sender, recipients, date and labels. Pick what else.") {
        for ((index, part) in request.reads.withIndex()) {
            if (index > 0) SettingsRule()
            SettingsToggleRow(part.title, part.id in access.reads, part.detail, leading = { ChoiceLead(part) }) { change(access.togglingRead(part.id)) }
        }
    }
}

/** What it can do with that mail: a role, checked, and the privileges as switches. */
@Composable
private fun Abilities(request: ConnectRequest, access: Access, change: (Access) -> Unit) {
    SettingsGroup("What it can do", "With the mail it reaches.") {
        for ((index, role) in request.roles.withIndex()) {
            if (index > 0) SettingsRule()
            SettingsRow(role.title, detail = role.detail, leading = { ChoiceLead(role) }, wrap = true, checked = access.role == role.id) {
                change(access.withRole(role.id))
            }
        }
        for (privilege in request.privileges) {
            SettingsRule()
            SettingsToggleRow(privilege.title, privilege.id in access.privileges, privilege.detail, leading = { ChoiceLead(privilege) }) {
                change(access.togglingPrivilege(privilege.id))
            }
        }
    }
}

/** The icon of a condition's kind. */
private fun kindSymbol(kind: String): Symbol = when (kind) {
    "from" -> Symbol.User
    "to" -> Symbol.Send
    "label" -> Symbol.Tag
    "subject" -> Symbol.Type
    "body" -> Symbol.Text
    "attachment" -> Symbol.Paperclip
    "date" -> Symbol.Calendar
    else -> Symbol.ListFilter
}

/**
 * A line's page: its conditions as rows, a swipe removing one and a tap changing it, and Add. A
 * removal leaves at once, before the core has counted what is left.
 */
@Composable
private fun ColumnScope.LinePage(
    client: String,
    line: FilterLine,
    summary: AccessSummary,
    edits: List<FilterEdit>,
    edit: (FilterEdit) -> Unit,
    pick: (FilterChip?) -> Unit,
) {
    val leaving = edits.filter { it.line == line }.mapNotNull { it.remove }
    val chips = summary.chips(line).filter { it.filterValue !in leaving }
    val about = if (line == FilterLine.Only) "Mail must match these to reach $client." else "Mail that matches any of these is kept from $client."
    val explained = if (chips.isEmpty()) about else "$about Swipe a row to remove it, tap it to change it."
    val footer = "$explained\n${summary.reach}"
    Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState())) {
        Column(
            Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp).padding(Space.l),
            verticalArrangement = Arrangement.spacedBy(Space.xl),
        ) {
            SettingsGroup(footer = footer) {
                for (chip in chips) {
                    key(chip.kind, chip.value) {
                        SwipeToRemove("${chip.kind}:${chip.value}", "Remove", { edit(FilterEdit(line, remove = chip.filterValue)) }) {
                            ChipRow(chip) { pick(chip) }
                        }
                        SettingsRule()
                    }
                }
                SettingsRow("Add", leading = { SettingsTile(Symbol.Plus, Tokens.primary.color) }, action = true) { pick(null) }
            }
        }
    }
}

/** A condition as a row: "From @chase.com", how much mail it matches, in the warning colour when none. */
@Composable
private fun ChipRow(chip: FilterChip, onClick: () -> Unit) {
    val none = chip.matches == 0
    val label = chip.label.replaceFirstChar { it.uppercase() }
    SettingsRow(
        "$label ${chip.text}", detail = if (none) "Matches no mail, so it changes nothing" else chip.help, wrap = true,
        detailColor = if (none) Tokens.warning.color else null,
        leading = { if (none) SettingsTile(Symbol.CircleAlert, Tokens.warning.color) else SettingsTile(kindSymbol(chip.kind)) },
        onClick = onClick,
    )
}

/**
 * What to add to a line, or what to change a chip to: a sheet of the whole screen with the options
 * in groups and the search field along the bottom, with the kind picked as a tag in it. A kind
 * narrows the options to it; a value is applied and the sheet closes. The options shown stay
 * until the answer to the latest ask replaces them.
 */
@Composable
private fun FilterPicker(store: MailStore, access: Access, line: FilterLine, editing: FilterChip?, apply: (FilterEdit) -> Unit, close: () -> Unit) {
    var kind by remember { mutableStateOf(editing?.kind) }
    var query by remember { mutableStateOf("") }
    var options by remember { mutableStateOf<FilterOptions?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    var shownKind by remember { mutableStateOf(editing?.kind) }
    var slow by remember { mutableStateOf(false) }
    val scroll = rememberScrollState()
    LaunchedEffect(Unit) {
        delay(400)
        slow = true
    }
    LaunchedEffect(kind, query) {
        try {
            options = store.filterOptions(access, line, kind, query, editing?.filterValue)
            failure = null
        } catch (error: CoreError) {
            failure = error.message
        }
        if (shownKind != kind) scroll.scrollTo(0)
        shownKind = kind
    }
    fun pick(option: FilterOption) {
        val value = option.value
        if (value == null) {
            kind = option.kind
            query = ""
            return
        }
        val picked = FilterValue(option.kind, value)
        val old = editing?.filterValue
        if (picked != old) apply(FilterEdit(line, remove = old, add = picked))
        close()
    }
    val title = when {
        editing != null -> "Change"
        line == FilterLine.Only -> "Narrow it"
        else -> "Keep out"
    }
    Sheet(title, close) {
        val shown = options
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(scroll)) {
            Column(
                Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp).padding(Space.l),
                verticalArrangement = Arrangement.spacedBy(Space.xl),
            ) {
                if (shown == null && failure == null && slow) {
                    Box(Modifier.fillMaxWidth().padding(Space.xl), contentAlignment = Alignment.Center) { Spinner(20f) }
                }
                for (section in shown?.sections.orEmpty()) {
                    SettingsGroup(section.title) {
                        for ((index, option) in section.options.withIndex()) {
                            key(option.id) {
                                if (index > 0) SettingsRule()
                                OptionRow(option) { pick(option) }
                            }
                        }
                    }
                }
                val note = failure ?: shown?.note
                note?.let { Notice(it, Modifier.padding(horizontal = Space.l), error = failure != null, symbol = if (failure != null) Symbol.CircleAlert else null) }
            }
        }
        if (editing != null) {
            Rule()
            Row(
                Modifier.fillMaxWidth().padding(start = Space.l, end = Space.l - Dims.buttonGap / 2, top = Space.xs, bottom = Space.xs),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("Pick another to replace it", TextStyle.Caption, Modifier.weight(1f), color = Tokens.mutedForeground.color)
                ActionButton("Remove", symbol = Symbol.Trash, variant = ButtonVariant.Destructive, size = ControlSize.Small) {
                    apply(FilterEdit(line, remove = editing.filterValue))
                    close()
                }
            }
        }
        val clearScope = if (editing == null && kind != null) ({ kind = null; query = "" }) else null
        SearchField(
            shown?.placeholder ?: "Search", query, { query = it }, Modifier.padding(horizontal = Space.l, vertical = Space.s), style = SearchStyle.Bar,
            tag = shown?.scope, clearTag = clearScope,
            submit = { shown?.sections?.firstOrNull()?.options?.firstOrNull()?.let(::pick) },
            escape = { if (query.isEmpty()) clearScope?.invoke() ?: close() },
        )
    }
}

/**
 * An option of the picker: a person's avatar or its icon, its title over its detail, how much
 * mail it matches, and the check's column, kept whether it shows or not. A kind goes on to its
 * own options, with a chevron.
 */
@Composable
private fun OptionRow(option: FilterOption, onClick: () -> Unit) {
    val avatar = option.avatar
    SettingsRow(
        option.title, detail = option.detail, value = option.count, chevron = option.value == null,
        checked = if (option.value == null) null else option.checked,
        leading = {
            if (avatar != null) {
                Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Avatar(avatar.initials, avatar.email, 22f) }
            } else {
                SettingsTile(option.symbol?.let(Symbol::named) ?: kindSymbol(option.kind), Tokens.mutedForeground.color)
            }
        },
        onClick = onClick,
    )
}

/** Approved: the browser goes back on its own, and Settings can take it back. */
@Composable
private fun Approved(client: String, close: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(Space.xl),
        verticalArrangement = Arrangement.spacedBy(Space.l, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.size((56 * SCALE).dp).background(Tokens.success.opacity(Opacity.colorTint).color, CircleShape), contentAlignment = Alignment.Center) {
            Icon(Symbol.Check, 26f, Tokens.success.color)
        }
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
            Text("Approved", TextStyle.Heading, Modifier.semantics { heading() })
            Text(
                "Your browser takes you back to $client. You can change or revoke it in Settings → Connected apps.", TextStyle.Body,
                Modifier.widthIn(max = 360.dp), color = Tokens.mutedForeground.color, align = TextAlign.Center,
            )
        }
        ActionButton("Done", variant = ButtonVariant.Primary, onClick = close)
    }
}
