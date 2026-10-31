package app.wonnet.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitHorizontalTouchSlopOrCancellation
import androidx.compose.foundation.gestures.horizontalDrag
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.key
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.wonnet.BuildConfig
import app.wonnet.core.AccountView
import app.wonnet.core.Appearance
import app.wonnet.core.CustomSplit
import app.wonnet.core.MailStore
import app.wonnet.core.Mailbox
import app.wonnet.core.SettingsEntry
import app.wonnet.core.SettingsSection
import app.wonnet.core.Snippet
import app.wonnet.core.SwipePreference
import app.wonnet.core.ThreadCommand
import app.wonnet.core.blockedSenders
import app.wonnet.core.confirmRemoving
import app.wonnet.core.confirmSigningOut
import app.wonnet.core.customSplits
import app.wonnet.core.flag
import app.wonnet.core.mailboxesForTabs
import app.wonnet.core.moveSplit
import app.wonnet.core.moveSwipeCommand
import app.wonnet.core.moveTab
import app.wonnet.core.newSplit
import app.wonnet.core.notifies
import app.wonnet.core.openSettings
import app.wonnet.core.ownSignature
import app.wonnet.core.providedSignature
import app.wonnet.core.providerName
import app.wonnet.core.removeSplit
import app.wonnet.core.saveSnippet
import app.wonnet.core.saveSplit
import app.wonnet.core.setFlag
import app.wonnet.core.setSignature
import app.wonnet.core.setSwipeCommands
import app.wonnet.core.snippets
import app.wonnet.core.splitLabels
import app.wonnet.core.status
import app.wonnet.core.subtitle
import app.wonnet.core.summary
import app.wonnet.core.swipeChoicesLeft
import app.wonnet.core.swipeCommands
import app.wonnet.core.unblock
import app.wonnet.core.undoDelayName
import app.wonnet.core.undoDelays
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.Radius
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.ChoiceMenu
import app.wonnet.ui.IconButton
import app.wonnet.ui.InputField
import app.wonnet.ui.MovableList
import app.wonnet.ui.Pressable
import app.wonnet.ui.Rule
import app.wonnet.ui.SearchField
import app.wonnet.ui.Sheet
import app.wonnet.ui.SwatchRow
import app.wonnet.ui.Toggle
import kotlinx.coroutines.launch
import java.util.UUID
import kotlin.math.roundToInt

/** A page of the settings past the sections: one section's, or one of the pages pushed from it. */
private sealed interface Page {
    val title: String

    data class Section(val section: SettingsSection) : Page {
        override val title get() = section.title
    }

    data class Account(val id: String, override val title: String) : Page
    data object AddAccount : Page {
        override val title get() = "Add Account"
    }

    data class Split(val split: CustomSplit, val isNew: Boolean) : Page {
        override val title get() = if (isNew) "New Split" else "Edit Split"
    }

    data class SnippetEdit(val snippet: Snippet, val isNew: Boolean) : Page {
        override val title get() = if (isNew) "New Snippet" else "Edit Snippet"
    }

    data object AddTab : Page {
        override val title get() = "Add Tab"
    }

    data object AddSwipe : Page {
        override val title get() = "Add Action"
    }

    data object Blocked : Page {
        override val title get() = "Blocked Senders"
    }
}

/**
 * The settings in a sheet: a narrow one lists the sections and pushes the open one over the list,
 * a wide one (a tablet's) has the list beside it. Pages are grouped lists of rows on cards, with
 * switches, menus and pages pushed for what is written; a hold reorders and a swipe removes.
 */
