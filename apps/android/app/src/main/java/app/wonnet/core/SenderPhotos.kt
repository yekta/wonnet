package app.wonnet.core

import android.graphics.BitmapFactory
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/** Senders' pictures from the server, their Gravatar or their company's logo, for the people the contacts have no photo of. */
object SenderPhotos {
    val photos = mutableStateMapOf<String, ImageBitmap>()
    var bridge: CoreBridge? = null
    private val asked = HashSet<String>()

    /** Asks the core once a launch; the avatar shows the picture when it comes. */
    suspend fun load(email: String) {
        val key = email.lowercase()
        if (bridge == null || !asked.add(key)) return
        val file = file(key) ?: return
        val image = withContext(Dispatchers.IO) { BitmapFactory.decodeFile(file.path) } ?: return
        photos[key] = image.asImageBitmap()
    }

    /** The picture's file, for a notification. */
    suspend fun file(email: String): File? {
        val bridge = bridge ?: return null
        val path = runCatching { bridge.call("avatar", mapOf("email" to email.lowercase())).optionalString("path") }.getOrNull() ?: return null
        return File(path)
    }
}
