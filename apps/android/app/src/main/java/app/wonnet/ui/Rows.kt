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
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.wonnet.core.Address
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.Radius
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow

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
 * A row that goes somewhere: a mailbox in the sidebar, Settings. `selected` is where the app is;
 * `count` is its unread, shown when above zero. Half of `Dims.rowGap` above and below keeps its
 * face off the next row's while the hit areas touch. Without `onClick` it is the face alone, for
 * a list that takes the taps itself and says when it is pressed.
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
                .padding(start = 18.dp + indent, end = Space.l),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.m),
        ) {
            Box(Modifier.width(18.dp), contentAlignment = Alignment.Center) {
                when {
                    dot != null -> Box(Modifier.size(8.dp).background(dot, CircleShape))
                    symbol != null -> Icon(symbol, 15f, Tokens.foreground.color)
                }
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
 * `symbol`, an account's `dot` or a person's `avatar`; `checked` is the one in force, with a
 * check at its end. `highlighted` is the one the arrow keys are on.
 */
@Composable
fun ChoiceRow(
    title: String,
    modifier: Modifier = Modifier,
    detail: String? = null,
    symbol: Symbol? = null,
    dot: Color? = null,
    avatar: Address? = null,
    checked: Boolean = false,
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
            if (checked) Icon(Symbol.Check, 14f, Tokens.foreground.color)
        }
    }
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
