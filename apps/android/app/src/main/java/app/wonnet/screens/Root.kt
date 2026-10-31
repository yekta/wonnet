package app.wonnet.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import app.wonnet.core.AppUpdater
import app.wonnet.core.MailStore
import app.wonnet.core.Version
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.LocalDark
import app.wonnet.theme.ShadowSize
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.theme.shadow
import app.wonnet.ui.ActionButton
import app.wonnet.ui.Banner
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.ConfirmDialog
import app.wonnet.ui.LocalSheetOverlay
import app.wonnet.ui.PlainButton
import kotlinx.coroutines.delay

/** Changed by the hardware keyboard's `/` to put the cursor in the search field. */
val LocalSearchFocus = compositionLocalOf { 0 }

/**
 * The app: the page's colour until the core says where the app was left, then the first screen
 * or the mail; over them the message being written, the sheets, the questions, the toast and the
 * update.
 */
@Composable
fun Root(store: MailStore) {
    val notes: @Composable BoxScope.() -> Unit = {
        // Over the bars along the bottom, so it hides none of their buttons.
        Column(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = Dims.barHeight + Space.l), horizontalAlignment = Alignment.CenterHorizontally) {
            if (store.booted) UpdateBanner(store.updater)
            ToastView(store)
        }
    }
    // WebView starts before the first message shows, and the thread the app was left on is read ahead.
    val context = LocalContext.current
    val dark = LocalDark.current
    LaunchedEffect(store.booted) {
        if (!store.booted) return@LaunchedEffect
        WebViewPool.warm(context)
        val open = store.conversation ?: return@LaunchedEffect
        WebViewPool.preload(context, open.messages.filter { !store.isFolded(it) }.mapNotNull { it.html?.let { html -> shown(html, dark) } })
    }
    CompositionLocalProvider(LocalSheetOverlay provides notes) {
        Box(Modifier.fillMaxSize().background(Tokens.background.color)) {
            when {
                !store.booted -> Unit
                !store.signedIn -> Onboarding(store)
                else -> Shell(store)
            }
            notes()
            store.compose?.let { compose -> androidx.compose.runtime.key(compose.id) { ComposeSheet(store, compose) } }
            Sheets(store)
            store.confirmation?.let { question ->
                ConfirmDialog(question.title, question.message, question.action, { store.confirmation = null }) {
                    store.confirmation = null
                    question.perform()
                }
            }
        }
    }
}

/** The note at the bottom: a line of text and maybe an action, gone after a while. */
@Composable
fun ToastView(store: MailStore) {
    val toast = store.toast
    LaunchedEffect(toast?.id) {
        val shown = toast ?: return@LaunchedEffect
        delay(shown.duration)
        store.dismissToast(shown)
    }
    AnimatedVisibility(toast != null, enter = fadeIn() + slideInVertically { it / 2 }, exit = fadeOut()) {
        val shown = toast ?: return@AnimatedVisibility
        Row(
            Modifier
                .padding(horizontal = Space.l)
                .widthIn(max = 560.dp)
                .height(Dims.searchHeight)
                .shadow(ShadowSize.MD, CircleShape)
                .background(Tokens.popover.color, CircleShape)
                .border(Dims.hairline, Tokens.border.color, CircleShape)
                .padding(horizontal = Space.l + 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.l),
        ) {
            Text(shown.message, TextStyle.Label, Modifier.weight(1f, fill = false), color = Tokens.foreground.color, maxLines = 2)
            shown.action?.let { action ->
                PlainButton(store::toastTapped) { Text(action, TextStyle.LabelStrong, color = Tokens.primary.color) }
            }
        }
    }
}

/** A new version, floating at the bottom: the offer, the download as it goes, and the install. Closed, it stays away until the state changes. */
@Composable
fun UpdateBanner(updater: AppUpdater) {
    val state = updater.state
    var closed by remember { mutableStateOf<AppUpdater.State?>(null) }
    val shown = state != AppUpdater.State.Idle && state != closed
    val close = { closed = state }
    val current = Version.label(updater.current)
    AnimatedVisibility(shown, enter = fadeIn() + slideInVertically { it }, exit = fadeOut() + slideOutVertically { it }) {
        val modifier = Modifier.padding(horizontal = Space.l, vertical = Space.s)
        when (state) {
            AppUpdater.State.Idle -> Unit
            AppUpdater.State.Checking -> Banner(Symbol.RefreshCw, "Checking for updates", "You have $current.", modifier, Tokens.process, pending = true)
            AppUpdater.State.UpToDate -> Banner(Symbol.CircleCheck, "Up to date", "$current is the newest version.", modifier, Tokens.success, close = close)
            is AppUpdater.State.Available -> Banner(
                Symbol.CircleArrowDown, "${Version.label(state.version)} is out", "You have $current.", modifier, Tokens.primary,
                action = { ActionButton("Update", variant = ButtonVariant.Primary, size = ControlSize.Small, onClick = updater::install) }, close = close,
            )
            is AppUpdater.State.Downloading -> Banner(
                Symbol.CircleArrowDown, "Downloading ${Version.label(state.version)}", "${(state.fraction * 100).toInt()}%", modifier, Tokens.process,
                progress = state.fraction,
            )
            is AppUpdater.State.Installing -> Banner(Symbol.CircleArrowDown, "Installing ${Version.label(state.version)}", "Android asks before it replaces $current.", modifier, Tokens.process, pending = true)
            is AppUpdater.State.Failed -> Banner(
                Symbol.CircleAlert, "Update failed", state.message, modifier, Tokens.destructive,
                action = { ActionButton("Try again", size = ControlSize.Small, onClick = updater::retry) }, close = close,
            )
        }
    }
}
