package app.wonnet.core

import app.wonnet.theme.Symbol
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import java.util.UUID

/** A page of the settings, pushed from their list, or beside it on a wide screen. */
enum class SettingsSection(val title: String, val symbol: Symbol) {
    General("General", Symbol.Settings),
    Accounts("Accounts", Symbol.AtSign),
    Inbox("Inbox", Symbol.Inbox),
    Reading("Reading", Symbol.MailOpen),
    Writing("Writing", Symbol.Pencil),
}

/** One group of settings, as the settings' search finds it. Its `id` is the group's on its page. */
data class SettingsEntry(val id: String, val title: String, val section: SettingsSection, val keywords: String) {
    companion object {
        val all = listOf(
            SettingsEntry("appearance", "Theme", SettingsSection.General, "appearance light dark system mode"),
            SettingsEntry("updates", "Updates", SettingsSection.General, "version check for updates release about"),
            SettingsEntry("server", "Server", SettingsSection.General, "connection connected offline sync"),
            SettingsEntry("accounts", "Accounts", SettingsSection.Accounts, "add remove gmail jmap fastmail colour color notifications"),
            SettingsEntry("signatures", "Signatures", SettingsSection.Accounts, "sign off footer"),
            SettingsEntry("sign-out", "Sign out", SettingsSection.Accounts, "log out leave"),
            SettingsEntry("split-inbox", "Split Inbox", SettingsSection.Inbox, "important other newsletters notifications splits"),
            SettingsEntry("tabs", "Tabs", SettingsSection.Inbox, "mailboxes along the bottom order"),
            SettingsEntry("swipe", "Swipe Left", SettingsSection.Inbox, "swipe actions gestures archive snooze read unread"),
            SettingsEntry("mail", "Dark mode for mail", SettingsSection.Reading, "designed newsletters colours remote images pictures privacy tracking"),
            SettingsEntry("blocked", "Blocked senders", SettingsSection.Reading, "block unblock trash spam"),
            SettingsEntry("sending", "Undo send", SettingsSection.Writing, "delay cancel sending"),
            SettingsEntry("snippets", "Snippets", SettingsSection.Writing, "templates canned replies text"),
        )

        /** The groups every word of the query is found in. */
        fun matching(query: String): List<SettingsEntry> {
            val words = query.split(Regex("\\s+")).filter { it.isNotEmpty() }
            if (words.isEmpty()) return emptyList()
            return all.filter { entry ->
                val text = "${entry.title} ${entry.section.title} ${entry.keywords}"
                words.all { text.contains(it, ignoreCase = true) }
            }
        }
    }
}

fun MailStore.openSettings(section: SettingsSection = SettingsSection.General, target: String? = null) {
    settings = section
    settingsTarget = target
    scope.launch { loadPreferences() }
}

fun MailStore.closeSettings() {
    settings = null
    settingsTarget = null
    settingsQuery = ""
}

/** A preference that is on or off, as it starts while unset. */
fun MailStore.flag(key: String, default: Boolean): Boolean = preferences[key]?.flag ?: default

/** Sets a preference that is on or off, removed while it is as it starts. */
fun MailStore.setFlag(key: String, default: Boolean, on: Boolean) = setPreference(key, if (on == default) null else JsonPrimitive(on))

fun MailStore.notifies(account: AccountView): Boolean = flag("notify:${account.id}", true)

val MailStore.connectionText: String
    get() = when (connection) {
        "online" -> "Connected"
        "connecting" -> "Connecting"
        else -> "Offline"
    }

fun MailStore.status(account: AccountView): String = when (account.status) {
    "syncing" -> "Syncing"
    "reauth" -> "Sign in again: remove the account and add it back"
    "error" -> "Can't reach the provider right now"
    else -> if (account.provider == "gmail") "Gmail" else "JMAP"
}

/** The line under an account's name: its status, after its address when the name hides it. */
fun MailStore.subtitle(account: AccountView): String =
    if (account.name.isEmpty()) status(account) else "${account.address} · ${status(account)}"

/** The senders and domains whose new mail goes to the trash. */
val MailStore.blockedSenders: List<String>
    get() = preferences.filter { it.key.startsWith("blocked:") && it.value.flag != false }.keys.map { it.removePrefix("blocked:") }.sorted()

fun MailStore.unblock(address: String) = setPreference("blocked:$address", null)

val MailStore.customSplits: List<CustomSplit> get() = CustomSplit.all(preferences)

fun MailStore.newSplit(): CustomSplit {
    val order = (customSplits.maxOfOrNull { it.order } ?: -1) + 1
    return CustomSplit(UUID.randomUUID().toString().lowercase(), "", emptyList(), null, order)
}

