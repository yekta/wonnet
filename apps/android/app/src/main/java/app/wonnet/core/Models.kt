package app.wonnet.core

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import java.util.UUID

// What the core sends, as crates/core/src/api.rs defines it.

data class ThreadRow(
    val id: String,
    val accountId: String,
    val color: String,
    val senders: String,
    val subject: String,
    val snippet: String,
    val date: String,
    val timestamp: Long,
    val unread: Boolean,
    val starred: Boolean,
    val attachment: Boolean,
    val snoozed: Boolean,
    /** The row is a saved draft: open it with `open_draft`. */
    val draftId: String?,
) {
    companion object {
        fun read(json: JSON) = ThreadRow(
            json.string("id"), json.string("account_id"), json.string("color"), json.string("senders"), json.string("subject"),
            json.string("snippet"), json.string("date"), json.long("timestamp"), json.bool("unread"), json.bool("starred"),
            json.bool("attachment"), json.bool("snoozed"), json.optionalString("draft_id"),
        )

        fun list(json: JSON, key: String) = json.objects(key).map(::read)
    }
}

data class Mailbox(val id: String, val name: String, val symbol: String, val unread: Int) {
    companion object {
        fun read(json: JSON) = Mailbox(json.string("id"), json.string("name"), json.string("symbol"), json.int("unread"))
    }
}

data class Identity(val name: String?, val email: String, val signature: String?) {
    companion object {
        fun read(json: JSON) = Identity(json.optionalString("name"), json.string("email"), json.optionalString("signature"))
    }
}

data class AccountView(
    val id: String,
    val address: String,
    /** What the user calls it; empty when they haven't named it. */
    val name: String,
    /** What it is shown as: its name, or its address. */
    val title: String,
    val provider: String,
    val color: String,
    val status: String,
    val identities: List<Identity>,
    val mailboxes: List<Mailbox>,
) {
    companion object {
        /** The longest name the core takes, its `ACCOUNT_NAME_MAX`. */
        const val NAME_MAX = 40

        fun read(json: JSON) = AccountView(
            json.string("id"), json.string("address"), json.string("name"), json.string("title"), json.string("provider"),
            json.string("color"), json.string("status"), json.objects("identities").map(Identity::read),
            json.objects("mailboxes").map(Mailbox::read),
        )
    }
}

data class Mailboxes(val unified: List<Mailbox>, val accounts: List<AccountView>) {
    companion object {
        fun read(json: JSON) = Mailboxes(json.objects("unified").map(Mailbox::read), json.objects("accounts").map(AccountView::read))
    }
}

/** A tab over a split inbox. */
data class SplitTab(val mailbox: String, val name: String, val unread: Int, val total: Int) {
    companion object {
        fun read(json: JSON) = SplitTab(json.string("mailbox"), json.string("name"), json.int("unread"), json.int("total"))
    }
}

data class ThreadPage(val rows: List<ThreadRow>, val total: Int, val splits: List<SplitTab>) {
    companion object {
        fun read(json: JSON) = ThreadPage(ThreadRow.list(json, "rows"), json.int("total"), json.objects("splits").map(SplitTab::read))
    }
}

data class Attachment(val name: String, val mime: String, val size: Long) {
    companion object {
        fun read(json: JSON) = Attachment(json.string("name"), json.string("mime"), json.long("size"))
    }
}

/** A line of a page's placeholder: text of so many characters, none a blank line, or an image. */
sealed interface ShapeLine {
    data class Text(val length: Int) : ShapeLine
    data class Image(val width: Int, val height: Int) : ShapeLine

    companion object {
        /** `{"text": 40}` or `{"image": {"width": 600, "height": 300}}`. */
        fun read(json: JSON): ShapeLine {
            val image = json.obj("image") ?: return Text(json.int("text"))
            return Image(image.int("width"), image.int("height"))
        }
    }
}

