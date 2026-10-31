package app.wonnet.screens

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitHorizontalTouchSlopOrCancellation
import androidx.compose.foundation.gestures.horizontalDrag
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.wonnet.core.Action
import app.wonnet.core.AttachmentPreviews
import app.wonnet.core.Conversation
import app.wonnet.core.MailStore
import app.wonnet.core.MessageItem
import app.wonnet.core.Platform
import app.wonnet.core.ReplyKind
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.LocalDark
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.AttachmentCard
import app.wonnet.ui.Avatar
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Chip
import app.wonnet.ui.FoldedCount
import app.wonnet.ui.HeaderBar
import app.wonnet.ui.IconButton
import app.wonnet.ui.IconMenu
import app.wonnet.ui.MenuItem
import app.wonnet.ui.Notice
import app.wonnet.ui.PageCard
import app.wonnet.ui.Pressable
import app.wonnet.ui.Rule
import app.wonnet.ui.Sheet
import app.wonnet.ui.Spinner
import app.wonnet.ui.StarButton
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import android.content.Context
import java.io.File
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * A thread, with what can be done to it along the bottom. A swipe across it turns to the thread
 * beside it in the list, which is drawn there ahead of it: to the left the one under it, to the
 * right the one over it, as Newton's did.
 */
@Composable
fun ThreadScreenStack(store: MailStore, opened: String, dismiss: () -> Unit) {
    var thread by remember(opened) { mutableStateOf(opened) }
    val slide = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val current by rememberUpdatedState(thread)

    LaunchedEffect(store.sides) { store.peekAround() }
    LaunchedEffect(Unit) {
        // The thread is read after the page is pushed; once shown, it going away takes the page with it.
        var shown = false
        snapshotFlow { store.conversation?.id }.collect { open ->
            if (open != null) {
                shown = true
                thread = open
            } else if (shown) {
                dismiss()
            }
        }
    }
    DisposableEffect(Unit) {
        onDispose { if (store.conversation?.id == current) store.close() }
    }

    Column(Modifier.fillMaxSize().background(Tokens.background.color)) {
    Box(Modifier.statusBarsPadding()) {
        HeaderBar(leading = { IconButton(Symbol.ArrowLeft, "Back", circled = false, onClick = dismiss) })
    }
    BoxWithConstraints(Modifier.weight(1f)) {
        val width = with(density) { maxWidth.toPx() }
        val stride = width + with(density) { Space.l.toPx() }
        // The swipe outlives a composition: it reads the page's width as it is now.
        val sizes by rememberUpdatedState(width to stride)
        Box(
            Modifier
                .fillMaxSize()
                .pointerInput(Unit) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        // A swipe from the screen's edge is the system's way back.
                        if (down.position.x < 24.dp.toPx() || down.position.x > size.width - 24.dp.toPx()) return@awaitEachGesture
                        val velocity = VelocityTracker()
                        val slop = awaitHorizontalTouchSlopOrCancellation(down.id) { change, over ->
                            if (store.sibling(if (over < 0) 1 else -1, current) != null) change.consume()
                        } ?: return@awaitEachGesture
                        if (!slop.isConsumed) return@awaitEachGesture
                        var moved = 0f
                        horizontalDrag(slop.id) { change ->
                            moved += change.positionChange().x
                            velocity.addPosition(change.uptimeMillis, change.position)
                            change.consume()
                            // The pages give only a little toward a side with no thread.
                            val there = store.sibling(if (moved < 0) 1 else -1, current) != null
                            scope.launch { slide.snapTo(if (there) moved else moved * 0.15f) }
                        }
                        val (width, stride) = sizes
                        val speed = velocity.calculateVelocity().x
                        val step = if (moved < 0) 1 else -1
                        val far = abs(moved) > width * 0.3f || (abs(speed) > 400 * density.density && (speed < 0) == (moved < 0))
                        val next = store.sibling(step, current)
                        scope.launch {
                            if (moved == 0f || !far || next == null) {
                                slide.animateTo(0f, tween(250))
                                return@launch
                            }
                            slide.animateTo(-step * stride, tween(250))
                            thread = next
                            slide.snapTo(0f)
                            store.turn(next)
                        }
                    }
                },
        ) {
            for (side in listOf(-1, 0, 1)) {
                val id = if (side == 0) thread else store.sibling(side, thread) ?: continue
                key(id) {
                    val open = store.conversation
                    val conversation = if (open != null && open.id == id) open else store.peeks[id]
                    Box(
                        Modifier
                            .fillMaxSize()
                            .offset { IntOffset((side * stride + slide.value).roundToInt(), 0) },
                    ) {
                        if (side != 0 && slide.value == 0f) return@Box
                        if (conversation == null) {
                            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Spinner() }
                        } else {
                            ThreadPage(store, conversation, live = side == 0 && open?.id == id)
                        }
                    }
                }
            }
        }
        CommandBar(store, listOf(thread), Modifier.align(Alignment.BottomCenter), reply = true)
    }
    }
}

