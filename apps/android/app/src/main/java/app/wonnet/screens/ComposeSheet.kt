package app.wonnet.screens

import android.net.Uri
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import app.wonnet.App
import app.wonnet.theme.Radius
import androidx.compose.foundation.background
import androidx.compose.foundation.content.consume
import androidx.compose.foundation.content.contentReceiver
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import app.wonnet.core.AccountView
import app.wonnet.core.Address
import app.wonnet.core.Compose
import app.wonnet.core.Identity
import app.wonnet.core.MailStore
import app.wonnet.core.Outgoing
import app.wonnet.core.Snippet
import app.wonnet.core.TimeChoice
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Chip
import app.wonnet.ui.ChoiceRow
import app.wonnet.ui.Combobox
import app.wonnet.ui.Choice
import app.wonnet.ui.ConfirmDialog
import app.wonnet.ui.Edge
import app.wonnet.ui.IconButton
import app.wonnet.ui.LocalOnCard
import app.wonnet.ui.Notice
import app.wonnet.ui.PopupCard
import app.wonnet.ui.Rule
import app.wonnet.ui.Sheet
import app.wonnet.ui.rule
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/** An address a message can be from: one of an account's identities, the first its own. */
private data class Sender(val account: AccountView, val identity: Identity, val first: Boolean) {
    val id get() = account.id + "\n" + identity.email

    /** The sender as the From field offers it: its name, then its address, or its account's name when it has none. */
    fun choice(color: androidx.compose.ui.graphics.Color) = Choice(
        id, identity.name ?: identity.email, if (identity.name == null) account.title else identity.email, dot = color,
        keywords = "${identity.email} ${account.title}",
    )

    companion object {
        /** Every address the accounts can send as, in the sidebar's order. */
        fun all(accounts: List<AccountView>): List<Sender> = accounts.flatMap { account ->
            val identities = account.identities.ifEmpty { listOf(Identity(null, account.address, null)) }
            identities.mapIndexed { index, identity -> Sender(account, identity, index == 0) }
        }
    }
}

private enum class Timing { SendLater, Remind }

/** Something to confirm before sending. */
private data class Warning(val message: String, val at: Long?, val archive: Boolean)

/** The `;word` just before the caret: a `;` at the start of a word, then letters. Its word and where it starts and ends. */
fun snippetTrigger(text: String, caret: Int): Pair<String, IntRange>? {
    if (caret > text.length) return null
    var start = caret
    while (start > 0 && caret - start < 30) {
        val character = text[start - 1]
        if (character == ';') break
        if (!(character.isLetterOrDigit() || character == '_' || character == '-')) return null
        start--
    }
    if (start == 0 || text[start - 1] != ';') return null
    val semicolon = start - 1
    if (semicolon > 0 && !text[semicolon - 1].isWhitespace()) return null
    return text.substring(start, caret) to (semicolon until caret)
}

