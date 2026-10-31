package app.wonnet.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.wonnet.core.AccountView
import app.wonnet.core.MailStore
import app.wonnet.core.Mailbox
import app.wonnet.core.SidebarOrder
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Tokens
import app.wonnet.ui.Edge
import app.wonnet.ui.MenuItem
import app.wonnet.ui.MovableList
import app.wonnet.ui.NavRow
import app.wonnet.ui.SectionHeading
import app.wonnet.ui.Spinner
import app.wonnet.ui.rule

/** A row of the sidebar: a mailbox of every account together, the accounts' heading, an account, or a mailbox under an open account. */
private sealed interface SidebarRow {
    val id: String

    data class Unified(val mailbox: Mailbox) : SidebarRow {
        override val id get() = mailbox.id
    }

    data object Heading : SidebarRow {
        override val id get() = "accounts"
    }

    data class Account(val account: AccountView) : SidebarRow {
        override val id get() = "account:${account.id}"
    }

    data class Inside(val mailbox: Mailbox) : SidebarRow {
        override val id get() = mailbox.id
    }
}

/**
 * The mailboxes of every account together, then each account with its colour, then Settings. The
 * mailboxes are held and dragged into the order wanted, and so are the accounts; both orders are
 * synced, so every device shows the same. Held and let go, a mailbox can be put among the tabs.
 */
@Composable
fun Sidebar(store: MailStore, showSettings: () -> Unit, picked: (String) -> Unit) {
    var expanded by remember { mutableStateOf(setOf<String>()) }
    val rows = buildList {
        addAll(store.unified.map { SidebarRow.Unified(it) })
        if (store.accounts.isNotEmpty()) add(SidebarRow.Heading)
        for (account in store.accounts) {
            add(SidebarRow.Account(account))
            if (account.id in expanded) addAll(account.mailboxes.map { SidebarRow.Inside(it) })
        }
    }

    fun clicked(row: SidebarRow) {
        when (row) {
            is SidebarRow.Unified -> pick(store, row.mailbox.id, picked)
            is SidebarRow.Inside -> pick(store, row.mailbox.id, picked)
            is SidebarRow.Account -> expanded = if (row.account.id in expanded) expanded - row.account.id else expanded + row.account.id
            SidebarRow.Heading -> Unit
        }
    }

    /** The row landed where `index` is among the rows without it and the mailboxes under it: after the rows of its kind before that. */
    fun moved(row: SidebarRow, index: Int) {
        val from = rows.indexOfFirst { it.id == row.id }
        var end = from + 1
        while (end < rows.size && rows[end] is SidebarRow.Inside) end++
        val rest = rows.filterIndexed { at, _ -> at !in from until end }
        val before = rest.take(index)
        when (row) {
            is SidebarRow.Unified -> {
                val ids = before.filterIsInstance<SidebarRow.Unified>().map { it.mailbox.id }
                store.setSidebarOrder(SidebarOrder.placed(row.mailbox.id, ids, store.unified.map { it.id }))
            }
            is SidebarRow.Account -> {
                val ids = before.filterIsInstance<SidebarRow.Account>().map { it.account.id }
                store.setAccountOrder(SidebarOrder.placed(row.account.id, ids, store.accounts.map { it.id }))
            }
            else -> Unit
        }
    }

    fun menu(row: SidebarRow): List<MenuItem> {
        val mailbox = (row as? SidebarRow.Unified)?.mailbox ?: (row as? SidebarRow.Inside)?.mailbox ?: return emptyList()
        return listOf(MenuItem(if (store.isTab(mailbox.id)) "Remove from Tabs" else "Add to Tabs", Symbol.Inbox) { store.toggleTab(mailbox.id) })
    }

    Column(Modifier.fillMaxSize().background(Tokens.background.color).statusBarsPadding()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(top = Space.s)) {
            MovableList(
                rows, key = { it.id },
                group = {
                    when (it) {
                        is SidebarRow.Unified -> "mailboxes"
                        is SidebarRow.Account -> "accounts"
                        else -> null
                    }
                },
                follows = { it is SidebarRow.Inside },
                clicked = ::clicked, moved = ::moved, menu = ::menu,
            ) { row, hovered ->
                when (row) {
                    is SidebarRow.Unified -> MailboxRow(store, row.mailbox, hovered)
                    is SidebarRow.Inside -> MailboxRow(store, row.mailbox, hovered, indent = true)
                    is SidebarRow.Account -> NavRow(row.account.title, dot = Dims.accountColor(row.account.color).color, hovered = hovered) {
                        when (row.account.status) {
                            "syncing" -> Spinner(tint = Tokens.mutedStrongerForeground.color)
                            "reauth", "error" -> Icon(Symbol.CircleAlert, 13f, Tokens.destructive.color)
                        }
                        Icon(if (row.account.id in expanded) Symbol.ChevronDown else Symbol.ChevronRight, 12f, Tokens.mutedStrongestForeground.color)
                    }
                    SidebarRow.Heading -> SectionHeading("Accounts", modifier = Modifier.fillMaxWidth().padding(start = 18.dp, top = Space.xl + 2.dp, bottom = Space.xs))
                }
            }
            Spacer(Modifier.height(Dims.sidebarEnd))
        }
        Box(Modifier.rule(Edge.Top).navigationBarsPadding()) {
            NavRow("Settings", symbol = Symbol.Settings, verticalPadding = 2.dp, onClick = showSettings) {
                if (store.connection == "offline") Icon(Symbol.WifiOff, 14f, Tokens.mutedStrongerForeground.color)
            }
        }
    }
}

@Composable
private fun MailboxRow(store: MailStore, mailbox: Mailbox, hovered: Boolean, indent: Boolean = false) {
    NavRow(
        mailbox.name, symbol = Symbol.named(mailbox.symbol), count = mailbox.unread,
        selected = store.baseMailbox == mailbox.id && store.searchRows == null, indent = if (indent) Space.l else 0.dp, hovered = hovered,
    )
}

private fun pick(store: MailStore, mailbox: String, picked: (String) -> Unit) {
    store.select(mailbox)
    picked(mailbox)
}
