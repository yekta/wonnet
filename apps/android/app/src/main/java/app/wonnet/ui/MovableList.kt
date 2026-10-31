package app.wonnet.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.zIndex
import app.wonnet.theme.Radius
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow
import kotlin.math.abs

/**
 * Rows the list picks up and moves itself, in groups: a row held and pulled is lifted on its
 * light, a little smaller, and follows the finger; the rows of its group it passes slide out of
 * its way, and let go it lands in its place. The rows under one that `follows` go along with it.
 * A row held and let go without moving opens its `menu`. Rows are faces: the list takes the taps
 * and tells each row when it is pressed.
 */
@Composable
fun <T> MovableList(
    items: List<T>,
    key: (T) -> String,
    modifier: Modifier = Modifier,
    group: (T) -> String? = { "rows" },
    follows: (T) -> Boolean = { false },
    clicked: (T) -> Unit,
    moved: (T, Int) -> Unit,
    menu: (T) -> List<MenuItem> = { emptyList() },
    row: @Composable (T, hovered: Boolean) -> Unit,
) {
    val places = remember { mutableStateMapOf<String, Pair<Float, Float>>() }
    var lifted by remember { mutableStateOf<List<String>>(emptyList()) }
    var offset by remember { mutableStateOf(0f) }
    var pressed by remember { mutableStateOf<String?>(null) }
    var menuFor by remember { mutableStateOf<String?>(null) }
    val haptics = LocalHapticFeedback.current
    // The gestures outlive a composition: they read what the list holds now.
    val current by rememberUpdatedState(items)
    val keyOf by rememberUpdatedState(key)
    val groupOf by rememberUpdatedState(group)
    val followsNow by rememberUpdatedState(follows)
    val clickedNow by rememberUpdatedState(clicked)
    val movedNow by rememberUpdatedState(moved)
    val menuNow by rememberUpdatedState(menu)

    /** Where the lifted rows would land, as an index among the rows without them. */
    fun landing(): Int? {
        val first = lifted.firstOrNull() ?: return null
        val list = current
        val start = list.indexOfFirst { keyOf(it) == first }
        if (start < 0) return null
        val rest = list.filter { keyOf(it) !in lifted }
        val (top, _) = places[first] ?: return null
        val height = lifted.sumOf { (places[it]?.second ?: 0f).toDouble() }.toFloat()
        val center = top + offset + height / 2
        val kind = groupOf(list[start])
        var target = rest.indexOfFirst { groupOf(it) == kind }.takeIf { it >= 0 } ?: return null
        var index = 0
        while (index < rest.size) {
            val item = rest[index]
            if (groupOf(item) != kind) {
                index++
                continue
            }
            var end = index + 1
            while (end < rest.size && followsNow(rest[end])) end++
            val blockTop = places[keyOf(item)]?.first ?: break
            val blockBottom = rest.subList(index, end).maxOf { (places[keyOf(it)]?.let { place -> place.first + place.second } ?: blockTop) }
            if (center > (blockTop + blockBottom) / 2) target = end
            index = end
        }
        return target
    }

    Column(modifier) {
        val start = items.indexOfFirst { key(it) == lifted.firstOrNull() }
        val withoutLifted = items.filter { key(it) !in lifted }
        val liftedHeight = lifted.sumOf { (places[it]?.second ?: 0f).toDouble() }.toFloat()
        val target = landing()
        val origin = if (start < 0) -1 else items.take(start).count { key(it) !in lifted }
        for (item in items) {
            val id = key(item)
            androidx.compose.runtime.key(id) {
            val isLifted = id in lifted
            val index = withoutLifted.indexOfFirst { key(it) == id }
            val shift = when {
                isLifted || target == null || origin < 0 -> 0f
                target > origin && index in origin until target -> -liftedHeight
                target < origin && index in target until origin -> liftedHeight
                else -> 0f
            }
            val slide by animateFloatAsState(shift, tween(180), label = "slide")
            val draggable = group(item) != null
            Box(
                Modifier
                    .fillMaxWidth()
                    .onPlaced { if (id !in lifted) places[id] = it.positionInParent().y to it.size.height.toFloat() }
                    .zIndex(if (isLifted) 1f else 0f)
                    .graphicsLayer { translationY = if (isLifted) offset else slide }
                    .then(if (isLifted && lifted.first() == id) Modifier.scale(0.97f).shadow(ShadowSize.LG, RoundedCornerShape(Radius.md)).background(Tokens.popover.color, RoundedCornerShape(Radius.md)) else Modifier)
                    .pointerInput(id) {
                        detectTapGestures(
                            onPress = {
                                pressed = id
                                tryAwaitRelease()
                                pressed = null
                            },
                            onTap = { current.firstOrNull { keyOf(it) == id }?.let(clickedNow) },
                        )
                    }
                    .pointerInput(id, draggable) {
                        if (!draggable) return@pointerInput
                        var travelled = 0f
                        detectDragGesturesAfterLongPress(
                            onDragStart = {
                                val list = current
                                val at = list.indexOfFirst { keyOf(it) == id }
                                if (at < 0) return@detectDragGesturesAfterLongPress
                                var end = at + 1
                                while (end < list.size && followsNow(list[end])) end++
                                lifted = list.subList(at, end).map(keyOf)
                                offset = 0f
                                travelled = 0f
                                pressed = null
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            },
                            onDrag = { change, amount ->
                                change.consume()
                                offset += amount.y
                                travelled += abs(amount.y) + abs(amount.x)
                            },
                            onDragEnd = {
                                val landed = landing()
                                val list = current
                                val begin = list.indexOfFirst { keyOf(it) == id }
                                val from = list.take(begin.coerceAtLeast(0)).count { keyOf(it) !in lifted }
                                val item = list.getOrNull(begin)
                                if (travelled < 12f) {
                                    if (item != null && menuNow(item).isNotEmpty()) menuFor = id
                                } else if (landed != null && item != null && landed != from) {
                                    movedNow(item, landed)
                                }
                                lifted = emptyList()
                                offset = 0f
                            },
                            onDragCancel = {
                                lifted = emptyList()
                                offset = 0f
                            },
                        )
                    },
            ) {
                row(item, pressed == id || isLifted)
                if (menuFor == id) Menu(true, { menuFor = null }, menu(item))
            }
            }
        }
    }
}
