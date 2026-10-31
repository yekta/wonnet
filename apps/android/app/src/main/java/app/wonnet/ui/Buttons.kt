package app.wonnet.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
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

/** The icon of a button, or the spinner in its place while the button is pending. */
@Composable
fun ButtonIcon(symbol: Symbol?, pending: Boolean = false, size: Float = 15f, tint: Color) {
    if (pending) return Spinner(size, tint)
    symbol?.let { Icon(it, size, tint) }
}

/**
 * `Primary` is filled, `Success` filled in green for what finishes a thing done, `Outline` is
 * Newton's pill, `Ghost` is words alone, `Destructive` a pill in red.
 */
enum class ButtonVariant { Primary, Success, Outline, Ghost, Destructive }

/**
 * A button with words. `pending` shows the spinner where the icon goes and ignores taps until it
 * is over. `iconOnly` is the same button shrunk to its icon in a circle. Half of `Dims.buttonGap`
 * on each side keeps it off its neighbours while the hit areas touch.
 */
@Composable
fun ActionButton(
    title: String,
    modifier: Modifier = Modifier,
    symbol: Symbol? = null,
    variant: ButtonVariant = ButtonVariant.Outline,
    size: ControlSize = ControlSize.Regular,
    pending: Boolean = false,
    wide: Boolean = false,
    iconOnly: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Pressable({ if (!pending) onClick() }, modifier, enabled, help = if (iconOnly) title else null) { hovered ->
        ButtonFace(title, symbol, variant, size, pending, wide, iconOnly, hovered = hovered)
    }
}

@Composable
fun ButtonFace(
    title: String,
    symbol: Symbol?,
    variant: ButtonVariant,
    size: ControlSize,
    pending: Boolean,
    wide: Boolean,
    iconOnly: Boolean = false,
    chevron: Boolean = false,
    hovered: Boolean,
) {
    val foreground = when (variant) {
        ButtonVariant.Primary -> Tokens.primaryForeground.color
        ButtonVariant.Success -> Tokens.successForeground.color
        ButtonVariant.Outline -> Tokens.primary.color
        ButtonVariant.Ghost -> if (hovered) Tokens.foreground.color else Tokens.mutedForeground.color
        ButtonVariant.Destructive -> Tokens.destructive.color
    }
    val background = when (variant) {
        ButtonVariant.Primary -> (if (hovered) Tokens.primary.opacity(Opacity.lit) else Tokens.primary).color
        ButtonVariant.Success -> (if (hovered) Tokens.success.opacity(Opacity.lit) else Tokens.success).color
        else -> if (hovered) Tokens.backgroundAccent.color else Color.Transparent
    }
    val border = when (variant) {
        ButtonVariant.Outline -> Tokens.primary.opacity(Opacity.colorBorder).color
        ButtonVariant.Destructive -> Tokens.destructive.opacity(Opacity.colorBorder).color
        else -> Color.Transparent
    }
    val height = size.scaled
    Row(
        Modifier
            .padding(horizontal = Dims.buttonGap / 2)
            .background(background, CircleShape)
            .border(Dims.hairline, border, CircleShape)
            .height(height)
            .then(if (iconOnly) Modifier.width(height) else Modifier.padding(horizontal = height / 2))
            .let { if (wide && !iconOnly) it.fillMaxWidth() else it },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Space.xs, Alignment.CenterHorizontally),
    ) {
        if (iconOnly) return@Row ButtonIcon(symbol, pending, size.icon - 1, foreground)
        ButtonIcon(symbol, pending, size.icon - 1, foreground)
        Text(title, size.text, color = foreground, maxLines = 1)
        if (chevron) Icon(Symbol.ChevronDown, size.icon - 1, foreground)
    }
}

/**
 * An icon in a thin circle, as Newton's toolbars drew them. `active` is lit in the primary colour;
 * `quiet` is fainter; `strong` in the foreground's colour, for an icon on a bar.
 */
@Composable
fun IconButton(
    symbol: Symbol,
    help: String,
    modifier: Modifier = Modifier,
    size: ControlSize = ControlSize.Regular,
    active: Boolean = false,
    circled: Boolean = true,
    quiet: Boolean = false,
    strong: Boolean = false,
    pending: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Pressable({ if (!pending) onClick() }, modifier, enabled, help = help) { hovered ->
        IconCircle(symbol, size, active, circled, quiet, strong, pending, hovered)
    }
}