/**
 * Writing a message: who it goes to and from, its subject, its text with the signature and the
 * quote under it, and its files. It is kept as a draft as it is written. The bar on top has Send
 * alone; everything else about the message is in the toolbar under it. Sending waits for the undo
 * delay, or for a time picked to send it at.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ComposeSheet(store: MailStore, start: Compose) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var compose by remember { mutableStateOf(start) }
    var to by remember { mutableStateOf(start.typing?.to ?: "") }
    var cc by remember { mutableStateOf(start.typing?.cc ?: "") }
    var bcc by remember { mutableStateOf(start.typing?.bcc ?: "") }
    var kept by remember { mutableStateOf(start.draft) }
    var edited by remember { mutableStateOf(false) }
    var closing by remember { mutableStateOf(false) }
    var showQuote by remember { mutableStateOf(false) }
    var timing by remember { mutableStateOf<Timing?>(null) }
    var pickingSnippet by remember { mutableStateOf(false) }
    var revealed by remember { mutableStateOf<String?>(null) }
    var trigger by remember { mutableStateOf<String?>(null) }
    var triggerIndex by remember { mutableIntStateOf(0) }
    var warning by remember { mutableStateOf<Warning?>(null) }
    var discarding by remember { mutableStateOf(false) }
    var notice by remember { mutableStateOf<String?>(null) }
    val body = rememberTextFieldState(start.draft.text)
    val bodyFocus = remember { FocusRequester() }
    val subjectFocus = remember { FocusRequester() }

    val assembled = compose.copy(
        draft = compose.draft.copy(
            text = body.text.toString(),
            to = compose.draft.to + Address.typed(to), cc = compose.draft.cc + Address.typed(cc), bcc = compose.draft.bcc + Address.typed(bcc),
        ),
    )
    val live = compose.copy(draft = compose.draft.copy(text = body.text.toString()), typing = Compose.Typing(to, cc, bcc))
    val senders = Sender.all(store.accounts)
    val sender = senders.firstOrNull { option ->
        if (option.account.id != compose.draft.accountId) return@firstOrNull false
        val from = compose.draft.from ?: return@firstOrNull option.first
        option.identity.email.equals(from.email, ignoreCase = true)
    }
    val signature = store.signature(compose.draft.accountId, sender?.identity?.email)
    val cannotSend = assembled.draft.to.isEmpty() && assembled.draft.cc.isEmpty() && assembled.draft.bcc.isEmpty()
    val title = when {
        compose.draft.inReplyTo != null -> "Reply"
        compose.draft.forwardAttachmentsOf != null || compose.draft.subject.startsWith("Fwd:") -> "Forward"
        else -> "New message"
    }
    val matches = trigger?.let { word ->
        val all = store.snippetsList()
        if (word.isEmpty()) all else all.filter { it.name.contains(word, ignoreCase = true) }
    } ?: emptyList()

    // The draft is kept a little after each change, and the store hears of what is written.
    val latest by rememberUpdatedState(assembled)
    LaunchedEffect(live) { store.noteCompose(live) }
    LaunchedEffect(assembled.draft) {
        if (assembled.draft == kept) return@LaunchedEffect
        edited = true
        delay(2000)
        if (closing) return@LaunchedEffect
        kept = latest.draft
        compose = compose.copy(draftId = store.saveDraft(latest))
    }
    LaunchedEffect(Unit) {
        if (!store.preferencesLoaded) store.loadPreferences()
        if (start.draft.to.isNotEmpty()) {
            delay(200)
            runCatching { bodyFocus.requestFocus() }
        }
    }
    LaunchedEffect(body) {
        snapshotFlow { body.text.toString() to body.selection }.drop(1).collect { (text, selection) ->
            trigger = if (selection.collapsed) snippetTrigger(text, selection.start)?.first else null
            triggerIndex = 0
        }
    }

    fun close() {
        closing = true
        store.closeCompose(latest, keep = edited || compose.keep || compose.draftId != null)
    }

    fun add(files: List<app.wonnet.core.DraftAttachment>) {
        var total = compose.draft.attachments.sumOf { it.size }
        val added = compose.draft.attachments.toMutableList()
        for (file in files) {
            if (total + file.size > Outgoing.LIMIT) {
                notice = "${file.name} wasn't added: attachments can be 25 MB together at most."
                continue
            }
            total += file.size
            added += file
        }
        compose = compose.copy(draft = compose.draft.copy(attachments = added))
    }

    fun stage(uris: List<Uri>) {
        if (uris.isEmpty()) return
        scope.launch { add(Outgoing.stage(context, uris)) }
    }

    fun finish(at: Long?, archive: Boolean) {
        closing = true
        store.send(latest, at, archive)
    }

    /** Checks the message, asks about what looks forgotten, then sends. */
    fun send(at: Long? = null, archive: Boolean = false) {
        val draft = latest.draft
        val everyone = draft.to + draft.cc + draft.bcc
        if (everyone.isEmpty()) {
            notice = "Add who it goes to."
            return
        }
        everyone.firstOrNull { !it.isValid }?.let {
            notice = "“${it.email}” isn't an address."
            return
        }
        notice = null
        val worries = mutableListOf<String>()
        val placeholders = Outgoing.placeholders(draft.text)
        if (placeholders.isNotEmpty()) worries += "${placeholders.joinToString(", ")} still ${if (placeholders.size == 1) "needs" else "need"} filling in."
        if (draft.attachments.isEmpty() && draft.forwardAttachmentsOf == null && Outgoing.mentionsAttachment(draft.text)) {
            worries += "It mentions an attachment, but nothing is attached."
        }
        if (worries.isNotEmpty()) {
            warning = Warning(worries.joinToString(" "), at, archive)
            return
        }
        finish(at, archive)
    }

    /** Puts a snippet in at the caret, or in place of the `;word` being typed, with the first recipient's first name. */
    fun put(snippet: Snippet, replacingTrigger: Boolean) {
        val text = snippet.filled(latest.draft.to.firstOrNull()?.firstName)
        trigger = null
        val current = body.text.toString()
        val caret = body.selection.start
        val range = if (replacingTrigger) snippetTrigger(current, caret)?.second ?: (caret until caret) else (body.selection.min until body.selection.max)
        body.edit {
            replace(range.first, range.last + 1, text)
            selection = TextRange(range.first + text.length)
        }
        scope.launch {
            delay(50)
            runCatching { bodyFocus.requestFocus() }
        }
    }

    /** Moves whoever made the introduction to Bcc and thanks them first thing. */
    fun instantIntro() {
        val draft = latest.draft
        if (draft.to.isEmpty()) return
        val introducer = draft.to.first()
        val thanks = introducer.firstName?.let { "Thanks for the intro, $it! Moving you to Bcc." } ?: "Thanks for the intro! Moving you to Bcc."
        compose = compose.copy(
            draft = draft.copy(to = draft.to.drop(1) + draft.cc, cc = emptyList(), bcc = draft.bcc + introducer, text = thanks + "\n\n" + draft.text),
            showCc = true,
        )
        body.setTextAndPlaceCursorAtEnd(thanks + "\n\n" + draft.text)
        to = ""
        cc = ""
        bcc = ""
    }

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { stage(it) }
    val pickingSender = remember { mutableStateOf(false) }
    var dropping by remember { mutableStateOf(false) }

    /** Files dropped from another app beside this one are attached. */
    val drop = remember {
        object : DragAndDropTarget {
            override fun onEntered(event: DragAndDropEvent) {
                dropping = true
            }

            override fun onExited(event: DragAndDropEvent) {
                dropping = false
            }

            override fun onEnded(event: DragAndDropEvent) {
                dropping = false
            }

            override fun onDrop(event: DragAndDropEvent): Boolean {
                dropping = false
                App.activity?.requestDragAndDropPermissions(event.toAndroidDragEvent())
                stage(event.uris())
                return true
            }
        }
    }

    /** A hardware keyboard's shortcuts, with Ctrl for the Mac's ⌘. */
    fun shortcut(event: androidx.compose.ui.input.key.KeyEvent): Boolean {
        if (event.type != KeyEventType.KeyDown || !event.isCtrlPressed) return false
        val shift = event.isShiftPressed
        when {
            event.key == Key.Enter && shift -> if (compose.thread != null && !cannotSend) send(archive = true)
            event.key == Key.Enter -> if (!cannotSend) send()
            event.key == Key.U && shift -> picker.launch(arrayOf("*/*"))
            event.key == Key.Semicolon -> pickingSnippet = true
            event.key == Key.L && shift -> timing = Timing.SendLater
            event.key == Key.H && shift -> timing = Timing.Remind
            event.key == Key.I && shift -> if (compose.draft.inReplyTo != null) instantIntro()
            event.key == Key.Comma && shift -> discarding = true
            event.key == Key.W -> close()
            event.key == Key.C && shift -> {
                revealed = "Cc"
                compose = compose.copy(showCc = true)
            }
            event.key == Key.B && shift -> {
                revealed = "Bcc"
                compose = compose.copy(showCc = true)
            }
            event.key == Key.F && shift -> if (senders.size > 1) pickingSender.value = true
            else -> return false
        }
        return true
    }

    // Gone without Send or Cancel, as when the app's process ends: kept as when closed.
    DisposableEffect(Unit) {
        onDispose { if (!closing && store.compose?.id == start.id) store.closeCompose(latest, edited || compose.keep || compose.draftId != null) }
    }

    Sheet(title, ::close, close = "Cancel", background = Dims.cardFill, trailing = {
        ActionButton("Send", symbol = Symbol.Send, variant = ButtonVariant.Primary, size = ControlSize.Small, enabled = !cannotSend) { send() }
    }) {
        CompositionLocalProvider(LocalOnCard provides true) {
            Box(Modifier.weight(1f).onPreviewKeyEvent(::shortcut).dragAndDropTarget({ it.uris().isNotEmpty() }, drop)) {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    notice?.let {
                        Notice(it, Modifier.padding(horizontal = Dims.cardInset, vertical = Space.m), error = true, symbol = Symbol.CircleAlert)
                        Rule(Modifier.padding(horizontal = Dims.cardInset), color = Dims.cardRule)
                    }
                    AddressField(store, "To", compose.draft.to, { compose = compose.copy(draft = compose.draft.copy(to = it)) }, to, { to = it }, autofocus = compose.draft.to.isEmpty()) {
                        if (!compose.showCc) {
                            ActionButton("Cc Bcc", variant = ButtonVariant.Ghost, size = ControlSize.Small) {
                                revealed = "Cc"
                                compose = compose.copy(showCc = true)
                            }
                        }
                    }
                    if (compose.showCc) {
                        AddressField(store, "Cc", compose.draft.cc, { compose = compose.copy(draft = compose.draft.copy(cc = it)) }, cc, { cc = it }, autofocus = revealed == "Cc")
                        AddressField(store, "Bcc", compose.draft.bcc, { compose = compose.copy(draft = compose.draft.copy(bcc = it)) }, bcc, { bcc = it }, autofocus = revealed == "Bcc")
                    }
                    if (senders.size > 1) {
                        FieldRow("From") {
                            Combobox(
                                "Send as", "Find an address", senders.map { it.choice(Dims.accountColor(it.account.color).color) }, sender?.id, "No address by that name.",
                                pickingSender,
                            ) { choice ->
                                val picked = senders.firstOrNull { it.id == choice.id } ?: return@Combobox
                                compose = compose.copy(
                                    draft = compose.draft.copy(accountId = picked.account.id, from = if (picked.first) null else Address(picked.identity.name, picked.identity.email)),
                                    from = picked.identity.email,
                                )
                            }
                        }
                    }
                    FieldRow("Subject", focus = { runCatching { subjectFocus.requestFocus() } }) {
                        BasicTextField(
                            compose.draft.subject, { compose = compose.copy(draft = compose.draft.copy(subject = it)) }, Modifier.weight(1f).focusRequester(subjectFocus),
                            singleLine = true, textStyle = TextStyle.Body.compose(Tokens.foreground.color), cursorBrush = SolidColor(Tokens.foreground.color),
                            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Next),
                            keyboardActions = KeyboardActions(onNext = { runCatching { bodyFocus.requestFocus() } }),
                        )
                    }
                    // The rest of the card is for writing too: a tap anywhere in it puts the caret there.
                    Column(
                        Modifier.pointerInput(Unit) { detectTapGestures { runCatching { bodyFocus.requestFocus() } } }
                            .padding(horizontal = Dims.cardInset, vertical = Space.l).fillMaxWidth(),
                        verticalArrangement = Arrangement.spacedBy(Space.l),
                    ) {
                        BodyEditor(body, bodyFocus, menuOpen = matches.isNotEmpty(), onPaste = ::stage) { key ->
                            when (key) {
                                EditorKey.Up -> triggerIndex = (triggerIndex - 1).coerceAtLeast(0)
                                EditorKey.Down -> triggerIndex = (triggerIndex + 1).coerceAtMost(matches.size - 1)
                                EditorKey.Accept -> put(matches[triggerIndex.coerceAtMost(matches.size - 1)], replacingTrigger = true)
                                EditorKey.Cancel -> trigger = null
                            }
                        }
                        signature?.let { SelectionContainer { Text(it, TextStyle.Body, color = Tokens.mutedStrongerForeground.color) } }
                        compose.draft.quote?.takeIf { it.isNotEmpty() }?.let { quote ->
                            Chip("•••", onClick = { showQuote = !showQuote })
                            if (showQuote) {
                                val rule = Dims.cardRule.color
                                SelectionContainer {
                                    Text(
                                        quote, TextStyle.Label,
                                        Modifier.drawBehind { drawRect(rule, Offset.Zero, androidx.compose.ui.geometry.Size(2.dp.toPx(), size.height)) }.padding(start = Space.m),
                                        color = Tokens.mutedStrongerForeground.color,
                                    )
                                }
                            }
                        }
                        if (compose.draft.attachments.isNotEmpty() || compose.remindAt != null) {
                            FlowRow(horizontalArrangement = Arrangement.spacedBy(Space.xs), verticalArrangement = Arrangement.spacedBy(Space.xs)) {
                                for ((index, file) in compose.draft.attachments.withIndex()) {
                                    Chip(file.name, symbol = Symbol.Paperclip, detail = Formatter.formatShortFileSize(context, file.size), remove = {
                                        compose = compose.copy(draft = compose.draft.copy(attachments = compose.draft.attachments.filterIndexed { at, _ -> at != index }))
                                    })
                                }
                                compose.remindAt?.let { remind ->
                                    Chip("Remind me ${remind.label} if nobody replies", symbol = Symbol.AlarmClock, remove = { compose = compose.copy(remindAt = null) })
                                }
                            }
                        }
                        Spacer(Modifier.height(Space.xxl))
                    }
                }
                if (dropping) {
                    val outline = Tokens.primary.color
                    Box(
                        Modifier.matchParentSize().padding(Space.xs).drawBehind {
                            drawRoundRect(
                                outline, cornerRadius = androidx.compose.ui.geometry.CornerRadius(Radius.sm.toPx()),
                                style = Stroke(2.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))),
                            )
                        },
                    )
                }
                if (matches.isNotEmpty()) {
                    PopupCard(Modifier.align(Alignment.BottomStart).padding(start = Space.l, end = Space.l, bottom = Space.xxs).widthIn(max = Dims.popupWidth)) {
                        for ((index, snippet) in matches.withIndex()) {
                            ChoiceRow(snippet.name, detail = snippet.text.replace('\n', ' '), symbol = Symbol.Zap, highlighted = index == triggerIndex) { put(snippet, true) }
                        }
                    }
                }
            }
            Rule(color = Dims.cardRule)
            Row(Modifier.fillMaxWidth().height(Dims.barHeight).padding(horizontal = Space.l), verticalAlignment = Alignment.CenterVertically) {
                IconButton(Symbol.Paperclip, "Attach files") { picker.launch(arrayOf("*/*")) }
                IconButton(Symbol.Zap, "Snippets, or type ; and a name") { pickingSnippet = true }
                IconButton(Symbol.CalendarClock, "Send later") { timing = Timing.SendLater }
                IconButton(Symbol.AlarmClock, "Remind me if nobody replies", active = compose.remindAt != null) { timing = Timing.Remind }
                if (compose.thread != null) IconButton(Symbol.Archive, "Send and archive", enabled = !cannotSend) { send(archive = true) }
                if (compose.draft.inReplyTo != null) IconButton(Symbol.Handshake, "Instant Intro: move who introduced you to Bcc", onClick = ::instantIntro)
                Spacer(Modifier.weight(1f))
                IconButton(Symbol.Trash, "Discard the draft") { discarding = true }
            }
        }
    }
    timing?.let { picked ->
        TimeSheet(
            store, if (picked == Timing.SendLater) "Send later" else "Remind me if nobody replies", "Tomorrow 9am, in 2 hours, Friday",
            "That isn't a time I can read.", { timing = null },
        ) { choice: TimeChoice ->
            timing = null
            if (picked == Timing.Remind) compose = compose.copy(remindAt = choice) else send(at = choice.until)
        }
    }
    if (pickingSnippet) {
        SnippetPicker(store.snippetsList(), { pickingSnippet = false }) { snippet ->
            pickingSnippet = false
            put(snippet, replacingTrigger = false)
        }
    }
    warning?.let { shown ->
        ConfirmDialog("Send anyway?", shown.message, "Send", { warning = null }, destructive = false, cancel = "Keep writing") {
            warning = null
            finish(shown.at, shown.archive)
        }
    }
    if (discarding) {
        ConfirmDialog("Discard this draft?", "What you wrote is thrown away.", "Discard", { discarding = false }, cancel = "Keep it") {
            discarding = false
            closing = true
            store.discardDraft(compose)
        }
    }
}

