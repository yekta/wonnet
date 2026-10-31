use std::sync::{Arc, Mutex};
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Form, Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use mail_protocol::{Address, Op};
use ring::rand::SystemRandom;
use ring::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair, RSA_PKCS1_2048_8192_SHA256,
    RsaKeyPair,
};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::Server;
use crate::config::{ApnsConfig, FcmConfig};
use crate::db::{self, RemoteMessage};
use crate::ops;

const DEVICE: &str = "a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4";
/// A token APNs says is no longer registered.
const GONE: &str = "dead0000dead0000dead0000dead0000dead0000dead0000dead0000dead0000";
const DEVICE_KEY: [u8; 32] = [7; 32];
const ANDROID: &str = "dQw4w9WgXcQ:APA91bH-Kx_Yz0123456789AbCdEf";
/// Tokens FCM no longer takes: unregistered, and not a registration token.
const UNREGISTERED: &str = "gone:APA91bUnregistered";
const NOT_A_TOKEN: &str = "bad:APA91bNotAToken";
/// The service account's key, made for this test.
const SERVICE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDOuBbpPwFGJntA\nqGTP9lGAGqqzN/7bJjK2zRjT3aRCJQKE5Sdk+Tee6XJ/x9bjbGK5zMeOVAoCHPHv\nXOIKG49A6vFLt9MGc1LLA9EVqNebQVRuOOKrzR2cgHOOthjkv1AjpUw4838+MF5n\nmYZLi+0nayP9iEZHuCH1sUf09Myko98V5FoVa4pZPy3Wc3sPDav3y7abFZKuz3Bu\n6YElV9XsBZZ5dKXgDo0kPWL1VjqYWNcOyKTu4QKaEMREQPdnOR1bDR4F2RzNyjhm\nluJQKdZRlVfLu3DoCTqxRtcTroKE5oKwPXptyeE+MsB6mLSXqptWohkrP0WqgbXy\nlXVWTiChAgMBAAECggEAP7cuGKZ61gx2YO+GYzMg28NcafEQdKvJZd7BkEv4nQqb\nRCr9gXnvUuPRjuEhsudtZqdfutbcXtpdF9IhOcxU00WXKid2u74//qMmhu6LxibU\nx0RxWVLXla1h8NLfyWHOaa5BWCx1Q83Uy0newtpWNpgnd7XRM7nlfb0u2Km7ieMK\nv1Hw8N7zHUMYwHkUCROvwR9hiGeigGq6kAliGHQNuAqVgKmI2uTuxASNruA7fRnL\n3u8a4vPevQQSilVdwVSD4+VdvQqIv5TMxtBaF/92jWoTrX4t57WSNAhOxPZzZf3M\nHXwrkMGenL7Cr12+BCL5TsN5RYO7ujYDjw+dKolLgQKBgQDrI6Jh6TEfn4dRNi2N\ncRTQu2USKq4AlEgZhBv1SDTG+EA5WoEhd4fnlg5wCdo4qUI88KlZke4VSEBxj1ZL\nh1BhyDS7rbBIJyGvUk6g0MuC4aok6UDi99PoY2YRu9DKtIEpvXzgXMMNenFbRujS\ns3jXhzqpgnsDVopJQS/zoNI7vQKBgQDhDv3vnuW1frvxQ3tNMmepMQ6BDts3KNv1\njSxqr/EzGF4VGHmHzJ9DMuTQZ7V2qcZKuRqbiI+MFDyb7X3IE5ZXTCnGyZtszgb4\n3MJQMa/b11MbEHwIPJwAXSfOmp0Hcn9GjkZrD/J041HgtsYefxmQ5G6IYotDrAyK\niZzpTF20tQKBgAZY0uRDsCDDZJqrlLUxrTcpJ8RCxKmdVwjhSQPo7AvG3azupXeq\ny09T/JXHnk1TPEd604doaf3HEn4+OT0WrPPGvK2oNY7tB3q9BQRQPn4M61PNvAOk\nXuLx8+pcoHkoQChJRhqkRluRx4eINXaig4uFye6TaBCiAF+T0+CGocYdAoGACR7y\na6Ok65YXR8/UT2ybht/WFqTjfXjKcUknY6t9uvRh27kCM963FaIaY9fQIt7+iki2\nK1d9ec/YJPma/2DL1v3fM484HVC8N5m6xkU9O0uespoYbL0IFXuxAU6Czqp1PgbG\nE9CxossTpmfB37qG7rhTQnO78SIn3/VzxoCxy4ECgYEAow0j4xEXmhAxb26I9yHB\nb1kkGW6MVmG9Y5ybUGE6Lge3XEGFEPPxJgXprnlVNCs9APC9oXtIiGIP7tCaYm0p\nH2jOeuN2L3StSc67boMsXe9sKO/+D2vVxMsLoxRbaoiRkzhcQlQDBg7rZ7tVWtcF\nf+v0KjWc/6XwnUlrAbgTfgY=\n-----END PRIVATE KEY-----";

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

