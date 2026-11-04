//! The server's pages for a browser: the consent a connected app asks for, the list of connected
//! apps, and signing in to reach them. A signed-in browser holds a session cookie of its own,
//! apart from the apps' sessions, and every form it posts carries that session's CSRF token.

use axum::body::Bytes;
use axum::extract::{Query, RawQuery, State};
use axum::http::header::{CONTENT_SECURITY_POLICY, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use chrono::{Duration, Utc};
use mail_protocol::Provider;
use maud::Markup;
use uuid::Uuid;

use crate::access::{Access, Privilege, Role};
use crate::db::SignInUser;
use crate::oauth::{self, AuthorizeRequest, Refusal};
use crate::pages::{self, AccountChoice, Choice};
use crate::providers::Reauth;
use crate::providers::jmap::{Jmap, Login};
use crate::{AppState, db, random_token, sha256_hex};

const COOKIE_NAME: &str = "wonnet_session";
pub const SESSION_LIFETIME: Duration = Duration::hours(1);
const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src 'self' data:; base-uri 'none'; \
     frame-ancestors 'none'";

pub struct WebSession {
    pub user_id: Uuid,
    pub csrf: String,
}

fn page(markup: Markup) -> Response {
    page_with(StatusCode::OK, markup)
}

fn page_with(status: StatusCode, markup: Markup) -> Response {
    let policy = [(CONTENT_SECURITY_POLICY, POLICY)];
    (status, policy, Html(markup.into_string())).into_response()
}

fn error_page(message: &str) -> Response {
    page_with(StatusCode::BAD_REQUEST, pages::error(message))
}

fn went_wrong(error: impl std::fmt::Display) -> Response {
    tracing::error!("web: {error}");
    page_with(StatusCode::INTERNAL_SERVER_ERROR, pages::error("Something went wrong. Try again in a moment."))
}

/// Where a sign-in started on these pages may go back to, so it can't be sent elsewhere.
pub fn safe_next(next: &str) -> bool {
    next.starts_with("/oauth/authorize?") || next == "/connected-apps"
}

fn cookie_value(headers: &HeaderMap) -> Option<&str> {
    headers.get_all(COOKIE).iter().filter_map(|value| value.to_str().ok()).flat_map(|value| value.split(';')).find_map(
        |pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE_NAME).then_some(value)
        },
    )
}

pub async fn session(state: &AppState, headers: &HeaderMap) -> sqlx::Result<Option<WebSession>> {
    let Some(token) = cookie_value(headers) else { return Ok(None) };
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT user_id, csrf FROM web_sessions WHERE token_hash = $1 AND expires_at > now()")
            .bind(sha256_hex(token.as_bytes()))
            .fetch_optional(&state.db)
            .await?;
    Ok(row.map(|(user_id, csrf)| WebSession { user_id, csrf }))
}

fn cookie(state: &AppState, token: &str, max_age: i64) -> HeaderValue {
    let secure = if state.config.public_url.starts_with("https://") { "; Secure" } else { "" };
    let value = format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}");
    HeaderValue::from_str(&value).expect("a cookie of plain characters")
}

/// Signs the browser in as `user_id` and sends it on to `next`.
pub async fn start_session(state: &AppState, user_id: Uuid, next: &str) -> Response {
    let token = random_token();
    let saved = sqlx::query("INSERT INTO web_sessions (token_hash, user_id, csrf, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(sha256_hex(token.as_bytes()))
        .bind(user_id)
        .bind(random_token())
        .bind(Utc::now() + SESSION_LIFETIME)
        .execute(&state.db)
        .await;
    if let Err(error) = saved {
        return went_wrong(error);
    }
    let next = if safe_next(next) { next } else { "/connected-apps" };
    let mut response = Redirect::to(next).into_response();
    response.headers_mut().insert(SET_COOKIE, cookie(state, &token, SESSION_LIFETIME.num_seconds()));
    response
}

fn sign_in_page(state: &AppState, next: &str, error: Option<&str>) -> Response {
    let sign_in = pages::SignIn {
        next,
        google: state.config.google_enabled(),
        microsoft: state.config.microsoft_enabled(),
        error,
    };
    page(pages::sign_in(&sign_in))
}

/// Who a browser sign-in with this mailbox is: only someone already using Wonnet, since these
/// pages don't make new users.
pub async fn web_user(
    state: &AppState,
    provider: Provider,
    address: &str,
    login: &str,
) -> sqlx::Result<Result<Uuid, &'static str>> {
    Ok(match db::sign_in_user(&state.db, provider, address, login).await? {
        SignInUser::Existing(user_id) => Ok(user_id),
        SignInUser::New => Err("This address isn't in Wonnet. Add it in the app first."),
        SignInUser::Shared => Err("This email is used by more than one person, so it can't sign you in. \
             Sign in with the address you started with."),
        SignInUser::Added => Err("This email was added after another address, so it can't sign you in. \
             Sign in with the address you started with."),
    })
}

