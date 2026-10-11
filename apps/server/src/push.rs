//! Push notifications through APNs, for the iOS app, which loses the sync socket in the
//! background. New inbox mail goes to each of the user's devices as an alert whose sender, subject
//! and snippet are sealed with that device's key: its notification extension opens them, and
//! Apple only carries ciphertext. When pushed mail is read or leaves the inbox, a background push
//! tells the devices to take its notification down. Both carry the inbox's unread count.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::AppState;
use crate::config::ApnsConfig;

/// Apple takes a provider token for an hour and asks for a new one at most every 20 minutes.
const TOKEN_LIFETIME: Duration = Duration::from_secs(40 * 60);
/// How long changes gather before the devices hear of them.
const SETTLE: Duration = Duration::from_secs(1);
/// The most alerts one batch of new mail makes; the badge counts the rest.
const MOST_ALERTS: usize = 5;

pub const KEY_LEN: usize = 32;

pub struct Pusher {
    apns: Option<Apns>,
    changed: Mutex<HashSet<Uuid>>,
    wake: Notify,
    /// The badge each user's devices were last given.
    badges: Mutex<HashMap<Uuid, i64>>,
}

impl Pusher {
    pub fn new(config: Option<&ApnsConfig>) -> Self {
        let apns = config.and_then(|config| match Apns::new(config) {
            Ok(apns) => Some(apns),
            Err(error) => {
                tracing::error!("push notifications are off: {error:#}");
                None
            }
        });
        Self { apns, changed: Default::default(), wake: Notify::new(), badges: Default::default() }
    }

    /// The user's mail changed: their devices may have something to hear.
    pub fn changed(&self, user_id: Uuid) {
        if self.apns.is_none() {
            return;
        }
        self.changed.lock().unwrap().insert(user_id);
        self.wake.notify_one();
    }
}

pub fn start(state: AppState) {
    if state.pusher.apns.is_none() {
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
    token: String,
    topic: String,
    sandbox: bool,
    key: Vec<u8>,
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
    let Some(apns) = &state.pusher.apns else { return Ok(()) };
    let db = &state.background;
    let devices: Vec<Device> = sqlx::query_as("SELECT token, topic, sandbox, key FROM devices WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(db)
        .await?;
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
        let mut pushes: Vec<Push> = fresh.iter().take(MOST_ALERTS).rev().map(|mail| alert(&key, mail, badge)).collect();
        if !settled.is_empty() || (fresh.is_empty() && badge_changed) {
            pushes.push(background(&key, &settled, badge));
        }
        let token = short(&device.token);
        let (alerts, total) = (fresh.len().min(MOST_ALERTS), pushes.len());
        let mut sent = 0;
        for push in pushes {
            match apns.send(&device, &push).await {
                Sent::Ok => sent += 1,
                Sent::Failed => {}
                Sent::Gone => {
                    sqlx::query("DELETE FROM devices WHERE token = $1").bind(&device.token).execute(db).await?;
                    tracing::warn!("forgot device {token} of user {user_id}: APNs no longer takes its token");
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

struct Push {
    background: bool,
    /// Lets a newer push of the same message replace the notification.
    collapse: Option<String>,
    payload: Value,
}

/// New mail. Without the extension (or its key) it reads "New mail".
fn alert(key: &[u8; KEY_LEN], mail: &Fresh, badge: i64) -> Push {
    let from = mail_protocol::Address { name: mail.from_name.clone(), email: mail.from_email.clone() };
    let content = json!({
        "thread": thread_key(mail.account_id, &mail.thread_id),
        "account": mail.account_id,
        "message": mail.id,
        "from": from.display(),
        "subject": mail.subject,
        "snippet": mail.snippet,
    });
    let payload = json!({
        "aps": {
            "alert": { "title": "New mail" },
            "sound": "default",
            "badge": badge,
            "mutable-content": 1,
            "category": "message",
        },
        "sealed": seal(key, &content),
    });
    Push { background: false, collapse: Some(mail.id.to_string()), payload }
}

/// Takes down the notifications of threads that were read or left the inbox.
fn background(key: &[u8; KEY_LEN], settled: &[String], badge: i64) -> Push {
    let payload = json!({
        "aps": { "content-available": 1, "badge": badge },
        "sealed": seal(key, &json!({ "settled": settled })),
    });
    Push { background: true, collapse: None, payload }
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
        let der = pem_body(&config.key)?;
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

    async fn send(&self, device: &Device, push: &Push) -> Sent {
        let base = if device.sandbox { &self.sandbox_url } else { &self.production_url };
        let (kind, priority) = if push.background { ("background", "5") } else { ("alert", "10") };
        let mut request = self
            .http
            .post(format!("{base}/3/device/{}", device.token))
            .bearer_auth(self.token())
            .header("apns-topic", &device.topic)
            .header("apns-push-type", kind)
            .header("apns-priority", priority)
            .json(&push.payload);
        if let Some(collapse) = &push.collapse {
            request = request.header("apns-collapse-id", collapse);
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

/// The DER inside a PEM, which may come with its line breaks written as `\n`.
fn pem_body(pem: &str) -> anyhow::Result<Vec<u8>> {
    let body: String = pem
        .replace("\\n", "\n")
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .flat_map(|line| line.chars().filter(|character| !character.is_whitespace()))
        .collect();
    STANDARD.decode(body).map_err(|_| anyhow::anyhow!("APNS_KEY isn't a .p8 file's contents"))
}

/// An APNs device token: hex.
pub fn valid_token(token: &str) -> bool {
    (32..=200).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A bundle id.
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
    fn takes_only_hex_tokens_and_bundle_ids() {
        assert!(valid_token(&"ab12".repeat(16)));
        assert!(!valid_token("../../3/device"));
        assert!(valid_topic("app.wonnet.ios"));
        assert!(!valid_topic("app/wonnet"));
    }
}