/**
 * A thread: its subject, its labels, who is in it, and its messages stacked on the card that
 * fills the page; the earlier ones folded to a line, as Newton showed them, and a long run of
 * them tucked into one line with their count, as Gmail does. It opens on the first unread message.
 */
@Composable
private fun ThreadPage(store: MailStore, conversation: Conversation, live: Boolean) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val scroll = rememberScrollState()
    val places = remember(conversation.id) { mutableStateMapOf<String, Float>() }
    var personEmail by remember { mutableStateOf<String?>(null) }
    var showsTucked by remember(conversation.id) { mutableStateOf(false) }
    val saving = rememberSaving(store)
    val firstUnread = conversation.messages.firstOrNull { it.unread }?.id

    /** The message the thread opened on, which it scrolls to itself. */
    var opensOn by remember(conversation.id) { mutableStateOf<String?>(null) }

    // The message it was left on, else the first unread one; where it was scrolled to wins over both.
    LaunchedEffect(conversation.id, live) {
        if (!live) return@LaunchedEffect
        val kept = store.focusedMessage?.takeIf { id -> conversation.messages.any { it.id == id } }
        val target = kept ?: firstUnread
        opensOn = target
        store.focusedMessage = target
        val offset = store.threadOffset
        val y = when {
            offset != null -> (offset * density.density).roundToInt()
            target == null || target == conversation.messages.firstOrNull()?.id -> return@LaunchedEffect
            else -> snapshotFlow { places[target] }.filterNotNull().first().roundToInt()
        }
        // The pages are laid out a little after the thread: it scrolls once they are tall enough.
        withTimeoutOrNull(1500) { snapshotFlow { scroll.maxValue }.first { it >= y } }
        scroll.scrollTo(y)
        opensOn = null
    }
    // The keyboard's n and p go from message to message.
    LaunchedEffect(store.focusedMessage, live) {
        val focused = store.focusedMessage ?: return@LaunchedEffect
        if (!live || focused == opensOn) return@LaunchedEffect
        places[focused]?.let { scroll.animateScrollTo(it.roundToInt()) }
    }
    LaunchedEffect(live) {
        if (!live) return@LaunchedEffect
        snapshotFlow { scroll.value }.drop(1).collect { store.noteThreadScroll((it / density.density).toDouble()) }
    }

    Box(Modifier.fillMaxSize()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(scroll).padding(bottom = Dims.barHeight + Space.xxl * 2),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            PageCard(Modifier.widthIn(max = Dims.cardWidth).fillMaxWidth().padding(vertical = Space.s)) {
                Column(Modifier.padding(horizontal = Dims.cardInset, vertical = Space.xxl)) {
                    Header(store, conversation)
                    val tucked = tucked(store, conversation, showsTucked)
                    for (message in conversation.messages) {
                        val count = tucked.second[message.id]
                        if (count != null) {
                            FoldedCount(count) { showsTucked = true }
                            Rule(color = Dims.cardRule)
                            continue
                        }
                        if (message.id in tucked.first) continue
                        Box(Modifier.onPlaced { places[message.id] = it.positionInParent().y }) {
                            MessageCard(
                                store, conversation, message,
                                showPerson = { personEmail = message.fromEmail },
                                open = { index -> scope.launch { open(context, store, message, index) } },
                                save = { indices -> scope.launch { fetch(store, message, indices)?.let(saving.start) } },
                                share = { indices -> scope.launch { fetch(store, message, indices)?.let { share(context, it) } } },
                            )
                        }
                        if (message.id != conversation.messages.last().id) Rule(color = Dims.cardRule)
                    }
                    conversation.draftId?.let { DraftCard(store, it) }
                    Row(Modifier.offset(x = -Dims.buttonGap / 2).padding(top = Space.xl + 2.dp)) {
                        IconButton(Symbol.ReplyAll, "Reply all") { store.reply(ReplyKind.ReplyAll, conversation.id) }
                        IconButton(Symbol.Reply, "Reply") { store.reply(ReplyKind.Reply, conversation.id) }
                        IconButton(Symbol.Forward, "Forward") { store.reply(ReplyKind.Forward, conversation.id) }
                    }
                }
            }
        }
    }
    personEmail?.let { email ->
        Sheet("Contact", { personEmail = null }, background = Tokens.card) {
            PersonView(store, email, closeSheet = { personEmail = null }) { thread ->
                personEmail = null
                store.show(thread)
            }
        }
    }
}

