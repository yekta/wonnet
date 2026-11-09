package app.wonnet.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.wonnet.BuildConfig
import app.wonnet.core.AccountView
import app.wonnet.core.Appearance
import app.wonnet.core.CustomRule
import app.wonnet.core.CustomSplit
import app.wonnet.core.MailStore
import app.wonnet.core.Mailbox
import app.wonnet.core.NewLabel
import app.wonnet.core.NotificationButton
import app.wonnet.core.SettingsEntry
import app.wonnet.core.SettingsSection
import app.wonnet.core.Snippet
import app.wonnet.core.SwipePreference
import app.wonnet.core.ThreadCommand
import app.wonnet.core.blockedSenders
import app.wonnet.core.applyRule
import app.wonnet.core.confirmRemoving
import app.wonnet.core.confirmSigningOut
import app.wonnet.core.customRules
import app.wonnet.core.customSplits
import app.wonnet.core.flag
import app.wonnet.core.mailboxesForTabs
import app.wonnet.core.moveRule
import app.wonnet.core.moveSplit
import app.wonnet.core.moveSwipeCommand
import app.wonnet.core.moveTab
import app.wonnet.core.newRule
import app.wonnet.core.newSplit
import app.wonnet.core.notifies
import app.wonnet.core.openSettings
import app.wonnet.core.ownSignature
import app.wonnet.core.providedSignature
import app.wonnet.core.providerName
import app.wonnet.core.removeRule
import app.wonnet.core.removeSplit
import app.wonnet.core.saveRule
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
import app.wonnet.core.ConnectedApps
import app.wonnet.core.CoreError
import app.wonnet.core.mcpServer
import app.wonnet.core.connectedApps
import app.wonnet.core.revokeConnectedApp
import app.wonnet.core.confirmRevoking
import app.wonnet.ui.Chip
import app.wonnet.ui.CopyField
import app.wonnet.ui.Notice
import app.wonnet.ui.RowsCard
import app.wonnet.ui.RowsCardRule
import app.wonnet.ui.Spinner
import androidx.compose.runtime.mutableIntStateOf
import app.wonnet.ui.ActionButton
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.CardRow
import app.wonnet.ui.takesNoHeight
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

    data class RuleEdit(val rule: CustomRule, val isNew: Boolean) : Page {
        override val title get() = if (isNew) "New Rule" else "Edit Rule"
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
    val groups = listOf(listOf(SettingsSection.General, SettingsSection.Accounts), listOf(SettingsSection.Inbox, SettingsSection.Reading, SettingsSection.Writing), listOf(SettingsSection.ConnectedApps))
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
                    SettingsSection.ConnectedApps -> ConnectedAppsPage(store, anchor)
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
                is Page.RuleEdit -> RulePage(store, page.rule, page.isNew, back ?: close) { action, enabled ->
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
    val wide = with(LocalDensity.current) { LocalWindowInfo.current.containerSize.width.toDp() } >= Dims.wide
    Group("Appearance", modifier = anchor("appearance")) {
        SettingsRow("Theme", trailing = { ChoiceMenu(Appearance.entries, store.appearance, { it.title }, ControlSize.Small, store::changeAppearance) })
        if (wide) {
            RowRule()
            SettingsRow(
                "Reading Pane", detail = "Open mail next to the list when the screen is wide enough.",
                trailing = { Toggle(store.readingPane, store::changeReadingPane) },
            )
        }
    }
    Group(
        "Notifications", "The button a new mail's notification has next to Mark as Read and Reply. After it, the notification offers Undo.",
        anchor("notifications"),
    ) {
        SettingsRow("Button", trailing = {
            ChoiceMenu(NotificationButton.entries, store.notificationButton, { it.title }, ControlSize.Small, store::changeNotificationButton)
        })
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

// Connected apps

/**
 * The MCP server's address to add Wonnet to an application, and the applications let in, read
 * from the server each time the page opens. A revoked one leaves the list once the server has it.
 */
@Composable
private fun ConnectedAppsPage(store: MailStore, anchor: (String) -> Modifier) {
    var server by remember { mutableStateOf<String?>(null) }
    var serverError by remember { mutableStateOf<String?>(null) }
    var listed by remember { mutableStateOf<ConnectedApps?>(null) }
    var listError by remember { mutableStateOf<String?>(null) }
    var loads by remember { mutableIntStateOf(0) }
    var revoking by remember { mutableStateOf(emptySet<String>()) }
    LaunchedEffect(Unit) {
        try {
            server = store.mcpServer()
        } catch (error: CoreError) {
            serverError = error.message
        }
    }
    LaunchedEffect(loads, store.connectedAppsRevision) {
        listError = null
        try {
            listed = store.connectedApps()
        } catch (error: CoreError) {
            listError = error.message
        }
    }
    val address = listed?.address?.takeIf { it.isNotEmpty() } ?: server
    Group("MCP Server", "Use this URL with MCP clients such as Claude or ChatGPT. You can pick what each one can access.", anchor("connect"), card = false) {
        val error = serverError
        when {
            address != null -> CopyField(address, "the address")
            error != null -> Notice(error, Modifier.padding(horizontal = Space.l), error = true, symbol = Symbol.CircleAlert)
            else -> Notice("Loading", Modifier.padding(horizontal = Space.l))
        }
    }
    val apps = listed?.apps
    Group("Connected apps", "They reach your mail as you chose when you approved them. Revoke one to end its access at once.", anchor("connected")) {
        val error = listError
        when {
            error != null -> Notice(error, Modifier.padding(start = Space.l, end = Space.l - Dims.buttonGap / 2, top = Space.xs, bottom = Space.xs), error = true, symbol = Symbol.CircleAlert, action = "Try again") { loads += 1 }
            apps == null -> SettingsRow("Loading", leading = { Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Spinner() } }, muted = true)
            apps.isEmpty() -> SettingsRow("No applications are connected yet.", muted = true)
        }
        for ((index, app) in apps.orEmpty().withIndex()) {
            key(app.id) {
                if (index > 0 || error != null) RowRule()
                SettingsRow(
                    app.name, lines = listOf(app.access, app.used), wrap = true,
                    badge = if (app.verified) ({ Chip("Verified", symbol = Symbol.ShieldCheck, tint = Tokens.success) }) else null,
                    trailing = {
                        ActionButton("Revoke", size = ControlSize.Small, pending = app.id in revoking, help = "Revoke ${app.name}") {
                            store.confirmRevoking(app) {
                                revoking = revoking + app.id
                                store.scope.launch {
                                    try {
                                        store.revokeConnectedApp(app.id)
                                        listed = listed?.let { it.copy(apps = it.apps.filter { each -> each.id != app.id }) }
                                    } catch (error: CoreError) {
                                        store.show(error.message)
                                    }
                                    revoking = revoking - app.id
                                }
                            }
                        }
                    },
                )
            }
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
        MovableList(splits, { it.id }, rowRadius = Radius.md, clicked = { push(Page.Split(it, false)) }, moved = { split, index -> store.moveSplit(split, index) }) { split, hovered ->
            SwipeToRemove(split.id, "Remove", { store.removeSplit(split) }) {
                SettingsRow(split.name, detail = store.summary(split), leading = { Tile(Symbol.ListFilter) }, chevron = true, hovered = hovered)
            }
        }
        if (splits.isNotEmpty()) RowRule()
        SettingsRow("Add Split", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.Split(store.newSplit(), true)) }
    }
    val rules = store.customRules
    Group("Rules", "Mail that matches is labelled, kept out of the inbox or marked read as it arrives.", anchor("rules")) {
        MovableList(rules, { it.id }, rowRadius = Radius.md, clicked = { push(Page.RuleEdit(it, false)) }, moved = { rule, index -> store.moveRule(rule, index) }) { rule, hovered ->
            SwipeToRemove(rule.id, "Remove", { store.removeRule(rule) }) {
                SettingsRow(store.summary(rule), leading = { Tile(Symbol.Zap) }, chevron = true, wrap = true, hovered = hovered)
            }
        }
        if (rules.isNotEmpty()) RowRule()
        SettingsRow("Add Rule", leading = { Tile(Symbol.Plus, Tokens.primary.color) }, action = true) { push(Page.RuleEdit(store.newRule(), true)) }
    }
    val tabs = store.savedTabs
    Group("Tabs", "The mailboxes along the bottom. Hold a tab and drag it to move it; swipe it to remove it.", anchor("tabs")) {
        MovableList(tabs, { it.id }, rowRadius = Radius.md, moved = { tab, index -> store.moveTab(tab, index) }) { tab, _ ->
            SwipeToRemove(tab.id, "Remove", { store.toggleTab(tab.id) }) {
                SettingsRow(store.tabName(tab), leading = { Tile(Symbol.named(tab.symbol)) })
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
        MovableList(commands, { it.id }, rowRadius = Radius.md, moved = { command, index -> store.moveSwipeCommand(command, index) }) { command, _ ->
            val row = @Composable {
                SettingsRow(command.swipeName, leading = { SwipeTile(command) }, value = if (command == commands.first()) (if (commands.size == 1) "Short swipe" else "Long swipe") else null)
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

/** The label a rule puts on mail: none, one there is, or one made as the rule is saved. */
private sealed interface RuleLabel {
    data object None : RuleLabel
    data class Existing(val id: String) : RuleLabel
    data object New : RuleLabel
}

/** A rule being written: what it looks for, what it does, and for a new one whether it also does it to the mail already here. */
@Composable
private fun RulePage(store: MailStore, start: CustomRule, isNew: Boolean, back: () -> Unit, saving: (() -> Unit, Boolean) -> Unit) {
    var from by remember { mutableStateOf(start.from.joinToString(", ")) }
    var to by remember { mutableStateOf(start.to.joinToString(", ")) }
    var subject by remember { mutableStateOf(start.subject.joinToString(", ")) }
    var label by remember { mutableStateOf<RuleLabel>(start.label?.let { RuleLabel.Existing(it) } ?: RuleLabel.None) }
    var labelName by remember { mutableStateOf("") }
    var labelAccount by remember { mutableStateOf(store.accounts.firstOrNull()?.id) }
    var skipInbox by remember { mutableStateOf(start.skipInbox) }
    var markRead by remember { mutableStateOf(start.markRead) }
    var applyNow by remember { mutableStateOf(true) }
    var pending by remember { mutableStateOf(false) }
    val labels = store.splitLabels
    val accounts = store.accounts
    val rule = start.copy(
        from = CustomSplit.senders(from), to = CustomSplit.senders(to), subject = CustomRule.words(subject),
        label = (label as? RuleLabel.Existing)?.id, skipInbox = skipInbox, markRead = markRead,
    )
    val newLabel = labelAccount?.takeIf { label == RuleLabel.New }?.let { NewLabel(it, labelName) }
    val ready = when (label) {
        RuleLabel.New -> newLabel != null && labelName.isNotBlank()
        else -> rule.doesSomething
    }
    LaunchedEffect(rule, newLabel, applyNow, ready, pending) {
        saving({
            pending = true
            store.scope.launch {
                if (store.saveRule(rule, isNew && applyNow, newLabel)) back() else pending = false
            }
        }, rule.hasCondition && ready && !pending)
    }
    Group("From", "Addresses, or @ and a domain for everyone there.") {
        Box(Modifier.padding(Space.s)) { InputField(from, { from = it }, placeholder = "ann@example.com, @example.com", lines = 2, autofocus = isNew, keyboard = KeyboardType.Email) }
    }
    Group("To", "To or Cc, as From.") {
        Box(Modifier.padding(Space.s)) { InputField(to, { to = it }, placeholder = "team@example.com", lines = 2, keyboard = KeyboardType.Email) }
    }
    Group("Subject Contains", "Words the subject has, split by commas.") {
        Box(Modifier.padding(Space.s)) { InputField(subject, { subject = it }, placeholder = "[project], invoice") }
    }
    Group(footer = "Mail that meets every condition, any entry of one, is acted on.") {
        if (accounts.isNotEmpty()) {
            val choices = listOf(RuleLabel.None) + labels.map { RuleLabel.Existing(it.first) } + RuleLabel.New
            val title = { choice: RuleLabel ->
                when (choice) {
                    RuleLabel.None -> "None"
                    is RuleLabel.Existing -> labels.firstOrNull { it.first == choice.id }?.second ?: "None"
                    RuleLabel.New -> "New Label"
                }
            }
            SettingsRow("Label", trailing = { ChoiceMenu(choices, label, title, ControlSize.Small) { label = it } })
            RowRule()
        }
        if (label == RuleLabel.New) {
            SettingsRow("Name", trailing = {
                InputField(labelName, { labelName = it }, Modifier.widthIn(max = 220.dp), placeholder = "Receipts", autofocus = true, capitalize = true)
            })
            RowRule()
            if (accounts.size > 1) {
                SettingsRow("Account", trailing = {
                    ChoiceMenu(accounts.map { it.id }, labelAccount, { id -> accounts.firstOrNull { it.id == id }?.title ?: "" }, ControlSize.Small) { labelAccount = it }
                })
                RowRule()
            }
        }
        SettingsRow("Skip the Inbox", trailing = { Toggle(skipInbox, { skipInbox = it }) })
        RowRule()
        SettingsRow("Mark as Read", trailing = { Toggle(markRead, { markRead = it }) })
    }
    if (isNew) {
        Group { SettingsRow("Also Apply to Mail Already Here", trailing = { Toggle(applyNow, { applyNow = it }) }) }
        return
    }
    Group {
        SettingsRow("Remove Rule", destructive = true) {
            store.removeRule(start)
            back()
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

/**
 * A group of rows: its name over it, a card of rows, and the muted line under it saying what it
 * does. The rows' fills reach the card's edges and take its corners. `card` false puts the content
 * on the page instead, as wide as the cards: a control that stands alone, such as a field.
 */
@Suppress("ModifierParameter")
@Composable
private fun Group(title: String? = null, footer: String? = null, modifier: Modifier = Modifier, card: Boolean = true, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(Space.s)) {
        title?.let { Text(it, TextStyle.Subheading, Modifier.padding(horizontal = Space.l)) }
        if (card) RowsCard(content = content) else Column(Modifier.fillMaxWidth(), content = content)
        footer?.let { Text(it, TextStyle.Caption, Modifier.padding(horizontal = Space.l), color = Tokens.mutedForeground.color) }
    }
}

@Composable
private fun RowRule() = RowsCardRule()

/**
 * A row of a group: its picture, what it is with a line under (and more `lines`), and its value,
 * control or chevron at the end. `badge` stands after the title; `wrap` lets every line wrap.
 */
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
    lines: List<String> = emptyList(),
    badge: (@Composable () -> Unit)? = null,
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
        val control = trailing?.let { slot -> @Composable { Row(verticalAlignment = Alignment.CenterVertically) { slot() } } }
        CardRow(Modifier.background(fill), trailing = control) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                leading?.invoke()
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    val most = if (wrap) Int.MAX_VALUE else 1
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                        Text(title, TextStyle.Body, Modifier.weight(1f, fill = false), color = color, maxLines = most)
                        badge?.let { Box(Modifier.takesNoHeight()) { it() } }
                    }
                    detail?.let { Text(it, TextStyle.Caption, color = Tokens.mutedForeground.color, maxLines = most) }
                    for (line in lines) Text(line, TextStyle.Caption, color = Tokens.mutedForeground.color, maxLines = most)
                }
                value?.let { Text(it, TextStyle.Body, Modifier.widthIn(max = 220.dp), color = Tokens.mutedForeground.color, maxLines = 1) }
                if (chevron) Icon(Symbol.ChevronRight, 14f, Tokens.mutedStrongerForeground.color)
            }
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
                // Covers the action only while swiped, so that a lifted row lies on its light.
                .background(if (offset.value < 0f) Tokens.card.color else Color.Transparent)
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
