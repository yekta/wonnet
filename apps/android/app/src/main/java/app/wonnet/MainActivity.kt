package app.wonnet

import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.toArgb
import app.wonnet.core.AddressBook
import app.wonnet.core.Appearance
import app.wonnet.core.KeyHandler
import app.wonnet.core.Notifier
import app.wonnet.screens.Root
import app.wonnet.screens.LocalSearchFocus
import app.wonnet.theme.LocalDark
import app.wonnet.theme.Tokens

/**
 * The app's one screen. `--ez demo true` starts it on made-up mail that never connects; the
 * browser comes back here from a Google or Microsoft sign-in, and a notification's tap opens its thread.
 */
class MainActivity : ComponentActivity() {
    private val store get() = App.store
    private var searchFocus by mutableIntStateOf(0)
    private val keys by lazy { KeyHandler(store) { searchFocus += 1 } }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store.start(demo = intent.getBooleanExtra("demo", false))
        received(intent)
        setContent {
            val dark = when (store.appearance) {
                Appearance.System -> isSystemInDarkTheme()
                Appearance.Light -> false
                Appearance.Dark -> true
            }
            LaunchedEffect(dark) {
                val bar = if (dark) SystemBarStyle.dark(Tokens.background.color(true).toArgb())
                else SystemBarStyle.light(Tokens.background.color(false).toArgb(), Tokens.foreground.color(false).toArgb())
                enableEdgeToEdge(bar, bar)
            }
            CompositionLocalProvider(LocalDark provides dark, LocalSearchFocus provides searchFocus) { Root(store) }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        received(intent)
    }

    /** A sign-in coming back, or a notification's thread. */
    private fun received(intent: Intent) {
        intent.data?.takeIf { it.scheme == "wonnet" }?.let { store.finishSignIn(it.toString()) }
        intent.getStringExtra(Notifier.THREAD)?.let { thread ->
            intent.removeExtra(Notifier.THREAD)
            store.show(thread)
        }
    }

    override fun onResume() {
        super.onResume()
        store.cameBack()
    }

    override fun onPause() {
        super.onPause()
        store.saveUi()
    }

    private val permissions = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { AddressBook.load(this) }

    /** Asks for the permissions; the contacts are read again once answered. */
    fun ask(wanted: Array<String>) = permissions.launch(wanted)

    /** The hardware keyboard's keys the screen passed on: a field being typed in keeps its own. */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean = keys.handle(event) || super.onKeyDown(keyCode, event)
}
