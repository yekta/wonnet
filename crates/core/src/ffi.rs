//! The C functions the apps call. Everything crossing the boundary is JSON text: commands go in
//! through `mail_core_command`, events come out through the callback given to `mail_core_start`.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, OnceLock};

use crate::Running;
use crate::api::{Config, Envelope, Event};

/// Called on a background thread with one event as NUL-terminated JSON. The text is only valid
/// during the call.
pub type EventCallback = extern "C" fn(event_json: *const c_char, context: *mut c_void);

static RUNNING: OnceLock<Running> = OnceLock::new();

/// The app's context pointer, which it promises is safe to hand to any thread.
struct Context(*mut c_void);
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

fn text(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(pointer) }.to_str().ok().map(String::from)
}

/// Starts the core. `config_json` is an `api::Config`. Returns false if it couldn't start;
/// calling it again once started does nothing.
#[unsafe(no_mangle)]
pub extern "C" fn mail_core_start(config_json: *const c_char, callback: EventCallback, context: *mut c_void) -> bool {
    if RUNNING.get().is_some() {
        return true;
    }
    let Some(config) = text(config_json).and_then(|json| serde_json::from_str::<Config>(&json).ok()) else {
        return false;
    };
    let context = Context(context);
    let sink = Arc::new(move |event: Event| {
        let context = &context;
        let Ok(json) = serde_json::to_string(&event) else { return };
        let Ok(json) = CString::new(json) else { return };
        callback(json.as_ptr(), context.0);
    });
    match Running::start(config, sink) {
        Ok(running) => RUNNING.set(running).is_ok(),
        Err(error) => {
            tracing::error!("the core couldn't start: {error:#}");
            false
        }
    }
}

/// Sends one command: an `api::Envelope` as JSON. The answer arrives as a `reply` event.
#[unsafe(no_mangle)]
pub extern "C" fn mail_core_command(command_json: *const c_char) {
    let Some(running) = RUNNING.get() else { return };
    let Some(json) = text(command_json) else { return };
    match serde_json::from_str::<Envelope>(&json) {
        Ok(envelope) => running.send(envelope.id, envelope.command),
        Err(error) => tracing::warn!("ignored a malformed command: {error}"),
    }
}