@Composable
fun SettingsSheet(store: MailStore, close: () -> Unit) {
    var pages by remember { mutableStateOf(listOfNotNull<Page>(store.settings?.takeIf { it != SettingsSection.General || store.settingsTarget != null }?.let { Page.Section(it) })) }
    LaunchedEffect(store.settings, store.settingsTarget) {
        val section = store.settings ?: return@LaunchedEffect
        if (store.settingsTarget != null && (pages.firstOrNull() as? Page.Section)?.section != section) pages = listOf(Page.Section(section))
    }
    val push = { page: Page -> pages = pages + page }
    val back = { pages = pages.dropLast(1) }
    Sheet("Settings", close) {
        BackHandler(pages.isNotEmpty()) { pages = pages.dropLast(1) }
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val wide = maxWidth >= Dims.wide
            if (wide) {
                Row(Modifier.fillMaxSize()) {
                    Column(Modifier.width(Dims.sidebarWidth).fillMaxHeight()) {
                        SettingsSections(store, (pages.firstOrNull() as? Page.Section)?.section ?: SettingsSection.General) { section ->
                            store.settings = section
                            pages = listOf(Page.Section(section))
                        }
                    }
                    Rule(vertical = true)
                    Column(Modifier.weight(1f).fillMaxHeight()) {
                        val stack = pages.ifEmpty { listOf(Page.Section(SettingsSection.General)) }
                        PageView(store, stack, if (stack.size > 1) back else null, push, close)
                    }
                }
            } else if (pages.isEmpty()) {
                SettingsSections(store, null) { section ->
                    store.settings = section
                    pages = listOf(Page.Section(section))
                }
            } else {
                PageView(store, pages, back, push, close)
            }
        }
    }
}

/** The sections in groups, each pushing its page, or chosen beside it on a wide sheet. While a search is typed, the groups it finds take their place. */
@Composable
private fun SettingsSections(store: MailStore, selected: SettingsSection?, choose: (SettingsSection) -> Unit) {
    val groups = listOf(listOf(SettingsSection.General, SettingsSection.Accounts), listOf(SettingsSection.Inbox, SettingsSection.Reading, SettingsSection.Writing))
    Column(Modifier.fillMaxSize()) {
        SearchField("Search", store.settingsQuery, { store.settingsQuery = it })
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.xl)) {
            if (store.settingsQuery.isEmpty()) {
                for (group in groups) {
                    Group {
                        for ((index, section) in group.withIndex()) {
                            if (index > 0) RowRule()
                            SettingsRow(section.title, leading = { Tile(section.symbol) }, chevron = selected == null, selected = selected == section) { choose(section) }
                        }
                    }
                }
            } else {
                val found = SettingsEntry.matching(store.settingsQuery)
                Group {
                    if (found.isEmpty()) SettingsRow("No settings found", muted = true)
                    for ((index, entry) in found.withIndex()) {
                        if (index > 0) RowRule()
                        SettingsRow(entry.title, detail = entry.section.title, leading = { Tile(entry.section.symbol) }) {
                            store.openSettings(entry.section, entry.id)
                            choose(entry.section)
                        }
                    }
                }
            }
        }
    }
}

/** The page on top of the stack, under its own bar with the way back and what it is for. */
@Composable
private fun PageView(store: MailStore, pages: List<Page>, back: (() -> Unit)?, push: (Page) -> Unit, close: () -> Unit) {
    val page = pages.last()
    var save by remember(page) { mutableStateOf<(() -> Unit)?>(null) }
    var canSave by remember(page) { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().height(Dims.barHeight).padding(horizontal = Space.s), verticalAlignment = Alignment.CenterVertically) {
            if (back != null) IconButton(Symbol.ArrowLeft, "Back", circled = false, onClick = back)
            Text(page.title, TextStyle.Subheading, Modifier.weight(1f).padding(horizontal = Space.s), maxLines = 1)
            save?.let { ActionButton("Save", variant = ButtonVariant.Primary, size = ControlSize.Small, enabled = canSave, onClick = it) }
        }
        Rule()
        val places = remember(page) { mutableStateMapOf<String, Float>() }
        val scroll = rememberScrollState()
        LaunchedEffect(store.settingsTarget, places.size) {
            val target = store.settingsTarget ?: return@LaunchedEffect
            val y = places[target] ?: return@LaunchedEffect
            scroll.animateScrollTo(y.roundToInt())
            store.settingsTarget = null
        }
        val anchor = { id: String -> Modifier.onPlaced { places[id] = it.positionInParent().y } }
        Column(Modifier.weight(1f).verticalScroll(scroll).padding(Space.l).widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(Space.xl)) {
            when (page) {
                is Page.Section -> when (page.section) {
                    SettingsSection.General -> GeneralPage(store, anchor)
                    SettingsSection.Accounts -> AccountsPage(store, anchor, push)
                    SettingsSection.Inbox -> InboxPage(store, anchor, push)
                    SettingsSection.Reading -> ReadingPage(store, anchor, push)
                    SettingsSection.Writing -> WritingPage(store, anchor, push)
                }
                is Page.Account -> AccountPage(store, page.id, back ?: close)
                Page.AddAccount -> {
                    val count = remember { store.accounts.size }
                    LaunchedEffect(store.accounts.size) { if (store.accounts.size != count) (back ?: close)() }
                    AddAccountFlow(store)
                }
                is Page.Split -> SplitPage(store, page.split, page.isNew, back ?: close) { action, enabled ->
                    save = action
                    canSave = enabled
                }
                is Page.SnippetEdit -> SnippetPage(store, page.snippet, page.isNew, back ?: close) { action, enabled ->
                    save = action
                    canSave = enabled
                }
                Page.AddTab -> AddTabPage(store, back ?: close)
                Page.AddSwipe -> AddSwipePage(store, back ?: close)
                Page.Blocked -> BlockedPage(store)
            }
        }
    }
}