#[derive(Default)]
struct FakeFcm {
    /// The JWTs given for access tokens.
    assertions: Vec<String>,
    /// Whether it turned an access token down yet, as when one expired early.
    refused: bool,
}

/// FCM and Google's token endpoint, answering every registration token but the two gone ones.
async fn fake_fcm() -> (String, Pushes, Arc<Mutex<FakeFcm>>) {
    let pushes: Pushes = Default::default();
    let fcm: Arc<Mutex<FakeFcm>> = Default::default();
    let record = fcm.clone();
    let token = move |Form(form): Form<std::collections::HashMap<String, String>>| {
        assert_eq!(form["grant_type"], "urn:ietf:params:oauth:grant-type:jwt-bearer");
        let mut fcm = record.lock().unwrap();
        fcm.assertions.push(form["assertion"].clone());
        let access = format!("access-{}", fcm.assertions.len());
        async move { Json(json!({ "access_token": access, "expires_in": 3599, "token_type": "Bearer" })) }
    };
    let (record, state) = (pushes.clone(), fcm.clone());
    let send = move |Path(project): Path<String>, headers: HeaderMap, Json(body): Json<Value>| {
        assert_eq!(project, "wonnet-test");
        let token = body["message"]["token"].as_str().unwrap().to_string();
        let refuse = !std::mem::replace(&mut state.lock().unwrap().refused, true);
        if !refuse {
            record.lock().unwrap().push(Pushed { token: token.clone(), headers, body });
        }
        let error = |status: StatusCode, code: &str, message: &str| {
            let details =
                json!([{ "@type": "type.googleapis.com/google.firebase.fcm.v1.FcmError", "errorCode": code }]);
            (status, Json(json!({ "error": { "code": status.as_u16(), "message": message, "details": details } })))
        };
        async move {
            match token.as_str() {
                _ if refuse => error(StatusCode::UNAUTHORIZED, "UNAUTHENTICATED", "Request had invalid credentials."),
                UNREGISTERED => error(StatusCode::NOT_FOUND, "UNREGISTERED", "Requested entity was not found."),
                NOT_A_TOKEN => error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_ARGUMENT",
                    "The registration token is not a valid FCM registration token",
                ),
                _ => (StatusCode::OK, Json(json!({ "name": "projects/wonnet-test/messages/1" }))),
            }
        }
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route("/token", post(token)).route("/v1/projects/{project}/messages:send", post(send));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, pushes, fcm)
}

