package app.wonnet.core

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.Settings
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.net.toUri
import app.wonnet.App
import app.wonnet.BuildConfig
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/**
 * Finds out about new releases and replaces this copy with one, as the Mac app's updater does:
 * it looks now and then, offers the update, downloads the release's APK, checks that it is this
 * app signed with this app's key, and hands it to Android's installer, which asks the user.
 */
class AppUpdater(private val context: Context) {
    sealed interface State {
        data object Idle : State
        /** The user asked and the answer isn't here yet. */
        data object Checking : State
        data object UpToDate : State
        data class Available(val version: String) : State
        data class Downloading(val version: String, val fraction: Double) : State
        /** With Android's installer, which asks the user before it replaces this copy. */
        data class Installing(val version: String) : State
        data class Failed(val message: String) : State
    }

    var state by mutableStateOf<State>(State.Idle)
        private set
    val current: String = BuildConfig.VERSION_NAME
    private val main = Handler(Looper.getMainLooper())
    @Volatile
    private var downloading: String? = null
    private var lastCheck: Long? = null

    /** Looks for a new release now and every few hours. */
    fun start() {
        shared = this
        check()
        main.postDelayed(object : Runnable {
            override fun run() {
                check()
                main.postDelayed(this, CHECK_EVERY)
            }
        }, CHECK_EVERY)
    }

    /** Looks again if the timer missed its turn while the app was away. */
    fun checkIfDue() {
        val last = lastCheck ?: return
        if (SystemClock.elapsedRealtime() - last < CHECK_EVERY) return
        check()
    }

    /** `asked` is for when the user wants to know: finding nothing new is then said too. */
    fun check(asked: Boolean = false) {
        if (state is State.Checking || state is State.Downloading || state is State.Installing) return
        if (asked) state = State.Checking
        lastCheck = SystemClock.elapsedRealtime()
        thread(isDaemon = true) {
            // The address redirects to the release's own page, whose last part is its tag.
            val tag = runCatching { head("$RELEASES/latest").second.substringAfterLast('/') }.getOrNull()
            val version = tag?.takeIf { it.startsWith("v") }?.drop(1)
            // A release whose APK isn't uploaded yet is nothing new.
            val ready = version != null && Version.isOlder(current, version) &&
                runCatching { head(asset(version)).first == 200 }.getOrDefault(false)
            main.post { found(version, ready, asked) }
        }
    }

    /** The status and the address a HEAD request ends at, after its redirects. */
    private fun head(url: String): Pair<Int, String> {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.requestMethod = "HEAD"
        connection.useCaches = false
        connection.instanceFollowRedirects = true
        connection.connectTimeout = 15_000
        connection.readTimeout = 15_000
        val status = connection.responseCode
        val end = connection.url.toString()
        connection.disconnect()
        return status to end
    }

    private fun found(version: String?, ready: Boolean, asked: Boolean) {
        if (version == null) {
            if (asked) state = State.Failed("GitHub couldn’t be reached.")
            return
        }
        when {
            ready -> state = State.Available(version)
            asked -> {
                state = State.UpToDate
                main.postDelayed({ if (state == State.UpToDate) state = State.Idle }, 4000)
            }
            state == State.Checking -> state = State.Idle
        }
    }

    /** Why this copy can't replace itself, if it can't. */
    val obstacle: String?
        get() = if (BuildConfig.DEBUG) "This copy is a development build, so it can’t update itself." else null

    /** Whether Android lets this app install what it downloads, which the user allows once in its settings. */
    private val canInstall: Boolean get() = context.packageManager.canRequestPackageInstalls()

    /**
     * Downloads and installs the update. Without leave to install, the system's page for it opens
     * first; the user comes back and taps Update again.
     */
    fun install() {
        val version = (state as? State.Available)?.version ?: return
        obstacle?.let {
            state = State.Failed(it)
            return
        }
        if (!canInstall) return allowInstalling()
        downloading = version
        state = State.Downloading(version, 0.0)
        thread(isDaemon = true) {
            val result = runCatching { download(asset(version), version) }
            main.post {
                if (downloading != version) return@post
                downloading = null
                result.fold(
                    onSuccess = { apk -> hand(apk, version) },
                    onFailure = { state = State.Failed(it.message ?: "The new version couldn’t be downloaded.") },
                )
            }
        }
    }

