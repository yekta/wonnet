package app.wonnet.core

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.webkit.MimeTypeMap
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.content.FileProvider
import androidx.core.net.toUri
import app.wonnet.App
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/** What the system does for the app: the browser, other apps for files, the share sheet. */
object Platform {
    /** A web page in a Custom Tab over the app; anything else in the app that takes it. */
    fun open(context: Context, url: String) {
        val uri = url.toUri()
        if (uri.scheme == "http" || uri.scheme == "https") return customTab(context, url)
        start(context, Intent(Intent.ACTION_VIEW, uri))
    }

    /** The sign-in page in a Custom Tab; the browser comes back to `wonnet://auth`. */
    fun signIn(context: Context, url: String) = customTab(context, url)

    private fun customTab(context: Context, url: String) {
        val tab = CustomTabsIntent.Builder().setShowTitle(true).build()
        val from = App.activity ?: context
        if (from === context) tab.intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { tab.launchUrl(from, url.toUri()) }.onFailure { start(context, Intent(Intent.ACTION_VIEW, url.toUri())) }
    }

    /** A file the core keeps, in the app that opens its kind. False when none does. */
    fun openFile(context: Context, file: File, mime: String): Boolean {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val intent = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime.ifEmpty { mimeOf(file) }).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return start(context, Intent.createChooser(intent, file.name))
    }

    /** Files to the share sheet, to save or send them elsewhere. */
    fun share(context: Context, files: List<File>) {
        if (files.isEmpty()) return
        val uris = files.map { FileProvider.getUriForFile(context, "${context.packageName}.files", it) }
        val intent = if (uris.size == 1) {
            Intent(Intent.ACTION_SEND).setType(mimeOf(files[0])).putExtra(Intent.EXTRA_STREAM, uris[0])
        } else {
            Intent(Intent.ACTION_SEND_MULTIPLE).setType("*/*").putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
        }
        intent.clipData = ClipData.newRawUri(files[0].name, uris[0]).apply { uris.drop(1).forEach { addItem(ClipData.Item(it)) } }
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        start(context, Intent.createChooser(intent, null))
    }

    /** Writes a file where the user picked to save it. Off the main thread. */
    suspend fun saveTo(context: Context, file: File, target: Uri) = withContext(Dispatchers.IO) {
        context.contentResolver.openOutputStream(target)?.use { output -> file.inputStream().use { it.copyTo(output) } } ?: error("unwritable")
    }

    /** Writes files under their names into a folder the user picked. Off the main thread. */
    suspend fun saveInto(context: Context, files: List<Pair<File, String>>, folder: Uri) = withContext(Dispatchers.IO) {
        val parent = DocumentsContract.buildDocumentUriUsingTree(folder, DocumentsContract.getTreeDocumentId(folder))
        for ((file, name) in files) {
            val target = DocumentsContract.createDocument(context.contentResolver, parent, mimeOf(File(name)), name) ?: error("unwritable")
            context.contentResolver.openOutputStream(target)?.use { output -> file.inputStream().use { it.copyTo(output) } }
        }
    }

    fun mimeOf(file: File): String =
        MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "application/octet-stream"

    private fun start(context: Context, intent: Intent): Boolean {
        val from = App.activity ?: context
        if (from === context) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return try {
            from.startActivity(intent)
            true
        } catch (_: ActivityNotFoundException) {
            false
        }
    }
}