/** The face of an icon button and an icon menu, with half of `Dims.buttonGap` on each side. */
@Composable
fun IconCircle(
    symbol: Symbol,
    size: ControlSize = ControlSize.Regular,
    active: Boolean = false,
    circled: Boolean = true,
    quiet: Boolean = false,
    strong: Boolean = false,
    pending: Boolean = false,
    hovered: Boolean = false,
) {
    val tint = when {
        active -> Tokens.primary.color
        quiet -> if (hovered) Tokens.mutedForeground.color else Tokens.mutedStrongerForeground.color
        strong -> Tokens.foreground.color
        hovered -> Tokens.foreground.color
        else -> Tokens.mutedForeground.color
    }
    val fill = if (!hovered) Color.Transparent else if (LocalOnPopover.current) Tokens.popoverAccentStronger.color else Tokens.backgroundAccent.color
    val ring = if (!circled) Color.Transparent else if (LocalOnCard.current) Dims.cardRule.color else Tokens.border.color
    Box(
        Modifier.padding(horizontal = Dims.buttonGap / 2).size(size.scaled).background(fill, CircleShape).border(Dims.hairline, ring, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        ButtonIcon(symbol, pending, size.icon, tint)
    }
}

/** One thing a menu offers; `divider` draws a hairline over it. */
data class MenuItem(
    val title: String,
    val symbol: Symbol? = null,
    val destructive: Boolean = false,
    val divider: Boolean = false,
    val checked: Boolean = false,
    val action: () -> Unit,
)

/** A short menu on the popover's card, opened under what opens it. */
@Composable
fun Menu(expanded: Boolean, onDismiss: () -> Unit, items: List<MenuItem>) {
    DropdownMenu(
        expanded, onDismiss, shape = RoundedCornerShape(Radius.md), containerColor = Tokens.popover.color,
        border = BorderStroke(Dims.hairline, Tokens.borderPopover.color), shadowElevation = 6.dp, tonalElevation = 0.dp,
    ) {
        CompositionLocalProvider(LocalOnPopover provides true) {
            Column(Modifier.widthIn(min = 200.dp).padding(horizontal = Space.xxs)) {
                for (item in items) {
                    if (item.divider) Rule(Modifier.padding(vertical = Space.xxs), color = Tokens.borderPopover)
                    Pressable({
                        onDismiss()
                        item.action()
                    }) { hovered ->
                        val color = if (item.destructive) Tokens.destructive.color else Tokens.foreground.color
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = Dims.rowHeight)
                                .background(if (hovered) Tokens.popoverAccent.color else Color.Transparent, RoundedCornerShape(Radius.sm))
                                .padding(horizontal = Space.s + 2.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(Space.s + 2.dp),
                        ) {
                            item.symbol?.let { Icon(it, 15f, color) }
                            Text(item.title, TextStyle.Body, Modifier.weight(1f), color = color, maxLines = 1)
                            if (item.checked) Icon(Symbol.Check, 14f, Tokens.foreground.color)
                        }
                    }
                }
            }
        }
    }
}

/** An icon in a thin circle, like `IconButton`, that opens a menu. */
@Composable
fun IconMenu(
    symbol: Symbol,
    help: String,
    items: List<MenuItem>,
    modifier: Modifier = Modifier,
    size: ControlSize = ControlSize.Regular,
    circled: Boolean = true,
    strong: Boolean = false,
) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        IconButton(symbol, help, size = size, circled = circled, strong = strong) { open = true }
        Menu(open, { open = false }, items)
    }
}

/** One of a few choices, as a button with a chevron that opens them: the theme, the undo delay. */
@Composable
fun <T> ChoiceMenu(options: List<T>, selected: T, title: (T) -> String, size: ControlSize = ControlSize.Regular, pick: (T) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        Pressable({ open = true }) { hovered ->
            ButtonFace(title(selected), null, ButtonVariant.Outline, size, pending = false, wide = false, chevron = true, hovered = hovered || open)
        }
        Menu(open, { open = false }, options.map { option -> MenuItem(title(option), checked = option == selected) { pick(option) } })
    }
}

