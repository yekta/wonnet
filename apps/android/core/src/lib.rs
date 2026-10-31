//! The Android app's way into the core: the JNI functions of `app.wonnet.core.Native`, around
//! `mail_core::Running`. Commands go in as JSON; events wait in a queue that the app's event
//! thread takes one `next` at a time, so no thread of the core calls into Java.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use mail_core::Running;
use mail_core::api::{Config, Envelope, Event};

struct Queue {
    sender: Sender<String>,
    receiver: Mutex<Receiver<String>>,
}

static EVENTS: OnceLock<Queue> = OnceLock::new();
static RUNNING: OnceLock<Running> = OnceLock::new();

fn events() -> &'static Queue {
    EVENTS.get_or_init(|| {
        let (sender, receiver) = channel();
        Queue { sender, receiver: Mutex::new(receiver) }
    })
}

/// Starts the core with an `api::Config` as JSON; once started, starting again does nothing.
/// `context` is the application's, which the system's certificate checks need.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_wonnet_core_Native_start<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    context: JObject<'caller>,
    config: JString<'caller>,
) -> bool {
    env.with_env(|_env| -> Result<bool, jni::errors::Error> {
        if RUNNING.get().is_some() {
            return Ok(true);
        }
        #[cfg(target_os = "android")]
        {
            logcat::forward_stderr();
            rustls_platform_verifier::android::init_with_env(_env, context)?;
        }
        #[cfg(not(target_os = "android"))]
        let _ = context;
        Ok(start(&config.to_string()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

fn start(config: &str) -> bool {
    let Ok(config) = serde_json::from_str::<Config>(config) else { return false };
    let queue = events().sender.clone();
    let sink = Arc::new(move |event: Event| {
        let Ok(json) = serde_json::to_string(&event) else { return };
        let _ = queue.send(json);
    });
    match Running::start(config, sink) {
        Ok(running) => RUNNING.set(running).is_ok(),
        Err(error) => {
            tracing::error!("the core couldn't start: {error:#}");
            false
        }
    }
}

/// Sends one command, an `api::Envelope` as JSON. The answer arrives as a `reply` event.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_wonnet_core_Native_send<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    command: JString<'caller>,
) {
    env.with_env(|_env| -> Result<(), jni::errors::Error> {
        let Some(running) = RUNNING.get() else { return Ok(()) };
        match serde_json::from_str::<Envelope>(&command.to_string()) {
            Ok(envelope) => running.send(envelope.id, envelope.command),
            Err(error) => tracing::warn!("ignored a malformed command: {error}"),
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Waits for the next event and answers with it as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_wonnet_core_Native_next<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> JString<'caller> {
    env.with_env(|env| -> Result<JString<'caller>, jni::errors::Error> {
        let event = events().receiver.lock().ok().and_then(|receiver| receiver.recv().ok()).unwrap_or_default();
        JString::from_str(env, event)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The core logs to stderr, which Android throws away: this sends it to logcat instead.
#[cfg(target_os = "android")]
mod logcat {
    use std::ffi::{CString, c_char};
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;
    use std::sync::Once;

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(priority: i32, tag: *const c_char, text: *const c_char) -> i32;
    }

    const INFO: i32 = 4;

    pub fn forward_stderr() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let mut fds = [0; 2];
            if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 || unsafe { libc::dup2(fds[1], libc::STDERR_FILENO) } < 0 {
                return;
            }
            let reader = BufReader::new(unsafe { File::from_raw_fd(fds[0]) });
            std::thread::spawn(move || {
                let tag = c"wonnet";
                for line in reader.lines().map_while(Result::ok) {
                    let Ok(text) = CString::new(line) else { continue };
                    unsafe { __android_log_write(INFO, tag.as_ptr(), text.as_ptr()) };
                }
            });
        });
    }
}
