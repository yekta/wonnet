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

/// `GET /api/connect/{key}`: an application's request to reach the user's mail, made when it sent
/// the user's browser to `/oauth/authorize`. `key` is in the `wonnet://connect` link the page
/// opens the app with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    /// The name the application gave itself.
    pub client: String,
    /// The application is who it says it is: its description is published on its own website.
    pub verified: bool,
    /// The website that publishes its description, when it has one.
    pub publisher: Option<String>,
    /// Where the browser goes back to once the request is answered: `claude.ai`.
    pub redirect_host: String,
    /// It goes back to an application on the same device rather than to a website.
    pub on_device: bool,
}

/// `POST /api/connect/{key}`: the user's answer. `access` is what the application may do, needed
/// when it is approved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectAnswer {
    pub approve: bool,
    #[serde(default)]
    pub access: Option<crate::access::Access>,
}

/// `PUT /api/connected-apps/{id}`: what a connected application may do from now on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessChange {
    pub access: crate::access::Access,
}

/// `GET /api/connected-apps`: the address applications connect to, and the ones that can reach
/// the user's mail, newest first. `DELETE /api/connected-apps/{id}` revokes one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedApps {
    pub address: String,
    pub apps: Vec<ConnectedApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedApp {
    pub id: String,
    pub client: String,
    pub verified: bool,
    pub publisher: Option<String>,
    pub access: crate::access::Access,
    /// When it was approved and last used, in Unix milliseconds.
    pub approved_at: i64,
    pub used_at: Option<i64>,
}
