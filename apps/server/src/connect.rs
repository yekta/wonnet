//! An application asking to reach the user's mail, answered in the app. The application sends a
//! browser to `/oauth/authorize`; its page opens the app on the request (`wonnet://connect`, or a
//! QR code for a phone) and waits; the user chooses what the application may do in the app, which
//! answers through `/api/connect`; the page sees the answer and takes the browser back to the
//! application with a code. Only the browser that asked, holding the request's cookie, gets the
//! code, and a request lasts ten minutes. The apps list and revoke what they approved through
//! `/api/connected-apps`.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use chrono::{DateTime, Duration, Utc};
use mail_protocol::APP_CONNECT;
use mail_protocol::api::{ConnectAnswer, ConnectRequest, ConnectedApp, ConnectedApps};
use maud::Markup;
use serde_json::json;
use sqlx::types::Json as JsonColumn;
use uuid::Uuid;

use crate::access::Access;
use crate::error::{AppError, AppResult};
use crate::oauth::{self, Approved, AuthorizeRequest, Refusal};
use crate::pages::{self, Asking};
use crate::{AppState, api, random_bytes, random_token, sha256_hex};

pub const REQUEST_LIFETIME: Duration = Duration::minutes(10);
const COOKIE_NAME: &str = "wonnet_request";
const GONE: &str = "This request has expired or was already answered. Connect again from the application.";

fn page(status: StatusCode, markup: Markup, nonce: Option<&str>) -> Response {
    let script = nonce.map(|nonce| format!("script-src 'nonce-{nonce}'; connect-src 'self';")).unwrap_or_default();
    let policy = format!(
        "default-src 'none'; style-src 'unsafe-inline'; img-src 'self' data:; {script} base-uri 'none'; \
         form-action 'none'; frame-ancestors 'none'"
    );
    let policy = HeaderValue::from_str(&policy).expect("a policy of plain characters");
    (status, [(CONTENT_SECURITY_POLICY, policy)], Html(markup.into_string())).into_response()
}

fn error_page(message: &str) -> Response {
    page(StatusCode::BAD_REQUEST, pages::error(message), None)
}

fn went_wrong(error: impl std::fmt::Display) -> Response {
    tracing::error!("connect: {error}");
    page(StatusCode::INTERNAL_SERVER_ERROR, pages::error("Something went wrong. Try again in a moment."), None)
}

fn cookie(state: &AppState, id: Uuid, value: &str, max_age: i64) -> HeaderValue {
    let secure = if state.config.public_url.starts_with("https://") { "; Secure" } else { "" };
    let value =
        format!("{COOKIE_NAME}={value}; Path=/oauth/requests/{id}; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}");
    HeaderValue::from_str(&value).expect("a cookie of plain characters")
}

fn cookie_value(headers: &HeaderMap) -> Option<&str> {
    headers.get_all(COOKIE).iter().filter_map(|value| value.to_str().ok()).flat_map(|value| value.split(';')).find_map(
        |pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE_NAME).then_some(value)
        },
    )
}

/// The application asking, as the page and the app show it.
fn asking(client_name: &str, verified: bool, publisher: Option<&str>, redirect_uri: &str) -> Asking {
    Asking {
        client: client_name.to_string(),
        verified,
        publisher: publisher.map(String::from),
        redirect_host: oauth::host_of(redirect_uri),
        on_device: oauth::on_device(redirect_uri),
    }
}

