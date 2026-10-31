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
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import app.wonnet.theme.Opacity
import app.wonnet.theme.opacity

/** Whether the view is on the card that fills the page, where a chip fills with its accents. */
val LocalOnCard = compositionLocalOf { false }

/** Whether the view is on a popup or a banner, whose rows and buttons hover in their fills. */
val LocalOnPopover = compositionLocalOf { false }

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
