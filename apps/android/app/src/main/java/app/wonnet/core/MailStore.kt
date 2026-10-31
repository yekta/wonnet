package app.wonnet.core

import android.content.Context
import androidx.core.content.edit
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import app.wonnet.BuildConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.async
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import java.io.File
import java.text.DateFormat
import java.util.Date
import java.util.UUID
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/** A note at the bottom of the screen, sometimes with something to do about it. */
data class Toast(val message: String, val action: String? = null, val duration: Long = 4000, val id: String = UUID.randomUUID().toString())

/** Threads being labelled, or moved to a label. */
data class Labeling(val threads: List<String>, val move: Boolean, val id: String = UUID.randomUUID().toString())

/** A question asked before something that can't be taken back. */
data class Confirmation(val title: String, val message: String, val action: String, val perform: () -> Unit)

/** What the command palette lists. */
enum class PaletteScope { Everything, Labels }

/** How the app is drawn: as the system is, or always light or dark. */
enum class Appearance(val title: String) {
    System("System"), Light("Light"), Dark("Dark");
}

/**
 * Everything the app shows, kept on the main thread, and what it asks of the core. The iOS app's
 * `MailStore`, which this follows.
 */
class MailStore(private val context: Context) {
    val bridge = CoreBridge()
    val scope: CoroutineScope = MainScope()
    val updater = AppUpdater(context)

    var started = false
        private set
    var demo = false
        private set
    /** The core answered where the app was left: until then the screen shows nothing. */
    var booted by mutableStateOf(false)
    var signedIn by mutableStateOf(false)
    var connection by mutableStateOf("offline")
    var server by mutableStateOf("")
    /** The mailboxes of every account together and the accounts, as the core lists them. */
    private var listedUnified by mutableStateOf<List<Mailbox>>(emptyList())
    private var listedAccounts by mutableStateOf<List<AccountView>>(emptyList())
    var mailbox by mutableStateOf("inbox")
    var rows by mutableStateOf<List<ThreadRow>>(emptyList())
    /** False while the rows are still another mailbox's or filter's, until its page is read. */
    var listReady by mutableStateOf(true)
    var total by mutableStateOf(0)
    /** The next page of the list is being read, for the foot of the list to say so. */
    var loadingMore by mutableStateOf(false)
        private set
    /** The thread on screen, when one is open. */
    var conversation by mutableStateOf<Conversation?>(null)
    /** The threads beside the open one, drawn beside it so that a swipe brings one in. */
    var peeks by mutableStateOf<Map<String, Conversation>>(emptyMap())
        private set
    /** The row the keyboard is on. */
    var selected by mutableStateOf<String?>(null)
    var searchQuery by mutableStateOf("")
    /** The matches of the search, while searching. */
    var searchRows by mutableStateOf<List<ThreadRow>?>(null)
    var toast by mutableStateOf<Toast?>(null)
    var compose by mutableStateOf<Compose?>(null)
    /** The threads to snooze, while the snooze choices are shown. */
    var snoozing by mutableStateOf<List<String>?>(null)
    /** The tabs over a split inbox; empty without Split Inbox. */
    var splits by mutableStateOf<List<SplitTab>>(emptyList())
    /** What the list is narrowed to. */
    var filter by mutableStateOf<Filter?>(null)
    /** The threads picked to act on together. */
    var selection by mutableStateOf<Set<String>>(emptySet())
    /** Where a Shift+J/K selection grows from. */
    var selectionAnchor: String? = null
    /** The threads being labelled or moved, while the labels are shown. */
    var labeling by mutableStateOf<Labeling?>(null)
    /** What the command palette shows, while it is open. */
    var palette by mutableStateOf<PaletteScope?>(null)
    /** The sheet that shows every component, for working on the design. */
    var galleryOpen by mutableStateOf(false)
    /** The settings' page that is open, in their sheet. */
    var settings by mutableStateOf<SettingsSection?>(null)
    /** The group of the page a search picked, to scroll to. */
    var settingsTarget by mutableStateOf<String?>(null)
    var settingsQuery by mutableStateOf("")
    /** A question to answer before something that can't be taken back. */
    var confirmation by mutableStateOf<Confirmation?>(null)
    /** A thread a notification asked to show, for the screen to push. */
    var requestedThread by mutableStateOf<String?>(null)
    /** A notification's thread whose mail hasn't synced yet. */
    private var arrivingThread: String? = null
    /** Threads whose remote images were allowed. */
    var imagesShown by mutableStateOf<Set<String>>(emptySet())
    /** How far the list and the open thread are scrolled; a list made anew goes back to it. */
    var listOffset = 0.0
    var threadOffset: Double? = null
    /** How tall the open thread's bodies were, by message id, when they were `bodyWidth` wide. */
    private var bodyHeights: Map<String, Double> = emptyMap()
    private var bodyWidth = 0.0
    /** What this device keeps for itself, apart from the synced preferences. */
    private val local = context.getSharedPreferences("app", Context.MODE_PRIVATE)
    var undoDelay by mutableStateOf(local.getInt("undoDelay", 5))
        private set
    var appearance by mutableStateOf(Appearance.entries.firstOrNull { it.name == local.getString("appearance", "") } ?: Appearance.System)
        private set