/** The files a drop carries. */
private fun DragAndDropEvent.uris(): List<Uri> {
    val clip = toAndroidDragEvent().clipData ?: return emptyList()
    return (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri }
}

private fun MailStore.snippetsList() = Snippet.all(preferences)

/** A popup under what it belongs to, from its start. */
private object Under : PopupPositionProvider {
    override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize) =
        IntOffset(anchorBounds.left, anchorBounds.bottom)
}

/** Keys the body editor hands over while the snippets' menu over it is open. */
private enum class EditorKey { Up, Down, Accept, Cancel }

/** The text of a message, as tall as what is written. Images pasted into it are attached. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun BodyEditor(state: TextFieldState, focus: FocusRequester, menuOpen: Boolean, onPaste: (List<Uri>) -> Unit, onKey: (EditorKey) -> Unit) {
    BasicTextField(
        state,
        Modifier
            .fillMaxWidth()
            .heightIn(min = 80.dp)
            .focusRequester(focus)
            .contentReceiver { content ->
                // A file of any kind is attached; text is written in.
                val clip = content.clipEntry.clipData
                val uris = (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri }
                if (uris.isEmpty()) return@contentReceiver content
                onPaste(uris)
                content.consume { it.uri != null }
            }
            .onPreviewKeyEvent { event ->
                if (!menuOpen || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                val key = when (event.key) {
                    Key.DirectionUp -> EditorKey.Up
                    Key.DirectionDown -> EditorKey.Down
                    Key.Tab, Key.Enter -> EditorKey.Accept
                    Key.Escape -> EditorKey.Cancel
                    else -> return@onPreviewKeyEvent false
                }
                onKey(key)
                true
            },
        textStyle = TextStyle.Body.compose(Tokens.foreground.color),
        cursorBrush = SolidColor(Tokens.foreground.color),
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
    )
}

/**
 * A line of a message's head: its label at the start, what it holds, and a hairline under it, inset
 * as the card's content is.
 */
