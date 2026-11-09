package app.wonnet.ui

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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.wonnet.theme.Icon
import app.wonnet.theme.SCALE
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.theme.symbolSide
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * A group of rows, as the settings and the sheets like them have them: its name over it, a card
 * of rows, and the muted line under it saying what it does (and a `warning` after it, in its
 * colour). The rows' fills reach the card's edges and take its corners. `card` false puts the
 * content on the page instead, as wide as the cards: a control that stands alone, such as a field.
 */
@Suppress("ModifierParameter")
@Composable
fun SettingsGroup(
    title: String? = null,
    footer: String? = null,
    modifier: Modifier = Modifier,
    card: Boolean = true,
    warning: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(Space.s)) {
        title?.let { Text(it, TextStyle.Subheading, Modifier.padding(horizontal = Space.l)) }
        if (card) RowsCard(content = content) else Column(Modifier.fillMaxWidth(), content = content)
        if (footer == null && warning == null) return@Column
        Column(Modifier.padding(horizontal = Space.l), verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
            footer?.let { Text(it, TextStyle.Caption, color = Tokens.mutedForeground.color) }
            warning?.let { Text(it, TextStyle.Caption, color = Tokens.warning.color) }
        }
    }
}

/** The hairline between two rows of a group, in as far as the rows' words (and `indent` more). */
@Composable
fun SettingsRule(indent: Dp = 0.dp) = RowsCardRule(Space.l + indent)

/** How wide a row's check is; its column is kept for it whether the row is checked or not. */
private const val CHECK = 17f / SCALE

/**
 * A row of a group: its picture, what it is with a line under (and more `lines`), and its value,
 * control or chevron at the end. `badge` stands after the title; `wrap` lets every line wrap;
 * `indent` sets it under the row it belongs to; `below` stands under the lines, as wide as they
 * are: the buttons of a long row. `checked` non-null makes it one of a list's choices: the
 * check's column is always at its end, and the check, in the primary colour, shows in it once
 * the row is the one in force, so nothing on the row moves when it comes or goes.
 */
@Composable
fun SettingsRow(
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
    detailColor: Color? = null,
    checked: Boolean? = null,
    indent: Dp = 0.dp,
    badge: (@Composable () -> Unit)? = null,
    below: (@Composable () -> Unit)? = null,
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
        CardRow(Modifier.background(fill), indent, trailing = control) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                leading?.invoke()
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    val most = if (wrap) Int.MAX_VALUE else 1
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                        Text(title, TextStyle.Body, Modifier.weight(1f, fill = false), color = color, maxLines = most)
                        badge?.let { Box(Modifier.takesNoHeight()) { it() } }
                    }
                    detail?.let { Text(it, TextStyle.Caption, color = detailColor ?: Tokens.mutedForeground.color, maxLines = most) }
                    for (line in lines) Text(line, TextStyle.Caption, color = Tokens.mutedForeground.color, maxLines = most)
                    below?.let { Box(Modifier.padding(top = Space.xs)) { it() } }
                }
                value?.let { Text(it, TextStyle.Body, Modifier.widthIn(max = 220.dp), color = Tokens.mutedForeground.color, maxLines = 1, tabular = true) }
                if (chevron) Icon(Symbol.ChevronRight, 14f, Tokens.mutedStrongerForeground.color)
                if (checked != null) {
                    Box(Modifier.size(symbolSide(CHECK))) { if (checked) Icon(Symbol.Check, CHECK, Tokens.primary.color) }
                }
            }
        }
    }
    if (onClick == null) return Face(hovered)
    val state = if (checked != null) Modifier.semantics { this.selected = checked } else Modifier
    Pressable(onClick, state) { Face(it || hovered) }
}

/** A row's picture: its symbol, in a square as wide as a dot's beside it. */
@Composable
fun SettingsTile(symbol: Symbol, color: Color = Tokens.foreground.color) {
    Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Icon(symbol, 17f / SCALE, color) }
}

/** A row's account dot, in a square as wide as a tile. */
@Composable
fun SettingsDot(color: Color) {
    Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Box(Modifier.size(10.dp).background(color, CircleShape)) }
}

/** A switch row of a group: the switch at its end, flipped by a tap anywhere on the row. */
@Composable
fun SettingsToggleRow(title: String, on: Boolean, detail: String? = null, leading: (@Composable () -> Unit)? = null, onChange: (Boolean) -> Unit) {
    Pressable({ onChange(!on) }, role = Role.Switch) { lit ->
        SettingsRow(title, detail = detail, leading = leading, wrap = true, hovered = lit, trailing = { Toggle(on, onChange) })
    }
}

/** A row a swipe to the left removes: it shows the action in red behind it, and past half the row it is done. */
@Composable
fun SwipeToRemove(id: String, title: String, remove: () -> Unit, content: @Composable () -> Unit) {
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