/** The messages tucked into one line: of a run of at least five folded ones, all but its first and its last two. */
private fun tucked(store: MailStore, conversation: Conversation, shown: Boolean): Pair<Set<String>, Map<String, Int>> {
    if (shown) return emptySet<String>() to emptyMap()
    val ids = HashSet<String>()
    val starts = HashMap<String, Int>()
    val run = mutableListOf<MessageItem>()
    for (message in conversation.messages + listOf(null)) {
        if (message != null && store.isFolded(message)) {
            run += message
            continue
        }
        if (run.size >= 5) {
            val middle = run.subList(1, run.size - 2)
            ids += middle.map { it.id }
            starts[middle.first().id] = middle.size
        }
        run.clear()
    }
    return ids to starts
}

@Composable
private fun Header(store: MailStore, conversation: Conversation) {
    Column(Modifier.padding(bottom = Space.xl), verticalArrangement = Arrangement.spacedBy(Space.s + 2.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(Space.m)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Space.xs)) {
                SelectionContainer { Text(conversation.subject.ifEmpty { "(no subject)" }, TextStyle.Title) }
                val muted = Tokens.mutedStrongerForeground.color
                val people = buildAnnotatedString {
                    append(conversation.from)
                    if (conversation.to.isNotEmpty()) {
                        withStyle(SpanStyle(color = muted)) { append(" to ") }
                        append(conversation.to)
                    }
                }
                Text(people, TextStyle.Label, color = Tokens.mutedForeground.color)
            }
            StarButton(conversation.starred) { store.toggleStar(listOf(conversation.id)) }
        }
        if (conversation.labels.isNotEmpty() || conversation.muted || conversation.unsubscribe) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(Space.xs), verticalArrangement = Arrangement.spacedBy(Space.xs)) {
                    for (label in conversation.labels) Chip(label.name, symbol = Symbol.Tag)
                    if (conversation.muted) Chip("Muted", symbol = Symbol.BellOff)
                }
                if (conversation.unsubscribe) {
                    ActionButton("Unsubscribe", symbol = Symbol.MailX, variant = ButtonVariant.Ghost) { store.act(Action.Unsubscribe, listOf(conversation.id)) }
                }
            }
        }
        if (conversation.messages.any { it.blockedImages }) {
            Notice("Images from the sender were left out.", action = "Load images", perform = store::showImages)
        }
    }
}

