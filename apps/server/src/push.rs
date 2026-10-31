//! Push notifications through APNs for the iOS app and FCM for the Android app, which lose the
//! sync socket in the background. New inbox mail goes to each of the user's devices as an alert
//! whose sender, subject and snippet are sealed with that device's key: the app opens them, and
//! Apple and Google only carry ciphertext. When pushed mail is read or leaves the inbox, a silent
//! push tells the devices to take its notification down. Both carry the inbox's unread count.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use mail_protocol::api::Platform;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, RSA_PKCS1_SHA256, RsaKeyPair};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::AppState;
use crate::config::{ApnsConfig, Config, FcmConfig};

/// Apple takes a provider token for an hour and asks for a new one at most every 20 minutes.
const TOKEN_LIFETIME: Duration = Duration::from_secs(40 * 60);
const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// How long changes gather before the devices hear of them.
const SETTLE: Duration = Duration::from_secs(1);
/// The most alerts one batch of new mail makes; the badge counts the rest.
const MOST_ALERTS: usize = 5;

pub const KEY_LEN: usize = 32;

pub struct Pusher {
    apns: Option<Apns>,
    fcm: Option<Fcm>,
    changed: Mutex<HashSet<Uuid>>,
    wake: Notify,
    /// The badge each user's devices were last given.
    badges: Mutex<HashMap<Uuid, i64>>,
}

impl Pusher {
    pub fn new(config: &Config) -> Self {
        let apns = config.apns.as_ref().and_then(|config| {
            Apns::new(config).inspect_err(|error| tracing::error!("pushes to iPhones are off: {error:#}")).ok()
        });
        let fcm = config.fcm.as_ref().and_then(|config| {
            Fcm::new(config).inspect_err(|error| tracing::error!("pushes to Android phones are off: {error:#}")).ok()
        });
        Self { apns, fcm, changed: Default::default(), wake: Notify::new(), badges: Default::default() }
    }

    fn on(&self) -> bool {
        self.apns.is_some() || self.fcm.is_some()
    }

    fn reaches(&self, platform: Platform) -> bool {
        match platform {
            Platform::Apns => self.apns.is_some(),
            Platform::Fcm => self.fcm.is_some(),
        }
    }

    async fn send(&self, device: &Device, push: &Push) -> Sent {
        match (device.platform(), &self.apns, &self.fcm) {
            (Platform::Apns, Some(apns), _) => apns.send(device, push).await,
            (Platform::Fcm, _, Some(fcm)) => fcm.send(device, push).await,
            _ => Sent::Failed,
        }
    }

    /// The user's mail changed: their devices may have something to hear.
    pub fn changed(&self, user_id: Uuid) {
        if !self.on() {
            return;
        }
        self.changed.lock().unwrap().insert(user_id);
        self.wake.notify_one();
    }
}

pub fn start(state: AppState) {
    if !state.pusher.on() {
        return;
    }
    tokio::spawn(async move {
        let stopping = state.stopping.subscribe();
        loop {
            state.pusher.wake.notified().await;
            tokio::time::sleep(SETTLE).await;
            if *stopping.borrow() {
                return;
            }
            let users: Vec<Uuid> = state.pusher.changed.lock().unwrap().drain().collect();
            for user_id in users {
                if let Err(error) = push_user(&state, user_id).await {
                    tracing::warn!("couldn't push to a user's devices: {error:#}");
                }
            }
        }
    });
}

#[derive(sqlx::FromRow)]
struct Device {
    platform: String,
    token: String,
    topic: String,
    sandbox: bool,
    key: Vec<u8>,
}

impl Device {
    fn platform(&self) -> Platform {
        match self.platform == Platform::Fcm.as_str() {
            true => Platform::Fcm,
            false => Platform::Apns,
        }
    }
}

#[derive(sqlx::FromRow)]
struct Fresh {
    id: Uuid,
    account_id: Uuid,
    thread_id: String,
    from_name: Option<String>,
    from_email: String,
    subject: String,
    snippet: String,
}

