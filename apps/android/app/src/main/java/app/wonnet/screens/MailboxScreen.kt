package app.wonnet.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.wonnet.core.Filter
import app.wonnet.core.MailStore
import app.wonnet.core.ThreadCommand
import app.wonnet.core.applicable
import app.wonnet.core.canEmpty
import app.wonnet.core.clearSelection
import app.wonnet.core.emptyMailbox
import app.wonnet.core.run
import app.wonnet.core.selectAll
import app.wonnet.theme.Dims
import app.wonnet.theme.SCALE
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.BottomTabs
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Chip
import app.wonnet.ui.Edge
import app.wonnet.ui.EmptyState
import app.wonnet.ui.FloatingButton
import app.wonnet.ui.GlassBar
import app.wonnet.ui.GlassCircle
import app.wonnet.ui.HeaderBar
import app.wonnet.ui.IconButton
import app.wonnet.ui.IconMenu
import app.wonnet.ui.MenuItem
import app.wonnet.ui.SearchField
import app.wonnet.ui.SearchStyle
import app.wonnet.ui.Spinner
import app.wonnet.ui.TabStrip
import app.wonnet.ui.rule
import kotlinx.coroutines.launch

/** Opens a thread as a page over the list; a draft opens in compose instead. */
private fun openThread(store: MailStore, thread: String, push: (Route) -> Unit) {
    val draft = store.row(thread)?.draftId != null
    store.open(thread)
    if (!draft) push(Route.Thread(thread))
}

/**
 * A mailbox's threads under our own bar, with Compose floating over them and the tabs along the
 * bottom. Holding a row selects it, and the list picks threads to act on together until none is
 * left selected. Search is a page of its own.
 */
@Composable
fun MailboxScreen(store: MailStore, list: LazyListState, openDrawer: () -> Unit, push: (Route) -> Unit) {
    val editing = store.selection.isNotEmpty()
    var farDown by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    Column(Modifier.fillMaxSize().background(Tokens.background.color)) {
        Box(Modifier.statusBarsPadding().rule(Edge.Bottom)) {
            if (editing) EditingBar(store) else MailboxBar(store, openDrawer) { push(Route.Search) }
        }
        Box(Modifier.weight(1f)) {
            ThreadList(
                store, store.rows, list, Modifier.fillMaxSize(), editing, store.selection, selectable = true, ready = store.listReady,
                loadingMore = store.loadingMore, bottomInset = if (editing) Dims.barHeight + Space.l * 2 else Dims.floatingButton + Space.l * 2,
                refresh = store::checkMail, farDown = { farDown = it },
                header = if (ListHeader.shows(store)) ({ ListHeader(store) }) else null,
            ) { openThread(store, it, push) }
            if (store.rows.isEmpty() && store.listReady) EmptyList(store)
            ToTop(farDown, Modifier.align(Alignment.TopCenter)) { scope.launch { list.animateScrollToItem(0) } }
            if (!editing) {
                FloatingButton(Symbol.SquarePen, "Compose", Modifier.align(Alignment.BottomEnd).padding(Space.l), onClick = store::newMessage)
            } else {
                CommandBar(store, store.selection.toList(), Modifier.align(Alignment.BottomCenter))
            }
        }
        if (!editing) MailboxTabBar(store) else Spacer(Modifier.navigationBarsPadding())
    }
}

@Composable
private fun MailboxBar(store: MailStore, openDrawer: () -> Unit, search: () -> Unit) {
    HeaderBar(
        leading = { IconButton(Symbol.Menu, "Mailboxes", circled = false, onClick = openDrawer) },
        center = { MailboxTitle(store) },
        trailing = { IconButton(Symbol.Search, "Search", circled = false, onClick = search) },
    )
}

@Composable
private fun EditingBar(store: MailStore) {
    val all = store.selection.size == store.rows.size
    HeaderBar(
        leading = { ActionButton(if (all) "Select None" else "Select All", variant = ButtonVariant.Ghost) { if (all) store.clearSelection() else store.selectAll() } },
        center = { Text("${store.selection.size} Selected", TextStyle.Subheading, maxLines = 1) },
        trailing = { ActionButton("Done", variant = ButtonVariant.Ghost, onClick = store::clearSelection) },
    )
}