    /** The synced preferences, by key. */
    var preferences by mutableStateOf<Map<String, JsonElement>>(emptyMap())
    var preferencesLoaded = false
    /** Attachments being downloaded, as `<message>/<index>`. */
    var downloading by mutableStateOf<Set<String>>(emptySet())
    /** Messages of the open thread unfolded by a tap or the keyboard. */
    var unfoldedMessages by mutableStateOf<Set<String>>(emptySet())
    /** Messages of the open thread folded by a tap on their header, or all of them at once. */
    var foldedMessages by mutableStateOf<Set<String>>(emptySet())
    /** Designed messages of the open thread shown as their sender made them, not turned dark. */
    var originalMessages by mutableStateOf<Set<String>>(emptySet())
    /** The message of the open thread that n and p are on. */
    var focusedMessage by mutableStateOf<String?>(null)
    /** Each compose sheet's last save, so the next save and the send wait for it. */
    private val draftSaves = HashMap<String, Deferred<String?>>()
    private var toastAction: (() -> Unit)? = null
    private var searchRequest = 0L
    private var reloading = false
    /** A change came while the list was being read: read it again after. */
    private var staleWhileReloading = false
    /** The list's end was reached while it was being read: read the next page after. */
    private var moreWhileReloading = false
    /** The threads whose bodies were asked for ahead of time. */
    private val prefetched = HashSet<String>()

    /** The rows the list shows: the search's while searching. */
    val visibleRows: List<ThreadRow> get() = searchRows ?: rows

    /** The mailbox without its split: `inbox` for `inbox:other`. */
    val baseMailbox: String
        get() {
            val parts = mailbox.split("/")
            if (!parts.last().startsWith("inbox:")) return mailbox
            return (parts.dropLast(1) + "inbox").joinToString("/")
        }

    /** The mailboxes of every account together, in the sidebar's order. */
    val unified: List<Mailbox> get() = SidebarOrder.sorted(listedUnified, SidebarOrder.ids(SidebarOrder.MAILBOXES, preferences)) { it.id }

    /** The accounts, in the sidebar's order. */
    val accounts: List<AccountView> get() = SidebarOrder.sorted(listedAccounts, SidebarOrder.ids(SidebarOrder.ACCOUNTS, preferences)) { it.id }

    fun setSidebarOrder(mailboxes: List<String>) = setPreference(SidebarOrder.MAILBOXES, jsonOf(mailboxes))

    fun setAccountOrder(accounts: List<String>) = setPreference(SidebarOrder.ACCOUNTS, jsonOf(accounts))

    /** Every mailbox there is, unified first. */
    val allMailboxes: List<Mailbox> get() = unified + accounts.flatMap { it.mailboxes }

    /** The account whose mailbox is on screen; none in the mailboxes of every account together. */
    val listAccount: String?
        get() {
            val parts = mailbox.split("/")
            return if (parts.size > 1) parts[0] else null
        }

    /** Whether the list is Unread, where the threads on screen stay when they are read. */
    val inUnread: Boolean get() = baseMailbox.split("/").last() == "unread"

    /** The tabs as saved, in the user's order; one that is gone is skipped. */
    val savedTabs: List<Mailbox>
        get() {
            val all = allMailboxes
            return TabPreference.ids(preferences).mapNotNull { id -> all.firstOrNull { it.id == id } }
        }

    /** The tabs along the bottom, for the account on screen: there `inbox` is its inbox. */
    val tabs: List<Mailbox>
        get() {
            val all = allMailboxes
            val shown = HashSet<String>()
            return TabPreference.ids(preferences).mapNotNull { id ->
                val scoped = scopedTab(id)
                if (!shown.add(scoped)) return@mapNotNull null
                all.firstOrNull { it.id == scoped }
            }
        }

    private fun scopedTab(id: String): String {
        val account = listAccount
        if (id.contains("/") || account == null) return id
        return "$account/$id"
    }

    /** The saved id a tab along the bottom comes from. */
    fun savedTab(id: String): String = TabPreference.ids(preferences).firstOrNull { scopedTab(it) == id } ?: id

    fun setTabs(ids: List<String>) = setPreference(TabPreference.KEY, jsonOf(ids))

    fun isTab(id: String): Boolean = id in TabPreference.ids(preferences)

    /** Puts a mailbox among the tabs, or takes it off. */
    fun toggleTab(id: String) {
        val ids = TabPreference.ids(preferences).toMutableList()
        if (!ids.remove(id)) ids += id
        setTabs(ids)
    }

    /** A mailbox's name, with its account's after it when it is one account's other than `shownIn`. */
    fun tabName(mailbox: Mailbox, shownIn: String? = null): String {
        val parts = mailbox.id.split("/")
        if (parts.size < 2 || accounts.size < 2 || parts[0] == shownIn) return mailbox.name
        val account = accounts.firstOrNull { it.id == parts[0] } ?: return mailbox.name
        return "${mailbox.name} · ${account.title}"
    }

    /** The mailbox's name, or "Search" while searching. */
    val mailboxName: String get() = if (searchRows != null) "Search" else baseMailboxName

    /** The mailbox's name, whether or not a search is on over it. */
    val baseMailboxName: String
        get() {
            val (name, account) = baseMailboxTitle
            return account?.let { "$name · $it" } ?: name
        }