fun MailStore.saveSplit(split: CustomSplit) = setPreference(split.key, split.value)

fun MailStore.removeSplit(split: CustomSplit) = setPreference(split.key, null)

/** Puts the split at `index` among the others. */
fun MailStore.moveSplit(split: CustomSplit, index: Int) {
    val reordered = customSplits.filter { it.id != split.id }.toMutableList()
    reordered.add(index.coerceIn(0, reordered.size), split)
    for ((order, each) in reordered.withIndex()) {
        if (each.order == order) continue
        val moved = each.copy(order = order)
        setPreference(moved.key, moved.value)
    }
}

/** Every account's labels, by id, for a split to take mail of. */
val MailStore.splitLabels: List<Pair<String, String>>
    get() = accounts.flatMap { account ->
        account.mailboxes.mapNotNull { mailbox ->
            val id = MailStore.labelID(mailbox.id) ?: return@mapNotNull null
            id to (if (accounts.size > 1) "${mailbox.name} · ${account.title}" else mailbox.name)
        }
    }

fun MailStore.summary(split: CustomSplit): String {
    val parts = split.from.toMutableList()
    split.label?.let { label -> splitLabels.firstOrNull { it.first == label }?.let { parts += "label ${it.second}" } }
    return if (parts.isEmpty()) "Nothing yet" else parts.joinToString(", ")
}

/** Puts the tab at `index` among the others. */
fun MailStore.moveTab(tab: Mailbox, index: Int) {
    val ids = savedTabs.map { it.id }.filter { it != tab.id }.toMutableList()
    ids.add(index.coerceIn(0, ids.size), tab.id)
    setTabs(ids)
}

/** The mailboxes that could be tabs and aren't yet: those of every account together, then each account's. */
val MailStore.mailboxesForTabs: Pair<List<Mailbox>, List<Pair<AccountView, List<Mailbox>>>>
    get() {
        val left = unified.filter { !isTab(it.id) }
        val byAccount = accounts.mapNotNull { account ->
            val mailboxes = account.mailboxes.filter { !isTab(it.id) }
            if (mailboxes.isEmpty()) null else account to mailboxes
        }
        return left to byAccount
    }

val MailStore.swipeCommands: List<ThreadCommand> get() = SwipePreference.commands(preferences)

/** Keeps one to three swipe commands, removed while they are the usual ones. */
fun MailStore.setSwipeCommands(commands: List<ThreadCommand>) {
    if (commands.size !in 1..SwipePreference.MOST) return
    setPreference(SwipePreference.KEY, if (commands == SwipePreference.standard) null else jsonOf(commands.map { it.id }))
}

fun MailStore.moveSwipeCommand(command: ThreadCommand, index: Int) {
    val commands = swipeCommands.filter { it != command }.toMutableList()
    commands.add(index.coerceIn(0, commands.size), command)
    setSwipeCommands(commands)
}

/** The commands a swipe could have and doesn't yet. */
val MailStore.swipeChoicesLeft: List<ThreadCommand> get() = SwipePreference.choices.filter { it !in swipeCommands }

val MailStore.snippets: List<Snippet> get() = Snippet.all(preferences)

fun MailStore.saveSnippet(snippet: Snippet) {
    val trimmed = snippet.copy(name = snippet.name.trim())
    setPreference(trimmed.key, trimmed.value)
}

/** The signature the provider keeps for the account, used while the user's own is empty. */
fun providedSignature(account: AccountView): String? = account.identities.firstOrNull()?.signature?.takeIf { it.isNotEmpty() }

/** Who keeps the account's mail, as a sentence names them. */
fun providerName(account: AccountView): String = if (account.provider == "gmail") "Gmail" else "your provider"

fun MailStore.ownSignature(account: AccountView): String = preferences["signature:${account.id}"]?.text ?: ""

fun MailStore.setSignature(text: String, account: AccountView) =
    setPreference("signature:${account.id}", if (text.isEmpty()) null else JsonPrimitive(text))

val undoDelays = listOf(0, 5, 10, 20, 30)

fun undoDelayName(seconds: Int): String = if (seconds == 0) "Off" else "$seconds s"

fun MailStore.confirmSigningOut() {
    confirmation = Confirmation("Sign out of Wonnet?", "Every account leaves this device. Signing in again brings them back.", "Sign out") {
        closeSettings()
        signOut()
    }
}

fun MailStore.confirmRemoving(account: AccountView) {
    confirmation = Confirmation(
        "Remove ${account.address}?", "Its mail leaves this device and your other devices. You can add it again.", "Remove",
    ) { removeAccount(account.id) }
}
