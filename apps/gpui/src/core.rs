//! The app's end of the Rust core, which runs in this process: commands go in typed, answers
//! come back as JSON read on the core's threads, and events are handed to the window through a
//! channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use mail_core::Running;
use mail_core::api::{Command, Config, Event};
use serde::de::DeserializeOwned;
use serde_json::Value;

type Answer = Box<dyn FnOnce(bool, Value) + Send>;

/// A handle on the running core; clones share it.
#[derive(Clone)]
pub struct Core {
    running: Arc<Running>,
    next: Arc<AtomicU64>,
    answers: Arc<Mutex<HashMap<u64, Answer>>>,
}

impl Core {
    /// Starts the core; its events that aren't answers arrive on `events`.
    pub fn start(config: Config) -> anyhow::Result<(Self, async_channel::Receiver<Event>)> {
        let (sender, events) = async_channel::unbounded();
        let answers: Arc<Mutex<HashMap<u64, Answer>>> = Arc::default();
        let waiting = answers.clone();
        let sink = Arc::new(move |event: Event| {
            if let Event::Reply { id, ok, value } = event {
                let answer = waiting.lock().ok().and_then(|mut answers| answers.remove(&id));
                if let Some(answer) = answer {
                    answer(ok, value);
                }
                return;
            }
            let _ = sender.try_send(event);
        });
        let running = Running::start(config, sink)?;
        Ok((Self { running: Arc::new(running), next: Arc::new(AtomicU64::new(1)), answers }, events))
    }

    /// Sends a command and waits for its answer, read on the core's thread.
    pub fn call<T: DeserializeOwned + Send + 'static>(
        &self,
        command: Command,
    ) -> impl Future<Output = Result<T, String>> + use<T> {
        let (sender, receiver) = oneshot::channel();
        let id = self.register(Box::new(move |ok, value| {
            let _ = sender.send(read(ok, value));
        }));
        self.running.send(id, command);
        async move { receiver.await.unwrap_or_else(|_| Err("The core stopped.".into())) }
    }

    /// Sends a command whose answer doesn't matter.
    pub fn send(&self, command: Command) {
        let id = self.register(Box::new(|_, _| {}));
        self.running.send(id, command);
    }

    fn register(&self, answer: Answer) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut answers) = self.answers.lock() {
            answers.insert(id, answer);
        }
        id
    }
}

fn read<T: DeserializeOwned>(ok: bool, value: Value) -> Result<T, String> {
    if !ok {
        let message = value.get("error").and_then(Value::as_str).unwrap_or("Something went wrong.");
        return Err(message.to_string());
    }
    serde_json::from_value(value).map_err(|error| format!("The core's answer can't be read: {error}"))
}