fn form_pairs(body: &Bytes) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

fn field<'a>(pairs: &'a [(String, String)], name: &str) -> &'a str {
    pairs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str()).unwrap_or_default()
}

fn fields<'a>(pairs: &'a [(String, String)], name: &'a str) -> impl Iterator<Item = &'a str> {
    pairs.iter().filter(move |(key, _)| key == name).map(|(_, value)| value.as_str())
}

fn csrf_holds(session: &WebSession, pairs: &[(String, String)]) -> bool {
    let given = field(pairs, "csrf");
    !given.is_empty() && sha256_hex(given.as_bytes()) == sha256_hex(session.csrf.as_bytes())
}

pub async fn jmap_sign_in(State(state): State<AppState>, body: Bytes) -> Response {
    let pairs = form_pairs(&body);
    let next = field(&pairs, "next");
    let next = if safe_next(next) { next } else { "/connected-apps" };
    let url = field(&pairs, "url").trim().trim_end_matches('/').to_string();
    let username = field(&pairs, "username").trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return sign_in_page(&state, next, Some("The server's address should start with https://"));
    }
    let jmap = match Jmap::open(&state.http, &url, &username, field(&pairs, "password")).await {
        Ok(jmap) => jmap,
        Err(error) if error.is::<Reauth>() => {
            return sign_in_page(&state, next, Some("The email or password is wrong."));
        }
        Err(_) => return sign_in_page(&state, next, Some("That server can't be reached. Check the address.")),
    };
    let address = match jmap.address().await {
        Ok(address) => address.email,
        Err(error) => return sign_in_page(&state, next, Some(&error.to_string())),
    };
    let Ok(login) = serde_json::to_string(&Login { url, username }) else {
        return went_wrong("couldn't write a JMAP login");
    };
    match web_user(&state, Provider::Jmap, &address, &login).await {
        Ok(Ok(user_id)) => start_session(&state, user_id, next).await,
        Ok(Err(refusal)) => sign_in_page(&state, next, Some(refusal)),
        Err(error) => went_wrong(error),
    }
}

pub async fn sign_out(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let pairs = form_pairs(&body);
    let next = field(&pairs, "next");
    let next = if safe_next(next) { next } else { "/connected-apps" };
    if let (Ok(Some(session)), Some(token)) = (session(&state, &headers).await, cookie_value(&headers))
        && csrf_holds(&session, &pairs)
    {
        let ended = sqlx::query("DELETE FROM web_sessions WHERE token_hash = $1")
            .bind(sha256_hex(token.as_bytes()))
            .execute(&state.db)
            .await;
        if let Err(error) = ended {
            return went_wrong(error);
        }
    }
    let mut response = Redirect::to(next).into_response();
    response.headers_mut().insert(SET_COOKIE, cookie(&state, "", 0));
    response
}

/// The user's accounts, oldest first, and the address the pages say they are signed in as.
async fn accounts(state: &AppState, user_id: Uuid) -> sqlx::Result<(Vec<AccountChoice>, String)> {
    let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        "SELECT id, address, name, provider, color FROM accounts WHERE user_id = $1 AND NOT deleted ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let first = rows.first().map(|(_, address, ..)| address.clone()).unwrap_or_default();
    let accounts = rows
        .into_iter()
        .map(|(id, address, name, provider, color)| AccountChoice {
            id,
            address,
            name,
            provider: Provider::parse(&provider).unwrap_or(Provider::Jmap),
            color,
        })
        .collect();
    Ok((accounts, first))
}

fn refused(refusal: Refusal) -> Response {
    match refusal {
        Refusal::Here(message) => error_page(&message),
        Refusal::Back(url) => Redirect::to(&url).into_response(),
    }
}

pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
    Query(mut request): Query<AuthorizeRequest>,
) -> Response {
    let client = match oauth::validate(&state, &mut request).await {
        Ok(client) => client,
        Err(refusal) => return refused(refusal),
    };
    let session = match session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return sign_in_page(&state, &format!("/oauth/authorize?{}", raw.unwrap_or_default()), None),
        Err(error) => return went_wrong(error),
    };
    let (accounts, signed_in_as) = match accounts(&state, session.user_id).await {
        Ok(found) => found,
        Err(error) => return went_wrong(error),
    };
    let consent = pages::Consent {
        client: &client,
        request: &request,
        accounts: &accounts,
        choice: &Choice::default(),
        csrf: &session.csrf,
        signed_in_as: &signed_in_as,
        error: None,
    };
    page(pages::consent(&consent))
}

