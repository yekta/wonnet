package app.wonnet.core

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Typeface
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.Person
import androidx.core.app.RemoteInput
import androidx.core.content.ContextCompat
import androidx.core.content.edit
import androidx.core.graphics.createBitmap
import androidx.core.graphics.drawable.IconCompat
import app.wonnet.App
import app.wonnet.MainActivity
import app.wonnet.R
import app.wonnet.theme.Dims
import app.wonnet.theme.Tokens
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/** A new message, as a notification shows it. */
data class MailNotice(
    val thread: String,
    val account: String?,
    val from: String,
    val email: String,
    val initials: String,
    val subject: String,
    val snippet: String,
)

/**
 * New mail as notifications from its sender, with their avatar, while the app is away: a tap
 * opens its thread and its buttons archive, mark read or reply. With push, the server tells of
 * new mail, sealed with this device's key; without it, the app tells of the mail it syncs. A
 * thread's notification is taken down once it is read or leaves the inbox, here or elsewhere.
 */
object Notifier {
    private const val CHANNEL = "mail"
    const val THREAD = "thread"
    /** The tag of new mail a push couldn't say more of. */
    private const val PLAIN = "new-mail"
    private const val ASKED = "askedPermissions"

    /** The core, to ask which threads are still unread. */
    var bridge: CoreBridge? = null
    /** The server pushes new mail to this device, so the app doesn't tell of it itself. */
    @Volatile
    var pushing = false
    /** The unread threads of the inboxes, which launchers show as the icon's count. */
    @Volatile
    var unread = 0
    private val background = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    fun install(context: Context) {
        val channel = NotificationChannel(CHANNEL, "New mail", NotificationManager.IMPORTANCE_HIGH)
        channel.description = "Mail that arrives in your inboxes"
        context.getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
    }

    /**
     * Asks once, when there is mail to tell of and names to put on it: after an account is added,
     * not at first launch. The contacts' names and photos go along.
     */
    fun askPermission() {
        val activity = App.activity as? MainActivity ?: return
        val settings = activity.getSharedPreferences("app", Context.MODE_PRIVATE)
        if (settings.getBoolean(ASKED, false)) return
        settings.edit { putBoolean(ASKED, true) }
        val wanted = listOfNotNull(
            Manifest.permission.READ_CONTACTS,
            if (android.os.Build.VERSION.SDK_INT >= 33) Manifest.permission.POST_NOTIFICATIONS else null,
        ).filter { ContextCompat.checkSelfPermission(activity, it) != PackageManager.PERMISSION_GRANTED }
        if (wanted.isEmpty()) return
        activity.ask(wanted.toTypedArray())
    }

    /** Mail the core synced while the app is away, when the server doesn't push it. */
    fun notify(messages: List<NewMailItem>, store: MailStore) {
        if (pushing || App.frontmost) return
        for (message in messages.takeLast(5)) {
            val notice = MailNotice(message.thread, message.accountId, message.from, message.email, message.initials, message.subject, message.snippet)
            val color = store.accounts.firstOrNull { it.id == message.accountId }?.color
            background.launch {
                val avatar = SenderPhotos.file(message.email)?.let { BitmapFactory.decodeFile(it.path) }
                post(App.context, notice, avatar, color)
            }
        }
    }

    /** A pushed message's sender's picture, at the address the server gave. */
    suspend fun download(url: String): Bitmap? = withContext(Dispatchers.IO) {
        runCatching {
            val connection = URL(url).openConnection() as HttpURLConnection
            connection.connectTimeout = 5000
            connection.readTimeout = 5000
            if (connection.responseCode != 200) return@runCatching null
            connection.inputStream.use { BitmapFactory.decodeStream(it) }
        }.getOrNull()
    }

    /** Shows the message as one from its sender: their photo, else their picture, else their initials. */
    fun post(context: Context, notice: MailNotice, picture: Bitmap?, accountColor: String?) {
        val manager = NotificationManagerCompat.from(context)
        if (!allowed(context) || !manager.areNotificationsEnabled()) return
        val avatar = circle(AddressBook.photo(context, notice.email) ?: picture ?: initials(notice.initials, notice.email))
        val sender = Person.Builder().setName(notice.from).setKey(notice.email).setIcon(IconCompat.createWithBitmap(avatar)).build()
        val me = Person.Builder().setName("Me").build()
        val text = listOf(notice.subject, notice.snippet).filter { it.isNotEmpty() }.joinToString("\n")
        val style = NotificationCompat.MessagingStyle(me).addMessage(text, System.currentTimeMillis(), sender)
        val code = notice.thread.hashCode()
        val open = PendingIntent.getActivity(
            context, code, Intent(context, MainActivity::class.java).putExtra(THREAD, notice.thread).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val reply = RemoteInput.Builder(NotificationActions.TEXT).setLabel("Reply").build()
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setStyle(style)
            .setLargeIcon(avatar)
            .setColor(Dims.accountColor(accountColor ?: "").color(false).toArgbInt())
            .setContentIntent(open)
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_EMAIL)
            .setNumber(unread)
            .addAction(0, "Archive", NotificationActions.intent(context, NotificationActions.ARCHIVE, notice.thread))
            .addAction(0, "Mark as Read", NotificationActions.intent(context, NotificationActions.READ, notice.thread))
            .addAction(
                NotificationCompat.Action.Builder(0, "Reply", NotificationActions.intent(context, NotificationActions.REPLY, notice.thread, mutable = true))
                    .addRemoteInput(reply).setAllowGeneratedReplies(true).build(),
            )
            .build()
        try {
            manager.notify(notice.thread, 1, notification)
        } catch (_: SecurityException) {
            // The permission was taken back since.
        }
    }

