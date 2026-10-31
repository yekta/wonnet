package app.wonnet.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import app.wonnet.theme.Dims
import app.wonnet.theme.LocalDark
import app.wonnet.theme.Radius
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.ThemeColor
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow

/** What floats over every sheet as over the screen: the toast. */
val LocalSheetOverlay = compositionLocalOf<@Composable BoxScope.() -> Unit> { {} }

/**
 * What every sheet is made of: a bar with its title and a way to close it, and its content, over
 * the whole screen. The close is an X at the bar's start, or a word such as Cancel; `trailing`
 * is what the sheet is for, at the bar's end. The back gesture closes it too.
 */
@Composable
fun Sheet(
    title: String,
    onClose: () -> Unit,
    close: String = "Close",
    background: ThemeColor = Tokens.background,
    trailing: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Dialog(onClose, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val dark = LocalDark.current
        val view = LocalView.current
        SideEffect {
            val window = (view.parent as? DialogWindowProvider)?.window ?: return@SideEffect
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
        val shown = remember { MutableTransitionState(false).apply { targetState = true } }
        val overlay = LocalSheetOverlay.current
        Box(Modifier.fillMaxSize()) {
        AnimatedVisibility(shown, enter = slideInVertically(tween(220)) { it / 8 } + fadeIn(tween(160))) {
            Column(
                Modifier
                    .fillMaxSize()
                    .background(background.color)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal)),
            ) {
                HeaderBar(
                    leading = {
                        if (close == "Close") {
                            IconButton(Symbol.X, "Close", circled = false, strong = true, onClick = onClose)
                        } else {
                            ActionButton(close, variant = ButtonVariant.Ghost, onClick = onClose)
                        }
                    },
                    center = { Text(title, TextStyle.Subheading, maxLines = 1) },
                    trailing = trailing,
                )
                Rule(color = if (background == Tokens.background) Tokens.border else Tokens.borderCard)
                Column(Modifier.fillMaxWidth().weight(1f).windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom))) { content() }
            }
        }
        overlay()
        }
    }
}

/**
 * A bar across the top of a page or a sheet: what leads, what is in the middle of the whole bar,
 * and what trails. `inset` is the space between its edges and its first and last control.
 */
@Composable
fun HeaderBar(
    modifier: Modifier = Modifier,
    inset: Dp = Space.s,
    leading: @Composable RowScope.() -> Unit = {},
    center: @Composable () -> Unit = {},
    trailing: @Composable RowScope.() -> Unit = {},
) {
    Box(modifier.fillMaxWidth().height(Dims.barHeight).padding(horizontal = inset), contentAlignment = Alignment.Center) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, content = leading)
            Box(Modifier.widthIn(max = 420.dp)) { center() }
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.End, content = trailing)
        }
    }
}

/** A question before something that can't be taken back: its title, what it does, Cancel and the action, in red when it destroys. */
@Composable
fun ConfirmDialog(
    title: String,
    message: String,
    action: String,
    onCancel: () -> Unit,
    destructive: Boolean = true,
    cancel: String = "Cancel",
    onConfirm: () -> Unit,
) {
    Dialog(onCancel) {
        val shape = RoundedCornerShape(Radius.lg)
        Column(
            Modifier
                .widthIn(max = 360.dp)
                .shadow(ShadowSize.XL, shape)
                .background(Tokens.popover.color, shape)
                .border(Dims.hairline, Tokens.borderPopover.color, shape)
                .padding(Space.l),
            verticalArrangement = Arrangement.spacedBy(Space.s),
        ) {
            Text(title, TextStyle.Heading)
            Text(message, TextStyle.Body, color = Tokens.mutedForeground.color)
            Spacer(Modifier.height(Space.s))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                ActionButton(cancel, variant = ButtonVariant.Ghost, onClick = onCancel)
                ActionButton(action, variant = if (destructive) ButtonVariant.Destructive else ButtonVariant.Primary, onClick = onConfirm)
            }
        }
    }
}

