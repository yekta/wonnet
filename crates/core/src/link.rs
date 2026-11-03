//! The socket to the server. It reconnects by itself, waiting longer each time it fails, and
//! tells the core when it opened so the core can say hello and resend what waits. The server
//! answers every ping, so a socket that goes quiet is taken for dead and made anew.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mail_protocol::wire::{ClientMessage, ServerMessage};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message as Frame;

pub enum LinkEvent {
    Opened,
    Message(ServerMessage),
    Closed(Option<String>),
}

pub struct Link {
    out: mpsc::UnboundedSender<ClientMessage>,
    checks: Arc<Notify>,
    task: JoinHandle<()>,
}

const PING_EVERY: Duration = Duration::from_secs(25);
/// Two pings gone unanswered.
const QUIET_FOR: Duration = Duration::from_secs(60);
/// How long a check waits for the server to answer its ping.
const CHECK_WAIT: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub fn socket_url(server: &str) -> String {
    let server = server.trim_end_matches('/');
    let url = match server.strip_prefix("https://") {
        Some(rest) => format!("wss://{rest}"),
        None => format!("ws://{}", server.strip_prefix("http://").unwrap_or(server)),
    };
    format!("{url}/sync")
}

fn ping() -> Frame {
    Frame::text(serde_json::to_string(&ClientMessage::Ping).unwrap_or_default())
}

impl Link {
    pub fn start(server: &str, events: impl Fn(LinkEvent) + Send + Sync + 'static) -> Self {
        let url = socket_url(server);
        let (out, mut outbox) = mpsc::unbounded_channel::<ClientMessage>();
        let checks = Arc::new(Notify::new());
        let asked = checks.clone();
        let task = tokio::spawn(async move {
            let mut backoff = Duration::from_millis(500);
            loop {
                match tokio_tungstenite::connect_async(url.as_str()).await {
                    Ok((socket, _)) => {
                        backoff = Duration::from_millis(500);
                        while outbox.try_recv().is_ok() {}
                        events(LinkEvent::Opened);
                        let (mut sink, mut stream) = socket.split();
                        let mut ping_due = tokio::time::interval(PING_EVERY);
                        ping_due.tick().await;
                        let mut heard = Instant::now();
                        let mut answer_by: Option<Instant> = None;
                        let error = loop {
                            let dead_at = answer_by.unwrap_or(heard + QUIET_FOR);
                            tokio::select! {
                                message = outbox.recv() => {
                                    let Some(message) = message else { return };
                                    let Ok(text) = serde_json::to_string(&message) else { continue };
                                    if let Err(error) = sink.send(Frame::text(text)).await {
                                        break Some(error.to_string());
                                    }
                                }
                                frame = stream.next() => {
                                    heard = Instant::now();
                                    answer_by = None;
                                    match frame {
                                        Some(Ok(Frame::Text(text))) => match serde_json::from_str::<ServerMessage>(&text) {
                                            Ok(message) => events(LinkEvent::Message(message)),
                                            Err(error) => tracing::warn!("ignored a message from the server: {error}"),
                                        },
                                        Some(Ok(Frame::Close(_))) | None => break None,
                                        Some(Err(error)) => break Some(error.to_string()),
                                        Some(Ok(_)) => {}
                                    }
                                }
                                _ = ping_due.tick() => {
                                    if let Err(error) = sink.send(ping()).await {
                                        break Some(error.to_string());
                                    }
                                }
                                _ = asked.notified() => {
                                    if let Err(error) = sink.send(ping()).await {
                                        break Some(error.to_string());
                                    }
                                    answer_by.get_or_insert(Instant::now() + CHECK_WAIT);
                                }
                                _ = tokio::time::sleep_until(dead_at) => break Some("The server stopped answering.".into()),
                            }
                        };
                        events(LinkEvent::Closed(error));
                    }
                    Err(error) => events(LinkEvent::Closed(Some(error.to_string()))),
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        });
        Self { out, checks, task }
    }

    pub fn send(&self, message: ClientMessage) {
        let _ = self.out.send(message);
    }

    /// Pings the server now; without an answer soon, the socket is made anew.
    pub fn check(&self) {
        self.checks.notify_one();
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn socket_urls_follow_the_scheme() {
        assert_eq!(super::socket_url("https://mail.example.com/"), "wss://mail.example.com/sync");
        assert_eq!(super::socket_url("http://localhost:3000"), "ws://localhost:3000/sync");
    }
}
