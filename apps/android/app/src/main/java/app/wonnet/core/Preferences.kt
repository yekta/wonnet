package app.wonnet.core

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/** Text kept to put in messages, `snippet:<id>`. `{first_name}` is filled from the first recipient. */
data class Snippet(val id: String, val name: String, val text: String) {
    val key: String get() = "snippet:$id"
    val value: JsonElement get() = json("name" to name, "text" to text)

    /** The text with what is known filled in. */
    fun filled(firstName: String?): String = firstName?.let { text.replace("{first_name}", it) } ?: text

    companion object {
        fun all(preferences: Map<String, JsonElement>): List<Snippet> = preferences.mapNotNull { (key, value) ->
            val fields = value as? JsonObject ?: return@mapNotNull null
            val name = fields.optionalString("name") ?: return@mapNotNull null
            if (!key.startsWith("snippet:")) return@mapNotNull null
            Snippet(key.removePrefix("snippet:"), name, fields.string("text"))
        }.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
    }
}

/** A tab of Split Inbox the user made, `split:<id>`: mail from some senders, or with a label. */
data class CustomSplit(val id: String, val name: String, val from: List<String>, val label: String?, val order: Int) {
    val key: String get() = "split:$id"
    val value: JsonElement get() = jsonOf(mapOf("name" to name, "from" to from, "label" to label, "order" to order))

    companion object {
        fun all(preferences: Map<String, JsonElement>): List<CustomSplit> = preferences.mapNotNull { (key, value) ->
            val fields = value as? JsonObject ?: return@mapNotNull null
            val name = fields.optionalString("name") ?: return@mapNotNull null
            if (!key.startsWith("split:")) return@mapNotNull null
            CustomSplit(key.removePrefix("split:"), name, fields.strings("from"), fields.optionalString("label"), fields.int("order"))
        }.sortedWith(compareBy<CustomSplit> { it.order }.thenBy { it.name })

        /** The senders as they are typed: addresses and `@domains`, split on commas, spaces or lines. */
        fun senders(text: String): List<String> = text.split(',', ' ', ';', '\n').filter { it.isNotEmpty() }.map { it.lowercase() }
    }
}

/** A list of ids kept as a preference. */
private fun ids(preferences: Map<String, JsonElement>, key: String): List<String>? =
    (preferences[key] as? JsonArray)?.mapNotNull { it.text }

/**
 * The mailboxes shown as tabs along the bottom, `tabs`: their ids in order. Unset, the usual four.
 * A mailbox of every account together (`inbox`) is the account's own while one account's mailbox
 * is on screen.
 */
object TabPreference {
    const val KEY = "tabs"
    val standard = listOf("inbox", "unread", "sent", "snoozed")

    fun ids(preferences: Map<String, JsonElement>): List<String> = ids(preferences, KEY) ?: standard
}

/**
 * The sidebar's order: `sidebar`, the ids of the mailboxes of every account together, and
 * `accounts`, the accounts' ids. One not named comes after the named, as the core lists them.
 */
object SidebarOrder {
    const val MAILBOXES = "sidebar"
    const val ACCOUNTS = "accounts"

    fun ids(key: String, preferences: Map<String, JsonElement>): List<String> = ids(preferences, key) ?: emptyList()

    /** The items in the order of `ids`, then the rest as they came. */
    fun <T> sorted(items: List<T>, ids: List<String>, id: (T) -> String): List<T> {
        val ranks = ids.withIndex().reversed().associate { it.value to it.index }
        return items.withIndex().sortedWith(compareBy({ ranks[id(it.value)] ?: ids.size }, { it.index })).map { it.value }
    }

    /** `id` put after the ids `before` it, with the rest of `all` after. */
    fun placed(id: String, before: List<String>, all: List<String>): List<String> =
        before + id + all.filter { it != id && it !in before }
}

/**
 * What a swipe to the left shows on a thread row, `swipe`: one to three commands from the
 * screen's edge inwards, the first of which a long swipe does. Unset, read, snooze and archive.
 */
object SwipePreference {
    const val KEY = "swipe"
    val standard = listOf(ThreadCommand.Read, ThreadCommand.Snooze, ThreadCommand.Archive)
    val choices = listOf(
        ThreadCommand.Read, ThreadCommand.Snooze, ThreadCommand.Archive, ThreadCommand.Trash, ThreadCommand.Star, ThreadCommand.Label,
        ThreadCommand.Move, ThreadCommand.Mute, ThreadCommand.Spam,
    )
    const val MOST = 3

    fun commands(preferences: Map<String, JsonElement>): List<ThreadCommand> {
        val chosen = mutableListOf<ThreadCommand>()
        for (id in ids(preferences, KEY) ?: return standard) {
            val command = ThreadCommand.of(id) ?: continue
            if (command !in choices || command in chosen) continue
            chosen += command
        }
        return if (chosen.isEmpty()) standard else chosen.take(MOST)
    }
}