    /** The mailbox's name and, when it is one account's, that account's name. */
    val baseMailboxTitle: Pair<String, String?>
        get() {
            val parts = baseMailbox.split("/")
            val name = allMailboxes.firstOrNull { it.id == baseMailbox }?.name ?: "Inbox"
            val account = if (parts.size > 1) accounts.firstOrNull { it.id == parts[0] } else null
            if (account == null) return (if (name == "Inbox" && accounts.size > 1) "All Inboxes" else name) to null
            return name to account.title
        }

    /** The mailbox's mail is still coming: its account's, or any account's in a unified mailbox. */
    val mailboxSyncing: Boolean
        get() {
            if (connection == "connecting" && accounts.isEmpty()) return true
            val syncing = accounts.filter { it.status == "syncing" }
            val parts = baseMailbox.split("/")
            if (parts.size < 2) return syncing.isNotEmpty()
            return syncing.any { it.id == parts[0] }
        }

    /** Whether a sheet, the palette or a question is up, when the screen's keys don't apply. */
    val sheetOpen: Boolean
        get() = compose != null || snoozing != null || labeling != null || palette != null || settings != null || galleryOpen ||
            confirmation != null

    // Starting

    /** Starts the core, once; `demo` shows made-up mail and never connects. */
    fun start(demo: Boolean) {
        if (started) return
        started = true
        this.demo = demo
        bridge.onEvent = { handle(it) }
        val folder = if (demo) File(context.filesDir, "Demo") else context.filesDir
        folder.mkdirs()
        val config = json("data_dir" to folder.path, "server_url" to BuildConfig.SERVER_URL, "demo" to demo)
        if (!bridge.start(context, config)) {
            booted = true
            show("The mail store couldn't open.")
            return
        }
        Notifier.bridge = bridge
        SenderPhotos.bridge = bridge
        AttachmentPreviews.bridge = bridge
        Outgoing.prune(context)
        if (!demo) updater.start()
        Push.register(context, this)
        scope.launch { boot() }
    }

    /** Shows the screen the app was left on, all of it at once. */
    private suspend fun boot() {
        val boot = runCatching { bridge.call("boot") { Boot.read(it.objectOrEmpty) } }.getOrNull()
        if (boot != null) {
            val ui = boot.ui
            signedIn = boot.status.signedIn
            connection = boot.status.connection
            server = boot.status.server ?: ""
            listedUnified = boot.mailboxes.unified
            listedAccounts = boot.mailboxes.accounts
            preferences = boot.preferences
            preferencesLoaded = true
            mailbox = ui.mailbox
            filter = ui.filter
            rows = boot.page.rows
            total = boot.page.total
            splits = boot.page.splits
            selected = ui.selected
            selection = ui.selection.toSet()
            searchQuery = ui.search
            searchRows = boot.search
            unfoldedMessages = ui.unfolded.toSet()
            bodyHeights = ui.heights
            bodyWidth = ui.bodyWidth
            conversation = boot.thread
            focusedMessage = ui.focused
            val open = boot.thread?.id
            if (ui.images && open != null) imagesShown = imagesShown + open
            listOffset = ui.listOffset
            threadOffset = ui.threadOffset
            ui.compose?.let { compose = it.copy(keep = it.keep || it.hasContent) }
            Notifier.unread = inboxUnread
            if (accounts.isNotEmpty()) mailArrives()
            scope.launch { Notifier.settle(null) }
            if (searchQuery.isNotEmpty()) search(searchQuery)
        }
        booted = true
        trackUi()
    }

    /** The unread threads of every inbox together, for the notifications' count. */
    private val inboxUnread: Int get() = unified.firstOrNull { it.id == "inbox" }?.unread ?: 0

    /** Once there is mail: the contacts' names, and asking to notify. */
    private fun mailArrives() {
        AddressBook.start(context, bridge)
        Notifier.askPermission()
    }

    // Where the app is

    /** Where the app is, as the core keeps it between starts. */
    private val uiState: UiState
        get() = UiState(
            mailbox, filter, conversation?.id, selected, rows.size, listOffset, threadOffset, selection.toList(), searchQuery, compose,
            false, unfoldedMessages.toList(), focusedMessage, conversation?.let { it.id in imagesShown } ?: false, bodyHeights, bodyWidth,
        )

    /** Saves a little after the state changes, however much changes in the meantime. */
    @OptIn(FlowPreview::class)
    private fun trackUi() {
        scope.launch {
            snapshotFlow { uiState }.drop(1).debounce(300).collect { saveUi() }
        }
    }

    private var scrollSave: Job? = null

    /** Saves a little after a scroll, which isn't watched as the state is. */
    private fun scheduleUiSave() {
        scrollSave?.cancel()
        scrollSave = scope.launch {
            kotlinx.coroutines.delay(300)
            saveUi()
        }
    }

    fun saveUi() {
        if (!booted) return
        bridge.send("save_ui", mapOf("ui" to uiState.toJson()))
    }

    /** The compose view reports what it holds as it is written. */
    fun noteCompose(live: Compose) {
        if (compose?.id != live.id || compose == live) return
        compose = live
    }

    /** The list reports where it is scrolled to. */
    fun noteScroll(offset: Double) {
        if (offset == listOffset) return
        listOffset = offset
        scheduleUiSave()
    }

    /** The open thread reports where it is scrolled to. */
    fun noteThreadScroll(offset: Double) {
        if (offset == threadOffset) return
        threadOffset = offset
        scheduleUiSave()
    }

