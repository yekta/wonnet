package app.wonnet.core

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.provider.ContactsContract
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.content.ContextCompat
import androidx.core.net.toUri

/**
 * The device's contacts: their names go to the core, which shows them instead of the names mail
 * came with, and their photos to the avatars. Read again whenever they change.
 */
object AddressBook {
    /** Each address's contact photo, as the avatars draw it. */
    val photos = mutableStateMapOf<String, ImageBitmap>()
    private var thumbnails: Map<String, String> = emptyMap()
    private var bridge: CoreBridge? = null
    private val main = Handler(Looper.getMainLooper())
    /** The photos being read. */
    private val reading = HashSet<String>()

    fun start(context: Context, bridge: CoreBridge) {
        if (this.bridge != null) return
        this.bridge = bridge
        val observer = object : ContentObserver(main) {
            override fun onChange(selfChange: Boolean) = load(context)
        }
        runCatching { context.contentResolver.registerContentObserver(ContactsContract.Contacts.CONTENT_URI, true, observer) }
        load(context)
    }

    private fun allowed(context: Context) =
        ContextCompat.checkSelfPermission(context, Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED

    /** Reads every contact away from the main thread. Without access, the names given before are forgotten. */
    fun load(context: Context) {
        val bridge = bridge ?: return
        Thread {
            val people = mutableListOf<Map<String, String>>()
            val found = HashMap<String, String>()
            if (allowed(context)) {
                val columns = arrayOf(
                    ContactsContract.CommonDataKinds.Email.ADDRESS, ContactsContract.Contacts.DISPLAY_NAME_PRIMARY,
                    ContactsContract.Contacts.PHOTO_THUMBNAIL_URI,
                )
                runCatching {
                    context.contentResolver.query(ContactsContract.CommonDataKinds.Email.CONTENT_URI, columns, null, null, null)?.use { cursor ->
                        while (cursor.moveToNext()) {
                            val address = cursor.getString(0)?.trim()?.lowercase()?.takeIf { it.isNotEmpty() } ?: continue
                            val name = cursor.getString(1)?.takeIf { it.isNotEmpty() }
                            if (name != null) people += mapOf("name" to name, "email" to address)
                            cursor.getString(2)?.let { found[address] = it }
                        }
                    }
                }
            }
            bridge.send("set_address_book", mapOf("people" to people))
            main.post {
                thumbnails = found
                photos.keys.retainAll(found.keys)
            }
        }.start()
    }

    /** An address's contact photo, read the first time it is shown. */
    fun photoFor(context: Context, email: String): ImageBitmap? {
        val key = email.lowercase()
        photos[key]?.let { return it }
        if (key !in thumbnails || !reading.add(key)) return null
        Thread {
            val bitmap = photo(context, key)
            main.post {
                reading.remove(key)
                bitmap?.let { photos[key] = it.asImageBitmap() }
            }
        }.start()
        return null
    }

    /** An address's contact photo, read now: off the main thread. */
    fun photo(context: Context, email: String): Bitmap? {
        val uri = thumbnails[email.lowercase()] ?: return null
        return runCatching { context.contentResolver.openInputStream(uri.toUri())?.use(BitmapFactory::decodeStream) }.getOrNull()
    }
}
