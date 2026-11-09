package app.wonnet.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.wonnet.core.ConnectAccess
import app.wonnet.core.ConnectLink
import app.wonnet.core.ConnectRequest
import app.wonnet.core.CoreError
import app.wonnet.core.MailStore
import app.wonnet.core.approveConnectRequest
import app.wonnet.core.connectRequest
import app.wonnet.core.denyConnectRequest
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Opacity
import app.wonnet.theme.SCALE
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Card
import app.wonnet.ui.Chip
import app.wonnet.ui.FormSection
import app.wonnet.ui.Notice
import app.wonnet.ui.PickRow
import app.wonnet.ui.RowsCard
import app.wonnet.ui.RowsCardRule
import app.wonnet.ui.Rule
import app.wonnet.ui.Sheet
import app.wonnet.ui.Spinner
import kotlinx.coroutines.launch

/** How far an account under Only some accounts is set in: its dot under the choice's words. */
private val accountIndent = 28.dp + Space.m

/**
 * What a `wonnet://connect` link opens: who is asking, what it may reach and do, and Approve or
 * Deny. Closed any other way before it is answered, it denies quietly, so the browser hears at once.
 */
@Composable
fun ConnectSheet(store: MailStore, link: ConnectLink) {
    var request by remember { mutableStateOf<ConnectRequest?>(null) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var approving by remember { mutableStateOf(false) }
    var approved by remember { mutableStateOf(false) }
    var answerError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(link.id) {
        try {
            request = store.connectRequest(link.url)
        } catch (error: CoreError) {
            loadError = error.message
        }
    }
    val close = { store.connecting = null }
    val dismiss = {
        val key = request?.key ?: link.key
        if (key != null && loadError == null && !approving && !approved) store.denyConnectRequest(key)
        close()
    }
    val loaded = request
    Sheet(loaded?.let { "Connect ${it.client}" } ?: "Connect an application", dismiss) {
        val error = loadError
        when {
            error != null -> LoadFailed(error, close)
            loaded == null -> Box(Modifier.fillMaxSize().semantics { contentDescription = "Loading" }, contentAlignment = Alignment.Center) { Spinner(20f) }
            approved -> Approved(loaded.client, close)
            else -> ConnectForm(
                loaded, approving, answerError,
                approve = { access ->
                    approving = true
                    answerError = null
                    scope.launch {
                        try {
                            store.approveConnectRequest(loaded.key, access)
                            approved = true
                        } catch (failure: CoreError) {
                            answerError = failure.message
                        }
                        approving = false
                    }
                },
                deny = {
                    store.denyConnectRequest(loaded.key)
                    close()
                    store.show("Denied ${loaded.client}.")
                },
            )
        }
    }
}

@Composable
private fun LoadFailed(message: String, close: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.l)) {
        Notice(message, error = true, symbol = Symbol.CircleAlert)
        ActionButton("Close", Modifier.offset(x = -Dims.buttonGap / 2), onClick = close)
    }
}