data class MessageItem(
    val id: String,
    val fromName: String,
    val fromEmail: String,
    val initials: String,
    val to: String,
    val date: String,
    val snippet: String,
    val unread: Boolean,
    val folded: Boolean,
    val html: String?,
    val shape: List<ShapeLine>,
    val failed: Boolean,
    val blockedImages: Boolean,
    /** The mail brings its own design; in the dark its colours are turned unless the original is asked for. */
    val designed: Boolean,
    val attachments: List<Attachment>,
) {
    companion object {
        fun read(json: JSON) = MessageItem(
            json.string("id"), json.string("from_name"), json.string("from_email"), json.string("initials"), json.string("to"),
            json.string("date"), json.string("snippet"), json.bool("unread"), json.bool("folded"), json.optionalString("html"),
            json.objects("shape").map(ShapeLine::read), json.bool("failed"), json.bool("blocked_images"), json.bool("designed"),
            json.objects("attachments").map(Attachment::read),
        )
    }
}

data class LabelChip(val id: String, val name: String)

data class Conversation(
    val id: String,
    val accountId: String,
    val color: String,
    val subject: String,
    val from: String,
    val to: String,
    val starred: Boolean,
    val unread: Boolean,
    val muted: Boolean,
    val labels: List<LabelChip>,
    val unsubscribe: Boolean,
    /** The saved reply draft of this thread. */
    val draftId: String?,
    /** The person the thread is with. */
    val person: String?,
    val messages: List<MessageItem>,
) {
    companion object {
        fun read(json: JSON) = Conversation(
            json.string("id"), json.string("account_id"), json.string("color"), json.string("subject"), json.string("from"),
            json.string("to"), json.bool("starred"), json.bool("unread"), json.bool("muted"),
            json.objects("labels").map { LabelChip(it.string("id"), it.string("name")) }, json.bool("unsubscribe"),
            json.optionalString("draft_id"), json.optionalString("person"), json.objects("messages").map(MessageItem::read),
        )
    }
}

data class Address(val name: String?, val email: String) {
    val text: String get() = name?.let { "$it <$email>" } ?: email

    /** The first letters of the first two words of the name, else the address's first letter. */
    val initials: String
        get() {
            val letters = (name ?: "").split(Regex("\\s+")).filter { it.isNotEmpty() }.take(2).joinToString("") { it.first().uppercase() }
            if (letters.isNotEmpty()) return letters
            return email.firstOrNull()?.uppercase() ?: "?"
        }

    /** Whether mail can be sent to it. */
    val isValid: Boolean get() = VALID.matches(email)

    /** The first word of the name, for "Hi {first_name}". */
    val firstName: String?
        get() {
            val first = name?.split(" ")?.firstOrNull { it.isNotEmpty() } ?: return null
            return first.takeIf { !it.contains("@") }
        }

    fun toJson(): JSON = json("name" to name, "email" to email)

    companion object {
        private val VALID = Regex("[^@\\s<>,;\"]+@[^@\\s<>,;\"]+\\.[^@\\s<>,;\"]+")

        fun read(json: JSON) = Address(json.optionalString("name"), json.string("email"))

        /** Reads "Ann <ann@example.com>" or a bare address. */
        fun parse(raw: String): Address? {
            val text = raw.trim()
            val open = text.lastIndexOf('<')
            val close = text.lastIndexOf('>')
            if (open >= 0 && close > open) {
                val email = text.substring(open + 1, close).trim()
                val name = text.substring(0, open).trim(' ', '"')
                return if (email.contains("@")) Address(name.ifEmpty { null }, email) else null
            }
            return if (text.contains("@") && !text.contains(" ")) Address(null, text) else null
        }

        /** Reads typed or pasted text as addresses. What can't be read is kept as written, to be shown as wrong. */
        fun typed(raw: String): List<Address> {
            val text = raw.trim()
            if (text.isEmpty()) return emptyList()
            parse(text)?.let { return listOf(it) }
            val words = text.split(Regex("\\s+"))
            if (words.size > 1 && words.all { it.contains("@") }) return words.map { parse(it) ?: Address(null, it) }
            return listOf(Address(null, text))
        }
    }
}