#[sqlx::test]
async fn new_mail_reaches_android_as_sealed_data_through_fcm(db: PgPool) {
    let (fcm_url, pushes, fcm) = fake_fcm().await;
    let service_account = json!({
        "type": "service_account",
        "project_id": "wonnet-test",
        "private_key_id": "key1",
        "private_key": SERVICE_KEY,
        "client_email": "push@wonnet-test.iam.gserviceaccount.com",
        "token_uri": "https://oauth2.googleapis.com/token",
    });
    let server = Server::start_with(db.clone(), |config| {
        config.fcm = Some(FcmConfig {
            service_account: service_account.to_string(),
            url: fcm_url.clone(),
            token_url: Some(format!("{fcm_url}/token")),
        });
    })
    .await;
    let (token, user, account) = server.gmail_user("ann@gmail.com").await;

    let device = |token: &str| json!({ "platform": "fcm", "token": token, "topic": "app.wonnet.wonnet", "key": STANDARD.encode(DEVICE_KEY) });
    let response = server.post("/api/devices", Some(&token), device("abc/../../messages")).await;
    assert_eq!(response.status(), 400);
    for device_token in [ANDROID, UNREGISTERED, NOT_A_TOKEN] {
        assert!(server.post("/api/devices", Some(&token), device(device_token)).await.status().is_success());
    }

    let new = [mail("m1", "t1", "cy@example.com", "Plan"), mail("mine", "t2", "ann@gmail.com", "Mine")];
    db::upsert_messages(&db, &account, &new).await.unwrap();
    let alert = pushed_to(&pushes, ANDROID, 1).await.remove(0);
    let message = &alert.body["message"];
    assert!(message.get("notification").is_none(), "data only, so the app posts it: {message}");
    assert!(!alert.body.to_string().contains("Plan"), "Google sees no mail: {}", alert.body);
    let id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM messages WHERE provider_id = 'm1'").fetch_one(&db).await.unwrap();
    assert_eq!(message["android"], json!({ "priority": "HIGH", "collapse_key": id.to_string() }));
    assert_eq!((&message["data"]["kind"], &message["data"]["badge"]), (&json!("alert"), &json!("2")));
    let content = open(&message["data"]["sealed"]);
    assert_eq!((&content["from"], &content["email"]), (&json!("Cy Young"), &json!("cy@example.com")));
    assert_eq!((&content["subject"], &content["thread"]), (&json!("Plan"), &json!(format!("{}:t1", account.id))));
    assert_eq!(alert.headers["authorization"], "Bearer access-2", "a new access token after FCM refused one");

    let assertion = fcm.lock().unwrap().assertions[0].clone();
    let (unsigned, signature) = assertion.rsplit_once('.').unwrap();
    let der: String = SERVICE_KEY.lines().filter(|line| !line.starts_with("-----")).collect();
    let signer = RsaKeyPair::from_pkcs8(&STANDARD.decode(der).unwrap()).unwrap();
    let verifier = ring::signature::UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, signer.public().as_ref());
    assert!(verifier.verify(unsigned.as_bytes(), &URL_SAFE_NO_PAD.decode(signature).unwrap()).is_ok());
    let claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(unsigned.split('.').nth(1).unwrap()).unwrap()).unwrap();
    assert_eq!(claims["iss"], "push@wonnet-test.iam.gserviceaccount.com");
    assert_eq!(claims["scope"], "https://www.googleapis.com/auth/firebase.messaging");
    assert_eq!(claims["aud"], "https://oauth2.googleapis.com/token");

    for gone in [UNREGISTERED, NOT_A_TOKEN] {
        pushed_to(&pushes, gone, 1).await;
    }
    let kept: Vec<String> = sqlx::query_scalar("SELECT token FROM devices").fetch_all(&db).await.unwrap();
    assert_eq!(kept, [ANDROID], "tokens FCM no longer takes are forgotten, and its own kept as it came");

    let read = Op::SetUnread { ids: vec![id.to_string()], unread: false };
    ops::apply(&server.state, user, "read", read).await.unwrap();
    let pushed = pushed_to(&pushes, ANDROID, 2).await;
    let message = &pushed[1].body["message"];
    assert_eq!(message["android"], json!({ "priority": "NORMAL" }));
    assert_eq!((&message["data"]["kind"], &message["data"]["badge"]), (&json!("settle"), &json!("1")));
    assert_eq!(open(&message["data"]["sealed"]), json!({ "settled": [format!("{}:t1", account.id)] }));
    assert_eq!(fcm.lock().unwrap().assertions.len(), 2, "the access token is kept");

    assert!(server.delete(&format!("/api/devices/{ANDROID}"), Some(&token)).await.status().is_success());
    let devices: i64 = sqlx::query_scalar("SELECT count(*) FROM devices").fetch_one(&db).await.unwrap();
    assert_eq!(devices, 0);
}
