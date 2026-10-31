package app.wonnet.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.wonnet.core.AddressBook
import app.wonnet.core.SenderPhotos
import app.wonnet.theme.ControlSize
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
import app.wonnet.theme.ThemeColor
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow

/** Someone's contact photo, else their Gravatar or their company's logo, else their initials in a circle of their address's colour. */
@Composable
fun Avatar(initials: String, email: String, size: Float, modifier: Modifier = Modifier) {
    val side = (size * SCALE).dp
    val context = LocalContext.current
    val key = email.lowercase()
    val photo = AddressBook.photos[key] ?: AddressBook.photoFor(context, key) ?: SenderPhotos.photos[key]
    LaunchedEffect(key) { SenderPhotos.load(key) }
    if (photo != null) {
        Image(photo, null, modifier.size(side).clip(CircleShape), contentScale = ContentScale.Crop)
        return
    }
    val tint = Dims.accountColors[Dims.colorIndex(email, Dims.accountColors.size)].color
    Box(modifier.size(side).background(tint, CircleShape), contentAlignment = Alignment.Center) {
        androidx.compose.foundation.text.BasicText(
            initials,
            style = androidx.compose.ui.text.TextStyle(color = Tokens.primaryForeground.color, fontSize = (size * 0.4f * SCALE).sp, fontWeight = FontWeight.SemiBold),
        )
    }
}

/** A shape from an SVG path's `d`, drawn in a square `box` wide, scaled to fill the canvas. */
@Composable
private fun VectorShape(d: String, box: Float, color: Color, modifier: Modifier) {
    val path = remember(d) { PathParser().parsePathString(d).toPath() }
    Canvas(modifier) {
        val scale = minOf(size.width, size.height) / box
        scale(scale, scale, pivot = androidx.compose.ui.geometry.Offset.Zero) { drawPath(path, color) }
    }
}

private const val W = "M17.47 14.216l2.71-10.727H24l-4.55 16.715h-3.843l-3.56-11.08-3.583 11.08H4.55L0 3.49h3.96l2.76 10.703L10.184 3.49h3.819l3.466 10.727z"

/** The app's icon drawn small: its W on a tile, dark on light and light on dark. */
@Composable
fun AppMark(size: Float = 64f) {
    val side = (size * SCALE).dp
    val shape = RoundedCornerShape(side * 0.225f)
    Box(
        Modifier.size(side).shadow(ShadowSize.SM, shape).background(Tokens.card.color, shape).border(Dims.hairline, Tokens.border.color, shape),
        contentAlignment = Alignment.Center,
    ) {
        VectorShape(W, 24f, Tokens.emphasizedForeground.color, Modifier.size(side * 0.46f).padding(top = side * 0.03f))
    }
}

private val googleParts = listOf(
    "M45.12 24.5c0-1.56-.14-3.06-.4-4.5H24v8.51h11.84c-.51 2.75-2.06 5.08-4.39 6.64v5.52h7.11c4.16-3.83 6.56-9.47 6.56-16.17z" to Tokens.googleBlue,
    "M24 46c5.94 0 10.92-1.97 14.56-5.33l-7.11-5.52c-1.97 1.32-4.49 2.1-7.45 2.1-5.73 0-10.58-3.87-12.31-9.07H4.34v5.7C7.96 41.07 15.4 46 24 46z" to Tokens.googleGreen,
    "M11.69 28.18C11.25 26.86 11 25.45 11 24s.25-2.86.69-4.18v-5.7H4.34C2.85 17.09 2 20.45 2 24c0 3.55.85 6.91 2.34 9.88l7.35-5.7z" to Tokens.googleYellow,
    "M24 10.75c3.23 0 6.13 1.11 8.41 3.29l6.31-6.31C34.91 4.18 29.93 2 24 2 15.4 2 7.96 6.93 4.34 13.12l7.35 5.7c1.73-5.2 6.58-9.07 12.31-9.07z" to Tokens.googleRed,
)

/** Google's G, in its four colours, as its sign-in buttons carry it. */
@Composable
fun GoogleMark(size: Float = 20f) {
    val side = (size * SCALE).dp
    Box(Modifier.size(side)) {
        for ((d, color) in googleParts) VectorShape(d, 48f, color.color, Modifier.size(side))
    }
}

private val microsoftParts = listOf(
    "M1 1H11V11H1Z" to Tokens.microsoftRed,
    "M12 1H22V11H12Z" to Tokens.microsoftGreen,
    "M1 12H11V22H1Z" to Tokens.microsoftBlue,
    "M12 12H22V22H12Z" to Tokens.microsoftYellow,
)

/** Microsoft's four squares, as its sign-in buttons carry them. */
@Composable
fun MicrosoftMark(size: Float = 20f) {
    val side = (size * SCALE).dp
    Box(Modifier.size(side)) {
        for ((d, color) in microsoftParts) VectorShape(d, 23f, color.color, Modifier.size(side))
    }
}