    /** A body of the open thread reports how tall its page is at this width. A new width starts over. */
    fun noteBodyHeight(message: String, width: Double, height: Double) {
        if (width <= 0) return
        if (abs(width - bodyWidth) > 0.5) {
            bodyHeights = emptyMap()
            bodyWidth = width
        }
        if (bodyHeights[message] == height) return
        bodyHeights = bodyHeights + (message to height)
        scheduleUiSave()
    }

    /** The saved height of a body, when it was measured at this width. */
    fun bodyHeight(message: String, width: Double): Double? {
        if (abs(width - bodyWidth) > 0.5) return null
        return bodyHeights[message]
    }

    /** The app came back: the badge and the notifications may be stale. */
    fun cameBack() {
        if (!booted) return
        scope.launch {
            reloadMailboxes()
            Notifier.settle(null)
        }
        updater.checkIfDue()
    }

    /** Asks the server to check every account now; returns once it has, or gave up. */
    suspend fun checkMail() {
        runCatching { bridge.call("refresh") }
    }

    suspend fun refresh() {
        runCatching { bridge.call("status") { Status.read(it.objectOrEmpty) } }.getOrNull()?.let { status ->
            signedIn = status.signedIn
            connection = status.connection
            server = status.server ?: ""
        }
        reloadMailboxes()
        reloadThreads()
    }

    private fun handle(event: CoreEvent) {
        when (event) {
            is CoreEvent.Changed -> {
                prefetched.removeAll(event.threads.toSet())
                if (event.preferences) preferencesChanged()
                if (event.threads.isNotEmpty()) scope.launch { Notifier.settle(event.threads) }
                scope.launch {
                    if (event.mailboxes) reloadMailboxes()
                    reloadThreads()
                    val open = conversation?.id
                    if (open != null && open in event.threads) reopen()
                    if (peeks.isNotEmpty()) peekAround(event.threads)
                    val arriving = arrivingThread
                    if (arriving != null && arriving in event.threads && selected == arriving) open(arriving)
                }
            }
            is CoreEvent.Connection -> {
                connection = event.state
                signedIn = event.state != "signed_out"
                if (event.state == "signed_out") {
                    conversation = null
                    rows = emptyList()
                    listedUnified = emptyList()
                    listedAccounts = emptyList()
                    selection = emptySet()
                    Notifier.signedOut()
                }
            }
            is CoreEvent.Sent -> show("Sent.")
            is CoreEvent.SendFailed -> show(event.error, "Edit") {
                compose = Compose(event.draft, "", event.draft.cc.isNotEmpty(), event.draftId, keep = true)
            }
            is CoreEvent.SearchResults -> {
                if (event.request != searchRequest || searchRows == null) return
                searchRows = event.rows
            }
            is CoreEvent.NewMail -> Notifier.notify(event.messages, this)
            is CoreEvent.Error -> show(event.message)
        }
    }

    suspend fun reloadMailboxes() {
        val found = runCatching { bridge.call("mailboxes") { Mailboxes.read(it.objectOrEmpty) } }.getOrNull() ?: return
        if (found.unified != listedUnified) listedUnified = found.unified
        if (found.accounts != listedAccounts) listedAccounts = found.accounts
        Notifier.unread = inboxUnread
        if (accounts.isNotEmpty()) mailArrives()
    }

    private suspend fun page(offset: Int, limit: Int): ThreadPage? {
        val fields = mapOf("mailbox" to mailbox, "offset" to offset, "limit" to limit, "filter" to filter?.id, "keep" to keptRows)
        return runCatching { bridge.call("threads", fields) { ThreadPage.read(it.objectOrEmpty) } }.getOrNull()
    }

    /** In Unread or with its filter, the threads on screen, which stay there while they are in the mailbox. */
    val keptRows: List<String>?
        get() {
            if (!(inUnread || filter == Filter.Unread) || !listReady) return null
            return rows.map { it.id }
        }

    /** Reads the list again. What didn't change isn't set, so the views it would redraw don't. */
    suspend fun reloadThreads() {
        if (reloading) {
            staleWhileReloading = true
            return
        }
        reloading = true
        do {
            staleWhileReloading = false
            val asked = mailbox to filter
            val page = page(0, max(200, rows.size))
            if (page == null || asked != (mailbox to filter)) continue
            val oldIndex = selected?.let { id -> rows.indexOfFirst { it.id == id }.takeIf { it >= 0 } }
            if (page.rows != rows) rows = page.rows
            if (page.total != total) total = page.total
            if (page.splits != splits) splits = page.splits
            if (!listReady) listReady = true
            if (searchRows != null) continue
            if (selection.isNotEmpty()) {
                val kept = selection.intersect(rows.map { it.id }.toSet())
                if (kept != selection) selection = kept
            }
            val current = selected
            if (current != null && oldIndex != null && rows.none { it.id == current }) {
                selected = if (rows.isEmpty()) null else rows[min(oldIndex, rows.size - 1)].id
            }
        } while (staleWhileReloading)
        reloading = false
        if (!moreWhileReloading) return
        moreWhileReloading = false
        loadMore()
    }

