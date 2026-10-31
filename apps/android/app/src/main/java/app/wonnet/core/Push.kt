package app.wonnet.core

import android.annotation.SuppressLint
import android.content.Context
import android.util.Base64
import app.wonnet.App
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.io.File
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Push through Firebase Cloud Messaging. The server seals a new message's sender, subject and
 * snippet with this device's key (AES-256-GCM), so Google carries only ciphertext. Without the
 * Firebase project's file in the build there is no push, and the app tells of the mail it syncs.
 */
object Push {
    /** Asks Firebase for this device's token, which `PushService` gives the server with the key. */
    fun register(context: Context, store: MailStore) {
        if (store.demo || FirebaseApp.getApps(context).isEmpty()) return
        FirebaseMessaging.getInstance().register()
    }
}

/** This device's push key, kept in the app's own files, and what pushes carry sealed with it. */
object PushSeal {
    private const val NONCE = 12
    private const val TAG_BITS = 128

    private fun file(context: Context) = File(context.noBackupFilesDir, "push-key")

    /** The device's key as base64, made the first time. */
    @Synchronized
    fun key(context: Context): String {
        val file = file(context)
        if (file.length() != 32L) {
            file.writeBytes(ByteArray(32).also { SecureRandom().nextBytes(it) })
        }
        return Base64.encodeToString(file.readBytes(), Base64.NO_WRAP)
    }

    /** What a push's `sealed` says, or null when it isn't sealed with this device's key. */
    fun open(context: Context, sealed: String): JSON? = runCatching {
        val key = file(context).readBytes()
        val data = Base64.decode(sealed, Base64.DEFAULT)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "AES"), GCMParameterSpec(TAG_BITS, data, 0, NONCE))
        parseJSON(String(cipher.doFinal(data, NONCE, data.size - NONCE))) as? JSON
    }.getOrNull()
}

/** Hears Firebase: this device's token, given at each registration, and the data messages of new and settled mail. */
@SuppressLint("MissingFirebaseInstanceTokenRefresh")
class PushService : FirebaseMessagingService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Firebase has this device's token, at every registration and whenever it changes. */
    override fun onRegistered(token: String) {
        val store = App.store
        if (!store.started || store.demo) return
        store.setPushDevice(token, PushSeal.key(this))
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val data = message.data
        data["badge"]?.toIntOrNull()?.let { Notifier.unread = it }
        val opened = data["sealed"]?.let { PushSeal.open(this, it) }
        when (data["kind"]) {
            // Without this device's key, the push still says mail came.
            "alert" -> if (opened == null) plain() else alert(opened)
            "settle" -> opened?.let { Notifier.takeDown(this, it.strings("settled")) }
        }
    }

    private fun plain() {
        if (App.frontmost) return
        Notifier.plain(this)
    }

    /** New mail: a notification from its sender, unless the app is on screen. */
    private fun alert(mail: JSON) {
        val thread = mail.optionalString("thread") ?: return
        if (App.frontmost) return
        val notice = MailNotice(
            thread, mail.optionalString("account"), mail.string("from"), mail.string("email"), mail.string("initials"),
            mail.string("subject"), mail.string("snippet"),
        )
        scope.launch {
            val picture = mail.optionalString("avatar")?.let { Notifier.download(it) }
            Notifier.post(this@PushService, notice, picture, null)
        }
    }
}
