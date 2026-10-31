package app.wonnet.screens

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitHorizontalTouchSlopOrCancellation
import androidx.compose.foundation.gestures.horizontalDrag
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.wonnet.core.MailStore
import app.wonnet.core.ThreadCommand
import app.wonnet.core.ThreadRow
import app.wonnet.core.run
import app.wonnet.core.swipeCommands
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.Spinner
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** The list's place, kept across the pages over it and the app being made anew, from where the store says it was. */
@Composable
fun rememberListState(store: MailStore): LazyListState {
    val density = LocalDensity.current
    val rowPx = with(density) { Dims.rowHeightThread.toPx() }
    val saved = store.listOffset * density.density
    return remember { LazyListState((saved / rowPx).toInt(), (saved % rowPx).toInt()) }
}

/**
 * The thread list: three lines a row. A swipe to the left does what the settings pick; a pull
 * from the top refreshes. Holding a row selects it; while any is selected, a tap picks.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ThreadList(
    store: MailStore,
    rows: List<ThreadRow>,
    list: LazyListState,
    modifier: Modifier = Modifier,
    editing: Boolean,
    checked: Set<String>,
    selectable: Boolean = false,
    ready: Boolean = true,
    keepsScroll: Boolean = true,
    loadingMore: Boolean = false,
    topInset: Dp = 0.dp,
    bottomInset: Dp = 0.dp,
    refresh: (suspend () -> Unit)?,
    farDown: (Boolean) -> Unit = {},
    header: (@Composable () -> Unit)? = null,
    open: (String) -> Unit,
) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    var openRow by remember { mutableStateOf<String?>(null) }
    var refreshing by remember { mutableStateOf(false) }
    val farDownNow by rememberUpdatedState(farDown)
    val shownRows by rememberUpdatedState(rows)

    // A new mailbox's rows start at the top.
    var wasReady by remember { mutableStateOf(ready) }
    LaunchedEffect(ready) {
        if (ready && !wasReady) list.scrollToItem(0)
        wasReady = ready
    }
    LaunchedEffect(list) {
        snapshotFlow { list.firstVisibleItemIndex to list.firstVisibleItemScrollOffset }.collectLatest { (index, offset) ->
            val rowPx = with(density) { Dims.rowHeightThread.toPx() }
            if (keepsScroll) store.noteScroll(((index * rowPx + offset) / density.density).toDouble())
            farDownNow(index * rowPx + offset > list.layoutInfo.viewportSize.height)
            if (index + list.layoutInfo.visibleItemsInfo.size > shownRows.size - 30) store.loadMore()
            openRow = null
            delay(300)
            store.prefetch(list.layoutInfo.visibleItemsInfo.mapNotNull { it.key as? String })
        }
    }

    // Held, then dragged up or down, the list picks every row the finger crosses.
    var sweeping by remember { mutableStateOf(false) }
    fun rowAt(y: Float): String? =
        list.layoutInfo.visibleItemsInfo.firstOrNull { y >= it.offset && y < it.offset + it.size }?.key as? String
    val sweep = if (!selectable) Modifier else Modifier.pointerInput(Unit) {
        detectDragGesturesAfterLongPress(
            onDragStart = { at ->
                sweeping = true
                rowAt(at.y)?.let { store.selection = store.selection + it }
            },
            onDrag = { change, _ -> rowAt(change.position.y)?.let { if (it !in store.selection) store.selection = store.selection + it } },
            onDragEnd = { sweeping = false },
            onDragCancel = { sweeping = false },
        )
    }

    val pull = rememberPullToRefreshState()
    val content: @Composable () -> Unit = {
        LazyColumn(
            Modifier.fillMaxSize().background(Tokens.background.color).then(sweep), state = list, userScrollEnabled = !sweeping,
            contentPadding = PaddingValues(top = topInset, bottom = bottomInset),
        ) {
            if (header != null) item(key = "header") { header() }
            items(rows, key = { it.id }) { row ->
                Box(Modifier.animateItem(fadeInSpec = null, fadeOutSpec = null)) {
                ThreadRowView(
                    store, row, editing, row.id in checked, selectable,
                    isOpen = openRow == row.id,
                    onOpen = { openRow = it },
                    onTap = {
                        when {
                            openRow != null -> openRow = null
                            editing -> store.selection = if (row.id in store.selection) store.selection - row.id else store.selection + row.id
                            else -> open(row.id)
                        }
                    },
                )
                }
            }
            if (loadingMore) item(key = "more") { LoadingMore() }
        }
    }
    if (refresh == null) {
        Box(modifier) { content() }
        return
    }
    PullToRefreshBox(
        refreshing,
        onRefresh = {
            refreshing = true
            scope.launch {
                refresh()
                refreshing = false
            }
        },
        modifier = modifier,
        state = pull,
        indicator = {
            val progress = if (refreshing) 1f else pull.distanceFraction.coerceIn(0f, 1f)
            Box(Modifier.align(Alignment.TopCenter).padding(top = topInset + Space.l).alpha(progress)) {
                if (refreshing) Spinner(16f) else Icon(Symbol.Loader, 16f, Tokens.mutedForeground.color, Modifier.rotate(progress * 270))
            }
        },
    ) { content() }
}

/** The foot of the list while its next page is being read. */
@Composable
fun LoadingMore() {
    Row(Modifier.fillMaxWidth().height(Dims.rowHeight), horizontalArrangement = Arrangement.spacedBy(Space.s, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
        Spinner()
        Text("Loading more", TextStyle.Caption)
    }
}

/** What a row's swipe shows: its commands, where they rest, and how far a long swipe goes. */
private data class SwipeLayout(val commands: List<ThreadCommand>, val rest: Float, val longSwipe: Float)

/** Where a row swiped to the left is, and what it shows behind it. */
private object Swipe {
    /** The width each command takes behind the row. */
    val slot = 64.dp
    /** The space after the last command, to the screen's edge. */
    val inset = 8.dp
    val circle = 40.dp
    const val GLYPH = 19f / 1.15f
}

/**
 * One thread on three lines: who wrote and the star if starred, the subject, the snippet and the
 * date. A swipe to the left slides it aside over its commands: one to three from the screen's
 * edge inwards; it rests on them to tap, and a longer swipe does the first. What takes the thread
 * out of the list slides its row away.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ThreadRowView(
    store: MailStore,
    row: ThreadRow,
    selecting: Boolean,
    checked: Boolean,
    selectable: Boolean,
    isOpen: Boolean,
    onOpen: (String?) -> Unit,
    onTap: () -> Unit,
) {
    val density = LocalDensity.current
    val haptics = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val pulled = remember(row.id) { Animatable(0f) }
    var leaving by remember(row.id) { mutableStateOf<ThreadCommand?>(null) }
    var armedShown by remember(row.id) { mutableStateOf(false) }
    val commands = if (row.draftId == null) store.swipeCommands else listOf(ThreadCommand.Trash)
    val slot = with(density) { Swipe.slot.toPx() }
    val inset = with(density) { Swipe.inset.toPx() }
    val rest = commands.size * slot + inset
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val open by rememberUpdatedState(isOpen)

    LaunchedEffect(isOpen) {
        if (!isOpen && pulled.value != 0f && leaving == null) pulled.animateTo(0f, spring(dampingRatio = 0.95f, stiffness = 500f))
    }

    BoxWithConstraints(
        Modifier
            .fillMaxWidth()
            .height(Dims.rowHeightThread)
            .semantics {
                contentDescription = "${if (row.unread) "Unread, " else ""}${row.senders}, ${row.subject}, ${row.date}"
                // What the swipe offers, as TalkBack's actions on the row.
                customActions = commands.map { command ->
                    CustomAccessibilityAction(command.title(store, listOf(row.id))) {
                        store.run(command, listOf(row.id))
                        true
                    }
                }
            },
    ) {
        val width = with(density) { maxWidth.toPx() }
        val longSwipe = if (commands.size > 1) max(rest + with(density) { 40.dp.toPx() }, width * 0.45f) else rest + with(density) { 40.dp.toPx() }
        val offset = if (leaving != null) width else min(pulled.value, width)
        val armed = leaving != null || pulled.value >= longSwipe
        // The gesture outlives a composition: it reads the commands and the sizes as they are now.
        val layout by rememberUpdatedState(SwipeLayout(commands, rest, longSwipe))
        val openNow by rememberUpdatedState(onOpen)

        fun commit(command: ThreadCommand) {
            onOpen(null)
            if (!command.takesRowAway) {
                store.run(command, listOf(row.id))
                scope.launch { pulled.animateTo(0f, spring(dampingRatio = 0.95f, stiffness = 500f)) }
                return
            }
            leaving = command
            scope.launch {
                pulled.animateTo(width, tween(200, easing = FastOutLinearInEasing))
                store.run(command, listOf(row.id))
                // A list that keeps the thread, like All Mail's, gets its row back.
                delay(800)
                leaving = null
                pulled.animateTo(0f, spring(dampingRatio = 0.95f, stiffness = 500f))
            }
        }

        val commitNow by rememberUpdatedState(::commit)
        if (offset > 0f) {
            SwipeTray(commands, row, offset, rest, armed, leaving, width) { commit(it) }
        }
        Box(
            Modifier
                .fillMaxSize()
                .offset { IntOffset(-offset.roundToInt(), 0) }
                .background(if (offset > 0f) Tokens.background.color else Color.Transparent)
                .pointerInput(row.id, selecting) {
                    if (selecting) return@pointerInput
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        val velocity = VelocityTracker()
                        val slop = awaitHorizontalTouchSlopOrCancellation(down.id) { change, over ->
                            if (over < 0 || open || pulled.value > 0f) change.consume()
                        } ?: return@awaitEachGesture
                        if (!slop.isConsumed) return@awaitEachGesture
                        openNow(row.id)
                        val (commands, rest, longSwipe) = layout
                        val start = pulled.value
                        var moved = 0f
                        horizontalDrag(slop.id) { change ->
                            moved += change.positionChange().x
                            velocity.addPosition(change.uptimeMillis, change.position)
                            change.consume()
                            val next = max(0f, start - moved)
                            val nowArmed = next >= longSwipe
                            if (nowArmed != armedShown) {
                                armedShown = nowArmed
                                haptics.performHapticFeedback(if (nowArmed) HapticFeedbackType.LongPress else HapticFeedbackType.TextHandleMove)
                            }
                            scope.launch { pulled.snapTo(next) }
                        }
                        val speed = -velocity.calculateVelocity().x
                        val first = commands.firstOrNull()
                        if (first != null && pulled.value >= longSwipe) {
                            armedShown = false
                            commitNow(first)
                            return@awaitEachGesture
                        }
                        val opens = if (abs(speed) > 400 * density.density) speed > 0 else pulled.value > rest / 2
                        if (!opens) openNow(null)
                        scope.launch { pulled.animateTo(if (opens) rest else 0f, spring(dampingRatio = 0.95f, stiffness = 400f)) }
                    }
                }
                .combinedClickable(
                    interactionSource = interaction, indication = null,
                    onLongClick = if (selectable && !selecting) {
                        {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            onOpen(null)
                            store.selection = setOf(row.id)
                        }
                    } else null,
                    onClick = onTap,
                ),
        ) {
            RowFace(row, selecting, checked, pressed, current = store.selected == row.id)
        }
    }
}

/** The commands behind a row: a circle each, in a wash of its colour, filled with it once letting go would do it. */
@Composable
private fun SwipeTray(
    commands: List<ThreadCommand>,
    row: ThreadRow,
    offset: Float,
    rest: Float,
    armed: Boolean,
    leaving: ThreadCommand?,
    width: Float,
    tap: (ThreadCommand) -> Unit,
) {
    val density = LocalDensity.current
    val slot = with(density) { Swipe.slot.toPx() }
    val inset = with(density) { Swipe.inset.toPx() }
    val edge = width - offset
    val pulledAlong = max(0f, offset - rest)
    val doing = leaving ?: commands.firstOrNull()
    Box(Modifier.fillMaxSize()) {
        for ((index, command) in commands.withIndex()) {
            val center: Float
            val scale: Float
            val alpha: Float
            val filled: Boolean
            if (!armed) {
                val fromEdge = inset + index * slot
                val uncovered = ((offset - fromEdge) / slot).coerceIn(0f, 1f)
                center = width - fromEdge - slot / 2 - pulledAlong
                scale = 0.7f + 0.3f * uncovered
                alpha = uncovered
                filled = false
            } else {
                val done = command == doing
                center = edge + slot / 2
                scale = if (done) 1f else 0.6f
                alpha = if (done) 1f else 0f
                filled = done
            }
            val interaction = remember { MutableInteractionSource() }
            val pressed by interaction.collectIsPressedAsState()
            val (fill, glyph) = command.swipeTint
            val lit = filled || pressed
            Box(
                Modifier
                    .offset { IntOffset((center - slot / 2).roundToInt(), 0) }
                    .width(Swipe.slot)
                    .fillMaxHeight()
                    .alpha(alpha)
                    .semantics { contentDescription = command.swipeName }
                    .combinedClickable(interactionSource = interaction, indication = null, onClick = { tap(command) }),
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    Modifier
                        .size(Swipe.circle)
                        .scale(scale)
                        .background((if (lit) fill else fill.opacity(Opacity.colorTintStronger)).color, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(command.swipeSymbol(row), Swipe.GLYPH, (if (lit) glyph else fill).color)
                }
            }
        }
    }
}

/** A row's face, as Newton drew them: unread rows bold and dark, read rows grey. */
@Composable
private fun RowFace(row: ThreadRow, selecting: Boolean, checked: Boolean, pressed: Boolean, current: Boolean) {
    val strong = if (row.unread) Tokens.foreground.color else Tokens.mutedForeground.color
    val fill = when {
        checked -> Tokens.primary.opacity(Opacity.colorTint).color
        pressed -> Tokens.backgroundAccentLargerStronger.color
        // The row the keyboard is on, or the thread last open, stays in the hover's fill.
        current -> Tokens.backgroundAccentLarger.color
        else -> Color.Transparent
    }
    Row(Modifier.fillMaxSize().background(fill)) {
        val bar = Dims.accountColor(row.color).color
        if (selecting) {
            Box(Modifier.padding(vertical = Dims.rowGap).width(Dims.accountBarWidth).fillMaxHeight().background(bar, RoundedCornerShape(50)))
            Box(Modifier.fillMaxHeight().width(44.dp), contentAlignment = Alignment.Center) { app.wonnet.ui.SelectCircleFace(checked) }
        } else {
            Box(Modifier.width(Dims.accountBarWidth).fillMaxHeight().background(bar))
        }
        Column(
            Modifier.weight(1f).padding(start = Space.l - Dims.accountBarWidth, end = Space.l, top = 11.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (row.unread) {
                    Box(Modifier.size(8.dp).background(Tokens.primary.color, CircleShape))
                    Spacer(Modifier.width(7.dp))
                }
                Text(row.senders, if (row.unread) TextStyle.Subheading else TextStyle.Body, Modifier.weight(1f), color = strong, maxLines = 1)
                if (row.starred) Icon(Symbol.StarFilled, 18f / 1.15f, Tokens.star.color, Modifier.padding(start = Space.s))
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(row.subject.ifEmpty { "(no subject)" }, if (row.unread) TextStyle.LabelStrong else TextStyle.Label, Modifier.weight(1f), color = strong, maxLines = 1)
                if (row.attachment) Icon(Symbol.Paperclip, 15f / 1.15f, Tokens.mutedStrongerForeground.color, Modifier.padding(start = Space.s))
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(row.snippet.replace('\n', ' '), TextStyle.Label, Modifier.weight(1f), color = Tokens.mutedStrongerForeground.color, maxLines = 1)
                Text(row.date, if (row.unread) TextStyle.LabelStrong else TextStyle.Caption, Modifier.padding(start = Space.s), color = if (row.unread) strong else Tokens.mutedStrongerForeground.color, maxLines = 1)
            }
        }
    }
}