/// The access the form asks for, or why it can't be given.
fn chosen(pairs: &[(String, String)], accounts: &[AccountChoice]) -> (Choice, Result<Access, &'static str>) {
    let all_accounts = field(pairs, "access") != "some";
    let picked: Vec<Uuid> = fields(pairs, "accounts")
        .filter_map(|id| id.parse().ok())
        .filter(|id| accounts.iter().any(|account| account.id == *id))
        .collect();
    let role = Role::parse(field(pairs, "role"));
    let mut privileges: Vec<Privilege> = fields(pairs, "privileges").filter_map(Privilege::parse).collect();
    privileges.dedup();
    let choice = Choice {
        all_accounts,
        accounts: picked.clone(),
        role: role.unwrap_or(Role::Reader),
        privileges: privileges.clone(),
    };
    let Some(role) = role else { return (choice, Err("Pick a role.")) };
    if !all_accounts && picked.is_empty() {
        return (choice, Err("Pick at least one account."));
    }
    let accounts = if all_accounts { Vec::new() } else { picked };
    (choice, Ok(Access { role, all_accounts, accounts, privileges }))
}

pub async fn decide(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let pairs = form_pairs(&body);
    let text = |name: &str| field(&pairs, name).to_string();
    let mut request = AuthorizeRequest {
        response_type: text("response_type"),
        client_id: text("client_id"),
        redirect_uri: text("redirect_uri"),
        state: text("state"),
        code_challenge: text("code_challenge"),
        code_challenge_method: text("code_challenge_method"),
        resource: text("resource"),
        scope: text("scope"),
    };
    let session = match session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return error_page("This page has expired. Start again from the application."),
        Err(error) => return went_wrong(error),
    };
    if !csrf_holds(&session, &pairs) {
        return error_page("This page has expired. Start again from the application.");
    }
    let client = match oauth::validate(&state, &mut request).await {
        Ok(client) => client,
        Err(refusal) => return refused(refusal),
    };
    if field(&pairs, "decision") != "approve" {
        let url = oauth::error_redirect(&state, &request, "access_denied", "the user denied the request");
        return Redirect::to(&url).into_response();
    }
    let (accounts, signed_in_as) = match accounts(&state, session.user_id).await {
        Ok(found) => found,
        Err(error) => return went_wrong(error),
    };
    let (choice, access) = chosen(&pairs, &accounts);
    let access = match access {
        Ok(access) => access,
        Err(problem) => {
            let consent = pages::Consent {
                client: &client,
                request: &request,
                accounts: &accounts,
                choice: &choice,
                csrf: &session.csrf,
                signed_in_as: &signed_in_as,
                error: Some(problem),
            };
            return page_with(StatusCode::BAD_REQUEST, pages::consent(&consent));
        }
    };
    match oauth::approve(&state, session.user_id, &mut request, &access).await {
        Ok(url) => {
            tracing::info!("user {} connected {} ({})", session.user_id, client.name, client.client_id);
            Redirect::to(&url).into_response()
        }
        Err(refusal) => refused(refusal),
    }
}

pub async fn connected_apps(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let session = match session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return sign_in_page(&state, "/connected-apps", None),
        Err(error) => return went_wrong(error),
    };
    let found = async {
        let grants = oauth::grants(&state.db, session.user_id).await?;
        let (accounts, signed_in_as) = accounts(&state, session.user_id).await?;
        Ok::<_, sqlx::Error>((grants, accounts, signed_in_as))
    };
    let (grants, accounts, signed_in_as) = match found.await {
        Ok(found) => found,
        Err(error) => return went_wrong(error),
    };
    let mcp_url = oauth::resource(&state.config);
    let apps = pages::ConnectedApps {
        grants: &grants,
        accounts: &accounts,
        mcp_url: &mcp_url,
        csrf: &session.csrf,
        signed_in_as: &signed_in_as,
        now: Utc::now(),
    };
    page(pages::connected_apps(&apps))
}

pub async fn revoke(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let pairs = form_pairs(&body);
    let session = match session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return Redirect::to("/connected-apps").into_response(),
        Err(error) => return went_wrong(error),
    };
    if !csrf_holds(&session, &pairs) {
        return error_page("This page has expired. Open it again.");
    }
    let Ok(grant_id) = field(&pairs, "id").parse::<Uuid>() else { return error_page("No such app.") };
    match oauth::revoke(&state.db, session.user_id, grant_id).await {
        Ok(true) => {
            tracing::info!("user {} revoked connected app {grant_id}", session.user_id);
            Redirect::to("/connected-apps").into_response()
        }
        Ok(false) => error_page("No such app."),
        Err(error) => went_wrong(error),
    }
}

pub async fn cleanup(db: &sqlx::PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM web_sessions WHERE expires_at < now()").execute(db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_back_only_to_its_own_pages() {
        assert!(safe_next("/oauth/authorize?client_id=x"));
        assert!(safe_next("/connected-apps"));
        for elsewhere in ["//evil.com/oauth/authorize?", "https://evil.com", "/connected-apps/../x", "/oauth/authorize"]
        {
            assert!(!safe_next(elsewhere), "{elsewhere}");
        }
    }
}
