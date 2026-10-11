//! The JSON of the server's HTTP API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRequest {
    pub code: String,
    pub verifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkTicket {
    pub ticket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapAccountRequest {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevLoginRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// The answer to `POST /api/uploads`, whose body is the file. Its name comes in `x-file-name`
/// (percent-encoded) and its type in `content-type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub id: String,
}

/// `POST /api/devices`: a device to push notifications to. `token` is its APNs token in hex,
/// `topic` the app's bundle id, `sandbox` true for a development build, `key` the 32 bytes
/// (base64) that what a notification says is sealed with (AES-256-GCM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRequest {
    pub token: String,
    pub topic: String,
    pub sandbox: bool,
    pub key: String,
}