// General

@Composable
private fun GeneralPage(store: MailStore, anchor: (String) -> Modifier) {
    Group("Appearance", modifier = anchor("appearance")) {
        SettingsRow("Theme", trailing = { ChoiceMenu(Appearance.entries, store.appearance, { it.title }, ControlSize.Small, store::changeAppearance) })
    }
    Group("About", modifier = anchor("updates")) {
        SettingsRow("Version", value = BuildConfig.VERSION_NAME)
        RowRule()
        SettingsRow("Check for Updates", action = true) { store.updater.check(asked = true) }
    }
    Group(
        "Server", "Wonnet's server syncs your accounts and keeps this device's copy of the mail current. Offline, changes wait and are sent once connected.",
        anchor("server"),
    ) {
        SettingsRow("Address", value = store.server)
        RowRule()
        SettingsRow("Status", value = when (store.connection) {
            "online" -> "Connected"
            "connecting" -> "Connecting"
            else -> "Offline"
        })
    }
}

// Accounts

@Composable
private fun AccountsPage(store: MailStore, anchor: (String) -> Modifier, push: (Page) -> Unit) {
    Group("Accounts", "Each account's colour marks its mail in the list and its mailboxes in the sidebar.", anchor("accounts")) {
        SettingsRow("Add Account", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.AddAccount) }
        for (account in store.accounts) {
            RowRule()
            SettingsRow(account.title, detail = store.subtitle(account), leading = { Dot(Dims.accountColor(account.color).color) }, chevron = true) {
                push(Page.Account(account.id, account.title))
            }
        }
    }
    Group(footer = "Every account leaves this device. Signing in again brings them back.", modifier = anchor("sign-out").then(anchor("signatures"))) {
        SettingsRow("Sign Out", destructive = true, onClick = store::confirmSigningOut)
    }
}

/** An account: what it is, its name, its colour, its notifications, its signature, and the way to remove it. It goes back once the account is gone. */
@Composable
private fun AccountPage(store: MailStore, id: String, back: () -> Unit) {
    val account = store.accounts.firstOrNull { it.id == id }
    LaunchedEffect(account == null) { if (account == null) back() }
    if (account == null) return
    Group(footer = "Shown in the sidebar in place of the address.") {
        SettingsRow("Name", trailing = { AccountNameField(store, account) })
        RowRule()
        SettingsRow("Address", value = account.address)
        RowRule()
        SettingsRow("Status", value = store.status(account))
    }
    Group("Colour") {
        Box(Modifier.padding(horizontal = Space.s, vertical = Space.xs)) { SwatchRow(account.color) { store.setAccountColor(account.id, it) } }
    }
    Group(footer = "New mail makes a notification.") {
        SettingsRow("Notifications", trailing = { Toggle(store.notifies(account), { store.setFlag("notify:${account.id}", true, it) }) })
    }
    SignatureGroup(store, account)
    Group {
        SettingsRow("Remove Account", destructive = true) { store.confirmRemoving(account) }
    }
}

/**
 * What the user calls an account, saved on Return, when the field is left and when the page goes;
 * a name given on another device shows here as it comes.
 */