/** A file going out with a draft: `path` on this device until the core uploads it. */
data class DraftAttachment(val name: String, val mime: String, val size: Long, val upload: String? = null, val path: String? = null) {
    fun toJson(): JSON = json("name" to name, "mime" to mime, "size" to size, "upload" to upload, "path" to path)

    companion object {
        fun read(json: JSON) =
            DraftAttachment(json.string("name"), json.string("mime"), json.long("size"), json.optionalString("upload"), json.optionalString("path"))
    }
}

data class Draft(
    val accountId: String,
    val to: List<Address> = emptyList(),
    val cc: List<Address> = emptyList(),
    val bcc: List<Address> = emptyList(),
    val subject: String = "",
    val text: String = "",
    val inReplyTo: String? = null,
    val references: List<String> = emptyList(),
    val threadId: String? = null,
    /** One of the account's identities; none for its own address. */
    val from: Address? = null,
    val html: String? = null,
    /** The quoted text under a reply, kept apart while writing. */
    val quote: String? = null,
    /** The message the quote is of. */
    val quoted: String? = null,
    val attachments: List<DraftAttachment> = emptyList(),
    /** A message whose attachments go along, for a forward. */
    val forwardAttachmentsOf: String? = null,
) {
    fun toJson(): JSON = json(
        "account_id" to accountId, "to" to to.map { it.toJson() }, "cc" to cc.map { it.toJson() }, "bcc" to bcc.map { it.toJson() },
        "subject" to subject, "text" to text, "in_reply_to" to inReplyTo, "references" to references, "thread_id" to threadId,
        "from" to from?.toJson(), "html" to html, "quote" to quote, "quoted" to quoted, "attachments" to attachments.map { it.toJson() },
        "forward_attachments_of" to forwardAttachmentsOf,
    )

    companion object {
        fun read(json: JSON) = Draft(
            json.string("account_id"), json.objects("to").map(Address::read), json.objects("cc").map(Address::read),
            json.objects("bcc").map(Address::read), json.string("subject"), json.string("text"), json.optionalString("in_reply_to"),
            json.strings("references"), json.optionalString("thread_id"), json.obj("from")?.let(Address::read), json.optionalString("html"),
            json.optionalString("quote"), json.optionalString("quoted"), json.objects("attachments").map(DraftAttachment::read),
            json.optionalString("forward_attachments_of"),
        )
    }
}

/** The answer to `new_draft`, `reply_draft` and `open_draft`. */
data class DraftReply(val id: String?, val draft: Draft, val from: String) {
    companion object {
        fun read(json: JSON) = DraftReply(json.optionalString("id"), Draft.read(json.obj("draft") ?: EmptyJSON), json.string("from"))
    }
}

/** What an action did: a toast's message, whether `undo` takes it back, a page to open. */
data class ActReply(val message: String?, val undo: Boolean, val url: String?) {
    companion object {
        fun read(json: JSON) = ActReply(json.optionalString("message"), json.bool("undo"), json.optionalString("url"))
    }
}

/** A time a snooze, a reminder or a send-later can be for. */
data class TimeChoice(val id: String, val name: String, val label: String, val until: Long) {
    fun toJson(): JSON = json("id" to id, "name" to name, "label" to label, "until" to until)

    companion object {
        fun read(json: JSON) = TimeChoice(json.string("id"), json.string("name"), json.string("label"), json.long("until"))
    }
}

/** Someone, and the latest threads with them. */
data class Person(val name: String, val email: String, val initials: String, val threads: List<ThreadRow>) {
    companion object {
        fun read(json: JSON) = Person(json.string("name"), json.string("email"), json.string("initials"), ThreadRow.list(json, "threads"))
    }
}

