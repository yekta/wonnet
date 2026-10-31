package app.wonnet

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Application
import android.content.Context
import android.os.Bundle
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ProcessLifecycleOwner
import app.wonnet.core.MailStore
import app.wonnet.core.Notifier
import java.lang.ref.WeakReference

/**
 * The app's process: the store, made once for the screen, a push or a notification's button,
 * whichever wakes the app first; and the activity on screen, for what needs one.
 */
class App : Application() {
    override fun onCreate() {
        super.onCreate()
        context = this
        Notifier.install(this)
        registerActivityLifecycleCallbacks(object : ActivityLifecycleCallbacks {
            override fun onActivityResumed(activity: Activity) {
                current = WeakReference(activity)
            }

            override fun onActivityDestroyed(activity: Activity) {
                if (current.get() === activity) current = WeakReference(null)
            }

            override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit
            override fun onActivityStarted(activity: Activity) = Unit
            override fun onActivityPaused(activity: Activity) = Unit
            override fun onActivityStopped(activity: Activity) = Unit
            override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
        })
    }

    companion object {
        /** The application's context, which lives as long as the process. */
        @SuppressLint("StaticFieldLeak")
        lateinit var context: Context
            private set

        private var current = WeakReference<Activity>(null)

        /** The activity on screen, for printing, the browser and the system's questions. */
        val activity: Activity? get() = current.get()

        val store: MailStore by lazy { MailStore(context) }

        /** Whether the app is on screen. */
        val frontmost: Boolean get() = ProcessLifecycleOwner.get().lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)
    }
}