/**
 * A button that is only its content: a star, a sender's name, a word in a toast. It still hovers,
 * presses and disables as the others do, and is never under the smallest control in height.
 * `tint` is the colour it hovers in, as a wash of it.
 */
@Composable
fun PlainButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    tint: ThemeColor? = null,
    enabled: Boolean = true,
    content: @Composable RowScope.(hovered: Boolean) -> Unit,
) {
    Pressable(onClick, modifier, enabled, help = help) { hovered ->
        val fill = if (!hovered) Color.Transparent else tint?.opacity(Opacity.colorTint)?.color ?: Tokens.backgroundAccent.color
        Row(
            Modifier
                .defaultMinSize(ControlSize.Small.scaled, ControlSize.Small.scaled)
                .background(fill, RoundedCornerShape(Radius.sm))
                .padding(horizontal = Space.xxs),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center,
        ) { content(hovered) }
    }
}

/** A thread's star: lit in the star colour when starred, and when pressed, over a wash of it. */
@Composable
fun StarButton(starred: Boolean, size: Float = 20f, onClick: () -> Unit) {
    PlainButton(onClick, help = if (starred) "Unstar" else "Star", tint = Tokens.star) { hovered ->
        Icon(if (starred) Symbol.StarFilled else Symbol.Star, size, if (starred || hovered) Tokens.star.color else Tokens.mutedStrongerForeground.color)
    }
}

/**
 * An icon on a circle over a page: Compose over the list, and smaller, the way back to the top of
 * a list. The page's colour with its border and a little shadow, so it stands off the list.
 */
@Composable
fun FloatingButton(symbol: Symbol, help: String, modifier: Modifier = Modifier, small: Boolean = false, onClick: () -> Unit) {
    Pressable(onClick, modifier, help = help) { hovered ->
        val side = if (small) Dims.floatingButtonSmall else Dims.floatingButton
        Box(
            Modifier
                .size(side)
                .shadow(ShadowSize.SM)
                .background((if (hovered) Tokens.backgroundAccent else Tokens.background).color, CircleShape)
                .border(Dims.hairline, Tokens.border.color, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(symbol, if (small) ControlSize.Large.icon else 22f, Tokens.foreground.color)
        }
    }
}

/** The circle that selects threads: empty, or filled in the primary colour with a check. */
@Composable
fun SelectCircle(checked: Boolean, help: String, onClick: () -> Unit) {
    Pressable(onClick, help = help) { hovered -> SelectCircleFace(checked, hovered) }
}

@Composable
fun SelectCircleFace(checked: Boolean, hovered: Boolean = false) {
    val side = (Dims.selectCircle * SCALE).dp
    Box(Modifier.padding(horizontal = Dims.buttonGap / 2).size(Dims.rowHeight), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .size(side)
                .background(if (checked) Tokens.primary.color else Color.Transparent, CircleShape)
                .border(Dims.selectCircleLine, if (checked) Color.Transparent else (if (hovered) Tokens.mutedForeground else Tokens.border).color, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (checked) Icon(Symbol.Check, Dims.selectCircle * 0.67f, Tokens.primaryForeground.color)
        }
    }
}

/**
 * A bar floating over a page: icon buttons spread evenly across a capsule, as the commands under
 * an open thread. The page's colour on a bordered capsule with a little shadow.
 */
@Composable
fun GlassBar(modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    Row(
        modifier
            .height(Dims.barHeight)
            .shadow(ShadowSize.SM, CircleShape)
            .background(Tokens.background.color, CircleShape)
            .border(Dims.hairline, Tokens.border.color, CircleShape)
            .padding(horizontal = Space.xxs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceEvenly,
        content = content,
    )
}

/** One action on a circle of the same face, beside a `GlassBar`: Reply beside a thread's commands. */
@Composable
fun GlassCircle(symbol: Symbol, help: String, onClick: () -> Unit) {
    Pressable(onClick, help = help) { hovered ->
        Box(
            Modifier
                .size(Dims.barHeight)
                .shadow(ShadowSize.SM)
                .background((if (hovered) Tokens.backgroundAccent else Tokens.background).color, CircleShape)
                .border(Dims.hairline, Tokens.border.color, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(symbol, ControlSize.Large.icon, Tokens.foreground.color)
        }
    }
}