/** Mail that just arrived, for a notification. */
data class NewMailItem(
    val thread: String,
    val accountId: String,
    val from: String,
    val email: String,
    val initials: String,
    val subject: String,
    val snippet: String,
) {
    companion object {
        fun read(json: JSON) = NewMailItem(
            json.string("thread"), json.string("account_id"), json.string("from"), json.string("email"), json.string("initials"),
            json.string("subject"), json.string("snippet"),
        )
    }
}

data class Status(val signedIn: Boolean, val server: String?, val connection: String) {
    companion object {
        fun read(json: JSON) = Status(json.bool("signed_in"), json.optionalString("server"), json.string("connection"))
    }
}

/** The synced preferences, by key. */
fun readPreferences(json: JSON): Map<String, JsonElement> =
    json.objects("values").associate { it.string("key") to (it["value"] ?: kotlinx.serialization.json.JsonNull) }

/** A message being written. */
data class Compose(
    val draft: Draft,
    val from: String,
    val showCc: Boolean,
    /** The saved draft it is kept as, once it is. */
    val draftId: String? = null,
    /** The thread it answers, to archive along with sending. */
    val thread: String? = null,
    /** When to bring the thread back if nobody answers. */
    val remindAt: TimeChoice? = null,
    /** Closing keeps it as a draft even unchanged: a message brought back from sending. */
    val keep: Boolean = false,
    /** What is typed in the address fields and not yet an address. */
    val typing: Typing? = null,
    val id: String = UUID.randomUUID().toString(),
) {
    data class Typing(val to: String = "", val cc: String = "", val bcc: String = "")

    /** Whether anything was written in it. */
    val hasContent: Boolean
        get() = draft.to.isNotEmpty() || draft.cc.isNotEmpty() || draft.bcc.isNotEmpty() || draft.subject.isNotEmpty() ||
            draft.text.isNotEmpty() || draft.attachments.isNotEmpty()

    fun toJson(): JSON = json(
        "draft" to draft.toJson(), "from" to from, "show_cc" to showCc, "draft_id" to draftId, "thread" to thread,
        "remind_at" to remindAt?.toJson(), "keep" to keep,
        "typing" to typing?.let { json("to" to it.to, "cc" to it.cc, "bcc" to it.bcc) },
    )

    companion object {
        fun read(json: JSON): Compose? {
            val draft = json.obj("draft") ?: return null
            val typing = json.obj("typing")?.let { Typing(it.string("to"), it.string("cc"), it.string("bcc")) }
            return Compose(
                Draft.read(draft), json.string("from"), json.bool("show_cc"), json.optionalString("draft_id"), json.optionalString("thread"),
                json.obj("remind_at")?.let(TimeChoice::read), json.bool("keep"), typing,
            )
        }
    }
}

/** Where the app is, kept by the core so the next start shows the same screen. */
data class UiState(
    val mailbox: String = "inbox",
    val filter: Filter? = null,
    val thread: String? = null,
    val selected: String? = null,
    val rows: Int = 0,
    val listOffset: Double = 0.0,
    val threadOffset: Double? = null,
    val selection: List<String> = emptyList(),
    val search: String = "",
    val compose: Compose? = null,
    val sidebar: Boolean = false,
    val unfolded: List<String> = emptyList(),
    val focused: String? = null,
    val images: Boolean = false,
    val heights: Map<String, Double> = emptyMap(),
    val bodyWidth: Double = 0.0,
) {
    fun toJson(): JSON = json(
        "mailbox" to mailbox, "filter" to filter?.id, "thread" to thread, "selected" to selected, "rows" to rows,
        "list_offset" to listOffset, "thread_offset" to threadOffset, "selection" to selection, "search" to search,
        "compose" to compose?.toJson(), "sidebar" to sidebar, "unfolded" to unfolded, "focused" to focused, "images" to images,
        "heights" to heights, "body_width" to bodyWidth,
    )

    companion object {
        fun read(json: JSON) = UiState(
            json.optionalString("mailbox") ?: "inbox", Filter.of(json.optionalString("filter")), json.optionalString("thread"),
            json.optionalString("selected"), json.int("rows"), json.double("list_offset"), json.optionalDouble("thread_offset"),
            json.strings("selection"), json.string("search"), json.obj("compose")?.let(Compose::read), json.bool("sidebar"),
            json.strings("unfolded"), json.optionalString("focused"), json.bool("images"),
            json.obj("heights")?.let { heights -> heights.keys.associateWith { heights.double(it) } } ?: emptyMap(), json.double("body_width"),
        )
    }
}