/** The reply the user started and didn't send. */
@Composable
private fun DraftCard(store: MailStore, id: String) {
    Column {
        Rule(color = Dims.cardRule)
        Pressable({ store.openDraft(id) }) {
            Row(Modifier.fillMaxWidth().padding(vertical = Space.l), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                Icon(Symbol.Pencil, 14f, Tokens.destructive.color)
                Text("Draft", TextStyle.Subheading, color = Tokens.destructive.color)
                Text("A reply you started", TextStyle.Label, Modifier.weight(1f), color = Tokens.mutedStrongerForeground.color, maxLines = 1)
                ActionButton("Open") { store.openDraft(id) }
            }
        }
    }
}

/** One message of a thread. Its sender opens the person; its files open in the app that takes their kind. */
@Composable
private fun MessageCard(
    store: MailStore,
    conversation: Conversation,
    message: MessageItem,
    showPerson: () -> Unit,
    open: (Int) -> Unit,
    save: (List<Int>) -> Unit,
    share: (List<Int>) -> Unit,
) {
    val folded = store.isFolded(message)
    val original = message.id in store.originalMessages
    val dark = LocalDark.current
    val canShowOriginal = !folded && message.designed && message.html != null && dark && store.darkMail
    val reply = { kind: ReplyKind -> store.reply(kind, conversation.id, message.id) }
    Column(Modifier.padding(vertical = Space.l), verticalArrangement = Arrangement.spacedBy(Space.m)) {
        // A tap on the line of an open message folds it again, as in Gmail; on a folded one, unfolds it.
        Pressable({ store.toggleFold(message) }) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                Pressable({ if (folded) store.toggleFold(message) else showPerson() }, help = message.fromEmail) {
                    Avatar(message.initials, message.fromEmail, Dims.messageAvatar)
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Pressable({ if (folded) store.toggleFold(message) else showPerson() }) {
                        Text(message.fromName, if (folded) TextStyle.Body else TextStyle.Subheading, color = if (folded) Tokens.mutedStrongerForeground.color else Tokens.foreground.color, maxLines = 1)
                    }
                    Text(if (folded) message.snippet else message.to, TextStyle.Caption, maxLines = 1)
                }
                if (message.attachments.isNotEmpty()) Icon(Symbol.Paperclip, 13f, Tokens.mutedStrongerForeground.color)
                Text(message.date, TextStyle.Caption, maxLines = 1)
                if (!folded) {
                    Row(Modifier.offset(x = Dims.buttonGap / 2)) {
                        if (canShowOriginal) {
                            IconButton(if (original) Symbol.Moon else Symbol.Sun, if (original) "Show in dark colours" else "Show the original", size = ControlSize.Small, circled = false, quiet = true) {
                                store.toggleOriginal(message.id)
                            }
                        }
                        IconButton(Symbol.Reply, "Reply", size = ControlSize.Small, circled = false) { reply(ReplyKind.Reply) }
                        IconMenu(
                            Symbol.Ellipsis, "More",
                            listOf(MenuItem("Reply all", Symbol.ReplyAll) { reply(ReplyKind.ReplyAll) }, MenuItem("Forward", Symbol.Forward) { reply(ReplyKind.Forward) }),
                            size = ControlSize.Small, circled = false,
                        )
                    }
                }
            }
        }
        if (folded) return@Column
        val html = message.html
        when {
            html != null -> MessageWebView(store, message.id, html, message.shape, original)
            message.failed -> Text("This message couldn't be loaded. Open the conversation again to retry.", TextStyle.Body, color = Tokens.mutedStrongerForeground.color)
            else -> app.wonnet.ui.BodyPlaceholder(message.shape)
        }
        if (message.attachments.isNotEmpty()) Attachments(store, message, open, save, share)
    }
}

