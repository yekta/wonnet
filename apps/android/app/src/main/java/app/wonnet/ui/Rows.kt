package app.wonnet.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.AlignmentLine
import androidx.compose.ui.layout.HorizontalAlignmentLine
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.VerticalAlignmentLine
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.max
import kotlin.math.min
import app.wonnet.core.Address
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.Radius
import app.wonnet.theme.SCALE
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow
import app.wonnet.theme.symbolSide

/** A thing in a list with its controls at the end: an account, a snippet, a split. `symbol` or `dot` starts it. */
@Composable
fun ItemRow(
    title: String,
    modifier: Modifier = Modifier,
    detail: String? = null,
    symbol: Symbol? = null,
    dot: Color? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    Row(modifier.fillMaxWidth().heightIn(min = Dims.rowHeight), verticalAlignment = Alignment.CenterVertically) {
        when {
            dot != null -> Box(Modifier.padding(end = Space.s + 2.dp).size(8.dp).background(dot, CircleShape))
            symbol != null -> Icon(symbol, 13f, Tokens.mutedStrongerForeground.color, Modifier.padding(end = Space.s + 2.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, TextStyle.Body, maxLines = 1)
            detail?.let { Text(it, TextStyle.Caption, maxLines = 1) }
        }
        Spacer(Modifier.width(Space.s))
        trailing()
    }
}

/**
 * A row that goes somewhere: a mailbox in the sidebar, Settings, a person's thread. `selected` is
 * where the app is; `count` is its unread, shown when above zero. Half of `Dims.rowGap` above and
 * below keeps its face off the next row's while the hit areas touch. `inset` is the space at both
 * its ends, for words that line up with a page's. Without `onClick` it is the face alone, for a
 * list that takes the taps itself and says when it is pressed.
 */
@Composable
fun NavRow(
    title: String,
    modifier: Modifier = Modifier,
    symbol: Symbol? = null,
    dot: Color? = null,
    count: Int = 0,
    selected: Boolean = false,
    indent: Dp = 0.dp,
    inset: Dp? = null,
    verticalPadding: Dp = 0.dp,
    hovered: Boolean = false,
    onClick: (() -> Unit)? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    @Composable
    fun Face(lit: Boolean) {
        val fill = when {
            selected -> Tokens.backgroundAccentStronger.color
            lit -> Tokens.backgroundAccent.color
            else -> Color.Transparent
        }
        Row(
            Modifier
                .padding(vertical = Dims.rowGap / 2)
                .fillMaxWidth()
                .background(fill)
                .padding(vertical = verticalPadding)
                .height(Dims.rowHeight)
                .padding(start = (inset ?: 18.dp) + indent, end = inset ?: Space.l),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.m),
        ) {
            when {
                dot != null -> Box(Modifier.width(18.dp), contentAlignment = Alignment.Center) { Box(Modifier.size(8.dp).background(dot, CircleShape)) }
                symbol != null -> Box(Modifier.width(18.dp), contentAlignment = Alignment.Center) { Icon(symbol, 15f, Tokens.foreground.color) }
            }
            Text(title, if (selected) TextStyle.LabelStrong else TextStyle.Label, Modifier.weight(1f), color = Tokens.foreground.color, maxLines = 1)
            if (count > 0) Text("$count", TextStyle.Caption)
            trailing()
        }
    }
    if (onClick == null) return Box(modifier) { Face(hovered) }
    Pressable(onClick, modifier) { Face(it) }
}

/**
 * One choice of a short list: a suggestion, a time, a snippet, a sender. It starts with a
 * `symbol`, an account's `dot` or a person's `avatar`. In a list of choices `checked` is given:
 * the check's column is always at the row's end and the check shows in it on the one in force,
 * so nothing moves when it comes or goes. `highlighted` is the one the arrow keys are on.
 */