    fun loadMore() {
        if (rows.size >= total) return
        if (reloading) {
            moreWhileReloading = true
            return
        }
        reloading = true
        loadingMore = true
        val asked = mailbox to filter
        val offset = rows.size
        scope.launch {
            val page = page(offset, 200)
            if (page != null && asked == (mailbox to filter) && offset == rows.size) {
                rows = rows + page.rows
                total = page.total
            }
            reloading = false
            loadingMore = false
            moreWhileReloading = false
            if (staleWhileReloading) reloadThreads()
        }
    }

    fun select(mailbox: String) {
        this.mailbox = mailbox
        conversation = null
        searchRows = null
        searchQuery = ""
        listReady = false
        selection = emptySet()
        selectionAnchor = null
        selected = null
        listOffset = 0.0
        scope.launch { reloadThreads() }
    }

    /** One of the mailboxes every account has (`inbox`, `sent`, ...), in the account on screen when one is. */
    fun go(kind: String) = select(listAccount?.let { "$it/$kind" } ?: kind)

    /** Shows only unread or starred threads, or everything again. */
    fun toggleFilter(filter: Filter) = narrow(if (this.filter == filter) null else filter)

    fun narrow(filter: Filter?) {
        if (filter == this.filter) return
        this.filter = filter
        selection = emptySet()
        listReady = false
        listOffset = 0.0
        scope.launch { reloadThreads() }
    }

    /** The next or previous tab of a split inbox. */
    fun moveSplit(step: Int) {
        if (splits.isEmpty()) return
        val index = splits.indexOfFirst { it.mailbox == mailbox }.coerceAtLeast(0)
        val next = splits[(index + step + splits.size) % splits.size].mailbox
        if (next == mailbox) return
        select(next)
    }

    // Threads

    fun row(thread: String): ThreadRow? = visibleRows.firstOrNull { it.id == thread }

    fun open(thread: String) = open(thread, arriving = false)

    /** One `arriving` may not be on the device yet: it opens once its mail syncs. */
    private fun open(thread: String, arriving: Boolean) {
        selected = thread
        arrivingThread = null
        row(thread)?.draftId?.let {
            openDraft(it)
            return
        }
        if (conversation?.id != thread) {
            unfoldedMessages = emptySet()
            foldedMessages = emptySet()
            originalMessages = emptySet()
            focusedMessage = null
            threadOffset = null
            bodyHeights = emptyMap()
        }
        scope.launch {
            try {
                conversation = openThread(thread)
            } catch (error: CoreError) {
                if (!arriving || selected != thread) {
                    show(error.message)
                    return@launch
                }
                arrivingThread = thread
            }
        }
    }

    private suspend fun openThread(thread: String): Conversation =
        bridge.call("open_thread", mapOf("thread" to thread, "images" to (thread in imagesShown))) { Conversation.read(it.objectOrEmpty) }

    /** Opens the thread of a notification, whose mail may not have synced yet. */
    fun show(thread: String) {
        open(thread, arriving = true)
        requestedThread = thread
    }

    private suspend fun reopen() {
        val open = conversation?.id ?: return
        val fresh = runCatching { openThread(open) }.getOrNull() ?: return
        if (conversation?.id != open) return
        conversation = fresh
    }

    fun showImages() {
        val open = conversation?.id ?: return
        imagesShown = imagesShown + open
        scope.launch { reopen() }
    }

    /** Whether designed mail is turned dark in the dark scheme. */
    val darkMail: Boolean get() = preferences["dark_mail"]?.flag ?: true

    fun toggleOriginal(message: String) {
        originalMessages = if (message in originalMessages) originalMessages - message else originalMessages + message
    }

    fun close() {
        conversation = null
        peeks = emptyMap()
    }

    /** The thread an action from the keyboard or a bar is for: the open one, else the selected one. */
    val current: String? get() = conversation?.id ?: selected

    /** The threads an action is for: the selection over the list, else the current thread. */
    val targets: List<String>
        get() {
            if (conversation != null || selection.isEmpty()) return listOfNotNull(current)
            return visibleRows.map { it.id }.filter { it in selection }
        }

    /** The threads an action on a row is for: the whole selection when the row is in it. */
    fun targets(thread: String): List<String> = if (thread in selection) targets else listOf(thread)

