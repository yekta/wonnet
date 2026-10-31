package app.wonnet.core

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.util.UUID

/**
 * Files going out with drafts, copied where the core can read them until it uploads them, and
 * the checks made on a message before it goes.
 */
object Outgoing {
    /** The most the attachments of a message can weigh together. */
    const val LIMIT = 25L * 1024 * 1024

    private fun folder(context: Context) = File(context.filesDir, "Outgoing")

    /** Copies what was picked or shared. Runs off the main thread: files can be large. */
    suspend fun stage(context: Context, uris: List<Uri>): List<DraftAttachment> = withContext(Dispatchers.IO) {
        uris.mapNotNull { uri -> runCatching { copy(context, uri) }.getOrNull() }
    }

    private fun copy(context: Context, uri: Uri): DraftAttachment {
        val resolver = context.contentResolver
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: uri.lastPathSegment ?: "Attachment"
        val target = File(File(folder(context), UUID.randomUUID().toString()).apply { mkdirs() }, name.ifEmpty { "Attachment" })
        resolver.openInputStream(uri)?.use { input -> target.outputStream().use { input.copyTo(it) } } ?: error("unreadable")
        val mime = resolver.getType(uri) ?: Platform.mimeOf(target)
        return DraftAttachment(target.name, mime, target.length(), path = target.path)
    }

    /** Forgets files staged over a week ago: their drafts were sent or set aside long since. */
    fun prune(context: Context) {
        Thread {
            val old = System.currentTimeMillis() - 7L * 24 * 3600 * 1000
            folder(context).listFiles()?.filter { it.lastModified() < old }?.forEach { it.deleteRecursively() }
        }.start()
    }

    /** Whether the text speaks of an attachment. */
    fun mentionsAttachment(text: String): Boolean =
        Regex("\\b(attach(ed|es|ing|ment|ments)?|enclosed)\\b", RegexOption.IGNORE_CASE).containsMatchIn(text)

    /** The `{placeholders}` of a snippet still in the text. */
    fun placeholders(text: String): List<String> = Regex("\\{[A-Za-z_]+\\}").findAll(text).map { it.value }.toList()
}