@Composable
private fun AccountNameField(store: MailStore, account: AccountView) {
    var name by remember(account.id) { mutableStateOf(account.name) }
    LaunchedEffect(account.name) { name = account.name }
    val latest by rememberUpdatedState(account to name)
    DisposableEffect(account.id) {
        onDispose { store.setAccountName(latest.first, latest.second) }
    }
    InputField(name, { name = it.take(AccountView.NAME_MAX) }, Modifier.widthIn(max = 220.dp), placeholder = "Work, Personal", capitalize = true) {
        store.setAccountName(account, name)
    }
}

/** The signature put under what the account sends, and Edit, which opens it in a box to change. */
@Composable
private fun SignatureGroup(store: MailStore, account: AccountView) {
    val own = store.ownSignature(account)
    val provided = providedSignature(account)
    val shown = own.ifEmpty { null } ?: provided
    var draft by remember(account.id) { mutableStateOf<String?>(null) }
    val footer = when {
        provided == null || (own.isNotEmpty() && draft == null) -> "Put under what you write from this account."
        draft != null -> "Left empty, the one kept by ${providerName(account)} is used."
        else -> "Kept by ${providerName(account)}, until you write your own."
    }
    Group("Signature", footer) {
        val editing = draft
        if (editing != null) {
            Column(Modifier.padding(Space.s), verticalArrangement = Arrangement.spacedBy(Space.s)) {
                InputField(editing, { draft = it }, placeholder = "Best,\n${account.address}", lines = 4, autofocus = true, capitalize = true)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    ActionButton("Cancel", variant = ButtonVariant.Ghost) { draft = null }
                    ActionButton("Save", variant = ButtonVariant.Primary, enabled = editing != (shown ?: "")) {
                        store.setSignature(editing.trim(), account)
                        draft = null
                    }
                }
            }
            return@Group
        }
        if (shown != null) {
            SelectionContainer { Text(shown, TextStyle.Body, Modifier.fillMaxWidth().padding(horizontal = Space.l, vertical = Space.s), color = Tokens.mutedForeground.color) }
        } else {
            SettingsRow("No signature", muted = true)
        }
        RowRule()
        SettingsRow(if (shown == null) "Add Signature" else "Edit Signature", leading = { Tile(if (shown == null) Symbol.Plus else Symbol.Pencil, Tokens.primary.color) }, action = true) {
            draft = shown ?: ""
        }
    }
}

// Inbox

@Composable
private fun InboxPage(store: MailStore, anchor: (String) -> Modifier, push: (Page) -> Unit) {
    Group(footer = "Important mail first, then your splits, then Other: newsletters and notifications.", modifier = anchor("split-inbox")) {
        SettingsRow("Split Inbox", trailing = { Toggle(store.flag("split_inbox", false), { store.setFlag("split_inbox", false, it) }) })
    }
    val splits = store.customSplits
    Group("Splits", "Hold a split and drag it to move it; swipe it to remove it.") {
        MovableList(splits, { it.id }, clicked = { push(Page.Split(it, false)) }, moved = { split, index -> store.moveSplit(split, index) }) { split, hovered ->
            SwipeToRemove(split.id, "Remove", { store.removeSplit(split) }) {
                SettingsRow(split.name, detail = store.summary(split), leading = { Tile(Symbol.ListFilter) }, chevron = true, hovered = hovered)
            }
        }
        if (splits.isNotEmpty()) RowRule()
        SettingsRow("Add Split", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.Split(store.newSplit(), true)) }
    }
    val tabs = store.savedTabs
    Group("Tabs", "The mailboxes along the bottom. Hold a tab and drag it to move it; swipe it to remove it.", anchor("tabs")) {
        MovableList(tabs, { it.id }, clicked = {}, moved = { tab, index -> store.moveTab(tab, index) }) { tab, hovered ->
            SwipeToRemove(tab.id, "Remove", { store.toggleTab(tab.id) }) {
                SettingsRow(store.tabName(tab), leading = { Tile(Symbol.named(tab.symbol)) }, hovered = hovered)
            }
        }
        if (tabs.isNotEmpty()) RowRule()
        SettingsRow("Add Tab", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.AddTab) }
    }
    val commands = store.swipeCommands
    val footer = if (commands.size > 1) {
        "Swipe a thread to the left to choose one, the first at the edge; swipe it all the way to do the first. Hold one and drag it to move it; swipe it to remove it."
    } else {
        "Swipe a thread to the left to show it; swipe a little further to do it. Add another to choose between them."
    }
    Group("Swipe Left", footer, anchor("swipe")) {
        MovableList(commands, { it.id }, clicked = {}, moved = { command, index -> store.moveSwipeCommand(command, index) }) { command, hovered ->
            val row = @Composable {
                SettingsRow(command.swipeName, leading = { SwipeTile(command) }, value = if (command == commands.first()) (if (commands.size == 1) "Short swipe" else "Long swipe") else null, hovered = hovered)
            }
            if (commands.size == 1) row() else SwipeToRemove(command.id, "Remove", { store.setSwipeCommands(store.swipeCommands - command) }) { row() }
        }
        if (commands.size < SwipePreference.MOST) {
            RowRule()
            SettingsRow("Add Action", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.AddSwipe) }
        }
    }
}

