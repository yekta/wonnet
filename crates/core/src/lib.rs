//! The client core: the local copy of the mail, the link to the server and everything the apps
//! draw. The Apple apps link it as a static library (`ffi.rs`: JSON commands in, JSON events
//! out); the Android app through JNI (`apps/android/core`) and the desktop app as a crate, both
//! with `Running`.

pub mod api;
pub mod core;
mod demo;
pub mod ffi;
mod http;
mod link;
pub mod render;
mod search;
pub mod store;
mod undo;

/// The core in a runtime of its own, as the apps run it.
pub struct Running {
    _runtime: tokio::runtime::Runtime,
    handle: core::Handle,
}

impl Running {
    /// `sink` gets every event, on one of the core's threads.
    pub fn start(config: api::Config, sink: core::Sink) -> anyhow::Result<Self> {
        init_logging();
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build()?;
        let handle = {
            let _guard = runtime.enter();
            core::start(config, sink)?
        };
        Ok(Self { _runtime: runtime, handle })
    }

    /// The answer arrives as a `reply` event with this `id`.
    pub fn send(&self, id: u64, command: api::Command) {
        self.handle.send(id, command);
    }
}

fn init_logging() {
    let filter = std::env::var("MAIL_LOG").unwrap_or_else(|_| "warn,mail_core=info".to_string());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_ansi(false).try_init();
}
