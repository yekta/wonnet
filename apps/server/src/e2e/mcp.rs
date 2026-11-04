use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use mail_protocol::{Address, Provider, Recipients, Unsubscribe};
use reqwest::Url;
use reqwest::header::{COOKIE, LOCATION, SET_COOKIE, WWW_AUTHENTICATE};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::Server;
use crate::db::{self, AccountRow, NewAccount, RemoteMessage};

const REDIRECT: &str = "http://localhost/callback";
const VERIFIER: &str = "a-verifier-long-enough-to-be-a-pkce-verifier-0123456789";

fn challenge() -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()))
}

impl Server {
    async fn register(&self) -> String {
        let registration = json!({ "client_name": "Claude Code", "redirect_uris": [REDIRECT] });
        let response = self.post("/oauth/register", None, registration).await;
        assert_eq!(response.status(), 201);
        response.json::<Value>().await.unwrap()["client_id"].as_str().unwrap().to_string()
    }

    fn authorize_path(&self, client_id: &str, challenge: &str) -> String {
        let query: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", "http://localhost:51234/callback")
            .append_pair("state", "client-state")
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .finish();
        format!("/oauth/authorize?{query}")
    }

    /// Signs a browser in through the fake Google as `email`, from the page at `next`, and answers
    /// the cookie it is given.
    async fn browser_sign_in(&self, email: &str, next: &str) -> reqwest::Response {
        let next: String = url::form_urlencoded::byte_serialize(next.as_bytes()).collect();
        let start = self.http.get(format!("{}/auth/google/start?next={next}", self.base)).send().await.unwrap();
        assert!(start.status().is_redirection(), "{}", start.text().await.unwrap());
        let google = Url::parse(start.headers()[LOCATION].to_str().unwrap()).unwrap();
        let query: HashMap<String, String> = google.query_pairs().into_owned().collect();
        {
            let mut fake = self.google.lock().unwrap();
            (fake.email, fake.nonce) = (email.into(), query["nonce"].clone());
        }
        let callback = format!("{}/auth/google/callback?code=google-code&state={}", self.base, query["state"]);
        self.http.get(callback).send().await.unwrap()
    }

    async fn browser(&self, email: &str, next: &str) -> String {
        let response = self.browser_sign_in(email, next).await;
        assert!(response.status().is_redirection(), "{}", response.text().await.unwrap());
        assert_eq!(response.headers()[LOCATION], next);
        let cookie = response.headers()[SET_COOKIE].to_str().unwrap();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"));
        cookie.split(';').next().unwrap().to_string()
    }

    async fn page(&self, path: &str, cookie: &str) -> reqwest::Response {
        self.http.get(format!("{}{path}", self.base)).header(COOKIE, cookie).send().await.unwrap()
    }

    async fn form(&self, path: &str, cookie: &str, fields: &[(&str, &str)]) -> reqwest::Response {
        let body: String = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(fields).finish();
        self.http
            .post(format!("{}{path}", self.base))
            .header(COOKIE, cookie)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap()
    }

    /// Approves `client_id` on the consent page with `choices`, and answers where the browser goes.
    async fn consent(&self, cookie: &str, client_id: &str, choices: &[(&str, &str)]) -> reqwest::Response {
        let page = self.page(&self.authorize_path(client_id, &challenge()), cookie).await;
        assert_eq!(page.status(), 200);
        let html = page.text().await.unwrap();
        assert!(html.contains("Claude Code") && html.contains("Unverified"), "{html}");
        let fields = hidden_fields(&html);
        let mut form: Vec<(&str, &str)> = fields.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
        form.extend_from_slice(choices);
        self.form("/oauth/authorize", cookie, &form).await
    }

    async fn tokens(&self, client_id: &str, code: &str) -> reqwest::Response {
        let fields = [
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", "http://localhost:51234/callback"),
            ("code_verifier", VERIFIER),
        ];
        self.form("/oauth/token", "", &fields).await
    }