/// Inbox mail, unread and not from the user, that came since its account was added, in the last
/// day, and wasn't pushed yet; marked pushed as it is read. Accounts with `notify:<id>` false are
/// left out.
const FRESH: &str = "WITH fresh AS (
        SELECT m.id, m.account_id, m.thread_id, m.from_name, m.from_email, m.subject, m.snippet, m.date
        FROM messages m JOIN accounts a ON a.id = m.account_id
        WHERE m.user_id = $1 AND m.unread AND NOT m.deleted AND 'inbox' = ANY(m.labels)
            AND m.date > now() - interval '1 day' AND m.date > a.created_at AND NOT a.deleted
            AND NOT EXISTS (SELECT 1 FROM pushes WHERE pushes.message_id = m.id)
            AND NOT EXISTS (SELECT 1 FROM preferences p WHERE p.user_id = $1 AND NOT p.deleted
                AND p.key = 'notify:' || m.account_id AND p.value = 'false'::jsonb)
            AND m.from_email <> lower(a.address)
            AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(a.identities) identity
                WHERE lower(identity->>'email') = m.from_email)),
    pushed AS (
        INSERT INTO pushes (message_id, user_id) SELECT id, $1 FROM fresh
        ON CONFLICT DO NOTHING RETURNING message_id)
    SELECT fresh.id, fresh.account_id, fresh.thread_id, fresh.from_name, fresh.from_email, fresh.subject,
        fresh.snippet
    FROM fresh JOIN pushed ON pushed.message_id = fresh.id ORDER BY fresh.date DESC";