/** The files under a message, as Gmail shows them: their count, Save all, and a card each, two to a line. */
@Composable
private fun Attachments(store: MailStore, message: MessageItem, open: (Int) -> Unit, save: (List<Int>) -> Unit, share: (List<Int>) -> Unit) {
    val count = message.attachments.size
    Column(Modifier.padding(top = Space.xs), verticalArrangement = Arrangement.spacedBy(Space.s)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(if (count == 1) "1 attachment" else "$count attachments", TextStyle.Caption, Modifier.weight(1f))
            if (count > 1) {
                ActionButton("Save all", Modifier.offset(x = Dims.buttonGap / 2), symbol = Symbol.Download, variant = ButtonVariant.Ghost, size = ControlSize.Small) {
                    save((0 until count).toList())
                }
            }
        }
        for (pair in message.attachments.indices.chunked(2)) {
            Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                for (index in pair) {
                    val attachment = message.attachments[index]
                    val key = AttachmentPreviews.key(message.id, index)
                    LaunchedEffect(key) { AttachmentPreviews.load(message.id, index, attachment) }
                    Box(Modifier.weight(1f)) {
                        var menu by remember { mutableStateOf(false) }
                        AttachmentCard(
                            attachment.name, attachment.mime, attachment.size, AttachmentPreviews.previews[key],
                            pending = "${message.id}/$index" in store.downloading, onLongClick = { menu = true },
                        ) { open(index) }
                        app.wonnet.ui.Menu(menu, { menu = false }, listOf(
                                MenuItem("Open", Symbol.File) { open(index) }, MenuItem("Save", Symbol.Download) { save(listOf(index)) },
                                MenuItem("Share", Symbol.Share) { share(listOf(index)) },
                            ),
                        )
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

private suspend fun open(context: android.content.Context, store: MailStore, message: MessageItem, index: Int) {
    val file = store.openAttachment(message.id, index) ?: return
    if (!Platform.openFile(context, file, message.attachments[index].mime)) store.show("No app here opens ${message.attachments[index].name}.")
    AttachmentPreviews.load(message.id, index, file, message.attachments[index].mime)
}

/** The attachments' files, downloaded first where they have to be, with the names they go by. */
private suspend fun fetch(store: MailStore, message: MessageItem, indices: List<Int>): List<Pair<File, String>>? {
    val files = mutableListOf<Pair<File, String>>()
    for (index in indices) {
        val file = store.openAttachment(message.id, index) ?: return null
        files += file to message.attachments[index].name.ifEmpty { file.name }
    }
    return files
}

/** Copies under their own names, handed to the share sheet to send them elsewhere. */
private suspend fun share(context: Context, files: List<Pair<File, String>>) {
    val copies = withContext(Dispatchers.IO) {
        files.map { (file, name) ->
            val folder = File(context.cacheDir, "saving/${UUID.randomUUID()}").apply { mkdirs() }
            file.copyTo(File(folder, name), overwrite = true)
        }
    }
    Platform.share(context, copies)
}

/** Saving attachments where the user picks, as the system's file picker asks: one under its name, several into a folder. */
private class Saving(val start: (List<Pair<File, String>>) -> Unit)

@Composable
private fun rememberSaving(store: MailStore): Saving {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var files by remember { mutableStateOf<List<Pair<File, String>>>(emptyList()) }
    fun saved(write: suspend () -> Unit) {
        scope.launch {
            try {
                write()
                store.show(if (files.size == 1) "Saved." else "${files.size} files saved.")
            } catch (_: Exception) {
                store.show("The files couldn't be saved there.")
            }
        }
    }
    val one = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { target ->
        val file = files.firstOrNull() ?: return@rememberLauncherForActivityResult
        if (target != null) saved { Platform.saveTo(context, file.first, target) }
    }
    val many = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { folder ->
        if (folder != null) saved { Platform.saveInto(context, files, folder) }
    }
    return remember {
        Saving { picked ->
            files = picked
            if (picked.size == 1) one.launch(picked[0].second) else many.launch(null)
        }
    }
}
