package app.wonnet.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import app.wonnet.core.CoreError
import app.wonnet.core.MailStore
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.AppMark
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.GoogleMark
import app.wonnet.ui.MicrosoftMark
import app.wonnet.ui.InputField
import app.wonnet.ui.Notice
import app.wonnet.ui.OptionButton
import kotlinx.coroutines.launch

/** The pages of adding an account past its first: a JMAP server's form, or the server this app talks to. */
enum class AddPage { Jmap, Server }

/** The first screen: add a mail account, which also signs in. */
@Composable
fun Onboarding(store: MailStore) {
    Box(Modifier.fillMaxSize().background(Tokens.background.color).windowInsetsPadding(WindowInsets.safeDrawing)) {
        AddAccountFlow(store, welcome = true)
    }
}

/**
 * The ways to add an account: Google, Microsoft, or a JMAP server on a page of its own. On the first screen
 * (`welcome`) the app's name stands over them and the server this app talks to can be changed
 * under them, on a page of its own too.
 */
@Composable
fun AddAccountFlow(store: MailStore, welcome: Boolean = false) {
    var page by remember { mutableStateOf<AddPage?>(null) }
    BackHandler(page != null) { page = null }
    val shown = page
    if (shown != null) {
        FormColumn(centered = welcome) {
            ActionButton("Back", Modifier.padding(bottom = Space.m), symbol = Symbol.ArrowLeft, variant = ButtonVariant.Ghost, size = ControlSize.Small) { page = null }
            when (shown) {
                AddPage.Jmap -> JmapForm(store)
                AddPage.Server -> ServerForm(store) { page = null }
            }
        }
        return
    }
    val choices = @Composable {
        Column(verticalArrangement = Arrangement.spacedBy(Space.s)) {
            OptionButton("Continue with Google", "Gmail and Google Workspace", store::signInWithGoogle) { GoogleMark() }
            OptionButton("Continue with Microsoft", "Outlook, Hotmail and Microsoft 365", store::signInWithMicrosoft) { MicrosoftMark() }
            OptionButton("Continue with JMAP", "Fastmail, Stalwart and others", { page = AddPage.Jmap }) { Icon(Symbol.Server, 20f, Tokens.foreground.color) }
        }
    }
    if (!welcome) return FormColumn { choices() }
    Column(Modifier.fillMaxSize()) {
        FormColumn(Modifier.weight(1f), centered = true) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xxl + Space.s)) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.l)) {
                    AppMark()
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
                        Text("Wonnet", TextStyle.Display)
                        Text("Email that feels instant.", TextStyle.Body, color = Tokens.mutedForeground.color)
                    }
                }
                choices()
            }
        }
        ActionButton(
            "Use your own server", Modifier.align(Alignment.CenterHorizontally).padding(Space.l), symbol = Symbol.Server,
            variant = ButtonVariant.Ghost, size = ControlSize.Small,
        ) { page = AddPage.Server }
    }
}

/**
 * A form's column: as wide as a form, in the middle of the page, scrolled when the keyboard leaves
 * it no room. `centered` on a whole screen, a little above the middle, where the eye rests.
 */
@Composable
fun FormColumn(modifier: Modifier = Modifier, centered: Boolean = false, content: @Composable () -> Unit) {
    BoxWithConstraints(modifier.fillMaxWidth().background(Tokens.background.color)) {
        val height = maxHeight
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).heightIn(min = if (centered) height else androidx.compose.ui.unit.Dp.Unspecified),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = if (centered) Arrangement.Center else Arrangement.Top,
        ) {
            Column(
                Modifier.widthIn(max = Dims.formWidth).fillMaxWidth().padding(horizontal = Space.xl, vertical = if (centered) Space.xxl else Space.xl),
            ) { content() }
            if (centered) Spacer(Modifier.height(height * 0.08f))
        }
    }
}

/** A page's name and the line under it, over its form. */
@Composable
private fun FormHeading(title: String, detail: String) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
        Text(title, TextStyle.Title)
        Text(detail, TextStyle.Body, color = Tokens.mutedForeground.color)
    }
}

/** A JMAP server, an address and an app password. */
@Composable
private fun JmapForm(store: MailStore) {
    var url by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var adding by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    fun add() {
        if (adding || url.isEmpty() || email.isEmpty() || password.isEmpty()) return
        adding = true
        error = null
        scope.launch {
            try {
                store.addJmap(url, email, password)
                password = ""
            } catch (failure: CoreError) {
                error = failure.message
            }
            adding = false
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(Space.xl)) {
        FormHeading("JMAP server", "Any JMAP server, with an app password.")
        Column(verticalArrangement = Arrangement.spacedBy(Space.m)) {
            InputField(url, { url = it }, label = "Server", placeholder = "https://api.fastmail.com", autofocus = true, keyboard = KeyboardType.Uri)
            InputField(email, { email = it }, label = "Email", placeholder = "you@example.com", keyboard = KeyboardType.Email)
            InputField(password, { password = it }, label = "Password", placeholder = "App password", secure = true, commit = ::add)
        }
        error?.let { Notice(it, error = true) }
        ActionButton("Add account", variant = ButtonVariant.Primary, pending = adding, wide = true, enabled = url.isNotEmpty() && email.isNotEmpty() && password.isNotEmpty(), onClick = ::add)
    }
}

/** The server this app talks to: ours, or one the user runs. */
@Composable
private fun ServerForm(store: MailStore, done: () -> Unit) {
    var server by remember { mutableStateOf(store.server) }
    var saving by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    fun save() {
        if (saving || server.isEmpty()) return
        saving = true
        error = null
        scope.launch {
            try {
                store.setServer(server)
                done()
            } catch (failure: CoreError) {
                error = failure.message
            }
            saving = false
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(Space.xl)) {
        FormHeading("Your own server", "The app starts on Wonnet's server. Running your own, point this device at it.")
        InputField(server, { server = it }, label = "Server", placeholder = "https://mail.example.com", autofocus = true, keyboard = KeyboardType.Uri, commit = ::save)
        error?.let { Notice(it, error = true) }
        ActionButton("Use this server", variant = ButtonVariant.Primary, pending = saving, wide = true, enabled = server.isNotEmpty(), onClick = ::save)
    }
}
