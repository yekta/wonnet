use mail_protocol::Provider;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Server, code_of};
use crate::db;

async fn token(server: &Server, code: &str, verifier: &str) -> reqwest::Response {
    server.post("/api/auth/exchange", None, json!({ "code": code, "verifier": verifier })).await
}

async fn token_text(response: reqwest::Response) -> String {
    assert!(response.status().is_success(), "{}", response.status());
    let body: Value = response.json().await.unwrap();
    body["token"].as_str().unwrap().to_string()
}

#[sqlx::test]
async fn adding_gmail_makes_the_user_and_hands_the_app_a_session(db: PgPool) {
    let server = Server::start(db).await;
    let back = server.google_sign_in("verifier-1", None, "ann@gmail.com").await;
    let code = code_of(&back);

    let session = token_text(token(&server, &code, "verifier-1").await).await;
    let user = server.user_of(&session).await.unwrap();
    let account = db::account_by_login(&server.state.db, Provider::Gmail, "ann@gmail.com", "").await.unwrap().unwrap();
    assert_eq!(account.user_id, user);
    let secret = server.state.sealer.open(&account.credentials).unwrap();
    assert!(String::from_utf8(secret).unwrap().contains("refresh-ann@gmail.com"));
    assert!(!String::from_utf8_lossy(&account.credentials).contains("refresh-ann"));

    // A code works once.
    assert_eq!(token(&server, &code, "verifier-1").await.status(), 400);
}

#[sqlx::test]
async fn a_code_only_works_with_the_secret_that_started_it(db: PgPool) {
    let server = Server::start(db).await;
    let code = code_of(&server.google_sign_in("right", None, "ann@gmail.com").await);
    assert_eq!(token(&server, &code, "wrong").await.status(), 401);
}

#[sqlx::test]
async fn signing_in_again_finds_the_same_user(db: PgPool) {
    let server = Server::start(db).await;
    let first = code_of(&server.google_sign_in("v1", None, "ann@gmail.com").await);
    let first = token_text(token(&server, &first, "v1").await).await;
    let second = code_of(&server.google_sign_in("v2", None, "ann@gmail.com").await);
    let second = token_text(token(&server, &second, "v2").await).await;
    assert_ne!(first, second);
    assert_eq!(server.user_of(&first).await, server.user_of(&second).await);
}

#[sqlx::test]
async fn a_link_ticket_adds_a_second_account_to_the_same_user(db: PgPool) {
    let server = Server::start(db).await;
    let code = code_of(&server.google_sign_in("v1", None, "ann@gmail.com").await);
    let session = token_text(token(&server, &code, "v1").await).await;

    let response = server.post("/api/link-ticket", Some(&session), json!({})).await;
    let ticket: Value = response.json().await.unwrap();
    let ticket = ticket["ticket"].as_str().unwrap();
    let code = code_of(&server.google_sign_in("v2", Some(ticket), "ann.work@gmail.com").await);
    token_text(token(&server, &code, "v2").await).await;

    let user = server.user_of(&session).await.unwrap();
    let work =
        db::account_by_login(&server.state.db, Provider::Gmail, "ann.work@gmail.com", "").await.unwrap().unwrap();
    assert_eq!(work.user_id, user);
    assert_eq!(work.color, "account-2");

    // A ticket works once.
    let reused =
        format!("{}/auth/google/start?challenge={}&state=s&ticket={ticket}", server.base, crate::sha256_hex(b"v3"));
    assert_eq!(server.http.get(reused).send().await.unwrap().status(), 400);
}

#[sqlx::test]
async fn the_dev_login_is_off_unless_asked_for(db: PgPool) {
    let server = Server::start_with(db, |config| config.dev_login = false).await;
    let response = server.post("/api/dev/login", None, json!({ "email": "ann@example.com" })).await;
    assert_eq!(response.status(), 404);
}