@Composable
fun ChoiceRow(
    title: String,
    modifier: Modifier = Modifier,
    detail: String? = null,
    symbol: Symbol? = null,
    dot: Color? = null,
    avatar: Address? = null,
    checked: Boolean? = null,
    highlighted: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val popover = LocalOnPopover.current
    Pressable(onClick, modifier, enabled) { hovered ->
        val fill = when {
            highlighted -> (if (popover) Tokens.popoverAccentStronger else Tokens.backgroundAccentLargerStronger).color
            hovered -> (if (popover) Tokens.popoverAccent else Tokens.backgroundAccentLarger).color
            else -> Color.Transparent
        }
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = Dims.rowHeight)
                .background(fill, RoundedCornerShape(Radius.sm))
                .padding(horizontal = Space.s + 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.s + 2.dp),
        ) {
            when {
                avatar != null -> Avatar(avatar.initials, avatar.email, 24f)
                dot != null -> Box(Modifier.width(14.dp), contentAlignment = Alignment.Center) { Box(Modifier.size(8.dp).background(dot, CircleShape)) }
                symbol != null -> Icon(symbol, 14f, Tokens.mutedStrongerForeground.color)
            }
            Text(title, TextStyle.Body, Modifier.weight(1f), maxLines = 1)
            detail?.let { Text(it, TextStyle.Caption, Modifier.padding(start = Space.xs), maxLines = 1) }
            if (checked != null) CheckColumn(checked)
        }
    }
}

/** The column of a choice's check, as wide as the check whether it shows or not. */
@Composable
fun CheckColumn(checked: Boolean) {
    Box(Modifier.size(symbolSide(14f))) { if (checked) Icon(Symbol.Check, 14f, Tokens.foreground.color) }
}

/** Choices under a field, one highlighted and scrolled to as the keys move; `empty` when there are none. */
@Composable
fun <T> ChoiceList(
    items: List<T>,
    highlighted: Int,
    modifier: Modifier = Modifier,
    empty: String? = null,
    key: (T) -> Any,
    contentPadding: PaddingValues = PaddingValues(Space.s),
    row: @Composable (Int, T) -> Unit,
) {
    val list = rememberLazyListState()
    LaunchedEffect(highlighted) {
        if (highlighted in items.indices) list.animateScrollToItem(highlighted)
    }
    LazyColumn(modifier, state = list, contentPadding = contentPadding) {
        itemsIndexed(items, key = { _, item -> key(item) }) { index, item -> row(index, item) }
        if (items.isEmpty() && empty != null) {
            item {
                Text(empty, TextStyle.Label, Modifier.fillMaxWidth().padding(Space.s + 2.dp), color = Tokens.mutedStrongerForeground.color)
            }
        }
    }
}

/** A card floating over the page, for suggestions and short menus. `inset` is the space to its edges. */
@Composable
fun PopupCard(modifier: Modifier = Modifier, inset: Dp = Space.xs, content: @Composable ColumnScope.() -> Unit) {
    val shape = RoundedCornerShape(Radius.md)
    Column(
        modifier
            .shadow(ShadowSize.MD, shape, Opacity.shadowStronger)
            .background(Tokens.popover.color, shape)
            .border(Dims.hairline, Tokens.borderPopover.color, shape)
            .padding(inset),
    ) {
        CompositionLocalProvider(LocalOnPopover provides true) { content() }
    }
}

/**
 * One of a few choices on a `RowsCard`, its mark at the end: a circle for one of them, a square
 * when several can be picked (`multiple`), filled in the primary colour with a check once picked.
 * It starts with a `symbol` or an account's `dot`; `detail` wraps under the title; `indent` sets
 * it under the choice it belongs to.
 */
@Composable
fun PickRow(
    title: String,
    picked: Boolean,
    modifier: Modifier = Modifier,
    detail: String? = null,
    symbol: Symbol? = null,
    dot: Color? = null,
    multiple: Boolean = false,
    indent: Dp = 0.dp,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val state = Modifier.semantics {
        if (multiple) toggleableState = ToggleableState(picked) else selected = picked
    }
    Pressable(onClick, modifier.then(state), enabled, role = if (multiple) Role.Checkbox else Role.RadioButton) { hovered ->
        CardRow(Modifier.background(if (hovered) Tokens.cardAccent.color else Color.Transparent), indent, trailing = { PickMark(picked, multiple) }) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                when {
                    dot != null -> Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Box(Modifier.size(10.dp).background(dot, CircleShape)) }
                    symbol != null -> Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) { Icon(symbol, 17f / 1.15f, Tokens.foreground.color) }
                }
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(title, TextStyle.Body)
                    detail?.let { Text(it, TextStyle.Caption, color = Tokens.mutedForeground.color) }
                }
            }
        }
    }
}