async fn push_user(state: &AppState, user_id: Uuid) -> anyhow::Result<()> {
    let pusher = &state.pusher;
    let db = &state.background;
    let mut devices: Vec<Device> =
        sqlx::query_as("SELECT platform, token, topic, sandbox, key FROM devices WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(db)
            .await?;
    devices.retain(|device| pusher.reaches(device.platform()));
    if devices.is_empty() {
        state.pusher.badges.lock().unwrap().remove(&user_id);
        return Ok(());
    }
    let fresh: Vec<Fresh> = sqlx::query_as(FRESH).bind(user_id).fetch_all(db).await?;
    let settled: Vec<(Uuid, String)> = sqlx::query_as(
        "DELETE FROM pushes USING messages
         WHERE pushes.user_id = $1 AND messages.id = pushes.message_id
             AND (NOT messages.unread OR messages.deleted OR NOT 'inbox' = ANY(messages.labels))
         RETURNING messages.account_id, messages.thread_id",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    let badge: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT (account_id, thread_id)) FROM messages
         WHERE user_id = $1 AND unread AND NOT deleted AND 'inbox' = ANY(labels)",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;
    let badge_changed = state.pusher.badges.lock().unwrap().insert(user_id, badge) != Some(badge);

    let settled: HashSet<String> = settled.iter().map(|(account, thread)| thread_key(*account, thread)).collect();
    let settled: Vec<String> = settled.into_iter().collect();
    for device in devices {
        let Some(key) = state.sealer.open(&device.key).and_then(|key| <[u8; KEY_LEN]>::try_from(key).ok()) else {
            tracing::warn!("device {} of user {user_id} has a key that doesn't open", short(&device.token));
            continue;
        };
        let mut pushes: Vec<Push> =
            fresh.iter().take(MOST_ALERTS).rev().map(|mail| alert(state, &key, mail, badge)).collect();
        if !settled.is_empty() || (fresh.is_empty() && badge_changed) {
            pushes.push(settle(&key, &settled, badge));
        }
        let token = short(&device.token);
        let (alerts, total) = (fresh.len().min(MOST_ALERTS), pushes.len());
        if total == 0 {
            continue;
        }
        let mut sent = 0;
        for push in pushes {
            match pusher.send(&device, &push).await {
                Sent::Ok => sent += 1,
                Sent::Failed => {}
                Sent::Gone => {
                    sqlx::query("DELETE FROM devices WHERE token = $1").bind(&device.token).execute(db).await?;
                    let carrier = device.platform().as_str();
                    tracing::warn!("forgot device {token} of user {user_id}: {carrier} no longer takes its token");
                    break;
                }
            }
        }
        tracing::info!(
            "pushed {sent} of {total} to device {token} of user {user_id} ({alerts} new mail, badge {badge})"
        );
    }
    Ok(())
}

/// The start of a device token, enough to tell devices apart in the logs.
pub fn short(token: &str) -> &str {
    token.get(..8).unwrap_or(token)
}

/// A thread as the core names it.
fn thread_key(account_id: Uuid, thread_id: &str) -> String {
    format!("{account_id}:{thread_id}")
}

enum Push {
    /// New mail; a newer push of the same message replaces its notification.
    Alert { message: Uuid, sealed: String, badge: i64 },
    /// Threads read or gone from the inbox, whose notifications come down.
    Settle { sealed: String, badge: i64 },
}

fn alert(state: &AppState, key: &[u8; KEY_LEN], mail: &Fresh, badge: i64) -> Push {
    let from = mail_protocol::Address { name: mail.from_name.clone(), email: mail.from_email.clone() };
    let content = json!({
        "thread": thread_key(mail.account_id, &mail.thread_id),
        "account": mail.account_id,
        "message": mail.id,
        "from": from.display(),
        "email": from.email,
        "initials": from.initials(),
        "avatar": crate::avatars::signed_url(state, &from.email),
        "subject": mail.subject,
        "snippet": mail.snippet,
    });
    Push::Alert { message: mail.id, sealed: seal(key, &content), badge }
}

fn settle(key: &[u8; KEY_LEN], settled: &[String], badge: i64) -> Push {
    Push::Settle { sealed: seal(key, &json!({ "settled": settled })), badge }
}

/// AES-256-GCM: the nonce, the ciphertext and the tag, in base64, as CryptoKit's sealed box.
pub fn seal(key: &[u8; KEY_LEN], content: &Value) -> String {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce: [u8; 12] = crate::random_bytes();
    let sealed = cipher.encrypt(&Nonce::from(nonce), content.to_string().as_bytes()).expect("AES-GCM encrypts");
    STANDARD.encode([nonce.as_slice(), &sealed].concat())
}

#[derive(PartialEq)]
enum Sent {
    Ok,
    /// The token is no longer good for the app: the device is forgotten.
    Gone,
    Failed,
}

struct Apns {
    key: EcdsaKeyPair,
    key_id: String,
    team_id: String,
    production_url: String,
    sandbox_url: String,
    http: reqwest::Client,
    token: Mutex<Option<(String, Instant)>>,
}

impl Apns {
    fn new(config: &ApnsConfig) -> anyhow::Result<Self> {
        let der = pem_body(&config.key).ok_or_else(|| anyhow::anyhow!("APNS_KEY isn't a .p8 file's contents"))?;
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &SystemRandom::new())
            .map_err(|_| anyhow::anyhow!("APNS_KEY isn't a P-256 key in PKCS #8"))?;
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            key,
            key_id: config.key_id.clone(),
            team_id: config.team_id.clone(),
            production_url: config.production_url.clone(),
            sandbox_url: config.sandbox_url.clone(),
            http,
            token: Mutex::new(None),
        })
    }

    /// The provider token: a JWT signed with the key, kept for a while.
    fn token(&self) -> String {
        let mut cached = self.token.lock().unwrap();
        if let Some((token, made)) = &*cached
            && made.elapsed() < TOKEN_LIFETIME
        {
            return token.clone();
        }
        let header = URL_SAFE_NO_PAD.encode(json!({ "alg": "ES256", "kid": self.key_id }).to_string());
        let claims = json!({ "iss": self.team_id, "iat": chrono::Utc::now().timestamp() });
        let unsigned = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
        let signature = self.key.sign(&SystemRandom::new(), unsigned.as_bytes()).expect("ECDSA signs");
        let token = format!("{unsigned}.{}", URL_SAFE_NO_PAD.encode(signature.as_ref()));
        *cached = Some((token.clone(), Instant::now()));
        token
    }

    /// An alert reads "New mail" without the extension (or its key). A settle is an alert with no
    /// text, not a background push: iOS sets its badge even when the app was quit, and throttles
    /// background pushes or drops them.
    fn payload(push: &Push) -> Value {
        match push {
            Push::Alert { sealed, badge, .. } => json!({
                "aps": {
                    "alert": { "title": "New mail" },
                    "sound": "default",
                    "badge": badge,
                    "mutable-content": 1,
                    "category": "message",
                },
                "sealed": sealed,
            }),
            Push::Settle { sealed, badge } => json!({
                "aps": { "content-available": 1, "badge": badge },
                "sealed": sealed,
            }),
        }
    }

    async fn send(&self, device: &Device, push: &Push) -> Sent {
        let base = if device.sandbox { &self.sandbox_url } else { &self.production_url };
        let mut request = self
            .http
            .post(format!("{base}/3/device/{}", device.token))
            .bearer_auth(self.token())
            .header("apns-topic", &device.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .json(&Self::payload(push));
        if let Push::Alert { message, .. } = push {
            request = request.header("apns-collapse-id", message.to_string());
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!("APNs can't be reached: {error}");
                return Sent::Failed;
            }
        };
        let status = response.status();
        if status.is_success() {
            return Sent::Ok;
        }
        let body: Value = response.json().await.unwrap_or_default();
        let reason = body["reason"].as_str().unwrap_or_default();
        let environment = if device.sandbox { "sandbox" } else { "production" };
        match reason {
            "BadDeviceToken" | "DeviceTokenNotForTopic" | "Unregistered" => {
                tracing::warn!(
                    "APNs ({environment}, {}) refused device {}: {reason}",
                    device.topic,
                    short(&device.token)
                );
                return Sent::Gone;
            }
            "ExpiredProviderToken" => *self.token.lock().unwrap() = None,
            _ => {}
        }
        if status == reqwest::StatusCode::GONE {
            tracing::warn!("APNs ({environment}) says device {} is gone: {status} {reason}", short(&device.token));
            return Sent::Gone;
        }
        tracing::warn!("APNs ({environment}, {}) refused a push: {status} {reason}", device.topic);
        Sent::Failed
    }
}