@Composable
private fun FieldRow(label: String, top: Boolean = false, focus: (() -> Unit)? = null, content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    // The whole line is the field: a tap anywhere on it, the label too, puts the caret in it.
    val area = if (focus == null) Modifier else Modifier.pointerInput(Unit) { detectTapGestures { focus() } }
    Row(
        Modifier
            .fillMaxWidth()
            .then(area)
            .padding(horizontal = Dims.cardInset)
            .rule(Edge.Bottom, Dims.cardRule)
            .heightIn(min = Dims.searchHeight)
            .padding(vertical = if (top) Space.s else 0.dp),
        verticalAlignment = if (top) Alignment.Top else Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Space.s + 2.dp),
    ) {
        Box(Modifier.width(56.dp).heightIn(min = if (top) Dims.chipHeight else 0.dp), contentAlignment = Alignment.CenterStart) {
            Text(label, TextStyle.Label, color = Tokens.mutedStrongerForeground.color)
        }
        content()
    }
}

/**
 * To, Cc or Bcc: the addresses as chips, a field to type more, and below it the people it could
 * be, each with their picture. A comma, a semicolon or a paste of many addresses makes chips of
 * them; what can't be sent to is shown in red.
 */
@Composable
private fun AddressField(
    store: MailStore,
    label: String,
    addresses: List<Address>,
    setAddresses: (List<Address>) -> Unit,
    text: String,
    setText: (String) -> Unit,
    autofocus: Boolean = false,
    accessory: @Composable () -> Unit = {},
) {
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    var suggestions by remember { mutableStateOf<List<Address>>(emptyList()) }
    var highlighted by remember { mutableIntStateOf(0) }
    val current by rememberUpdatedState(addresses)
    LaunchedEffect(autofocus) {
        if (!autofocus) return@LaunchedEffect
        delay(100)
        runCatching { focus.requestFocus() }
    }
    LaunchedEffect(text) {
        val query = text.trim()
        if (query.isEmpty()) {
            suggestions = emptyList()
            return@LaunchedEffect
        }
        delay(60)
        val taken = current.map { it.email.lowercase() }.toSet()
        suggestions = store.contacts(query).filter { it.email.lowercase() !in taken }
        highlighted = 0
    }

    fun add(address: Address) {
        if (current.none { it.email.equals(address.email, ignoreCase = true) }) setAddresses(current + address)
        setText("")
        suggestions = emptyList()
    }

    fun typed(piece: String): List<Address> = Address.typed(piece).filter { new -> current.none { it.email.equals(new.email, ignoreCase = true) } }

    /** Takes the highlighted suggestion, or what was typed. */
    fun pick(): Boolean {
        suggestions.getOrNull(highlighted)?.let {
            add(it)
            return true
        }
        if (text.isBlank()) return false
        setAddresses(current + typed(text))
        setText("")
        return true
    }

    /** Makes chips of what is before a comma, a semicolon or a line break. */
    fun changed(value: String) {
        if (value.none { it == ',' || it == ';' || it == '\n' }) return setText(value)
        val parts = value.split(',', ';', '\n')
        setAddresses(current + parts.dropLast(1).flatMap(::typed))
        setText(parts.last().trim())
    }

    Box {
        FieldRow(label, top = true, focus = { runCatching { focus.requestFocus() } }) {
            FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(Space.xs), verticalArrangement = Arrangement.spacedBy(Space.xs)) {
                for ((index, address) in addresses.withIndex()) {
                    Chip(address.name ?: address.email, invalid = !address.isValid, remove = { setAddresses(addresses.filterIndexed { at, _ -> at != index }) })
                }
                BasicTextField(
                    text, ::changed,
                    Modifier
                        .widthIn(min = 80.dp)
                        .weight(1f)
                        .height(Dims.chipHeight)
                        .focusRequester(focus)
                        .onFocusChanged { focused = it.isFocused }
                        .onPreviewKeyEvent { event ->
                            if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                            when (event.key) {
                                Key.DirectionDown -> highlighted = (highlighted + 1).coerceAtMost((suggestions.size - 1).coerceAtLeast(0))
                                Key.DirectionUp -> highlighted = (highlighted - 1).coerceAtLeast(0)
                                Key.Tab, Key.Enter -> return@onPreviewKeyEvent pick()
                                Key.Escape -> {
                                    if (suggestions.isEmpty()) return@onPreviewKeyEvent false
                                    suggestions = emptyList()
                                }
                                Key.Backspace -> {
                                    if (text.isNotEmpty() || current.isEmpty()) return@onPreviewKeyEvent false
                                    setAddresses(current.dropLast(1))
                                }
                                else -> return@onPreviewKeyEvent false
                            }
                            true
                        },
                    singleLine = true,
                    textStyle = TextStyle.Body.compose(Tokens.foreground.color),
                    cursorBrush = SolidColor(Tokens.foreground.color),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
                    keyboardActions = KeyboardActions(onNext = { pick() }),
                    decorationBox = { inner -> Box(contentAlignment = Alignment.CenterStart) { inner() } },
                )
            }
            accessory()
        }
        if (focused && suggestions.isNotEmpty()) {
            Popup(popupPositionProvider = Under, properties = PopupProperties(focusable = false)) {
                PopupCard(Modifier.padding(start = Dims.cardInset + 66.dp, end = Dims.cardInset).padding(top = Space.xxs).widthIn(max = Dims.popupWidth)) {
                    for ((index, address) in suggestions.withIndex()) {
                        ChoiceRow(address.name ?: address.email, detail = if (address.name == null) null else address.email, avatar = address, highlighted = index == highlighted) { add(address) }
                    }
                }
            }
        }
    }
}

