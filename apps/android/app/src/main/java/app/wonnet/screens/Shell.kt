package app.wonnet.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitHorizontalTouchSlopOrCancellation
import androidx.compose.foundation.gestures.horizontalDrag
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.wonnet.core.MailStore
import app.wonnet.core.openSettings
import app.wonnet.theme.Dims
import app.wonnet.theme.LocalDark
import app.wonnet.theme.Opacity
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Symbol
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow
import app.wonnet.ui.EmptyState
import app.wonnet.ui.Rule
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.min

/** A page over the mailbox: the search, or a thread. */
sealed interface Route {
    data object Search : Route
    data class Thread(val id: String) : Route
}

/**
 * The drawer with the mailboxes, under the mailbox on screen and the pages opened over it,
 * starting where the app was left. Back goes back a page, then closes the drawer. On a wide
 * screen with the reading pane on, the open thread sits beside the list instead.
 */
@Composable
fun Shell(store: MailStore) = BoxWithConstraints(Modifier.fillMaxSize()) {
    // maxWidth misses a change of density alone, which keeps the constraints' pixels.
    val width = with(LocalDensity.current) { constraints.maxWidth.toDp() }
    val pane = store.readingPane && width >= Dims.readingFrom
    val paneNow by rememberUpdatedState(pane)
    var path by remember {
        mutableStateOf((if (store.searchRows == null) emptyList() else listOf<Route>(Route.Search)) + listOfNotNull(store.conversation?.takeIf { !pane }?.let { Route.Thread(it.id) }))
    }
    var drawerOpen by remember { mutableStateOf(false) }
    val list = rememberListState(store)
    val push = { route: Route -> if (!pane || route !is Route.Thread) path = path + route }

    LaunchedEffect(store.requestedThread) {
        val thread = store.requestedThread ?: return@LaunchedEffect
        store.requestedThread = null
        drawerOpen = false
        path = if (pane) emptyList() else listOf(Route.Thread(thread))
    }
    // Another mailbox closes the pages over the one before; the pages restored at the start stay.
    LaunchedEffect(Unit) { snapshotFlow { store.baseMailbox }.drop(1).collect { path = emptyList() } }
    LaunchedEffect(path) {
        // Leaving the search page ends the search.
        if (Route.Search !in path && store.searchQuery.isNotEmpty()) store.endSearch()
    }
    // The open thread moves between the pane and a page over the list as the screen changes.
    LaunchedEffect(pane) {
        if (!pane) {
            val open = store.conversation ?: return@LaunchedEffect
            if (path.none { it is Route.Thread }) path = path + Route.Thread(open.id)
            return@LaunchedEffect
        }
        path = path.filter { it !is Route.Thread }
        snapshotFlow { store.selection.isNotEmpty() }.collect { if (it) store.close() }
    }

    // The hardware keyboard's / opens the search.
    val searchKey by rememberUpdatedState(LocalSearchFocus.current)
    LaunchedEffect(Unit) { snapshotFlow { searchKey }.drop(1).collect { if (Route.Search !in path) path = listOf(Route.Search) } }

    BackHandler(path.isNotEmpty()) { path = path.dropLast(1) }
    BackHandler(path.isEmpty() && store.selection.isNotEmpty()) { store.selection = emptySet() }
    BackHandler(pane && store.conversation != null) { store.close() }
    BackHandler(drawerOpen) { drawerOpen = false }

    val lists: @Composable () -> Unit = {
        MailboxScreen(store, list, openDrawer = { drawerOpen = true }, push = push)
        AnimatedVisibility(Route.Search in path, Modifier.zIndex(1f), enter = slideInHorizontally(tween(220)) { it }, exit = slideOutHorizontally(tween(200)) { it }) {
            SearchScreen(store, back = { path = path.filter { it != Route.Search } }, push = push)
        }
    }
    Drawer(
        open = drawerOpen,
        enabled = path.isEmpty() && store.selection.isEmpty(),
        onChange = { drawerOpen = it },
        sidebar = {
            Sidebar(store, showSettings = {
                drawerOpen = false
                store.openSettings()
            }, picked = {
                drawerOpen = false
                path = emptyList()
            })
        },
    ) {
        if (pane) {
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.width(Dims.listColumn).fillMaxHeight()) { lists() }
                Rule(vertical = true)
                Box(Modifier.weight(1f).fillMaxHeight()) {
                    val open = store.conversation
                    if (open == null) EmptyState(Symbol.MailOpen, "No conversation open") else ThreadScreenStack(store, open.id, dismiss = null)
                }
            }
            return@Drawer
        }
        Box(Modifier.fillMaxSize()) {
            lists()
            val thread = path.lastOrNull() as? Route.Thread
            AnimatedVisibility(thread != null, Modifier.zIndex(2f), enter = slideInHorizontally(tween(220)) { it }, exit = slideOutHorizontally(tween(200)) { it }) {
                val shown = remember { mutableStateOf(thread) }
                if (thread != null) shown.value = thread
                shown.value?.let { route ->
                    ThreadScreenStack(
                        store, route.id, dismiss = { path = path.filter { it !is Route.Thread } },
                        leave = { if (!paneNow) store.close() },
                    )
                }
            }
        }
    }
}