/// A Firebase service account's JSON key.
#[derive(Deserialize)]
struct ServiceAccount {
    project_id: String,
    private_key_id: String,
    private_key: String,
    client_email: String,
    token_uri: String,
}

struct Fcm {
    key: RsaKeyPair,
    key_id: String,
    client_email: String,
    /// The service account's `token_uri`, which the JWT is for.
    audience: String,
    token_url: String,
    send_url: String,
    http: reqwest::Client,
    /// The access token and when to stop using it.
    token: tokio::sync::Mutex<Option<(String, Instant)>>,
}

impl Fcm {
    fn new(config: &FcmConfig) -> anyhow::Result<Self> {
        let account: ServiceAccount = serde_json::from_str(&config.service_account)
            .map_err(|error| anyhow::anyhow!("FCM_SERVICE_ACCOUNT isn't a service account's JSON key: {error}"))?;
        let der = pem_body(&account.private_key)
            .ok_or_else(|| anyhow::anyhow!("FCM_SERVICE_ACCOUNT's private key isn't PEM"))?;
        let key = RsaKeyPair::from_pkcs8(&der)
            .map_err(|_| anyhow::anyhow!("FCM_SERVICE_ACCOUNT's private key isn't an RSA key in PKCS #8"))?;
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            key,
            key_id: account.private_key_id,
            client_email: account.client_email,
            token_url: config.token_url.clone().unwrap_or_else(|| account.token_uri.clone()),
            audience: account.token_uri,
            send_url: format!("{}/v1/projects/{}/messages:send", config.url, account.project_id),
            http,
            token: Default::default(),
        })
    }

    /// An OAuth access token, for a JWT signed with the service account's key, kept until shortly
    /// before it expires.
    async fn access_token(&self) -> anyhow::Result<String> {
        let mut cached = self.token.lock().await;
        if let Some((token, until)) = &*cached
            && Instant::now() < *until
        {
            return Ok(token.clone());
        }
        let now = chrono::Utc::now().timestamp();
        let header = json!({ "alg": "RS256", "typ": "JWT", "kid": self.key_id });
        let claims = json!({
            "iss": self.client_email,
            "scope": FCM_SCOPE,
            "aud": self.audience,
            "iat": now,
            "exp": now + 3600,
        });
        let unsigned =
            format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), URL_SAFE_NO_PAD.encode(claims.to_string()));
        let mut signature = vec![0; self.key.public().modulus_len()];
        self.key
            .sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), unsigned.as_bytes(), &mut signature)
            .map_err(|_| anyhow::anyhow!("RSA couldn't sign"))?;
        let assertion = format!("{unsigned}.{}", URL_SAFE_NO_PAD.encode(signature));
        let form = [("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &assertion)];
        let response: Value =
            self.http.post(&self.token_url).form(&form).send().await?.error_for_status()?.json().await?;
        let Some(token) = response["access_token"].as_str() else {
            anyhow::bail!("Google answered without an access token");
        };
        let lifetime = response["expires_in"].as_u64().unwrap_or(3600);
        let until = Instant::now() + Duration::from_secs(lifetime.saturating_sub(5 * 60));
        *cached = Some((token.to_string(), until));
        Ok(token.to_string())
    }

    /// Data only, all strings: the app opens `sealed` and posts the notification itself.
    fn message(device: &Device, push: &Push) -> Value {
        let (data, android) = match push {
            Push::Alert { message, sealed, badge } => (
                json!({ "kind": "alert", "sealed": sealed, "badge": badge.to_string() }),
                json!({ "priority": "HIGH", "collapse_key": message.to_string() }),
            ),
            Push::Settle { sealed, badge } => (
                json!({ "kind": "settle", "sealed": sealed, "badge": badge.to_string() }),
                json!({ "priority": "NORMAL" }),
            ),
        };
        json!({ "message": { "token": device.token, "data": data, "android": android } })
    }

    async fn send(&self, device: &Device, push: &Push) -> Sent {
        let message = Self::message(device, push);
        // A second try after FCM turned the access token down.
        for _ in 0..2 {
            let token = match self.access_token().await {
                Ok(token) => token,
                Err(error) => {
                    tracing::warn!("FCM gave no access token: {error:#}");
                    return Sent::Failed;
                }
            };
            let response = match self.http.post(&self.send_url).bearer_auth(token).json(&message).send().await {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!("FCM can't be reached: {error}");
                    return Sent::Failed;
                }
            };
            let status = response.status();
            if status.is_success() {
                return Sent::Ok;
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                *self.token.lock().await = None;
                continue;
            }
            let body: Value = response.json().await.unwrap_or_default();
            let error = &body["error"];
            let code = error["details"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|detail| detail["errorCode"].as_str())
                .or_else(|| error["status"].as_str())
                .unwrap_or_default();
            let text = error["message"].as_str().unwrap_or_default();
            if fcm_token_gone(code, text) {
                tracing::warn!("FCM ({}) refused device {}: {code} {text}", device.topic, short(&device.token));
                return Sent::Gone;
            }
            tracing::warn!("FCM ({}) refused a push: {status} {code} {text}", device.topic);
            return Sent::Failed;
        }
        tracing::warn!("FCM refused a fresh access token");
        Sent::Failed
    }
}

