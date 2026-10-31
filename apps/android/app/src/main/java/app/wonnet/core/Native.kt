package app.wonnet.core

import android.content.Context

/** The Rust core, as apps/android/core gives it: JSON commands in, JSON events out. */
object Native {
    init {
        System.loadLibrary("wonnet_core")
    }

    /** Starts the core with an `api::Config` as JSON. */
    external fun start(context: Context, config: String): Boolean

    /** Sends one command, an `api::Envelope` as JSON. */
    external fun send(command: String)

    /** Waits for the next event and answers with it as JSON. */
    external fun next(): String
}