/** The answer to `boot`: everything the first frame shows, as the app was left. */
data class Boot(
    val status: Status,
    val mailboxes: Mailboxes,
    val page: ThreadPage,
    val thread: Conversation?,
    val search: List<ThreadRow>?,
    val preferences: Map<String, JsonElement>,
    val ui: UiState,
) {
    companion object {
        fun read(json: JSON) = Boot(
            Status.read(json.obj("status") ?: EmptyJSON), Mailboxes.read(json.obj("mailboxes") ?: EmptyJSON),
            ThreadPage.read(json.obj("page") ?: EmptyJSON), json.obj("thread")?.let(Conversation::read),
            (json["search"] as? JsonArray)?.let { ThreadRow.list(json, "search") }, readPreferences(json.obj("preferences") ?: EmptyJSON),
            UiState.read(json.obj("ui") ?: EmptyJSON),
        )
    }
}

sealed interface CoreEvent {
    data class Changed(val mailboxes: Boolean, val threads: List<String>, val preferences: Boolean) : CoreEvent
    data class Connection(val state: String, val error: String?) : CoreEvent
    data class Sent(val opId: String) : CoreEvent
    data class SendFailed(val opId: String, val error: String, val draft: Draft, val draftId: String?) : CoreEvent
    data class SearchResults(val request: Long, val rows: List<ThreadRow>) : CoreEvent
    data class NewMail(val messages: List<NewMailItem>) : CoreEvent
    data class Error(val message: String) : CoreEvent

    companion object {
        fun read(json: JSON): CoreEvent? = when (json.string("type")) {
            "changed" -> Changed(json.bool("mailboxes"), json.strings("threads"), json.bool("preferences"))
            "connection" -> Connection(json.string("state"), json.optionalString("error"))
            "sent" -> Sent(json.string("op_id"))
            "send_failed" -> SendFailed(
                json.string("op_id"), json.string("error"), Draft.read(json.obj("draft") ?: EmptyJSON), json.optionalString("draft_id"),
            )
            "search_results" -> SearchResults(json.long("request"), ThreadRow.list(json, "rows"))
            "new_mail" -> NewMail(json.objects("messages").map(NewMailItem::read))
            "error" -> Error(json.string("message"))
            else -> null
        }
    }
}

/** What can be done to threads, as the core names it. */
enum class Action(val id: String) {
    Archive("archive"), Trash("trash"), Read("read"), Unread("unread"), Star("star"), Unstar("unstar"), Inbox("inbox"),
    Spam("spam"), Snooze("snooze"), Move("move"), Mute("mute"), Unmute("unmute"), Unsubscribe("unsubscribe"), Block("block"),
    Delete("delete"), AddLabel("add_label"), RemoveLabel("remove_label"),
}

/** What a list can be narrowed to. */
enum class Filter(val id: String) {
    Unread("unread"), Starred("starred");

    companion object {
        fun of(id: String?): Filter? = entries.firstOrNull { it.id == id }
    }
}

enum class ReplyKind(val id: String) {
    Reply("reply"), ReplyAll("reply_all"), Forward("forward"),
}

/** An object the answer is, or an empty one. */
val JsonElement.objectOrEmpty: JSON get() = this as? JsonObject ?: EmptyJSON