/** The choices, Every account and Reader with no privileges at first, over Approve and Deny; `error` is why answering failed. */
@Composable
private fun ColumnScope.ConnectForm(
    request: ConnectRequest,
    approving: Boolean,
    error: String?,
    approve: (ConnectAccess) -> Unit,
    deny: () -> Unit,
) {
    var allAccounts by remember { mutableStateOf(true) }
    var accounts by remember { mutableStateOf(emptySet<String>()) }
    var role by remember { mutableStateOf(request.roles.firstOrNull { it.id == "reader" }?.id ?: request.roles.firstOrNull()?.id ?: "reader") }
    var privileges by remember { mutableStateOf(emptySet<String>()) }
    Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState())) {
        Column(
            Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp).padding(Space.l),
            verticalArrangement = Arrangement.spacedBy(Space.xl),
        ) {
            WhoIsAsking(request)
            FormSection("Accounts", "Which mail it can reach.") {
                RowsCard {
                    PickRow("Every account", allAccounts, detail = "Including the ones you add later.", symbol = Symbol.Inbox) { allAccounts = true }
                    RowsCardRule()
                    PickRow("Only some accounts", !allAccounts, detail = "Only the ones you pick.", symbol = Symbol.ListFilter) { allAccounts = false }
                    if (!allAccounts) {
                        for (account in request.accounts) {
                            RowsCardRule(Space.l + accountIndent)
                            PickRow(
                                account.title, account.id in accounts, detail = account.detail, dot = Dims.accountColor(account.color ?: "").color,
                                multiple = true, indent = accountIndent,
                            ) { accounts = if (account.id in accounts) accounts - account.id else accounts + account.id }
                        }
                    }
                }
            }
            FormSection("Role", "The most it can do with that mail.") {
                RowsCard {
                    for ((index, choice) in request.roles.withIndex()) {
                        if (index > 0) RowsCardRule()
                        PickRow(choice.title, role == choice.id, detail = choice.detail, symbol = choice.symbol?.let(Symbol::named)) { role = choice.id }
                    }
                }
            }
            FormSection("Privileges", "What the role doesn't cover.") {
                RowsCard {
                    for ((index, choice) in request.privileges.withIndex()) {
                        if (index > 0) RowsCardRule()
                        PickRow(choice.title, choice.id in privileges, detail = choice.detail, symbol = choice.symbol?.let(Symbol::named), multiple = true) {
                            privileges = if (choice.id in privileges) privileges - choice.id else privileges + choice.id
                        }
                    }
                }
            }
        }
    }
    Rule()
    Column(
        Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp).fillMaxWidth().padding(horizontal = Space.l, vertical = Space.s),
        verticalArrangement = Arrangement.spacedBy(Space.xs),
    ) {
        error?.let { Notice(it, Modifier.padding(vertical = Space.xs), error = true, symbol = Symbol.CircleAlert) }
        Row(Modifier.fillMaxWidth().offset(x = Dims.buttonGap / 2), horizontalArrangement = Arrangement.End) {
            ActionButton("Deny", variant = ButtonVariant.Ghost, enabled = !approving, onClick = deny)
            ActionButton("Approve", variant = ButtonVariant.Primary, pending = approving, enabled = allAccounts || accounts.isNotEmpty()) {
                approve(ConnectAccess(role, allAccounts, if (allAccounts) emptySet() else accounts, privileges))
            }
        }
    }
}

/** Who is asking: verified or not, who published it, where it sends the browser back to, and what to beware of. */
@Composable
private fun WhoIsAsking(request: ConnectRequest) {
    Card(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            Icon(if (request.verified) Symbol.ShieldCheck else Symbol.Box, 20f / SCALE, Tokens.foreground.color)
            Text(request.client, TextStyle.Heading, Modifier.weight(1f, fill = false).semantics { heading() })
            if (request.verified) {
                Chip("Verified", symbol = Symbol.ShieldCheck, tint = Tokens.success)
            } else {
                Chip("Unverified", symbol = Symbol.CircleAlert, tint = Tokens.warning)
            }
        }
        Column(Modifier.padding(top = Space.l), verticalArrangement = Arrangement.spacedBy(Space.s)) {
            Notice(request.publisher, symbol = Symbol.Globe)
            Notice(request.backTo, symbol = Symbol.ArrowRight)
            if (request.onDevice) Notice("Sends you back to an application on this device", warning = true, symbol = Symbol.Monitor)
            Notice("Only approve if you started this yourself", warning = true, symbol = Symbol.CircleAlert)
        }
    }
}

/** Approved: the browser goes back on its own, and Settings can take it back. */
@Composable
private fun Approved(client: String, close: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(Space.xl),
        verticalArrangement = Arrangement.spacedBy(Space.l, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.size((56 * SCALE).dp).background(Tokens.success.opacity(Opacity.colorTint).color, CircleShape), contentAlignment = Alignment.Center) {
            Icon(Symbol.Check, 26f, Tokens.success.color)
        }
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
            Text("Approved", TextStyle.Heading, Modifier.semantics { heading() })
            Text(
                "Your browser takes you back to $client. You can revoke it in Settings → Connected apps.", TextStyle.Body,
                Modifier.widthIn(max = 360.dp), color = Tokens.mutedForeground.color, align = TextAlign.Center,
            )
        }
        ActionButton("Done", variant = ButtonVariant.Primary, onClick = close)
    }
}