/** A choice's mark: a circle, or a square when several can be picked; filled with a check once picked. */
@Composable
private fun PickMark(picked: Boolean, multiple: Boolean) {
    val side = (Dims.selectCircle * SCALE).dp
    val shape = if (multiple) RoundedCornerShape(Radius.xs) else CircleShape
    Box(
        Modifier
            .size(side)
            .background(if (picked) Tokens.primary.color else Color.Transparent, shape)
            .border(Dims.selectCircleLine, if (picked) Color.Transparent else Tokens.mutedStrongestForeground.color, shape),
        contentAlignment = Alignment.Center,
    ) {
        if (picked) Icon(Symbol.Check, Dims.selectCircle * 0.67f, Tokens.primaryForeground.color)
    }
}

/** Where a control's face ends inside its hit area, for a row to line the face up with its edge. */
val FaceEnd = VerticalAlignmentLine(::max)

/** How far a control's face is from the top of its hit area, the same below. */
val FaceTop = HorizontalAlignmentLine(::min)

/** Tells a row where the face is in the control's hit area: `outset` past it at the end, and `height` tall in its middle. */
fun Modifier.face(outset: Dp = 0.dp, height: Dp? = null): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    val top = height?.let { ((placeable.height - it.roundToPx()) / 2).coerceAtLeast(0) } ?: 0
    layout(placeable.width, placeable.height, mapOf(FaceEnd to placeable.width - outset.roundToPx(), FaceTop to top)) { placeable.place(0, 0) }
}

/**
 * A row of a card (`RowsCard`): its content `Space.l` from the start (and `indent` more) and at
 * least `Space.m` from the top and bottom, as tall as a control at least. `trailing`'s face is
 * centred: in a row of several lines `Space.l` from the end, as the content is from the start;
 * in a row of one as far from the end as from the top and bottom.
 */
@Composable
fun CardRow(modifier: Modifier = Modifier, indent: Dp = 0.dp, trailing: (@Composable () -> Unit)? = null, content: @Composable () -> Unit) {
    Layout(listOf(content, trailing ?: {}), modifier.fillMaxWidth()) { (contents, controls), constraints ->
        val width = constraints.maxWidth
        val side = Space.l.roundToPx()
        val start = side + indent.roundToPx()
        val gap = Space.m.roundToPx()
        val control = controls.firstOrNull()?.measure(Constraints(maxWidth = width / 2))
        val faceEnd = control?.let { it[FaceEnd].takeIf { line -> line != AlignmentLine.Unspecified } ?: it.width } ?: 0
        val faceTop = control?.let { it[FaceTop].takeIf { line -> line != AlignmentLine.Unspecified } ?: 0 } ?: 0
        val faceHeight = (control?.height ?: 0) - faceTop * 2
        val room = if (control == null) side else side + faceEnd + gap
        val text = contents.firstOrNull()?.measure(Constraints(maxWidth = (width - start - room).coerceAtLeast(0)))
        val oneLine = (Dims.rowHeight.value + 8).dp.roundToPx()
        val height = maxOf(oneLine, (text?.height ?: 0) + Space.m.roundToPx() * 2, control?.height ?: 0, constraints.minHeight)
        val inset = if (height > oneLine) side else minOf(side, (height - faceHeight) / 2)
        layout(width, height) {
            text?.place(start, (height - text.height) / 2)
            control?.place(width - inset - faceEnd, (height - control.height) / 2)
        }
    }
}

/** Lays a view out as if it took no height, centred where it stands: a badge beside a line of text, which keeps the line's height. */
fun Modifier.takesNoHeight(): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
    layout(placeable.width, 0) { placeable.place(0, -placeable.height / 2) }
}
