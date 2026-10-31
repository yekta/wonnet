package app.wonnet.core

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.pdf.PdfRenderer
import android.media.ThumbnailUtils
import android.util.Size
import android.os.ParcelFileDescriptor
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.core.graphics.createBitmap
import app.wonnet.theme.Tokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Pictures of messages' attachments, drawn from the files the core keeps: images, PDFs' first
 * pages and videos' frames. Images and PDFs small enough are fetched as their cards show; a video
 * once it was opened.
 */
object AttachmentPreviews {
    /** The pictures drawn so far, by `<message>/<index>`. */
    val previews = mutableStateMapOf<String, ImageBitmap>()
    var bridge: CoreBridge? = null
    private val asked = HashSet<String>()

    /** The largest file fetched only to draw its picture. */
    private const val FETCHED_AHEAD = 5_000_000L
    /** The picture's width, in pixels: a card's preview on a dense screen. */
    private const val SIDE = 480

    fun key(message: String, index: Int) = "$message/$index"

    /** Draws an image's or a PDF's picture, fetching the file ahead. */
    suspend fun load(message: String, index: Int, attachment: Attachment) {
        val drawable = attachment.mime.startsWith("image/") || attachment.mime == "application/pdf"
        if (!drawable || attachment.size > FETCHED_AHEAD) return
        val key = key(message, index)
        val bridge = bridge ?: return
        if (!asked.add(key)) return
        val path = runCatching { bridge.call("open_attachment", mapOf("message" to message, "index" to index)).string("path") }.getOrNull() ?: return
        load(message, index, File(path), attachment.mime)
    }

    /** Draws the picture of a file already fetched, as one is once opened. */
    suspend fun load(message: String, index: Int, file: File, mime: String) {
        val key = key(message, index)
        if (previews[key] != null) return
        val picture = withContext(Dispatchers.IO) { runCatching { thumbnail(file, mime) }.getOrNull() } ?: return
        previews[key] = picture.asImageBitmap()
    }

    private fun thumbnail(file: File, mime: String): Bitmap? {
        if (mime == "application/pdf") return pdf(file)
        if (mime.startsWith("video/")) return ThumbnailUtils.createVideoThumbnail(file, Size(SIDE, SIDE * 2 / 3), null)
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, bounds)
        if (bounds.outWidth <= 0) return null
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= SIDE) sample *= 2
        return BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply { inSampleSize = sample })
    }

    private fun pdf(file: File): Bitmap? = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY).use { descriptor ->
        PdfRenderer(descriptor).use { renderer ->
            if (renderer.pageCount == 0) return null
            renderer.openPage(0).use { page ->
                val bitmap = createBitmap(SIDE, SIDE * page.height / page.width.coerceAtLeast(1))
                // A page is paper: the light card, whatever the scheme.
                bitmap.eraseColor(Tokens.cardLarger.color(false).toArgb())
                page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
                bitmap
            }
        }
    }
}
