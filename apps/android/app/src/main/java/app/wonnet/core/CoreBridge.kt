package app.wonnet.core

import android.content.Context
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlin.concurrent.thread
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** What the core said went wrong. */
class CoreError(override val message: String) : Exception(message)

/**
 * The app's end of the Rust core: commands go in as JSON, events come out as JSON. Events and
 * answers are read on a thread of their own, in the order they were sent; events are handed to
 * the main thread.
 */
class CoreBridge {
    /** Called on the main thread with each event that isn't an answer. */
    var onEvent: ((CoreEvent) -> Unit)? = null

    private val main = Handler(Looper.getMainLooper())
    private val nextID = AtomicLong(0)
    /** What to do with each answer, on the event thread: whether it went well, and its value. */
    private val answers = ConcurrentHashMap<Long, (Boolean, JsonElement) -> Unit>()
    private var listening = false

    fun start(context: Context, config: JSON): Boolean {
        val started = runCatching { Native.start(context.applicationContext, config.toString()) }.getOrDefault(false)
        if (started && !listening) {
            listening = true
            thread(name = "wonnet-events", isDaemon = true) {
                while (true) received(Native.next())
            }
        }
        return started
    }

    /** Sends a command and waits for its answer, which `read` turns into what is kept off the main thread. */
    suspend fun <T> call(type: String, fields: Map<String, Any?> = emptyMap(), read: (JsonElement) -> T): T =
        suspendCancellableCoroutine { continuation ->
            val id = register { ok, value ->
                if (!ok) {
                    val message = (value as? JsonObject)?.optionalString("error") ?: "Something went wrong."
                    continuation.resumeWithException(CoreError(message))
                    return@register
                }
                val result = runCatching { read(value) }
                result.fold(
                    onSuccess = { continuation.resume(it) },
                    onFailure = { continuation.resumeWithException(CoreError("The core's answer can't be read: ${it.message}")) },
                )
            }
            post(type, fields, id)
        }

    /** Sends a command whose answer is an object. */
    suspend fun call(type: String, fields: Map<String, Any?> = emptyMap()): JSON = call(type, fields) { it as? JsonObject ?: EmptyJSON }

    /** Sends a command whose answer doesn't matter. */
    fun send(type: String, fields: Map<String, Any?> = emptyMap()) {
        post(type, fields, register { _, _ -> })
    }

    private fun register(answer: (Boolean, JsonElement) -> Unit): Long {
        val id = nextID.incrementAndGet()
        answers[id] = answer
        return id
    }

    /** A field left null is left out, as the core reads a missing field as none. */
    private fun post(type: String, fields: Map<String, Any?>, id: Long) {
        val command = jsonOf(fields.filterValues { it != null && it !is kotlinx.serialization.json.JsonNull } + mapOf("type" to type, "id" to id))
        Native.send(command.toString())
    }

    private fun received(text: String) {
        val event = parseJSON(text) as? JsonObject ?: return
        if (event.string("type") == "reply") {
            val answer = answers.remove(event.long("id")) ?: return
            answer(event.bool("ok"), event["value"] ?: EmptyJSON)
            return
        }
        val decoded = CoreEvent.read(event) ?: return
        main.post { onEvent?.invoke(decoded) }
    }
}