    fun act(action: Action, given: List<String>, until: Long? = null, label: String? = null) {
        val threads = given.filter { !isDraft(it) }
        if (threads.isEmpty()) return
        val leaving = setOf(Action.Archive, Action.Trash, Action.Spam, Action.Snooze, Action.Inbox, Action.Move, Action.Mute, Action.Block)
        if (action in leaving) {
            stepAway(threads.toSet())
            selection = selection - threads.toSet()
        }
        val fields = mapOf("action" to action.id, "threads" to threads, "until" to until, "label" to label)
        scope.launch {
            try {
                report(bridge.call("act", fields) { ActReply.read(it.objectOrEmpty) })
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Shows what an action did, with Undo when the core can take it back. */
    fun report(reply: ActReply) {
        reply.url?.let { Platform.open(context, it) }
        val message = reply.message ?: return
        if (!reply.undo) return show(message)
        show(message, "Undo") { undo() }
    }

    /** Takes back the last action that could be. */
    fun undo() {
        scope.launch {
            try {
                show(bridge.call("undo").string("message"))
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    fun toggleStar(threads: List<String>) = act(if (allStarred(threads)) Action.Unstar else Action.Star, threads)

    fun toggleRead(threads: List<String>) {
        val unread = anyUnread(threads)
        act(if (unread) Action.Read else Action.Unread, threads)
        val open = conversation?.id
        if (!unread && open != null && open in threads) conversation = null
    }

    fun allStarred(threads: List<String>): Boolean {
        val open = conversation
        if (threads.size == 1 && open?.id == threads[0]) return open.starred
        val wanted = threads.toSet()
        val found = visibleRows.filter { it.id in wanted }
        return found.isNotEmpty() && found.all { it.starred }
    }

    fun anyUnread(threads: List<String>): Boolean {
        val wanted = threads.toSet()
        return visibleRows.any { it.id in wanted && it.unread }
    }

    /** The row after the given one that isn't leaving with it, else the one before. */
    private fun neighbour(thread: String, leaving: Set<String>): String? {
        val list = visibleRows
        val index = list.indexOfFirst { it.id == thread }
        if (index < 0) return null
        list.drop(index + 1).firstOrNull { it.id !in leaving }?.let { return it.id }
        return list.take(index).lastOrNull { it.id !in leaving }?.id
    }

    /** Goes back to the list from threads that leave it, with the next one under the keyboard. */
    private fun stepAway(leaving: Set<String>) {
        val opened = conversation?.id
        val current = selected
        if (opened != null && opened in leaving) {
            val next = neighbour(opened, leaving)
            conversation = null
            selected = next
        } else if (current != null && current in leaving) {
            selected = neighbour(current, leaving)
        }
    }

    /** Moves the keyboard's selection, or the open thread, by `step` rows. */
    fun move(step: Int) {
        val list = visibleRows
        if (list.isEmpty()) return
        val index = current?.let { id -> list.indexOfFirst { it.id == id }.takeIf { it >= 0 } } ?: if (step > 0) -1 else list.size
        val nextIndex = (index + step).coerceIn(0, list.size - 1)
        val next = list[nextIndex].id
        if (conversation != null) open(next) else selected = next
        readyAround(nextIndex, list)
    }

    /** The thread beside `thread` in the list, under it with a `step` of 1, over it with -1. Drafts are passed over. */
    fun sibling(step: Int, thread: String): String? {
        val list = visibleRows
        val index = list.indexOfFirst { it.id == thread }
        if (index < 0) return null
        val past = if (step > 0) list.drop(index + 1) else list.take(index).reversed()
        return past.firstOrNull { it.draftId == null }?.id
    }

    /** The threads over and under the open one. */
    val sides: List<String>
        get() {
            val open = conversation?.id ?: return emptyList()
            return listOf(-1, 1).mapNotNull { sibling(it, open) }
        }

    /** Opens a thread beside the open one, as a swipe across it does. */
    fun turn(thread: String) {
        open(thread)
        val list = visibleRows
        val index = list.indexOfFirst { it.id == thread }
        if (index < 0) return
        readyAround(index, list)
    }

    /** Reads the threads beside the open one without marking them read. Those read before and not `changed` are kept. */
    suspend fun peekAround(changed: List<String> = emptyList()) {
        val next = HashMap<String, Conversation>()
        for (thread in sides) {
            val kept = peeks[thread]
            if (kept != null && thread !in changed) {
                next[thread] = kept
                continue
            }
            val fields = mapOf("thread" to thread, "images" to (thread in imagesShown), "peek" to true)
            runCatching { bridge.call("open_thread", fields) { Conversation.read(it.objectOrEmpty) } }.getOrNull()?.let { next[thread] = it }
        }
        if (conversation == null) return
        peeks = next
    }

    /** Fetches the bodies of the rows around one about to be opened, and the next page near the end. */
    private fun readyAround(index: Int, list: List<ThreadRow>) {
        prefetch(list.subList(max(index - 2, 0), min(index + 5, list.size)).map { it.id })
        if (index >= list.size - 20) loadMore()
    }

    /** Asks the core for the bodies of threads about to be opened. */
    fun prefetch(threads: List<String>) {
        val fresh = threads.filter { it !in prefetched }
        if (fresh.isEmpty()) return
        prefetched += fresh
        bridge.send("prefetch", mapOf("threads" to fresh))
    }

    // Writing

    /** A new message from the first account. */
    fun newMessage() = startDraft(emptyList())

    /** A new message to someone, from the contact page. */
    fun write(address: Address) = startDraft(listOf(address))

    private fun startDraft(to: List<Address>) {
        scope.launch {
            try {
                val made = bridge.call("new_draft") { DraftReply.read(it.objectOrEmpty) }
                compose = Compose(made.draft.copy(to = to), made.from, false)
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Answers `message`, else the thread's last message from someone else. */
    fun reply(kind: ReplyKind, thread: String? = null, message: String? = null) {
        val target = thread ?: current ?: return
        val fields = mapOf("thread" to target, "kind" to kind.id, "message" to message)
        scope.launch {
            try {
                val reply = bridge.call("reply_draft", fields) { DraftReply.read(it.objectOrEmpty) }
                compose = Compose(reply.draft, reply.from, reply.draft.cc.isNotEmpty(), thread = target)
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Opens a saved draft to write on. */
    fun openDraft(id: String) {
        scope.launch {
            try {
                val opened = bridge.call("open_draft", mapOf("draft_id" to id)) { DraftReply.read(it.objectOrEmpty) }
                val thread = if (conversation?.draftId == id) conversation?.id else null
                val showCc = opened.draft.cc.isNotEmpty() || opened.draft.bcc.isNotEmpty()
                compose = Compose(opened.draft, opened.from, showCc, opened.id ?: id, thread)
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Keeps what a compose sheet holds, after any save of it still on its way, and answers the saved draft's id. */
    suspend fun saveDraft(compose: Compose): String? {
        val previous = draftSaves[compose.id]
        val save = scope.async {
            val id = previous?.await() ?: compose.draftId
            val fields = mapOf("draft" to compose.draft.toJson(), "draft_id" to id)
            runCatching { bridge.call("save_draft", fields).optionalString("id") }.getOrNull() ?: id
        }
        draftSaves[compose.id] = save
        return save.await()
    }

    /** Closes the compose sheet. What was written is kept as a draft. */
    fun closeCompose(compose: Compose, keep: Boolean) {
        this.compose = null
        scope.launch {
            if (keep) {
                saveDraft(compose)
                show("Draft saved.")
            }
            draftSaves.remove(compose.id)
        }
    }

    fun discardDraft(compose: Compose) {
        this.compose = null
        scope.launch {
            val id = draftSaves[compose.id]?.await() ?: compose.draftId
            draftSaves.remove(compose.id)
            if (id != null) runCatching { bridge.call("delete_draft", mapOf("draft_id" to id)) }
            show("Draft discarded.")
        }
    }

    /** Sends after the undo delay, or at `sendAt`; archives the thread it answers with `archive`. */
    fun send(compose: Compose, sendAt: Long? = null, archive: Boolean = false) {
        this.compose = null
        val thread = compose.thread
        if (archive && thread != null) act(Action.Archive, listOf(thread))
        scope.launch {
            val draftId = draftSaves[compose.id]?.await() ?: compose.draftId
            draftSaves.remove(compose.id)
            val fields = mapOf(
                "draft" to compose.draft.toJson(), "delay" to undoDelay, "send_at" to sendAt, "remind_at" to compose.remindAt?.until,
                "draft_id" to draftId,
            )
            try {
                val opId = bridge.call("send", fields).string("op_id")
                if (sendAt != null) {
                    val time = DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(sendAt))
                    show("Sending $time.", "Undo") { undoSend(opId) }
                } else if (undoDelay > 0) {
                    show("Sending", "Undo", undoDelay * 1000L) { undoSend(opId) }
                }
            } catch (error: CoreError) {
                show(error.message)
                this@MailStore.compose = compose.copy(draftId = draftId)
            }
        }
    }

    private fun undoSend(opId: String) {
        scope.launch {
            try {
                val cancelled = bridge.call("cancel_send", mapOf("op_id" to opId))
                val draft = Draft.read(cancelled.obj("draft") ?: EmptyJSON)
                val from = draft.from?.email ?: accounts.firstOrNull { it.id == draft.accountId }?.address ?: ""
                compose = Compose(draft, from, draft.cc.isNotEmpty(), cancelled.optionalString("id"), keep = true)
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** The people an address field could mean, best first. */
    suspend fun contacts(query: String): List<Address> =
        runCatching { bridge.call("contacts", mapOf("query" to query)) { it.objectOrEmpty.objects("contacts").map(Address::read) } }
            .getOrDefault(emptyList())

    /** An attachment as a file on this device, downloaded first if it has to be. */
    suspend fun openAttachment(message: String, index: Int): File? {
        val key = "$message/$index"
        downloading = downloading + key
        try {
            return File(bridge.call("open_attachment", mapOf("message" to message, "index" to index)).string("path"))
        } catch (error: CoreError) {
            show(error.message)
            return null
        } finally {
            downloading = downloading - key
        }
    }

    suspend fun person(email: String): Person? =
        runCatching { bridge.call("person", mapOf("email" to email)) { Person.read(it.objectOrEmpty) } }.getOrNull()

    /** Whether a message of the open thread shows only its line: older read ones start so. */
    fun isFolded(message: MessageItem): Boolean =
        message.id in foldedMessages || (message.folded && message.id !in unfoldedMessages)

    fun toggleFold(message: MessageItem) {
        if (!isFolded(message)) {
            foldedMessages = foldedMessages + message.id
            unfoldedMessages = unfoldedMessages - message.id
            return
        }
        foldedMessages = foldedMessages - message.id
        unfoldedMessages = unfoldedMessages + message.id
    }

    /** Whether any message of the open thread is folded: then all unfold, else all fold. */
    val anyMessageFolded: Boolean get() = conversation?.messages?.any(::isFolded) ?: false

    /** Unfolds every message of the open thread, or folds them all when none is folded. */
    fun toggleAllMessages() {
        val ids = conversation?.messages?.map { it.id } ?: return
        if (!anyMessageFolded) {
            foldedMessages = ids.toSet()
            unfoldedMessages = emptySet()
            return
        }
        foldedMessages = emptySet()
        unfoldedMessages = unfoldedMessages + ids
    }

    /** Goes to the next (1) or the previous (-1) message of the open thread, unfolding it. */
    fun moveMessage(step: Int) {
        val messages = conversation?.messages ?: return
        if (messages.isEmpty()) return
        val index = focusedMessage?.let { id -> messages.indexOfFirst { it.id == id }.takeIf { it >= 0 } }
            ?: if (step > 0) -1 else messages.size
        val next = messages[(index + step).coerceIn(0, messages.size - 1)]
        unfoldedMessages = unfoldedMessages + next.id
        foldedMessages = foldedMessages - next.id
        focusedMessage = next.id
    }

    // Preferences

    /** Reads the synced preferences again: when one changed, or when a view needs them. */
    fun preferencesChanged() {
        scope.launch { loadPreferences() }
    }

    suspend fun loadPreferences() {
        val list = runCatching { bridge.call("preferences") { readPreferences(it.objectOrEmpty) } }.getOrNull() ?: return
        preferences = list
        preferencesLoaded = true
    }

    /** Changes a preference here at once and on the user's other devices after. Null removes it. */
    fun setPreference(key: String, value: JsonElement?) {
        preferences = if (value == null || value is JsonNull) preferences - key else preferences + (key to value)
        scope.launch {
            try {
                bridge.call("set_preference", mapOf("key" to key, "value" to (value ?: JsonNull)))
                if (key == "remote_images" || key == "dark_mail") reopen()
            } catch (error: CoreError) {
                show(error.message)
                loadPreferences()
            }
        }
    }

    /** What goes under a message from an identity: the user's own signature, else the provider's. */
    fun signature(account: String, email: String?): String? {
        preferences["signature:$account"]?.text?.takeIf { it.isNotEmpty() }?.let { return it }
        val identities = accounts.firstOrNull { it.id == account }?.identities ?: emptyList()
        val identity = identities.firstOrNull { it.email == email } ?: identities.firstOrNull()
        return identity?.signature?.takeIf { it.isNotEmpty() }
    }

    fun changeUndoDelay(seconds: Int) {
        undoDelay = seconds
        local.edit { putInt("undoDelay", seconds) }
    }

    fun changeAppearance(appearance: Appearance) {
        this.appearance = appearance
        local.edit { putString("appearance", appearance.name) }
    }

    // Search

    fun search(query: String) {
        searchQuery = query
        if (query.isBlank()) {
            searchRows = null
            return
        }
        scope.launch { searchAgain() }
    }

    /** Runs the current search, over the account on screen when there is one, for the server's newest matches too. */
    suspend fun searchAgain() {
        val query = searchQuery
        if (query.isBlank()) return
        val found = runCatching { bridge.call("search", mapOf("query" to query, "account" to listAccount)) }.getOrNull() ?: return
        if (searchQuery != query) return
        searchRequest = found.long("request")
        searchRows = ThreadRow.list(found, "rows")
    }

    fun endSearch() {
        searchQuery = ""
        searchRows = null
    }

    // Accounts

    fun signInWithGoogle() {
        scope.launch {
            try {
                Platform.signIn(context, bridge.call("sign_in_google").string("url"))
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Finishes a sign-in from the `wonnet://auth` URL the browser came back with. */
    fun finishSignIn(url: String) {
        scope.launch {
            try {
                bridge.call("finish_sign_in", mapOf("url" to url))
                refresh()
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    suspend fun addJmap(url: String, username: String, password: String) {
        bridge.call("add_jmap_account", mapOf("url" to url, "username" to username, "password" to password))
        refresh()
    }

    suspend fun setServer(url: String) {
        bridge.call("set_server", mapOf("url" to url))
        refresh()
    }

    fun removeAccount(id: String) {
        scope.launch {
            runCatching { bridge.call("remove_account", mapOf("account" to id)) }
            show("Removing the account")
        }
    }

    fun setAccountColor(id: String, color: String) {
        scope.launch {
            try {
                bridge.call("set_account_color", mapOf("account" to id, "color" to color))
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    /** Names the account, or takes its name away when `name` is empty. */
    fun setAccountName(account: AccountView, name: String) {
        val trimmed = name.trim()
        if (trimmed == account.name) return
        scope.launch {
            try {
                bridge.call("set_account_name", mapOf("account" to account.id, "name" to trimmed))
            } catch (error: CoreError) {
                show(error.message)
            }
        }
    }

    fun signOut() {
        scope.launch {
            runCatching { bridge.call("sign_out") }
            refresh()
        }
    }

    /** Gives the server this device's push token, kept by the core for every sign-in after. */
    fun setPushDevice(token: String, key: String) {
        val fields = mapOf("token" to token, "topic" to context.packageName, "sandbox" to false, "key" to key, "platform" to "fcm")
        // Once the server pushes new mail here, the app no longer tells of it itself.
        scope.launch { if (runCatching { bridge.call("set_push_device", fields) }.isSuccess) Notifier.pushing = true }
    }

    // Toasts

    fun show(message: String, action: String? = null, duration: Long = 4000, perform: (() -> Unit)? = null) {
        toast = Toast(message, action, duration)
        toastAction = perform
    }

    fun toastTapped() {
        val action = toastAction
        toast = null
        toastAction = null
        action?.invoke()
    }

    fun dismissToast(toast: Toast) {
        if (this.toast != toast) return
        this.toast = null
        toastAction = null
    }

    companion object {
        /** A saved draft's row, not a thread's: only deleting it applies. */
        fun isDraft(thread: String) = thread.startsWith("draft:")

        /** The label of a label's mailbox, `<account>/label/<label>`. */
        fun labelID(mailbox: String): String? {
            val at = mailbox.indexOf("/label/")
            if (at < 0) return null
            return mailbox.substring(at + "/label/".length)
        }
    }
}