/// FCM's answer for a registration token it no longer takes.
fn fcm_token_gone(code: &str, message: &str) -> bool {
    match code {
        "UNREGISTERED" | "SENDER_ID_MISMATCH" => true,
        "INVALID_ARGUMENT" => message.to_ascii_lowercase().contains("registration token"),
        _ => false,
    }
}

/// The DER inside a PEM, which may come with its line breaks written as `\n`.
fn pem_body(pem: &str) -> Option<Vec<u8>> {
    let body: String = pem
        .replace("\\n", "\n")
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .flat_map(|line| line.chars().filter(|character| !character.is_whitespace()))
        .collect();
    STANDARD.decode(body).ok()
}

/// An APNs token is hex, as it goes in APNs' path; an FCM registration token is base64url with
/// colons.
pub fn valid_token(platform: Platform, token: &str) -> bool {
    match platform {
        Platform::Apns => (32..=200).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_hexdigit()),
        Platform::Fcm => {
            (1..=4096).contains(&token.len())
                && token.bytes().all(|byte| byte.is_ascii_alphanumeric() || b":_-".contains(&byte))
        }
    }
}

/// A bundle id or package name.
pub fn valid_topic(topic: &str) -> bool {
    (1..=155).contains(&topic.len())
        && topic.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_p8_with_or_without_its_line_breaks() {
        let pem = "-----BEGIN PRIVATE KEY-----\nAAEC\nAwQ=\n-----END PRIVATE KEY-----\n";
        assert_eq!(pem_body(pem).unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(pem_body(&pem.replace('\n', "\\n")).unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn takes_only_tokens_each_service_makes_and_bundle_ids() {
        assert!(valid_token(Platform::Apns, &"ab12".repeat(16)));
        assert!(!valid_token(Platform::Apns, "../../3/device"));
        let fcm = "dQw4w9WgXcQ:APA91bH-x_Yz0123456789abcdefABCDEF";
        assert!(valid_token(Platform::Fcm, fcm));
        assert!(!valid_token(Platform::Apns, fcm), "an FCM token isn't hex");
        assert!(!valid_token(Platform::Fcm, ""));
        assert!(!valid_token(Platform::Fcm, "abc/../../messages"));
        assert!(!valid_token(Platform::Fcm, "abc def"));
        assert!(!valid_token(Platform::Fcm, &"a".repeat(4097)));
        assert!(valid_topic("app.wonnet.ios"));
        assert!(!valid_topic("app/wonnet"));
    }
}
