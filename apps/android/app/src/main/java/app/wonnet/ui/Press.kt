package app.wonnet.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.offset
import app.wonnet.theme.Dims
import app.wonnet.theme.Opacity
import app.wonnet.theme.ThemeColor
import app.wonnet.theme.Tokens
import app.wonnet.theme.opacity

/** Whether the view is on the card that fills the page, where a chip fills with its accents. */
val LocalOnCard = compositionLocalOf { false }

/** Whether the view is on a popup or a banner, whose rows and buttons hover in their fills. */
val LocalOnPopover = compositionLocalOf { false }

/** Whether the view is on a card of `card`: a settings group, a form's card. */
val LocalOnFormCard = compositionLocalOf { false }

/** What a small control hovers in, so that it shows on what it sits on: the page, the card that fills it, a settings card or a popover. */
val hoverFill: ThemeColor
    @Composable @ReadOnlyComposable get() = when {
        LocalOnPopover.current -> Tokens.popoverAccentStronger
        LocalOnCard.current -> Dims.cardAccentStronger
        LocalOnFormCard.current -> Tokens.cardAccent
        else -> Tokens.backgroundAccent
    }

/**
 * How every button answers a finger, a pointer and being disabled, so none does it a way of its
 * own: the face gets `hovered`, lit at once with no animation, and pressed is hovered, so a
 * touch shows what a pointer would. Disabled, it shows at `Opacity.disabled`.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Pressable(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    help: String? = null,
    onLongClick: (() -> Unit)? = null,
    role: Role = Role.Button,
    contentAlignment: Alignment = Alignment.TopStart,
    content: @Composable BoxScope.(hovered: Boolean) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val pointer by interaction.collectIsHoveredAsState()
    val described = if (help != null) Modifier.semantics { contentDescription = help } else Modifier
    Box(
        modifier
            .then(described)
            .opacity(Opacity.disabled, !enabled)
            .hoverable(interaction, enabled)
            .combinedClickable(
                interactionSource = interaction, indication = null, enabled = enabled, role = role, onLongClick = onLongClick,
                onClick = onClick,
            ),
        contentAlignment = contentAlignment,
    ) {
        content((pressed || pointer) && enabled)
    }
}

/** Lays a control out as if `by` at each side weren't there: its content stays where it would be, its face and hit area reach past. */
fun Modifier.pullOut(by: Dp): Modifier = layout { measurable, constraints ->
    val side = by.roundToPx()
    val placeable = measurable.measure(constraints.offset(horizontal = side * 2))
    layout((placeable.width - side * 2).coerceAtLeast(0), placeable.height) { placeable.place(-side, 0) }
}