/**
 * The sidebar under the page: a swipe to the right slides the page aside as a card and shows
 * the sidebar; a swipe to the left or a tap on what is left of the card closes it. The card
 * always stays partly in view. `enabled` is false while a swipe to the right is someone else's.
 */
@Composable
fun Drawer(
    open: Boolean,
    enabled: Boolean,
    onChange: (Boolean) -> Unit,
    sidebar: @Composable () -> Unit,
    content: @Composable () -> Unit,
) {
    BoxWithConstraints(Modifier.fillMaxSize().background(Tokens.background.color)) {
        val density = LocalDensity.current
        val sidebarWidth = min(340f, maxWidth.value - 56f).dp
        val widthPx = with(density) { sidebarWidth.toPx() }
        val progress = remember { Animatable(if (open) 1f else 0f) }
        val scope = rememberCoroutineScope()
        val change by rememberUpdatedState(onChange)
        val allowed by rememberUpdatedState(enabled)
        val isOpen by rememberUpdatedState(open)
        LaunchedEffect(open) {
            if (progress.isRunning) return@LaunchedEffect
            progress.animateTo(if (open) 1f else 0f, spring(stiffness = 500f, dampingRatio = 1f))
        }
        val dark = LocalDark.current
        val dimmed = Opacity.dimmedStronger.value(dark)
        if (progress.value > 0f) {
            Box(Modifier.width(sidebarWidth).fillMaxHeight()) { sidebar() }
        }
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer { translationX = progress.value * widthPx }
                .then(if (progress.value > 0f) Modifier.shadow(ShadowSize.MD, androidx.compose.ui.graphics.RectangleShape, Opacity.shadowStronger) else Modifier)
                .background(Tokens.background.color)
                .pointerInput(Unit) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        val velocity = VelocityTracker()
                        var start = 0f
                        val slop = awaitHorizontalTouchSlopOrCancellation(down.id) { change, over ->
                            val opening = !isOpen && over > 0 && allowed
                            val closing = isOpen && over < 0
                            if (opening || closing) {
                                change.consume()
                                start = progress.value
                            }
                        } ?: return@awaitEachGesture
                        if (!slop.isConsumed && !isOpen) return@awaitEachGesture
                        var moved = 0f
                        horizontalDrag(slop.id) { change ->
                            moved += change.positionChange().x
                            velocity.addPosition(change.uptimeMillis, change.position)
                            change.consume()
                            val wanted = start + moved / widthPx
                            val over = if (wanted > 1) wanted - 1 else if (wanted < 0) wanted else 0f
                            scope.launch { progress.snapTo(wanted.coerceIn(0f, 1f) + over * 0.12f) }
                        }
                        val speed = velocity.calculateVelocity().x
                        val opens = if (abs(speed) > 280 * density.density) speed > 0 else progress.value > 0.5f
                        scope.launch { progress.animateTo(if (opens) 1f else 0f, spring(stiffness = 500f, dampingRatio = 1f)) }
                        change(opens)
                    }
                },
        ) {
            val shade = ((progress.value - 0.5f) / 0.5f).coerceIn(0f, 1f)
            Box(Modifier.fillMaxSize().alpha(1f - (1f - dimmed) * shade)) { content() }
            if (progress.value > 0f) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .pointerInput(Unit) {
                            awaitEachGesture {
                                awaitFirstDown()
                                val up = waitForUpOrCancellation()
                                if (up != null) change(false)
                            }
                        },
                )
                app.wonnet.ui.Rule(Modifier.alpha(min(1f, progress.value * 6)), vertical = true)
            }
        }
    }
}