/**
 * The open mailbox's name over the list: "Inbox · Work", the dot fainter than the names, with a
 * spinner after it while its mail is still coming. The spinner's slot is always there, so the name
 * stays in the middle.
 */
@Composable
fun MailboxTitle(store: MailStore) {
    val (name, account) = store.baseMailboxTitle
    val muted = Tokens.mutedForeground.color
    val text = buildAnnotatedString {
        append(name)
        if (account != null) {
            withStyle(SpanStyle(color = muted)) { append(" · ") }
            append(account)
        }
    }
    val slot = (14 * SCALE).dp
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.xs)) {
        Spacer(Modifier.size(slot))
        Text(text, TextStyle.Subheading, Modifier.widthIn(max = 260.dp), maxLines = 1)
        Box(Modifier.size(slot)) { if (store.mailboxSyncing) Spinner(14f, Tokens.mutedStrongerForeground.color) }
    }
}

/** Over the list: the split inbox's tabs, the filter the list is narrowed to, and emptying Trash or Spam. */
object ListHeader {
    fun shows(store: MailStore) = store.searchRows == null && (store.splits.isNotEmpty() || store.filter != null || store.canEmpty)
}

@Composable
fun ListHeader(store: MailStore) {
    val chosen = store.splits.firstOrNull { it.mailbox == store.mailbox }?.mailbox ?: store.splits.firstOrNull()?.mailbox
    Row(
        Modifier.fillMaxWidth().height((38 * SCALE).dp).background(Tokens.background.color).rule(Edge.Bottom),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Space.m),
    ) {
        Box(Modifier.weight(1f).horizontalScroll(rememberScrollState()).padding(horizontal = Space.l)) {
            TabStrip(store.splits, { it.mailbox }, chosen, { it.name }, { it.unread }) { store.select(it.mailbox) }
        }
        store.filter?.let { filter ->
            Chip(if (filter == Filter.Unread) "Unread" else "Starred", Modifier.padding(end = Space.l), symbol = Symbol.ListFilter, remove = { store.narrow(null) })
        }
        if (store.canEmpty) {
            ActionButton("Empty ${store.mailboxName}", Modifier.padding(end = Space.l), variant = ButtonVariant.Ghost, onClick = store::emptyMailbox)
        }
    }
}

/** An empty mailbox: a calm line instead of a list. */
@Composable
fun EmptyList(store: MailStore) {
    val message = when {
        store.searchRows != null -> "Nothing matches."
        store.filter == Filter.Unread || store.inUnread -> "Nothing unread."
        store.filter == Filter.Starred -> "Nothing starred."
        store.mailboxSyncing -> "Fetching your mail"
        store.baseMailbox.endsWith("inbox") -> "All done. Enjoy the quiet."
        else -> "Nothing here."
    }
    EmptyState(if (store.searchRows != null) Symbol.Search else Symbol.Inbox, message)
}

/** The arrow back to the top, near the head of a list scrolled past its first page. */
@Composable
fun ToTop(shown: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    AnimatedVisibility(shown, modifier.padding(top = Space.m), enter = fadeIn() + scaleIn(initialScale = 0.8f), exit = fadeOut() + scaleOut(targetScale = 0.8f)) {
        FloatingButton(Symbol.ArrowUp, "Back to Top", small = true, onClick = onClick)
    }
}

/** The mailboxes kept as tabs, along the bottom. One held and dragged moves; the order is the `tabs` preference, so every device shows the same. */
@Composable
fun MailboxTabBar(store: MailStore) {
    val tabs = store.tabs
    if (tabs.isEmpty()) return Spacer(Modifier.navigationBarsPadding())
    BottomTabs(
        tabs, { it.id }, store.baseMailbox, { Symbol.named(it.symbol) }, { store.tabName(it, store.listAccount) }, { it.unread },
        pick = { store.select(it.id) }, reorder = { ids -> store.setTabs(ids.map(store::savedTab)) },
    )
}