/** A small pill: an address in a field, a file, a thread's label. `invalid` is an address that can't be sent to. */
@Composable
fun Chip(
    title: String,
    modifier: Modifier = Modifier,
    symbol: Symbol? = null,
    detail: String? = null,
    invalid: Boolean = false,
    pending: Boolean = false,
    remove: (() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    val onCard = LocalOnCard.current
    @Composable
    fun Face(hovered: Boolean) {
        val fill = when {
            onCard -> if (hovered) Dims.cardAccentStronger else Dims.cardAccent
            else -> if (hovered) Tokens.backgroundSecondaryAccent else Tokens.backgroundSecondary
        }
        val foreground = if (invalid) Tokens.destructive.color else Tokens.foreground.color
        Row(
            Modifier
                .height(Dims.chipHeight)
                .background(fill.color, CircleShape)
                .border(Dims.hairline, if (invalid) Tokens.destructive.opacity(Opacity.colorBorder).color else Color.Transparent, CircleShape)
                .padding(start = Space.s + 2.dp, end = if (remove != null) 0.dp else Space.s + 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.xs),
        ) {
            ButtonIcon(symbol, pending, 12f, if (invalid) Tokens.destructive.color else Tokens.mutedForeground.color)
            Text(title, TextStyle.Caption, Modifier.widthIn(max = 260.dp), color = foreground, maxLines = 1)
            detail?.let { Text(it, TextStyle.Footnote, maxLines = 1) }
            if (remove != null) {
                Pressable(remove, help = "Remove $title") { lit ->
                    Box(Modifier.size(Dims.chipHeight).padding(Space.xxs).background(if (lit) (if (onCard) Dims.cardAccentStronger else Tokens.backgroundSecondaryAccentStronger).color else Color.Transparent, CircleShape), contentAlignment = Alignment.Center) {
                        Icon(Symbol.X, 11f, if (lit) Tokens.foreground.color else Tokens.mutedStrongerForeground.color)
                    }
                }
            }
        }
    }
    if (onClick != null && remove == null) return Pressable(onClick, modifier, help = title) { Face(it) }
    Box(modifier) { Face(false) }
}

/**
 * A card floating at the bottom that stays until what it says is done: its icon in a wash of
 * `tint`, a title over one line, the one thing to do about it, and `close` to put it away. With
 * `progress`, the line is a bar in `tint` beside `detail`. Every banner is the same height.
 */
@Composable
fun Banner(
    symbol: Symbol,
    title: String,
    detail: String,
    modifier: Modifier = Modifier,
    tint: ThemeColor = Tokens.mutedForeground,
    progress: Double? = null,
    pending: Boolean = false,
    action: (@Composable () -> Unit)? = null,
    close: (() -> Unit)? = null,
) {
    val shape = RoundedCornerShape(Radius.lg)
    Row(
        modifier
            .widthIn(max = (400 * SCALE).dp)
            .fillMaxWidth()
            .shadow(ShadowSize.LG, shape)
            .background(Tokens.popover.color, shape)
            .border(Dims.hairline, Tokens.borderPopover.color, shape)
            .padding(start = Space.m, end = if (close == null) Space.m - Dims.buttonGap / 2 else Space.xxs, top = Space.m, bottom = Space.m),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Space.m),
    ) {
        androidx.compose.runtime.CompositionLocalProvider(LocalOnPopover provides true) {
            Box(Modifier.size((34 * SCALE).dp).background(tint.opacity(Opacity.colorTint).color, CircleShape), contentAlignment = Alignment.Center) {
                if (pending) Spinner(16f, tint.color) else Icon(symbol, 16f, tint.color)
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, TextStyle.LabelStrong, color = Tokens.foreground.color, maxLines = 1)
                if (progress == null) {
                    Text(detail, TextStyle.Caption, maxLines = 1)
                } else {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                        Box(Modifier.weight(1f).height(4.dp).background(Tokens.popoverAccent.color, CircleShape)) {
                            Box(Modifier.fillMaxWidth(progress.toFloat().coerceIn(0f, 1f)).height(4.dp).background(tint.color, CircleShape))
                        }
                        Text(detail, TextStyle.Caption, maxLines = 1)
                    }
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                action?.invoke()
                if (close != null) IconButton(Symbol.X, "Close", size = ControlSize.Small, circled = false, quiet = true, onClick = close)
            }
        }
    }
}

/** The account colours on a line, to pick one. The chosen one wears a ring; each presses in a wash of its own colour. */
@Composable
fun SwatchRow(selected: String, pick: (String) -> Unit) {
    Row {
        for ((index, name) in Dims.accountColorNames.withIndex()) {
            val color = Dims.accountColor(name)
            PlainButton({ pick(name) }, help = "Colour ${index + 1}", tint = color) {
                Box(
                    Modifier
                        .border(1.5.dp, if (name == selected) Tokens.foreground.color else Color.Transparent, CircleShape)
                        .padding(3.dp)
                        .size(12.dp)
                        .background(color.color, CircleShape),
                )
            }
        }
    }
}

/** A wide button on a card, for one of a few ways to do something: a mark, what it is, a line about it, and a chevron. */
@Composable
fun OptionButton(title: String, detail: String?, onClick: () -> Unit, mark: @Composable () -> Unit) {
    Pressable(onClick, Modifier.fillMaxWidth()) { hovered ->
        val shape = RoundedCornerShape(Radius.md)
        Row(
            Modifier
                .fillMaxWidth()
                .height(Dims.optionHeight)
                .background((if (hovered) Tokens.cardAccent else Tokens.card).color, shape)
                .border(Dims.hairline, Tokens.border.color, shape)
                .padding(horizontal = Space.l),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.m),
        ) {
            Box(Modifier.size((24 * SCALE).dp), contentAlignment = Alignment.Center) { mark() }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, TextStyle.BodyStrong, maxLines = 1)
                detail?.let { Text(it, TextStyle.Caption, maxLines = 1) }
            }
            Icon(Symbol.ChevronRight, 14f, Tokens.mutedStrongerForeground.color)
        }
    }
}