/** A swipe command as the swipe shows it: its icon in a wash of its colour. */
@Composable
private fun SwipeTile(command: ThreadCommand) {
    val tint = command.swipeTint.first
    Box(Modifier.size(28.dp).background(tint.opacity(Opacity.colorTintStronger).color, CircleShape), contentAlignment = Alignment.Center) {
        Icon(command.symbol, 15f, tint.color)
    }
}

/** The commands a swipe doesn't have yet; the one picked joins them and the page goes back. */
@Composable
private fun AddSwipePage(store: MailStore, back: () -> Unit) {
    Group {
        for ((index, command) in store.swipeChoicesLeft.withIndex()) {
            if (index > 0) RowRule()
            SettingsRow(command.swipeName, leading = { SwipeTile(command) }) {
                store.setSwipeCommands(store.swipeCommands + command)
                back()
            }
        }
    }
}

/** The mailboxes that are not tabs yet; the one picked joins them and the page goes back. */
@Composable
private fun AddTabPage(store: MailStore, back: () -> Unit) {
    val (unified, accounts) = store.mailboxesForTabs
    @Composable
    fun rows(mailboxes: List<Mailbox>) {
        for ((index, mailbox) in mailboxes.withIndex()) {
            if (index > 0) RowRule()
            SettingsRow(mailbox.name, leading = { Tile(Symbol.named(mailbox.symbol)) }) {
                store.toggleTab(mailbox.id)
                back()
            }
        }
    }
    if (unified.isNotEmpty()) Group { rows(unified) }
    for ((account, mailboxes) in accounts) Group(account.title) { rows(mailboxes) }
}

/** A split being written: its name, its senders, its label. */
@Composable
private fun SplitPage(store: MailStore, start: CustomSplit, isNew: Boolean, back: () -> Unit, saving: (() -> Unit, Boolean) -> Unit) {
    var name by remember { mutableStateOf(start.name) }
    var senders by remember { mutableStateOf(start.from.joinToString(", ")) }
    var label by remember { mutableStateOf(start.label) }
    val labels = store.splitLabels
    LaunchedEffect(name, senders, label) {
        saving({
            store.saveSplit(start.copy(name = name.trim(), from = CustomSplit.senders(senders), label = label))
            back()
        }, name.isNotBlank())
    }
    Group("Name") { Box(Modifier.padding(Space.s)) { InputField(name, { name = it }, placeholder = "Team", autofocus = isNew, capitalize = true) } }
    Group("From", "Addresses, or @ and a domain for everyone there.") {
        Box(Modifier.padding(Space.s)) { InputField(senders, { senders = it }, placeholder = "ann@example.com, @example.com", lines = 3, keyboard = KeyboardType.Email) }
    }
    if (labels.isNotEmpty()) {
        Group(footer = "Mail with the label goes to the split too.") {
            SettingsRow("Label", trailing = {
                ChoiceMenu(listOf<String?>(null) + labels.map { it.first }, label, { id -> labels.firstOrNull { it.first == id }?.second ?: "None" }, ControlSize.Small) { label = it }
            })
        }
    }
    if (!isNew) {
        Group {
            SettingsRow("Remove Split", destructive = true) {
                store.removeSplit(start)
                back()
            }
        }
    }
}

// Reading

