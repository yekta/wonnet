//! What the server and the client core agree on: the mail they sync, the messages on the sync
//! socket and the JSON of the server's HTTP API.

pub mod access;
pub mod api;
pub mod ops;
pub mod rules;
pub mod splits;
#[cfg(feature = "tls")]
pub mod tls;
pub mod types;
pub mod wire;

pub use ops::{MessageState, Op};
pub use rules::Rule;
pub use splits::Split;
pub use types::*;

/// Bump when an old client or server could no longer understand the other.
pub const PROTOCOL_VERSION: u32 = 3;
/// Where the server sends the browser once a Google or Microsoft sign-in is done; the apps own this scheme.
pub const APP_REDIRECT: &str = "wonnet://auth";
/// Opens the app on an application's request to reach the user's mail: `wonnet://connect?key=…`.
pub const APP_CONNECT: &str = "wonnet://connect";
/// How many messages one `Changes` batch holds at most.
pub const BATCH_SIZE: usize = 500;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