/**
 * What can be done to threads along the bottom, spread across a floating bar. Under a thread, the
 * usual few with menus for the rest, and Reply on a circle of its own beside it; over a selection,
 * the same as the Mac's selection bar, the rest in More.
 */
@Composable
fun CommandBar(store: MailStore, threads: List<String>, modifier: Modifier = Modifier, reply: Boolean = false) {
    fun items(commands: List<ThreadCommand>) = commands.map { command ->
        MenuItem(command.title(store, threads), command.symbol, destructive = command == ThreadCommand.DeleteForever) { store.run(command, threads) }
    }
    Row(
        modifier.navigationBarsPadding().widthIn(max = 640.dp).fillMaxWidth().padding(horizontal = Space.l).padding(bottom = Space.s),
        horizontalArrangement = Arrangement.spacedBy(Space.s),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlassBar(Modifier.weight(1f)) {
            if (reply) {
                for (command in listOf(ThreadCommand.Archive, ThreadCommand.Trash, ThreadCommand.Snooze)) {
                    IconButton(command.symbol, command.title(store, threads), circled = false, strong = true) { store.run(command, threads) }
                }
                IconMenu(Symbol.Tag, "Label", items(listOf(ThreadCommand.Label, ThreadCommand.Move)), circled = false, strong = true)
                IconMenu(
                    Symbol.Ellipsis, "More",
                    items(listOf(ThreadCommand.Read) + ThreadCommand.more) + items(listOf(ThreadCommand.ReplyAll, ThreadCommand.Forward)).mapIndexed { index, item ->
                        if (index == 0) item.copy(divider = true) else item
                    },
                    circled = false, strong = true,
                )
            } else {
                for (command in store.applicable(ThreadCommand.selection, threads)) {
                    IconButton(command.symbol(store, threads), command.title(store, threads), circled = false, strong = true) { store.run(command, threads) }
                }
                val more = store.applicable(ThreadCommand.selectionMore, threads)
                if (more.isNotEmpty()) IconMenu(Symbol.Ellipsis, "More", items(more), circled = false, strong = true)
            }
        }
        if (reply) GlassCircle(Symbol.Reply, "Reply") { store.run(ThreadCommand.Reply, threads) }
    }
}

/** Search as a page of its own: back and the field in the bar, the matches under them. */
@Composable
fun SearchScreen(store: MailStore, back: () -> Unit, push: (Route) -> Unit) {
    var query by remember { mutableStateOf(store.searchQuery) }
    var farDown by remember { mutableStateOf(false) }
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val focus = LocalSearchFocus.current
    LaunchedEffect(query) { store.search(query) }
    Column(Modifier.fillMaxSize().background(Tokens.background.color)) {
        Box(Modifier.statusBarsPadding().rule(Edge.Bottom)) {
            Row(Modifier.fillMaxWidth().height(Dims.barHeight).padding(horizontal = Space.s), verticalAlignment = Alignment.CenterVertically) {
                IconButton(Symbol.ArrowLeft, "Back", circled = false, onClick = back)
                SearchField(
                    "Search", query, { query = it }, Modifier.weight(1f).padding(end = Space.s), style = SearchStyle.Bar,
                    autofocus = store.searchQuery.isEmpty(), focusTrigger = focus, submit = { store.search(query) }, clear = store::endSearch,
                )
            }
        }
        Box(Modifier.weight(1f)) {
            ThreadList(
                store, store.searchRows ?: emptyList(), list, Modifier.fillMaxSize(), editing = false, checked = emptySet(), keepsScroll = false,
                refresh = store::searchAgain, farDown = { farDown = it }, bottomInset = Space.l,
            ) { openThread(store, it, push) }
            if (store.searchRows?.isEmpty() == true) EmptyList(store)
            ToTop(farDown, Modifier.align(Alignment.TopCenter)) { scope.launch { list.animateScrollToItem(0) } }
        }
        Spacer(Modifier.navigationBarsPadding())
    }
}
