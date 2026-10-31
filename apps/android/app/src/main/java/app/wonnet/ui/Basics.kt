package app.wonnet.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Radius
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.ThemeColor
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow

/**
 * The spinner that shows something is still coming: Lucide's loader turning once a second, in
 * the muted text colour unless given a `tint`.
 */
@Composable
fun Spinner(size: Float = 13f, tint: Color = Tokens.mutedForeground.color) {
    val turning = rememberInfiniteTransition(label = "spinner")
    val angle by turning.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Restart), label = "angle")
    Icon(Symbol.Loader, size, tint, Modifier.rotate(angle))
}

/** A hairline across or down: `border` on the page, `borderCard` on a card. */
@Composable
fun Rule(modifier: Modifier = Modifier, vertical: Boolean = false, color: ThemeColor = Tokens.border) {
    val shape = if (vertical) Modifier.fillMaxHeight().width(Dims.hairline) else Modifier.fillMaxWidth().height(Dims.hairline)
    Box(modifier.then(shape).background(color.color))
}

enum class Edge { Top, Bottom }

/** A hairline along one edge, drawn over the view so it takes no room. */
@Composable
fun Modifier.rule(edge: Edge, color: ThemeColor = Tokens.border): Modifier {
    val drawn = color.color
    return drawBehind {
        val y = if (edge == Edge.Top) 0f else size.height - Dims.hairline.toPx()
        drawRect(drawn, Offset(0f, y), androidx.compose.ui.geometry.Size(size.width, Dims.hairline.toPx()))
    }
}

/** A section's name, and maybe a muted line about it. */
@Composable
fun SectionHeading(title: String, modifier: Modifier = Modifier, detail: String? = null) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
        Text(title, TextStyle.Subheading)
        detail?.let { Text(it, TextStyle.Caption, color = Tokens.mutedForeground.color) }
    }
}

/** A part of a page: its heading, then its controls. */
@Composable
fun FormSection(title: String, detail: String? = null, content: @Composable ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(Space.m)) {
        SectionHeading(title, detail = detail)
        content()
    }
}

/** A rounded card on the page: a form being filled, a thing of its own. */
@Composable
fun Card(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val shape = RoundedCornerShape(Radius.md)
    Column(modifier.background(Tokens.card.color, shape).border(Dims.hairline, Tokens.border.color, shape).padding(Space.l), content = content)
}

/** The card that fills the page (`Dims.cardFill`), with what is on it told so. */
@Composable
fun PageCard(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val shape = RoundedCornerShape(Dims.cardRadius)
    Column(
        modifier.shadow(ShadowSize.LG, shape).background(Dims.cardFill.color, shape).border(Dims.hairline, Tokens.border.color, shape),
    ) {
        CompositionLocalProvider(LocalOnCard provides true) { content() }
    }
}

/** A line about something: a hint, an error, a note that something was left out; with `action`, the one thing to do about it. */
@Composable
fun Notice(text: String, modifier: Modifier = Modifier, error: Boolean = false, symbol: Symbol? = null, action: String? = null, perform: () -> Unit = {}) {
    val color = if (error) Tokens.destructive.color else Tokens.mutedStrongerForeground.color
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
        symbol?.let { Icon(it, 13f, color) }
        Text(text, TextStyle.Caption, Modifier.weight(1f, fill = action != null), color = color)
        action?.let { ActionButton(it, variant = ButtonVariant.Ghost, onClick = perform) }
    }
}

/** A list with nothing in it: an icon and a calm line. */
@Composable
fun EmptyState(symbol: Symbol, message: String, modifier: Modifier = Modifier) {
    Column(
        modifier.fillMaxSize().background(Tokens.background.color),
        verticalArrangement = Arrangement.spacedBy(Space.m, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(symbol, 30f, Tokens.mutedStrongestForeground.color)
        Text(message, TextStyle.Body, color = Tokens.mutedStrongerForeground.color)
    }
}