    async fn refresh(&self, client_id: &str, refresh_token: &str) -> reqwest::Response {
        let fields = [("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh_token)];
        self.form("/oauth/token", "", &fields).await
    }

    /// Connects an app with `choices` and answers its client id and tokens.
    async fn connect(&self, cookie: &str, choices: &[(&str, &str)]) -> (String, Value) {
        let client_id = self.register().await;
        let approved = self.consent(cookie, &client_id, choices).await;
        assert_eq!(approved.status(), 303, "{}", approved.text().await.unwrap());
        let code = code_in(approved.headers()[LOCATION].to_str().unwrap());
        let tokens = self.tokens(&client_id, &code).await;
        assert_eq!(tokens.status(), 200);
        (client_id, tokens.json().await.unwrap())
    }

    async fn rpc(&self, token: &str, method: &str, params: Value) -> reqwest::Response {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        self.http
            .post(format!("{}/mcp", self.base))
            .bearer_auth(token)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18")
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    async fn tool_names(&self, token: &str) -> Vec<String> {
        let response = self.rpc(token, "tools/list", json!({})).await;
        assert_eq!(response.status(), 200);
        let answer: Value = response.json().await.unwrap();
        let tools = answer["result"]["tools"].as_array().unwrap();
        tools.iter().map(|tool| tool["name"].as_str().unwrap().to_string()).collect()
    }

    /// A tool's structured answer, or its error's text.
    async fn call(&self, token: &str, tool: &str, arguments: Value) -> Result<Value, String> {
        let response = self.rpc(token, "tools/call", json!({ "name": tool, "arguments": arguments })).await;
        assert_eq!(response.status(), 200);
        let answer: Value = response.json().await.unwrap();
        let result = &answer["result"];
        match result["isError"].as_bool().unwrap_or(false) {
            true => Err(result["content"][0]["text"].as_str().unwrap_or_default().to_string()),
            false => Ok(result["structuredContent"].clone()),
        }
    }
}

fn hidden_fields(html: &str) -> Vec<(String, String)> {
    html.split("<input type=\"hidden\" ")
        .skip(1)
        .map(|rest| {
            let attribute = |name: &str| {
                let start = rest.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
                let value = &rest[start..];
                value[..value.find('"').unwrap()].replace("&amp;", "&").replace("&quot;", "\"")
            };
            (attribute("name"), attribute("value"))
        })
        .collect()
}

fn code_in(location: &str) -> String {
    let url = Url::parse(location).unwrap();
    assert_eq!(url.host_str(), Some("localhost"));
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(query["state"], "client-state");
    assert!(query["iss"].starts_with("http://127.0.0.1:"));
    query["code"].clone()
}

fn token(tokens: &Value, kind: &str) -> String {
    tokens[kind].as_str().unwrap().to_string()
}

async fn mail(server: &Server, account: &AccountRow, messages: &[RemoteMessage]) {
    db::upsert_messages(&server.state.db, account, messages).await.unwrap();
}

fn message(id: &str, from: &str, subject: &str, labels: &[&str]) -> RemoteMessage {
    RemoteMessage {
        provider_id: id.into(),
        thread_id: id.into(),
        from: Address::new(None, from),
        recipients: Recipients { to: vec![Address::new(None, "ann@gmail.com")], ..Default::default() },
        subject: subject.into(),
        snippet: format!("{subject}, in short"),
        date: chrono::Utc::now().timestamp_millis(),
        labels: labels.iter().map(|label| label.to_string()).collect(),
        ..Default::default()
    }
}

fn newsletter(id: &str, from: &str, unsubscribe: Option<Unsubscribe>) -> RemoteMessage {
    RemoteMessage { bulk: true, unsubscribe, ..message(id, from, "This week's deals", &["inbox"]) }
}

async fn second_account(server: &Server, user_id: Uuid, email: &str) -> AccountRow {
    let credentials = server.state.sealer.seal(b"{}");
    let account = NewAccount { user_id, provider: Provider::Jmap, address: email, login: "l", credentials };
    let id = db::upsert_account(&server.state.db, account).await.unwrap();
    db::account(&server.state.db, id).await.unwrap().unwrap()
}

const READER: [(&str, &str); 3] = [("decision", "approve"), ("access", "all"), ("role", "reader")];
const ORGANIZER: [(&str, &str); 3] = [("decision", "approve"), ("access", "all"), ("role", "organizer")];

#[sqlx::test]
async fn an_app_signs_in_through_the_consent_page_and_gets_what_it_was_given(db: PgPool) {
    let server = Server::start(db).await;
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    mail(&server, &account, &[message("m1", "cy@example.com", "Plan for Monday", &["inbox"])]).await;

    let unsigned = server.http.post(format!("{}/mcp", server.base)).json(&json!({})).send().await.unwrap();
    assert_eq!(unsigned.status(), 401);
    let challenge = unsigned.headers()[WWW_AUTHENTICATE].to_str().unwrap();
    assert!(challenge.contains(&format!("resource_metadata=\"{}/.well-known/oauth-protected-resource\"", server.base)));
    let metadata: Value = server.get("/.well-known/oauth-protected-resource", None).await.json().await.unwrap();
    assert_eq!(metadata["resource"], format!("{}/mcp", server.base));
    let issuer: Value = server.get("/.well-known/oauth-authorization-server", None).await.json().await.unwrap();
    assert_eq!(issuer["code_challenge_methods_supported"], json!(["S256"]));

    let client_id = server.register().await;
    let authorize = server.authorize_path(&client_id, &self::challenge());
    let signed_out = server.get(&authorize, None).await.text().await.unwrap();
    assert!(signed_out.contains("Continue with Google"));
    let cookie = server.browser("ann@gmail.com", &authorize).await;
    let approved = server.consent(&cookie, &client_id, &READER).await;
    assert_eq!(approved.status(), 303);
    let code = code_in(approved.headers()[LOCATION].to_str().unwrap());
    let tokens: Value = server.tokens(&client_id, &code).await.json().await.unwrap();
    let access = token(&tokens, "access_token");
    assert!(access.starts_with("wat_"));

    let stored: Vec<String> =
        sqlx::query_scalar("SELECT token_hash FROM oauth_tokens").fetch_all(&server.state.db).await.unwrap();
    assert!(!stored.contains(&access) && stored.contains(&crate::sha256_hex(access.as_bytes())));

    let tools = server.tool_names(&access).await;
    assert!(tools.contains(&"search".to_string()) && tools.contains(&"get_thread".to_string()));
    assert!(!tools.iter().any(|tool| ["modify", "unsubscribe", "block_sender"].contains(&tool.as_str())), "{tools:?}");
    let found = server.call(&access, "search", json!({ "query": "plan" })).await.unwrap();
    assert_eq!(found["messages"][0]["subject"], "Plan for Monday");
    assert_eq!(found["messages"][0]["labels"], json!(["inbox"]));
    let id = found["messages"][0]["id"].as_str().unwrap();
    let refused = server.call(&access, "modify", json!({ "action": "archive", "message_ids": [id] })).await;
    assert!(refused.unwrap_err().contains("can't use"));
    let thread = server.call(&access, "get_thread", json!({ "message_id": id })).await.unwrap();
    assert!(thread["messages"][0].get("body").is_none() && thread["note"].is_string());
}

#[sqlx::test]
async fn an_app_given_some_accounts_sees_only_their_mail(db: PgPool) {
    let server = Server::start(db).await;
    let (_, user_id, gmail) = server.gmail_user("ann@gmail.com").await;
    let work = second_account(&server, user_id, "ann@work.com").await;
    mail(&server, &gmail, &[message("home", "mom@example.com", "Dinner", &["inbox"])]).await;
    mail(&server, &work, &[message("office", "boss@work.com", "Dinner meeting", &["inbox"])]).await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;

    let work_id = work.id.to_string();
    let missing = server
        .consent(&cookie, &server.register().await, &[("decision", "approve"), ("access", "some"), ("role", "reader")])
        .await;
    assert_eq!(missing.status(), 400);
    assert!(missing.text().await.unwrap().contains("Pick at least one account."));
    let some = [("decision", "approve"), ("access", "some"), ("accounts", work_id.as_str()), ("role", "organizer")];
    let (_, tokens) = server.connect(&cookie, &some).await;
    let access = token(&tokens, "access_token");

    let accounts = server.call(&access, "list_accounts", json!({})).await.unwrap();
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(accounts["access"]["all_accounts"], false);
    let found = server.call(&access, "search", json!({ "query": "dinner" })).await.unwrap();
    let subjects: Vec<&str> =
        found["messages"].as_array().unwrap().iter().map(|m| m["subject"].as_str().unwrap()).collect();
    assert_eq!(subjects, ["Dinner meeting"]);
    let home: Uuid = sqlx::query_scalar("SELECT id FROM messages WHERE provider_id = 'home'")
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    let elsewhere = server.call(&access, "modify", json!({ "action": "trash", "message_ids": [home] })).await;
    assert_eq!(elsewhere.unwrap_err(), "No such message.");
    assert!(!server.tool_names(&access).await.contains(&"block_sender".to_string()));
    let rule = server.call(&access, "create_rule", json!({ "from": ["@shop.com"], "skip_inbox": true })).await;
    assert!(rule.unwrap_err().contains("only some accounts"));
}

#[sqlx::test]
async fn codes_and_refresh_tokens_work_once(db: PgPool) {
    let server = Server::start(db).await;
    server.gmail_user("ann@gmail.com").await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;
    let client_id = server.register().await;
    let approved = server.consent(&cookie, &client_id, &READER).await;
    let code = code_in(approved.headers()[LOCATION].to_str().unwrap());

    let other_verifier = "x".repeat(43);
    let wrong = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", "http://localhost:51234/callback"),
        ("code_verifier", other_verifier.as_str()),
    ];
    let mismatched: Value = server.form("/oauth/token", "", &wrong).await.json().await.unwrap();
    assert_eq!(mismatched["error"], "invalid_grant");
    let tokens: Value = server.tokens(&client_id, &code).await.json().await.unwrap();
    let access = token(&tokens, "access_token");
    assert_eq!(server.tool_names(&access).await.len(), 7);

    let renewed: Value = server.refresh(&client_id, &token(&tokens, "refresh_token")).await.json().await.unwrap();
    let renewed_access = token(&renewed, "access_token");
    assert_eq!(server.rpc(&renewed_access, "tools/list", json!({})).await.status(), 200);
    let replayed: Value = server.refresh(&client_id, &token(&tokens, "refresh_token")).await.json().await.unwrap();
    assert_eq!(replayed["error"], "invalid_grant");
    assert_eq!(server.rpc(&renewed_access, "tools/list", json!({})).await.status(), 401);

    let reused: Value = server.tokens(&client_id, &code).await.json().await.unwrap();
    assert_eq!(reused["error"], "invalid_grant");
}

#[sqlx::test]
async fn revoking_an_app_ends_its_access(db: PgPool) {
    let server = Server::start(db).await;
    server.gmail_user("ann@gmail.com").await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;
    let (_, tokens) = server.connect(&cookie, &READER).await;
    let access = token(&tokens, "access_token");

    let page = server.page("/connected-apps", &cookie).await.text().await.unwrap();
    assert!(page.contains("Claude Code") && page.contains("Reader") && page.contains("Every account"), "{page}");
    let fields = hidden_fields(&page);
    let csrf = fields.iter().find(|(name, _)| name == "csrf").unwrap().1.clone();
    let grant = fields.iter().find(|(name, _)| name == "id").unwrap().1.clone();
    let forged = server.form("/connected-apps/revoke", &cookie, &[("csrf", "forged"), ("id", &grant)]).await;
    assert_eq!(forged.status(), 400);
    assert_eq!(server.rpc(&access, "tools/list", json!({})).await.status(), 200);
    let revoked = server.form("/connected-apps/revoke", &cookie, &[("csrf", &csrf), ("id", &grant)]).await;
    assert_eq!(revoked.status(), 303);
    assert_eq!(server.rpc(&access, "tools/list", json!({})).await.status(), 401);

    server.gmail_user("bob@gmail.com").await;
    let bob = server.browser("bob@gmail.com", "/connected-apps").await;
    let page = server.page("/connected-apps", &bob).await.text().await.unwrap();
    let bob_csrf = hidden_fields(&page).into_iter().find(|(name, _)| name == "csrf").unwrap().1;
    let (_, ann_tokens) = server.connect(&cookie, &READER).await;
    let ann_grant: Uuid = sqlx::query_scalar("SELECT id FROM oauth_grants WHERE revoked_at IS NULL")
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    let theirs =
        server.form("/connected-apps/revoke", &bob, &[("csrf", &bob_csrf), ("id", &ann_grant.to_string())]).await;
    assert_eq!(theirs.status(), 400);
    assert_eq!(server.rpc(&token(&ann_tokens, "access_token"), "tools/list", json!({})).await.status(), 200);
}

#[sqlx::test]
async fn the_consent_page_refuses_what_it_cant_trust(db: PgPool) {
    let server = Server::start(db).await;
    server.gmail_user("ann@gmail.com").await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;
    let client_id = server.register().await;

    let elsewhere = server.authorize_path(&client_id, &challenge()).replace("localhost%3A51234", "evil.example");
    let refused = server.page(&elsewhere, &cookie).await;
    assert_eq!(refused.status(), 400);
    assert!(refused.headers().get(LOCATION).is_none());
    let weak = server.page(&server.authorize_path(&client_id, "short"), &cookie).await;
    assert_eq!(weak.status(), 303);
    let location = weak.headers()[LOCATION].to_str().unwrap();
    assert!(location.starts_with("http://localhost:51234/callback?error=invalid_request"));

    let page = server.page(&server.authorize_path(&client_id, &challenge()), &cookie).await.text().await.unwrap();
    let mut fields = hidden_fields(&page);
    fields.retain(|(name, _)| name != "csrf");
    let mut form: Vec<(&str, &str)> = fields.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
    form.extend_from_slice(&READER);
    let forged = server.form("/oauth/authorize", &cookie, &form).await;
    assert_eq!(forged.status(), 400);
    let codes: i64 = sqlx::query_scalar("SELECT count(*) FROM oauth_codes").fetch_one(&server.state.db).await.unwrap();
    assert_eq!(codes, 0);

    let denied = server.consent(&cookie, &client_id, &[("decision", "deny")]).await;
    assert!(denied.headers()[LOCATION].to_str().unwrap().contains("error=access_denied"));

    let stranger = server.browser_sign_in("stranger@gmail.com", "/connected-apps").await;
    assert_eq!(stranger.status(), 400);
    assert!(stranger.headers().get(SET_COOKIE).is_none());
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&server.state.db).await.unwrap();
    assert_eq!(users, 1);
}

#[sqlx::test]
async fn an_organizer_cleans_up_by_sender(db: PgPool) {
    let server = Server::start(db).await;
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    let web_only = Unsubscribe { url: Some("https://deals.example/leave".into()), mailto: None, one_click: false };
    mail(
        &server,
        &account,
        &[
            newsletter("d1", "news@deals.example", Some(web_only.clone())),
            newsletter("d2", "news@deals.example", Some(web_only)),
            newsletter("s1", "hello@shop.example", None),
            message("p1", "cy@example.com", "Lunch?", &["inbox"]),
        ],
    )
    .await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;
    let organizer = [ORGANIZER.as_slice(), &[("privileges", "unsubscribe")]].concat();
    let (_, tokens) = server.connect(&cookie, &organizer).await;
    let access = token(&tokens, "access_token");

    let senders = server.call(&access, "list_senders", json!({ "bulk": true })).await.unwrap();
    assert_eq!(senders["senders"][0]["sender"], "news@deals.example");
    assert_eq!(senders["senders"][0]["messages"], 2);
    assert_eq!(senders["senders"][0]["unsubscribe"], "web");
    assert_eq!(senders["senders"].as_array().unwrap().len(), 2);
    let important = server.call(&access, "search", json!({ "important": true })).await.unwrap();
    assert_eq!(important["messages"].as_array().unwrap().len(), 1);
    assert_eq!(important["messages"][0]["subject"], "Lunch?");

    let left = server
        .call(&access, "unsubscribe", json!({ "senders": ["news@deals.example", "hello@shop.example"] }))
        .await
        .unwrap();
    assert_eq!(left["results"][0]["result"], "web_page");
    assert_eq!(left["results"][0]["url"], "https://deals.example/leave");
    assert_eq!(left["results"][1]["result"], "no_way");

    let counted = server
        .call(&access, "modify", json!({ "action": "archive", "query": "from:news@deals.example" }))
        .await
        .unwrap();
    assert_eq!(counted["would_change_threads"], 2);
    let inbox: i64 = sqlx::query_scalar("SELECT count(*) FROM messages WHERE 'inbox' = ANY(labels)")
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    assert_eq!(inbox, 4);
    let archived = server
        .call(&access, "modify", json!({ "action": "archive", "query": "from:news@deals.example", "confirm": true }))
        .await
        .unwrap();
    assert_eq!(archived["changed_messages"], 2);
    let inbox: i64 = sqlx::query_scalar("SELECT count(*) FROM messages WHERE 'inbox' = ANY(labels)")
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    assert_eq!(inbox, 2);
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM provider_ops").fetch_one(&server.state.db).await.unwrap();
    assert_eq!(queued, 2);

    let label =
        server.call(&access, "create_label", json!({ "account_id": account.id, "name": "Shopping" })).await.unwrap();
    let rule = server
        .call(
            &access,
            "create_rule",
            json!({ "from": ["@shop.example"], "label": "Shopping", "account_id": account.id,
            "skip_inbox": true, "apply_to_existing": true }),
        )
        .await
        .unwrap();
    assert_eq!(rule["changed_messages"], 2);
    assert_eq!(rule["rule"]["label"]["id"], label["id"]);
    let rules = server.call(&access, "list_rules", json!({})).await.unwrap();
    assert_eq!(rules["rules"].as_array().unwrap().len(), 1);
    let blocked = server.call(&access, "block_sender", json!({ "sender": "@deals.example" })).await.unwrap();
    assert_eq!(blocked["blocked"], true);
    let preference: bool =
        sqlx::query_scalar("SELECT NOT deleted FROM preferences WHERE key = 'blocked:@deals.example'")
            .fetch_one(&server.state.db)
            .await
            .unwrap();
    assert!(preference);
}

#[sqlx::test]
async fn an_organizer_splits_the_inbox_by_label(db: PgPool) {
    let server = Server::start(db).await;
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    mail(
        &server,
        &account,
        &[
            message("g1", "notifications@github.com", "[Org/Repo] Fix the build", &["inbox"]),
            message("o1", "ship@track.shop.example", "Your order shipped", &["inbox"]),
            message("p1", "cy@example.com", "Lunch?", &["inbox"]),
        ],
    )
    .await;
    let cookie = server.browser("ann@gmail.com", "/connected-apps").await;
    let (_, tokens) = server.connect(&cookie, &ORGANIZER).await;
    let access = token(&tokens, "access_token");

    let found = server.call(&access, "search", json!({ "query": "subject:repo" })).await.unwrap();
    assert_eq!(found["messages"].as_array().unwrap().len(), 1);
    let others = server.call(&access, "search", json!({ "query": "-from:cy@example.com" })).await.unwrap();
    assert_eq!(others["messages"].as_array().unwrap().len(), 2);

    server.call(&access, "create_label", json!({ "account_id": account.id, "name": "Orders" })).await.unwrap();
    let rule = server
        .call(
            &access,
            "create_rule",
            json!({ "from": ["@shop.example"], "label": "Orders", "account_id": account.id, "apply_to_existing": true }),
        )
        .await
        .unwrap();
    assert_eq!(rule["changed_messages"], 1);
    let split = server
        .call(&access, "create_split", json!({ "name": "Orders", "label": "Orders", "account_id": account.id }))
        .await
        .unwrap();
    assert_eq!(split["split"]["label"]["name"], "Orders");
    assert!(split["note"].is_string());
    let on = server.call(&access, "set_split_inbox", json!({ "on": true })).await.unwrap();
    assert_eq!(on["split_inbox"], true);
    let splits = server.call(&access, "list_splits", json!({})).await.unwrap();
    assert_eq!(splits["split_inbox"], true);
    assert_eq!(splits["splits"].as_array().unwrap().len(), 1);

    let id = splits["splits"][0]["id"].as_str().unwrap();
    server.call(&access, "delete_split", json!({ "id": id })).await.unwrap();
    let splits = server.call(&access, "list_splits", json!({})).await.unwrap();
    assert!(splits["splits"].as_array().unwrap().is_empty());
}

async fn shared_mailbox(server: &Server, user_id: Uuid) -> AccountRow {
    let credentials = server.state.sealer.seal(json!({ "refresh_token": "refresh" }).to_string().as_bytes());
    let account = NewAccount { user_id, provider: Provider::Gmail, address: "team@gmail.com", login: "", credentials };
    let id = db::upsert_account(&server.state.db, account).await.unwrap();
    db::account(&server.state.db, id).await.unwrap().unwrap()
}

#[sqlx::test]
async fn people_who_share_a_mailbox_each_connect_their_own_copy(db: PgPool) {
    let server = Server::start(db).await;
    let (_, ann_id, _) = server.gmail_user("ann@gmail.com").await;
    let (_, bob_id, _) = server.gmail_user("bob@gmail.com").await;
    let ann_team = shared_mailbox(&server, ann_id).await;
    let bob_team = shared_mailbox(&server, bob_id).await;
    let invoice = message("invoice", "billing@vendor.example", "Invoice 42", &["inbox"]);
    mail(&server, &ann_team, std::slice::from_ref(&invoice)).await;
    mail(&server, &bob_team, &[invoice]).await;

    let shared = server.browser_sign_in("team@gmail.com", "/connected-apps").await;
    assert_eq!(shared.status(), 400);
    assert!(shared.headers().get(SET_COOKIE).is_none());
    assert!(shared.text().await.unwrap().contains("used by more than one person"));

    let ann = server.browser("ann@gmail.com", "/connected-apps").await;
    let bob = server.browser("bob@gmail.com", "/connected-apps").await;
    let theirs = bob_team.id.to_string();
    let borrowed = [("decision", "approve"), ("access", "some"), ("accounts", theirs.as_str()), ("role", "organizer")];
    let refused = server.consent(&ann, &server.register().await, &borrowed).await;
    assert!(refused.text().await.unwrap().contains("Pick at least one account."));

    let (_, ann_tokens) = server.connect(&ann, &ORGANIZER).await;
    let (_, bob_tokens) = server.connect(&bob, &ORGANIZER).await;
    let (ann_access, bob_access) = (token(&ann_tokens, "access_token"), token(&bob_tokens, "access_token"));

    let id_of = async |account: &AccountRow| -> Uuid {
        sqlx::query_scalar("SELECT id FROM messages WHERE account_id = $1")
            .bind(account.id)
            .fetch_one(&server.state.db)
            .await
            .unwrap()
    };
    let (ann_invoice, bob_invoice) = (id_of(&ann_team).await, id_of(&bob_team).await);
    let found = server.call(&ann_access, "search", json!({ "query": "invoice" })).await.unwrap();
    assert_eq!(found["messages"].as_array().unwrap().len(), 1);
    assert_eq!(found["messages"][0]["id"], ann_invoice.to_string());
    let accounts = server.call(&ann_access, "list_accounts", json!({})).await.unwrap();
    let ids: Vec<&str> = accounts["accounts"].as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&ann_team.id.to_string().as_str()) && !ids.contains(&theirs.as_str()));

    let crossing = server.call(&ann_access, "get_thread", json!({ "message_id": bob_invoice })).await;
    assert_eq!(crossing.unwrap_err(), "No such message.");
    server.call(&ann_access, "modify", json!({ "action": "archive", "message_ids": [ann_invoice] })).await.unwrap();
    let inbox_of = async |id: Uuid| -> bool {
        sqlx::query_scalar("SELECT 'inbox' = ANY(labels) FROM messages WHERE id = $1")
            .bind(id)
            .fetch_one(&server.state.db)
            .await
            .unwrap()
    };
    assert!(!inbox_of(ann_invoice).await);
    assert!(inbox_of(bob_invoice).await);
    let bob_senders = server.call(&bob_access, "list_senders", json!({ "query": "in:inbox" })).await.unwrap();
    assert_eq!(bob_senders["senders"][0]["sender"], "billing@vendor.example");

    let page = server.page("/connected-apps", &ann).await.text().await.unwrap();
    assert_eq!(page.matches("Revoke").count(), 1);
}
