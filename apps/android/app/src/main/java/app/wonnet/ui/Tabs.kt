package app.wonnet.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Radius
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import kotlin.math.roundToInt

/**
 * Tabs across the bottom of the screen, each an icon over its name and count, the chosen one in
 * the text's colour. They share the width equally; a long name is cut short. A tab held and
 * dragged goes to another place, and the new order is given when it is let go. The page's colour
 * reaches under the navigation bar.
 */
@Composable
fun <T> BottomTabs(
    tabs: List<T>,
    key: (T) -> String,
    selected: String?,
    symbol: (T) -> Symbol,
    title: (T) -> String,
    count: (T) -> Int,
    pick: (T) -> Unit,
    reorder: (List<String>) -> Unit,
) {
    var width by remember { mutableStateOf(0f) }
    var dragging by remember { mutableStateOf<String?>(null) }
    var offset by remember { mutableStateOf(0f) }
    var pressed by remember { mutableStateOf<String?>(null) }
    val haptics = LocalHapticFeedback.current
    val current by rememberUpdatedState(tabs)
    val share = if (tabs.isEmpty()) 0f else width / tabs.size
    val from = tabs.indexOfFirst { key(it) == dragging }
    val to = if (from < 0 || share == 0f) -1 else (from + (offset / share).roundToInt()).coerceIn(0, tabs.size - 1)
    Row(
        Modifier
            .fillMaxWidth()
            .rule(Edge.Top)
            .background(Tokens.background.color)
            .navigationBarsPadding()
            .padding(horizontal = Space.xxs, vertical = Space.xxs)
            .onSizeChanged { width = it.width.toFloat() },
    ) {
        for ((index, tab) in tabs.withIndex()) {
            val id = key(tab)
            val shift = when {
                from < 0 || index == from -> 0f
                to > from && index in (from + 1)..to -> -share
                to < from && index in to until from -> share
                else -> 0f
            }
            val slide by animateFloatAsState(shift, tween(160), label = "tab")
            val chosen = id == selected
            val lit = chosen || pressed == id || dragging == id
            val foreground = if (lit) Tokens.foreground.color else Tokens.mutedStrongerForeground.color
            Column(
                Modifier
                    .weight(1f)
                    .zIndex(if (dragging == id) 1f else 0f)
                    .graphicsLayer { translationX = if (dragging == id) offset else slide }
                    .background(if (dragging == id) Tokens.backgroundAccent.color else Color.Transparent, RoundedCornerShape(Radius.md))
                    .pointerInput(id) {
                        detectTapGestures(
                            onPress = {
                                pressed = id
                                tryAwaitRelease()
                                pressed = null
                            },
                            onTap = { current.firstOrNull { key(it) == id }?.let(pick) },
                        )
                    }
                    .pointerInput(id) {
                        detectDragGesturesAfterLongPress(
                            onDragStart = {
                                dragging = id
                                offset = 0f
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            },
                            onDrag = { change, amount ->
                                change.consume()
                                offset += amount.x
                            },
                            onDragEnd = {
                                val list = current.map(key)
                                val start = list.indexOf(id)
                                val step = if (width == 0f) 0 else (offset / (width / list.size)).roundToInt()
                                val end = (start + step).coerceIn(0, list.size - 1)
                                if (start >= 0 && end != start) reorder(list.toMutableList().apply { removeAt(start); add(end, id) })
                                dragging = null
                                offset = 0f
                            },
                            onDragCancel = {
                                dragging = null
                                offset = 0f
                            },
                        )
                    }
                    .padding(start = Space.xxs, end = Space.xxs, top = Space.m, bottom = Space.s),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(Space.xxs - 1.dp),
            ) {
                Icon(symbol(tab), 18f, foreground)
                Row(horizontalArrangement = Arrangement.spacedBy(Space.xxs - 1.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(title(tab), if (chosen) TextStyle.FootnoteStrong else TextStyle.Footnote, Modifier.weight(1f, fill = false), color = foreground, maxLines = 1)
                    if (count(tab) > 0) Text("${count(tab)}", TextStyle.Footnote, maxLines = 1)
                }
            }
        }
    }
}

/** Tabs on a line, each with a count; the chosen one underlined in the primary colour. */
@Composable
fun <T> TabStrip(tabs: List<T>, key: (T) -> String, selected: String?, title: (T) -> String, count: (T) -> Int, pick: (T) -> Unit) {
    Row {
        for (tab in tabs) {
            val chosen = key(tab) == selected
            Pressable({ pick(tab) }) { hovered ->
                Box(Modifier.height(Dims.rowHeight)) {
                    Row(
                        Modifier.padding(horizontal = Space.s + 1.dp).align(Alignment.Center),
                        horizontalArrangement = Arrangement.spacedBy(Space.xs),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(title(tab), if (chosen) TextStyle.LabelStrong else TextStyle.Label, color = if (chosen || hovered) Tokens.foreground.color else Tokens.mutedForeground.color, maxLines = 1)
                        if (count(tab) > 0) Text("${count(tab)}", TextStyle.Footnote)
                    }
                    if (chosen) Box(Modifier.matchParentSize().wrapContentHeight(Alignment.Bottom).height(2.dp).background(Tokens.primary.color))
                }
            }
        }
    }
}