#[sqlx::test]
async fn the_gmail_hook_needs_its_token(db: PgPool) {
    let server = Server::start(db).await;
    let push = json!({ "message": { "data": "e30=" } });
    let response =
        server.http.post(format!("{}/hooks/gmail?token=nope", server.base)).json(&push).send().await.unwrap();
    assert_eq!(response.status(), 403);
    let response =
        server.http.post(format!("{}/hooks/gmail?token=hook-token", server.base)).json(&push).send().await.unwrap();
    assert_eq!(response.status(), 204);
}

const MICROSOFT_SCOPE: &str =
    "openid email offline_access User.Read Mail.ReadWrite Mail.Send MailboxSettings.ReadWrite";

#[sqlx::test]
async fn adding_outlook_makes_the_user_and_hands_the_app_a_session(db: PgPool) {
    let server = Server::start(db).await;
    let response = server.microsoft_sign_in("verifier-1", "Ann@Outlook.com", MICROSOFT_SCOPE).await;
    assert!(response.status().is_redirection());
    let back = reqwest::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let code = code_of(&back);

    let session = token_text(token(&server, &code, "verifier-1").await).await;
    let user = server.user_of(&session).await.unwrap();
    let account =
        db::account_by_login(&server.state.db, Provider::Outlook, "ann@outlook.com", "").await.unwrap().unwrap();
    assert_eq!(account.user_id, user);
    let secret = server.state.sealer.open(&account.credentials).unwrap();
    assert!(String::from_utf8(secret).unwrap().contains("microsoft-Ann@Outlook.com"));
    assert!(!String::from_utf8_lossy(&account.credentials).contains("microsoft-Ann"));

    assert_eq!(token(&server, &code, "verifier-1").await.status(), 400);
    assert_eq!(token(&server, &code, "wrong").await.status(), 400);
}

#[sqlx::test]
async fn outlook_without_mail_access_is_refused(db: PgPool) {
    let server = Server::start(db).await;
    let response = server.microsoft_sign_in("v", "ann@outlook.com", "openid email User.Read").await;
    assert_eq!(response.status(), 400);
    let account = db::account_by_login(&server.state.db, Provider::Outlook, "ann@outlook.com", "").await.unwrap();
    assert!(account.is_none());
}

#[sqlx::test]
async fn outlook_keeps_the_refresh_token_microsoft_renews(db: PgPool) {
    let server = Server::start(db).await;
    let response = server.microsoft_sign_in("v", "ann@outlook.com", MICROSOFT_SCOPE).await;
    assert!(response.status().is_redirection());
    let account =
        db::account_by_login(&server.state.db, Provider::Outlook, "ann@outlook.com", "").await.unwrap().unwrap();

    crate::providers::Connection::open(&server.state, &account).await.unwrap();
    let account = db::account(&server.state.db, account.id).await.unwrap().unwrap();
    let secret = String::from_utf8(server.state.sealer.open(&account.credentials).unwrap()).unwrap();
    assert!(secret.contains("microsoft-renewed"), "{secret}");
}

#[sqlx::test]
async fn the_outlook_hook_wants_the_accounts_client_state(db: PgPool) {
    let server = Server::start(db).await;
    let hook = format!("{}/hooks/outlook", server.base);
    let validation = server.http.post(format!("{hook}?validationToken=echo%20me")).send().await.unwrap();
    assert_eq!(validation.status(), 200);
    assert_eq!(validation.text().await.unwrap(), "echo me");

    let account = uuid::Uuid::new_v4();
    let notice = |client_state: &str| json!({ "value": [{ "clientState": client_state, "changeType": "created" }] });
    let right = crate::providers::outlook::client_state("test-secret", account);
    let other = crate::providers::outlook::client_state("test-secret", uuid::Uuid::new_v4());
    let post = |body: Value| server.http.post(format!("{hook}?account={account}")).json(&body).send();
    assert_eq!(post(notice(&other)).await.unwrap().status(), 403);
    assert_eq!(post(notice("")).await.unwrap().status(), 403);
    assert_eq!(post(notice(&right)).await.unwrap().status(), 202);
}