@Composable
private fun ReadingPage(store: MailStore, anchor: (String) -> Modifier, push: (Page) -> Unit) {
    Group(footer = "In dark mode, newsletters and other designed mail are drawn in dark colours. Each message can show the original.", modifier = anchor("mail")) {
        SettingsRow("Dark Mode for Mail", trailing = { Toggle(store.flag("dark_mail", true), { store.setFlag("dark_mail", true, it) }) })
    }
    Group(footer = "Off, pictures hosted by the sender wait until you ask for them, so nobody learns when you read their mail.") {
        SettingsRow("Load Remote Images", trailing = { Toggle(store.flag("remote_images", true), { store.setFlag("remote_images", true, it) }) })
    }
    Group(footer = "Their new mail goes to the trash. Block someone from a message of theirs.", modifier = anchor("blocked")) {
        val blocked = store.blockedSenders
        SettingsRow("Blocked Senders", value = if (blocked.isEmpty()) "None" else "${blocked.size}", chevron = true) { push(Page.Blocked) }
    }
}

@Composable
private fun BlockedPage(store: MailStore) {
    val blocked = store.blockedSenders
    Group(footer = if (blocked.isEmpty()) null else "Swipe a sender to unblock them.") {
        if (blocked.isEmpty()) SettingsRow("No one is blocked.", muted = true)
        for ((index, address) in blocked.withIndex()) {
            key(address) {
                if (index > 0) RowRule()
                SwipeToRemove(address, "Unblock", { store.unblock(address) }) { SettingsRow(address) }
            }
        }
    }
}

// Writing

@Composable
private fun WritingPage(store: MailStore, anchor: (String) -> Modifier, push: (Page) -> Unit) {
    Group(footer = "How long a message waits after Send, to be taken back.", modifier = anchor("sending")) {
        SettingsRow("Undo Send", trailing = { ChoiceMenu(undoDelays, store.undoDelay, ::undoDelayName, ControlSize.Small, store::changeUndoDelay) })
    }
    val snippets = store.snippets
    Group("Snippets", "Type ; and a name while writing to put one in. {first_name} becomes the first recipient's first name.", anchor("snippets")) {
        for ((index, snippet) in snippets.withIndex()) {
            key(snippet.id) {
                if (index > 0) RowRule()
                SwipeToRemove(snippet.id, "Remove", { store.setPreference(snippet.key, null) }) {
                    SettingsRow(snippet.name, detail = snippet.text.replace('\n', ' '), chevron = true) { push(Page.SnippetEdit(snippet, false)) }
                }
            }
        }
        if (snippets.isNotEmpty()) RowRule()
        SettingsRow("Add Snippet", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) {
            push(Page.SnippetEdit(Snippet(UUID.randomUUID().toString().lowercase(), "", ""), true))
        }
    }
}

@Composable
private fun SnippetPage(store: MailStore, start: Snippet, isNew: Boolean, back: () -> Unit, saving: (() -> Unit, Boolean) -> Unit) {
    var name by remember { mutableStateOf(start.name) }
    var text by remember { mutableStateOf(start.text) }
    LaunchedEffect(name, text) {
        saving({
            store.saveSnippet(start.copy(name = name, text = text))
            back()
        }, name.isNotBlank())
    }
    Group("Name") { Box(Modifier.padding(Space.s)) { InputField(name, { name = it }, placeholder = "thanks", autofocus = isNew) } }
    Group("Text", "{first_name} becomes the first recipient's first name.") {
        Box(Modifier.padding(Space.s)) { InputField(text, { text = it }, placeholder = "Thanks {first_name}!", lines = 6, capitalize = true) }
    }
    if (!isNew) {
        Group {
            SettingsRow("Remove Snippet", destructive = true) {
                store.setPreference(start.key, null)
                back()
            }
        }
    }
}

// Parts

/** A group of rows: its name over it, a card of rows, and the muted line under it saying what it does. */
@Suppress("ModifierParameter")
@Composable
private fun Group(title: String? = null, footer: String? = null, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(Space.s)) {
        title?.let { Text(it, TextStyle.Subheading, Modifier.padding(horizontal = Space.l)) }
        val shape = RoundedCornerShape(Radius.lg)
        Column(Modifier.fillMaxWidth().background(Tokens.card.color, shape).border(Dims.hairline, Tokens.borderCard.color, shape).padding(vertical = Space.xxs), content = content)
        footer?.let { Text(it, TextStyle.Caption, Modifier.padding(horizontal = Space.l), color = Tokens.mutedForeground.color) }
    }
}