    /** New mail whose push couldn't be opened here: no sender, no thread, only that it came. */
    fun plain(context: Context) {
        val manager = NotificationManagerCompat.from(context)
        if (!allowed(context) || !manager.areNotificationsEnabled()) return
        val open = PendingIntent.getActivity(
            context, 0, Intent(context, MainActivity::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("New mail")
            .setContentIntent(open)
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_EMAIL)
            .setNumber(unread)
            .build()
        try {
            manager.notify(PLAIN, 1, notification)
        } catch (_: SecurityException) {
            // The permission was taken back since.
        }
    }

    private fun allowed(context: Context) = android.os.Build.VERSION.SDK_INT < 33 ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    /** Takes down the notifications of these threads. */
    fun takeDown(context: Context, threads: Collection<String>) {
        val manager = NotificationManagerCompat.from(context)
        threads.forEach { manager.cancel(it, 1) }
    }

    /** Takes down the notifications of these threads (all shown, when null) that are no longer unread in an inbox. */
    suspend fun settle(threads: List<String>?) {
        val bridge = bridge ?: return
        val manager = App.context.getSystemService(NotificationManager::class.java) ?: return
        val shown = manager.activeNotifications.mapNotNull { it.tag }.toSet()
        val changed = threads?.let { shown.intersect(it.toSet()) } ?: shown
        if (changed.isEmpty()) return
        val unread = runCatching { bridge.call("unread_in_inbox", mapOf("threads" to changed.toList())).strings("threads") }.getOrNull() ?: return
        takeDown(App.context, changed - unread.toSet())
    }

    /** Takes down every notification: they were of mail no longer on this device. */
    fun signedOut() {
        unread = 0
        NotificationManagerCompat.from(App.context).cancelAll()
    }

    private fun androidx.compose.ui.graphics.Color.toArgbInt(): Int = android.graphics.Color.argb(alpha, red, green, blue)

    /** The initials on the colour their address gets, as the app's avatars draw them. */
    private fun initials(initials: String, email: String): Bitmap {
        val side = 192
        val bitmap = createBitmap(side, side)
        val canvas = Canvas(bitmap)
        val color = Dims.accountColors[Dims.colorIndex(email, Dims.accountColors.size)]
        val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = color.color(false).toArgbInt() }
        canvas.drawCircle(side / 2f, side / 2f, side / 2f, fill)
        val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = Tokens.primaryForeground.color(false).toArgbInt()
            textSize = side * 0.4f
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            textAlign = Paint.Align.CENTER
        }
        val bounds = android.graphics.Rect()
        text.getTextBounds(initials, 0, initials.length, bounds)
        canvas.drawText(initials, side / 2f, side / 2f - bounds.exactCenterY(), text)
        return bitmap
    }

    /** The picture cut to a circle, as a person's avatar is shown. */
    private fun circle(picture: Bitmap): Bitmap {
        val side = minOf(picture.width, picture.height)
        val bitmap = createBitmap(side, side)
        val canvas = Canvas(bitmap)
        val path = Path().apply { addCircle(side / 2f, side / 2f, side / 2f, Path.Direction.CW) }
        canvas.clipPath(path)
        canvas.drawBitmap(picture, ((side - picture.width) / 2).toFloat(), ((side - picture.height) / 2).toFloat(), null)
        return bitmap
    }
}

/** What a notification's buttons do: the store wakes, does it, and waits until the server has it. */
class NotificationActions : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val thread = intent.getStringExtra(Notifier.THREAD) ?: return
        val answer = when (intent.action) {
            ARCHIVE -> NotificationAnswer.Archive
            READ -> NotificationAnswer.Read
            REPLY -> RemoteInput.getResultsFromIntent(intent)?.getCharSequence(TEXT)?.toString()?.trim()?.takeIf { it.isNotEmpty() }
                ?.let { NotificationAnswer.Reply(it) } ?: return
            else -> return
        }
        Notifier.takeDown(context, listOf(thread))
        val pending = goAsync()
        val store = App.store
        store.start(demo = false)
        store.scope.launch {
            store.answer(answer, thread)
            pending.finish()
        }
    }

    companion object {
        const val ARCHIVE = "app.wonnet.ARCHIVE"
        const val READ = "app.wonnet.READ"
        const val REPLY = "app.wonnet.REPLY"
        const val TEXT = "text"

        fun intent(context: Context, action: String, thread: String, mutable: Boolean = false): PendingIntent {
            val intent = Intent(context, NotificationActions::class.java).setAction(action).putExtra(Notifier.THREAD, thread)
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (mutable) PendingIntent.FLAG_MUTABLE else PendingIntent.FLAG_IMMUTABLE)
            return PendingIntent.getBroadcast(context, (action + thread).hashCode(), intent, flags)
        }
    }
}