/// `/oauth/authorize`: checks the application's request, keeps it for the app to answer, and
/// shows the page that opens the app and waits. Opened again, it makes a new request.
pub async fn authorize(State(state): State<AppState>, Query(mut request): Query<AuthorizeRequest>) -> Response {
    let client = match oauth::validate(&state, &mut request).await {
        Ok(client) => client,
        Err(Refusal::Here(message)) => return error_page(&message),
        Err(Refusal::Back(url)) => return Redirect::to(&url).into_response(),
    };
    // The key goes in a QR code: 128 bits keep it easy to scan.
    let (id, key, browser) = (Uuid::new_v4(), hex::encode(random_bytes::<16>()), random_token());
    let verified = client.verified().is_some();
    let publisher = client.publisher();
    let saved = sqlx::query(
        "INSERT INTO oauth_requests (id, key_hash, browser_hash, client_id, client_name, client_kind, client_verified,
             client_publisher, redirect_uri, state, code_challenge, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(id)
    .bind(sha256_hex(key.as_bytes()))
    .bind(sha256_hex(browser.as_bytes()))
    .bind(&client.client_id)
    .bind(&client.name)
    .bind(client.kind.as_str())
    .bind(verified)
    .bind(&publisher)
    .bind(&request.redirect_uri)
    .bind(&request.state)
    .bind(&request.code_challenge)
    .bind(Utc::now() + REQUEST_LIFETIME)
    .execute(&state.db)
    .await;
    if let Err(error) = saved {
        return went_wrong(error);
    }
    let nonce = random_token();
    let waiting = pages::Waiting {
        asking: &asking(&client.name, verified, publisher.as_deref(), &request.redirect_uri),
        id,
        open: &format!("{APP_CONNECT}?key={key}"),
        phone: &format!("{}/connect/{key}", oauth::issuer(&state.config)),
        nonce: &nonce,
    };
    let mut response = page(StatusCode::OK, pages::waiting(&waiting), Some(&nonce));
    let headers = response.headers_mut();
    headers.insert(SET_COOKIE, cookie(&state, id, &browser, REQUEST_LIFETIME.num_seconds()));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(sqlx::FromRow)]
struct RequestRow {
    browser_hash: String,
    client_id: String,
    client_name: String,
    client_kind: String,
    redirect_uri: String,
    state: String,
    code_challenge: String,
    user_id: Option<Uuid>,
    access: Option<JsonColumn<Access>>,
    approved: Option<bool>,
    finished_at: Option<DateTime<Utc>>,
    expires_at: DateTime<Utc>,
}

/// The request `id`, when `headers` carry the cookie of the browser that made it.
async fn browsers_request(state: &AppState, id: Uuid, headers: &HeaderMap) -> sqlx::Result<Option<RequestRow>> {
    let Some(browser) = cookie_value(headers) else { return Ok(None) };
    let row: Option<RequestRow> = sqlx::query_as(
        "SELECT browser_hash, client_id, client_name, client_kind, redirect_uri, state, code_challenge, user_id,
             access, approved, finished_at, expires_at
         FROM oauth_requests WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.filter(|row| row.browser_hash == sha256_hex(browser.as_bytes())))
}

/// What the waiting page asks, every second: `waiting`, `approved`, `denied` or `expired`.
pub async fn status(State(state): State<AppState>, Path(id): Path<Uuid>, headers: HeaderMap) -> Response {
    let row = match browsers_request(&state, id, &headers).await {
        Ok(row) => row,
        Err(error) => {
            tracing::error!("connect: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let answer = match row {
        Some(row) if row.finished_at.is_some() || row.expires_at <= Utc::now() => "expired",
        Some(RequestRow { approved: Some(true), .. }) => "approved",
        Some(RequestRow { approved: Some(false), .. }) => "denied",
        Some(_) => "waiting",
        None => "expired",
    };
    ([(CACHE_CONTROL, "no-store")], Json(json!({ "state": answer }))).into_response()
}

/// Where the page goes once the request is answered: back to the application, with a code when
/// it was approved. Only once, and only for the browser that asked.
pub async fn finish(State(state): State<AppState>, Path(id): Path<Uuid>, headers: HeaderMap) -> Response {
    let row = match browsers_request(&state, id, &headers).await {
        Ok(Some(row)) if row.finished_at.is_none() && row.expires_at > Utc::now() => row,
        Ok(_) => {
            return error_page("This request has expired. Close this window and connect again from the application.");
        }
        Err(error) => return went_wrong(error),
    };
    if row.approved.is_none() {
        return page(StatusCode::OK, pages::not_yet(id), None);
    }
    let finished = sqlx::query("UPDATE oauth_requests SET finished_at = now() WHERE id = $1 AND finished_at IS NULL")
        .bind(id)
        .execute(&state.db)
        .await;
    match finished {
        Ok(done) if done.rows_affected() == 1 => {}
        Ok(_) => return error_page("This request was already finished. Go back to the application."),
        Err(error) => return went_wrong(error),
    }
    let back = match (row.approved, row.user_id, &row.access) {
        (Some(true), Some(user_id), Some(access)) => {
            let approved = Approved {
                user_id,
                access: &access.0,
                client_id: &row.client_id,
                client_name: &row.client_name,
                client_kind: &row.client_kind,
                redirect_uri: &row.redirect_uri,
                state: &row.state,
                code_challenge: &row.code_challenge,
            };
            match oauth::issue_code(&state, &approved).await {
                Ok(url) => url,
                Err(error) => return went_wrong(error),
            }
        }
        _ => oauth::denied_redirect(&state, &row.redirect_uri, &row.state),
    };
    let mut response = Redirect::to(&back).into_response();
    response.headers_mut().insert(SET_COOKIE, cookie(&state, id, "", 0));
    response
}

#[derive(sqlx::FromRow)]
struct PendingRow {
    client_name: String,
    client_verified: bool,
    client_publisher: Option<String>,
    redirect_uri: String,
}

/// The request a key opens, while it waits for an answer.
async fn pending(state: &AppState, key: &str) -> sqlx::Result<Option<PendingRow>> {
    sqlx::query_as(
        "SELECT client_name, client_verified, client_publisher, redirect_uri FROM oauth_requests
         WHERE key_hash = $1 AND approved IS NULL AND expires_at > now()",
    )
    .bind(sha256_hex(key.as_bytes()))
    .fetch_optional(&state.db)
    .await
}

/// `/connect/{key}`, the QR code's address: on the phone that scanned it, a button that opens the
/// app on the request.
pub async fn open(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let row = match pending(&state, &key).await {
        Ok(Some(row)) => row,
        Ok(None) => return error_page(GONE),
        Err(error) => return went_wrong(error),
    };
    let asking = asking(&row.client_name, row.client_verified, row.client_publisher.as_deref(), &row.redirect_uri);
    let markup = pages::open_on_phone(&asking, &format!("{APP_CONNECT}?key={key}"));
    let mut response = page(StatusCode::OK, markup, None);
    response.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `GET /api/connect/{key}`: the request, for the app to ask the user about.
pub async fn request(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<ConnectRequest>> {
    api::user(&state, &headers).await?;
    let row = pending(&state, &key).await?.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, GONE))?;
    let asking = asking(&row.client_name, row.client_verified, row.client_publisher.as_deref(), &row.redirect_uri);
    Ok(Json(ConnectRequest {
        client: asking.client,
        verified: asking.verified,
        publisher: asking.publisher,
        redirect_host: asking.redirect_host,
        on_device: asking.on_device,
    }))
}

/// `POST /api/connect/{key}`: the user's answer. An approval reaches only the user's own accounts.
pub async fn answer(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(answer): Json<ConnectAnswer>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = api::user(&state, &headers).await?;
    let access = match (answer.approve, answer.access) {
        (false, _) => None,
        (true, None) => return Err(AppError::bad_request("Choose what the application may do.")),
        (true, Some(access)) => Some(checked(&state, user_id, access).await?),
    };
    let answered = sqlx::query(
        "UPDATE oauth_requests SET user_id = $2, access = $3, approved = $4, answered_at = now()
         WHERE key_hash = $1 AND approved IS NULL AND expires_at > now()",
    )
    .bind(sha256_hex(key.as_bytes()))
    .bind(user_id)
    .bind(access.map(JsonColumn))
    .bind(answer.approve)
    .execute(&state.db)
    .await?;
    if answered.rows_affected() == 0 {
        return Err(AppError::new(StatusCode::NOT_FOUND, GONE));
    }
    Ok(Json(json!({})))
}

/// The access as it is kept: the accounts picked, all of them the user's, or none when it reaches
/// every account.
async fn checked(state: &AppState, user_id: Uuid, mut access: Access) -> AppResult<Access> {
    access.privileges.sort_by_key(|privilege| privilege.as_str());
    access.privileges.dedup();
    if access.all_accounts {
        access.accounts.clear();
        return Ok(access);
    }
    access.accounts.sort();
    access.accounts.dedup();
    if access.accounts.is_empty() {
        return Err(AppError::bad_request("Pick at least one account."));
    }
    let ids: Vec<Uuid> = access.accounts.iter().filter_map(|id| id.parse().ok()).collect();
    let owned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM accounts WHERE user_id = $1 AND id = ANY($2) AND NOT deleted")
            .bind(user_id)
            .bind(&ids)
            .fetch_one(&state.db)
            .await?;
    if owned as usize != access.accounts.len() {
        return Err(AppError::bad_request("One of those accounts isn't yours anymore."));
    }
    Ok(access)
}

/// `GET /api/connected-apps`.
pub async fn connected_apps(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<ConnectedApps>> {
    let user_id = api::user(&state, &headers).await?;
    let apps = oauth::grants(&state.db, user_id)
        .await?
        .into_iter()
        .map(|grant| {
            let client = grant.client();
            ConnectedApp {
                id: grant.id.to_string(),
                client: grant.client_name,
                verified: client.verified().is_some(),
                publisher: client.publisher(),
                access: grant.access.0,
                approved_at: grant.created_at.timestamp_millis(),
                used_at: grant.last_used_at.map(|used| used.timestamp_millis()),
            }
        })
        .collect();
    Ok(Json(ConnectedApps { address: oauth::resource(&state.config), apps }))
}

/// `DELETE /api/connected-apps/{id}`: ends the app's access at once.
pub async fn revoke(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = api::user(&state, &headers).await?;
    match oauth::revoke(&state.db, user_id, id).await? {
        true => {
            tracing::info!("user {user_id} revoked connected app {id}");
            Ok(Json(json!({})))
        }
        false => Err(AppError::new(StatusCode::NOT_FOUND, "That application isn't connected.")),
    }
}