@Composable
private fun RowRule() = Rule(Modifier.padding(start = Space.l), color = Tokens.borderCard)

/** A row of a group: its picture, what it is with a line under, and its value, control or chevron at the end. */
@Composable
private fun SettingsRow(
    title: String,
    detail: String? = null,
    value: String? = null,
    leading: (@Composable () -> Unit)? = null,
    chevron: Boolean = false,
    selected: Boolean = false,
    destructive: Boolean = false,
    action: Boolean = false,
    muted: Boolean = false,
    wrap: Boolean = false,
    hovered: Boolean = false,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    @Composable
    fun Face(lit: Boolean) {
        val color = when {
            destructive -> Tokens.destructive.color
            action -> Tokens.primary.color
            muted -> Tokens.mutedForeground.color
            else -> Tokens.foreground.color
        }
        val fill = when {
            selected -> Tokens.backgroundAccentStronger.color
            lit -> Tokens.cardAccent.color
            else -> Color.Transparent
        }
        Row(
            Modifier.fillMaxWidth().heightIn(min = (Dims.rowHeight.value + 8).dp).background(fill).padding(horizontal = Space.l, vertical = Space.xs),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.m),
        ) {
            leading?.invoke()
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, TextStyle.Body, color = color, maxLines = if (wrap) Int.MAX_VALUE else 1)
                detail?.let { Text(it, TextStyle.Caption, color = Tokens.mutedForeground.color, maxLines = 1) }
            }
            value?.let { Text(it, TextStyle.Body, Modifier.widthIn(max = 220.dp), color = Tokens.mutedForeground.color, maxLines = 1) }
            trailing?.invoke(this)
            if (chevron) Icon(Symbol.ChevronRight, 14f, Tokens.mutedStrongerForeground.color)
        }
    }
    if (onClick == null) return Face(hovered)
    Pressable(onClick) { Face(it || hovered) }
}

/** A row's picture: its symbol, in a square as wide as a dot's beside it. */
@Composable
private fun Tile(symbol: Symbol, color: Color = Tokens.foreground.color) {
    Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Icon(symbol, 17f / 1.15f, color) }
}

@Composable
private fun Dot(color: Color) {
    Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Box(Modifier.size(10.dp).background(color, CircleShape)) }
}

/** A row a swipe to the left removes: it shows the action in red behind it, and past half the row it is done. */
@Composable
private fun SwipeToRemove(id: String, title: String, remove: () -> Unit, content: @Composable () -> Unit) {
    val offset = remember(id) { Animatable(0f) }
    val removeNow by rememberUpdatedState(remove)
    val scope = rememberCoroutineScope()
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val width = constraints.maxWidth.toFloat()
        if (offset.value < 0f) {
            Box(Modifier.matchParentSize().background(Tokens.destructive.color), contentAlignment = Alignment.CenterEnd) {
                Text(title, TextStyle.LabelStrong, Modifier.padding(horizontal = Space.l), color = Tokens.destructiveForeground.color)
            }
        }
        Box(
            Modifier
                .offset { IntOffset(offset.value.roundToInt(), 0) }
                .background(Tokens.card.color)
                .pointerInput(id) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        val slop = awaitHorizontalTouchSlopOrCancellation(down.id) { change, over -> if (over < 0 || offset.value < 0) change.consume() }
                            ?: return@awaitEachGesture
                        if (!slop.isConsumed) return@awaitEachGesture
                        var moved = offset.value
                        horizontalDrag(slop.id) { change ->
                            moved = (moved + change.positionChange().x).coerceAtMost(0f)
                            change.consume()
                            scope.launch { offset.snapTo(moved) }
                        }
                        scope.launch {
                            if (moved < -width / 2) {
                                offset.animateTo(-width, tween(160))
                                removeNow()
                                offset.snapTo(0f)
                            } else {
                                offset.animateTo(0f, tween(160))
                            }
                        }
                    }
                },
        ) { content() }
    }
}