    /** Opens the system's page that lets this app install apps. */
    private fun allowInstalling() {
        val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, "package:${context.packageName}".toUri())
        val from = App.activity ?: context
        if (from === context) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        from.startActivity(intent)
    }

    /** Looks again, after a failure. */
    fun retry() {
        state = State.Idle
        check(asked = true)
    }

    private fun download(url: String, version: String): File {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.instanceFollowRedirects = true
        val status = connection.responseCode
        if (status != 200) throw CoreError("The new version couldn’t be downloaded ($status).")
        val folder = File(context.cacheDir, "updates").apply {
            deleteRecursively()
            mkdirs()
        }
        val apk = File(folder, ASSET)
        val size = connection.contentLengthLong
        var received = 0L
        var shown = 0.0
        connection.inputStream.use { input ->
            apk.outputStream().use { output ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    output.write(buffer, 0, read)
                    received += read
                    val fraction = if (size > 0) received.toDouble() / size else 0.0
                    if (fraction - shown < 0.01) continue
                    shown = fraction
                    main.post { if (downloading == version) state = State.Downloading(version, fraction) }
                }
            }
        }
        verify(apk)
        return apk
    }

    /** The download has to be this app, newer, and signed with the key this copy was. */
    private fun verify(apk: File) {
        val manager = context.packageManager
        val flags = PackageManager.GET_SIGNING_CERTIFICATES
        val downloaded = manager.getPackageArchiveInfo(apk.path, flags)
        val running = manager.getPackageInfo(context.packageName, flags)
        if (downloaded == null || downloaded.packageName != running.packageName) throw CoreError("The download isn’t a copy of Wonnet.")
        if (signers(downloaded) != signers(running)) throw CoreError("The download isn’t signed by Wonnet’s developer, so it wasn’t installed.")
        if (downloaded.longVersionCode <= running.longVersionCode) throw CoreError("The download is no newer than this copy.")
    }

    private fun signers(info: PackageInfo): Set<String> = info.signingInfo?.apkContentsSigners?.map { it.toCharsString() }?.toSet() ?: emptySet()

    /** Gives the APK to Android's installer, which asks the user and then replaces this app and starts it. */
    private fun hand(apk: File, version: String) {
        state = State.Installing(version)
        thread(isDaemon = true) {
            val result = runCatching {
                val installer = context.packageManager.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
                params.setAppPackageName(context.packageName)
                val id = installer.createSession(params)
                installer.openSession(id).use { session ->
                    apk.inputStream().use { input ->
                        session.openWrite(ASSET, 0, apk.length()).use { output ->
                            input.copyTo(output)
                            session.fsync(output)
                        }
                    }
                    val told = Intent(context, UpdateReceiver::class.java).setPackage(context.packageName)
                    val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
                    session.commit(PendingIntent.getBroadcast(context, id, told, flags).intentSender)
                }
            }
            result.exceptionOrNull()?.let { error ->
                main.post { state = State.Failed("The new version couldn’t be installed: ${error.message}") }
            }
        }
    }

    /** What Android's installer said. */
    internal fun installerSaid(status: Int, message: String?, confirm: Intent?) {
        val version = (state as? State.Installing)?.version ?: return
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> confirm?.let {
                val from = App.activity ?: context
                if (from === context) it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                from.startActivity(it)
            }
            PackageInstaller.STATUS_SUCCESS -> Unit
            // The user said no; the update is still on offer.
            PackageInstaller.STATUS_FAILURE_ABORTED -> state = State.Available(version)
            else -> state = State.Failed(message?.let { "The new version couldn’t be installed: $it" } ?: "The new version couldn’t be installed.")
        }
    }

    companion object {
        private const val RELEASES = "https://github.com/yekta/wonnet/releases"
        /** The app's file in every release. */
        const val ASSET = "Wonnet.apk"
        private const val CHECK_EVERY = 6 * 3600 * 1000L

        private fun asset(version: String) = "$RELEASES/download/v$version/$ASSET"

        /** The updater of the running app, for the installer's answer. It holds the application's context only. */
        @SuppressLint("StaticFieldLeak")
        var shared: AppUpdater? = null
    }
}

/** Takes the installer's answers to `AppUpdater`. */
class UpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
        val confirm = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(Intent.EXTRA_INTENT)
        }
        AppUpdater.shared?.installerSaid(status, message, confirm)
    }
}

object Version {
    /** How a version is shown: `v0.1.188`. */
    fun label(version: String) = "v$version"

    /** Whether `version` comes before `other`, comparing their numbers from the left. */
    fun isOlder(version: String, other: String?): Boolean {
        if (other == null || version.isEmpty() || other.isEmpty()) return false
        val numbers = { text: String -> text.split('.').map { it.toIntOrNull() ?: 0 } }
        val ours = numbers(version)
        val theirs = numbers(other)
        for (index in 0 until maxOf(ours.size, theirs.size)) {
            val left = ours.getOrElse(index) { 0 }
            val right = theirs.getOrElse(index) { 0 }
            if (left != right) return left < right
        }
        return false
    }
}
