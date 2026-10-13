use std::sync::{Arc, Mutex};
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use mail_protocol::{Address, Op};
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::Server;
use crate::config::ApnsConfig;
use crate::db::{self, RemoteMessage};
use crate::ops;

const DEVICE: &str = "a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4";
/// A token APNs says is no longer registered.
const GONE: &str = "dead0000dead0000dead0000dead0000dead0000dead0000dead0000dead0000";
const DEVICE_KEY: [u8; 32] = [7; 32];

#[derive(Clone, Debug)]
struct Pushed {
    token: String,
    headers: HeaderMap,
    body: Value,
}

type Pushes = Arc<Mutex<Vec<Pushed>>>;

/// APNs, answering every token but `GONE`.
async fn fake_apns() -> (String, Pushes) {
    let pushes: Pushes = Default::default();
    let record = pushes.clone();
    let push = move |Path(token): Path<String>, headers: HeaderMap, Json(body): Json<Value>| {
        let gone = token == GONE;
        record.lock().unwrap().push(Pushed { token, headers, body });
        async move {
            match gone {
                true => (StatusCode::GONE, Json(json!({ "reason": "Unregistered" }))),
                false => (StatusCode::OK, Json(json!({}))),
            }
        }
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route("/3/device/{token}", post(push));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, pushes)
}

async fn pushed_to(pushes: &Pushes, token: &str, count: usize) -> Vec<Pushed> {
    for _ in 0..100 {
        let to: Vec<Pushed> = pushes.lock().unwrap().iter().filter(|push| push.token == token).cloned().collect();
        if to.len() >= count {
            return to;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{token} got fewer than {count} pushes");
}

fn open(sealed: &Value) -> Value {
    let sealed = STANDARD.decode(sealed.as_str().unwrap()).unwrap();
    let (nonce, body) = sealed.split_at(12);
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(DEVICE_KEY));
    let plain = cipher.decrypt(&Nonce::from(<[u8; 12]>::try_from(nonce).unwrap()), body).unwrap();
    serde_json::from_slice(&plain).unwrap()
}

fn mail(provider_id: &str, thread_id: &str, from: &str, subject: &str) -> RemoteMessage {
    RemoteMessage {
        provider_id: provider_id.into(),
        thread_id: thread_id.into(),
        from: Address::new(Some("Cy Young"), from),
        subject: subject.into(),
        snippet: "Here it is.".into(),
        date: chrono::Utc::now().timestamp_millis(),
        unread: true,
        labels: vec!["inbox".into()],
        ..Default::default()
    }
}

#[sqlx::test]
async fn new_mail_is_pushed_sealed_with_the_devices_key_and_taken_down_once_read(db: PgPool) {
    let (apns, pushes) = fake_apns().await;
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &SystemRandom::new()).unwrap();
    let signer = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &SystemRandom::new());
    let public_key = signer.unwrap().public_key().as_ref().to_vec();
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", STANDARD.encode(pkcs8.as_ref()));
    let server = Server::start_with(db.clone(), |config| {
        config.apns = Some(ApnsConfig {
            key: pem,
            key_id: "KEY1".into(),
            team_id: "TEAM1".into(),
            production_url: apns.clone(),
            sandbox_url: apns.clone(),
        });
    })
    .await;
    let (token, user, account) = server.gmail_user("ann@gmail.com").await;

    let device = |token: &str| json!({ "token": token, "topic": "app.wonnet.ios", "sandbox": true, "key": STANDARD.encode(DEVICE_KEY) });
    assert_eq!(server.post("/api/devices", None, device(DEVICE)).await.status(), 401);
    let response = server.post("/api/devices", Some(&token), device("../../2/device")).await;
    assert_eq!(response.status(), 400, "a token is hex, as it goes in APNs' path");
    let short_key =
        json!({ "token": DEVICE, "topic": "app.wonnet.ios", "sandbox": true, "key": STANDARD.encode([1; 8]) });
    assert_eq!(server.post("/api/devices", Some(&token), short_key).await.status(), 400);
    for device_token in [DEVICE, GONE] {
        assert!(server.post("/api/devices", Some(&token), device(device_token)).await.status().is_success());
    }

    let new = [mail("m1", "t1", "cy@example.com", "Plan"), mail("mine", "t2", "ann@gmail.com", "Mine")];
    db::upsert_messages(&db, &account, &new).await.unwrap();
    let alert = pushed_to(&pushes, DEVICE, 1).await.remove(0);
    assert_eq!(alert.headers["apns-topic"], "app.wonnet.ios");
    assert_eq!(alert.headers["apns-push-type"], "alert");
    assert!(!alert.body.to_string().contains("Plan"), "Apple sees no mail: {}", alert.body);
    assert_eq!(alert.body["aps"]["badge"], 2, "the inbox's unread threads, as the apps count them");
    let content = open(&alert.body["sealed"]);
    assert_eq!(
        (&content["from"], &content["email"], &content["initials"]),
        (&json!("Cy Young"), &json!("cy@example.com"), &json!("CY"))
    );
    assert_eq!((&content["subject"], &content["thread"]), (&json!("Plan"), &json!(format!("{}:t1", account.id))));

    let bearer = alert.headers["authorization"].to_str().unwrap().strip_prefix("Bearer ").unwrap().to_string();
    let (unsigned, signature) = bearer.rsplit_once('.').unwrap();
    let verifier = ring::signature::UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &public_key);
    assert!(verifier.verify(unsigned.as_bytes(), &URL_SAFE_NO_PAD.decode(signature).unwrap()).is_ok());
    let claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(unsigned.split('.').nth(1).unwrap()).unwrap()).unwrap();
    assert_eq!(claims["iss"], "TEAM1");

    let gone: i64 =
        sqlx::query_scalar("SELECT count(*) FROM devices WHERE token = $1").bind(GONE).fetch_one(&db).await.unwrap();
    assert_eq!(gone, 0, "a token APNs no longer takes is forgotten");

    // Read on another device: the notification is taken down, and the user's own mail never came.
    let id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM messages WHERE provider_id = 'm1'").fetch_one(&db).await.unwrap();
    let read = Op::SetUnread { ids: vec![id.to_string()], unread: false };
    ops::apply(&server.state, user, "read", read).await.unwrap();
    let pushed = pushed_to(&pushes, DEVICE, 2).await;
    assert_eq!(pushed.len(), 2);
    assert_eq!(pushed[1].headers["apns-push-type"], "alert", "so the badge is set even when the app was quit");
    assert_eq!(pushed[1].body["aps"]["badge"], 1);
    assert_eq!(open(&pushed[1].body["sealed"]), json!({ "settled": [format!("{}:t1", account.id)] }));

    // Only its user takes a device off.
    let (other, ..) = server.gmail_user("bo@gmail.com").await;
    server.delete(&format!("/api/devices/{DEVICE}"), Some(&other)).await;
    let devices = || sqlx::query_scalar::<_, i64>("SELECT count(*) FROM devices").fetch_one(&db);
    assert_eq!(devices().await.unwrap(), 1);
    assert!(server.delete(&format!("/api/devices/{DEVICE}"), Some(&token)).await.status().is_success());
    assert_eq!(devices().await.unwrap(), 0);
}
