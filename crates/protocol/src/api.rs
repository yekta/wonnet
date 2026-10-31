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

/// `POST /api/devices`: a device to push notifications to. For `apns`, `token` is its APNs token
/// in hex, `topic` the app's bundle id and `sandbox` true for a development build; for `fcm`,
/// `token` is its FCM registration token and `topic` the app's package name. `key` is the 32
/// bytes (base64) that what a notification says is sealed with (AES-256-GCM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRequest {
    #[serde(default)]
    pub platform: Platform,
    pub token: String,
    pub topic: String,
    #[serde(default)]
    pub sandbox: bool,
    pub key: String,
}

/// Who carries a device's push notifications.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// Apple's push service, for the iOS app.
    #[default]
    Apns,
    /// Firebase Cloud Messaging, for the Android app.
    Fcm,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
        }
    }
}
