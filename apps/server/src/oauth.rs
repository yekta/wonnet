//! The OAuth 2.1 server MCP clients connect with, as the MCP authorization spec has it: metadata
//! (RFC 8414, RFC 9728), clients that register (RFC 7591) or are described by a client ID metadata
//! document, PKCE, and the authorization code and refresh token grants. The consent page between
//! `/oauth/authorize` and the code is `web::authorize`.

use std::time::Duration as StdDuration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::types::Json as JsonColumn;
use uuid::Uuid;

use crate::access::{Access, Caller};
use crate::config::Config;
use crate::{AppState, random_token, sha256_hex, unsubscribe};

pub const CODE_LIFETIME: Duration = Duration::minutes(1);
pub const ACCESS_LIFETIME: Duration = Duration::hours(1);
pub const REFRESH_LIFETIME: Duration = Duration::days(30);
const ACCESS_PREFIX: &str = "wat_";
const REFRESH_PREFIX: &str = "wrt_";
const MAX_CLIENT_NAME: usize = 100;
const MAX_REDIRECT_URIS: usize = 10;
const MAX_URI: usize = 512;
const MAX_STATE: usize = 1024;
const MAX_SCOPE: usize = 512;
const DOCUMENT_LIMIT: usize = 8 * 1024;
const DOCUMENT_TIMEOUT: StdDuration = StdDuration::from_secs(5);
/// The private URI schemes of native clients we know. The list is closed: the browser is sent to
/// the redirect URI, so a scheme like `javascript:` must never get through.
const NATIVE_SCHEMES: [&str; 1] = ["cursor"];
/// Hosts whose client metadata documents the consent page vouches for.
const VERIFIED_HOSTS: [(&str, &str); 2] = [("claude.ai", "Claude"), ("chatgpt.com", "ChatGPT")];

pub fn issuer(config: &Config) -> String {
    config.public_url.trim_end_matches('/').to_string()
}

pub fn resource(config: &Config) -> String {
    format!("{}/mcp", issuer(config))
}

pub fn resource_metadata_url(config: &Config) -> String {
    format!("{}/.well-known/oauth-protected-resource", issuer(config))
}

/// An RFC 6749 error, answered as JSON.
#[derive(Debug)]
pub struct OAuthError {
    pub code: &'static str,
    pub description: String,
    pub status: StatusCode,
}

impl OAuthError {
    fn new(code: &'static str, description: impl Into<String>) -> Self {
        Self { code, description: description.into(), status: StatusCode::BAD_REQUEST }
    }

    fn invalid_request(description: impl Into<String>) -> Self {
        Self::new("invalid_request", description)
    }

    fn invalid_client(description: impl Into<String>) -> Self {
        Self::new("invalid_client", description)
    }

    fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new("invalid_grant", description)
    }

    fn invalid_metadata(description: impl Into<String>) -> Self {
        Self::new("invalid_client_metadata", description)
    }

    fn invalid_redirect(description: impl Into<String>) -> Self {
        Self::new("invalid_redirect_uri", description)
    }

    fn server_error() -> Self {
        let mut error = Self::new("server_error", "the server could not complete the request");
        error.status = StatusCode::INTERNAL_SERVER_ERROR;
        error
    }
}

impl From<sqlx::Error> for OAuthError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!("oauth: {error}");
        Self::server_error()
    }
}

impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.code, "error_description": self.description }));
        (self.status, no_store(), body).into_response()
    }
}

fn no_store() -> [(axum::http::HeaderName, &'static str); 2] {
    [(CACHE_CONTROL, "no-store"), (PRAGMA, "no-cache")]
}

// ---------- metadata ----------

pub async fn server_metadata(State(state): State<AppState>) -> Json<Value> {
    let issuer = issuer(&state.config);
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/oauth/token"),
        "registration_endpoint": format!("{issuer}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "client_id_metadata_document_supported": true,
        "authorization_response_iss_parameter_supported": true,
    }))
}

pub async fn resource_metadata(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "resource": resource(&state.config),
        "authorization_servers": [issuer(&state.config)],
        "bearer_methods_supported": ["header"],
        "resource_name": "Wonnet",
    }))
}

// ---------- clients ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// Registered here, with an id we made.
    Registered,
    /// Its client_id is the URL of a document it publishes.
    Document,
}

impl ClientKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientKind::Registered => "registered",
            ClientKind::Document => "document",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "document" => ClientKind::Document,
            _ => ClientKind::Registered,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub client_id: String,
    pub name: String,
    pub kind: ClientKind,
    pub redirect_uris: Vec<String>,
}

impl Client {
    pub fn allows(&self, redirect_uri: &str) -> bool {
        self.redirect_uris.iter().any(|registered| redirect_matches(registered, redirect_uri))
    }

    /// The first party a document client is, when its document is served by a host we know and
    /// every redirect stays on that host or on this device: then even a document planted on the
    /// host could only send codes back to it.
    pub fn verified(&self) -> Option<&'static str> {
        if self.kind != ClientKind::Document {
            return None;
        }
        let document = Url::parse(&self.client_id).ok()?;
        let host = document.host_str()?.to_ascii_lowercase();
        let (_, brand) = VERIFIED_HOSTS.iter().find(|(known, _)| *known == host)?;
        let stays = |raw: &String| {
            let Ok(redirect) = Url::parse(raw) else { return false };
            let on_host =
                redirect.scheme() == "https" && redirect.host_str().is_some_and(|it| it.eq_ignore_ascii_case(&host));
            on_host || is_loopback(redirect.host_str().unwrap_or_default())
        };
        (!self.redirect_uris.is_empty() && self.redirect_uris.iter().all(stays)).then_some(*brand)
    }

    /// Where the consent page says the client comes from.
    pub fn publisher(&self) -> Option<String> {
        match self.kind {
            ClientKind::Document => Some(host_of(&self.client_id)),
            ClientKind::Registered => None,
        }
    }
}

/// Https anywhere, plain http only to this device, and the private schemes of native clients we
/// know (RFC 8252).
pub fn validate_redirect(raw: &str) -> Result<(), &'static str> {
    if raw.is_empty() {
        return Err("redirect uri is required");
    }
    if raw.len() > MAX_URI {
        return Err("redirect uri is too long");
    }
    if raw.contains('#') {
        return Err("redirect uri must not contain a fragment");
    }
    let Ok(url) = Url::parse(raw) else { return Err("redirect uri must be an absolute url") };
    if url.host_str().is_none_or(str::is_empty) || !url.username().is_empty() || url.password().is_some() {
        return Err("redirect uri must be an absolute url without credentials");
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(url.host_str().unwrap_or_default()) => Ok(()),
        "http" => Err("http redirect uris are only allowed on localhost"),
        scheme if NATIVE_SCHEMES.contains(&scheme) => Ok(()),
        _ => Err("redirect uri must use https"),
    }
}

pub fn is_loopback(host: &str) -> bool {
    matches!(host.trim_matches(['[', ']']).to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1" | "::1")
}

/// Exact, except that loopback http redirects match whatever their port: native clients listen
/// on a new port each time (RFC 8252 §7.3). localhost and 127.0.0.1 never match each other.
pub fn redirect_matches(registered: &str, requested: &str) -> bool {
    if registered == requested {
        return true;
    }
    let (Ok(mut registered), Ok(mut requested)) = (Url::parse(registered), Url::parse(requested)) else {
        return false;
    };
    let loopback = |url: &Url| url.scheme() == "http" && is_loopback(url.host_str().unwrap_or_default());
    if !loopback(&registered) || !loopback(&requested) {
        return false;
    }
    let (Ok(()), Ok(())) = (registered.set_port(None), requested.set_port(None)) else { return false };
    registered == requested
}

/// Lands in an application on the user's own device rather than on a website.
pub fn on_device(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else { return false };
    is_loopback(url.host_str().unwrap_or_default()) || NATIVE_SCHEMES.contains(&url.scheme())
}

/// What the consent page shows for a URI. A native scheme stays, since its host alone would read
/// like a website.
pub fn host_of(raw: &str) -> String {
    let Ok(url) = Url::parse(raw) else { return String::new() };
    let host = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_string(),
    };
    match NATIVE_SCHEMES.contains(&url.scheme()) {
        true => format!("{}://{host}", url.scheme()),
        false => host,
    }
}

fn validate_client_uri(raw: &str) -> Result<(), &'static str> {
    if raw.is_empty() {
        return Ok(());
    }
    let url = Url::parse(raw).ok().filter(|url| url.scheme() == "https" && url.host_str().is_some());
    match (raw.len() > MAX_URI, url) {
        (true, _) => Err("client_uri is too long"),
        (false, None) => Err("client_uri must be an https url"),
        (false, Some(_)) => Ok(()),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Registration {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub client_uri: String,
}

/// Checks a public client and fills in the defaults the answer echoes back.
pub fn validate_registration(registration: &mut Registration) -> Result<(), OAuthError> {
    registration.client_name = registration.client_name.trim().to_string();
    if registration.client_name.is_empty() {
        return Err(OAuthError::invalid_metadata("client_name is required"));
    }
    if registration.client_name.chars().count() > MAX_CLIENT_NAME {
        return Err(OAuthError::invalid_metadata("client_name is too long"));
    }
    if registration.redirect_uris.is_empty() {
        return Err(OAuthError::invalid_redirect("redirect_uris is required"));
    }
    if registration.redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(OAuthError::invalid_redirect(format!("at most {MAX_REDIRECT_URIS} redirect_uris are allowed")));
    }
    if let Some(error) = registration.redirect_uris.iter().find_map(|uri| validate_redirect(uri).err()) {
        return Err(OAuthError::invalid_redirect(error));
    }
    if registration.grant_types.is_empty() {
        registration.grant_types = vec!["authorization_code".into(), "refresh_token".into()];
    }
    if let Some(grant) =
        registration.grant_types.iter().find(|grant| !["authorization_code", "refresh_token"].contains(&grant.as_str()))
    {
        return Err(OAuthError::invalid_metadata(format!("grant type {grant:?} is not supported")));
    }
    if !registration.grant_types.iter().any(|grant| grant == "authorization_code") {
        return Err(OAuthError::invalid_metadata("grant_types must include authorization_code"));
    }
    if registration.response_types.is_empty() {
        registration.response_types = vec!["code".into()];
    }
    if registration.response_types.iter().any(|response| response != "code") {
        return Err(OAuthError::invalid_metadata("only the code response type is supported"));
    }
    if !["", "none"].contains(&registration.token_endpoint_auth_method.as_str()) {
        return Err(OAuthError::invalid_metadata(
            "only public clients (token_endpoint_auth_method none) are supported",
        ));
    }
    registration.token_endpoint_auth_method = "none".into();
    validate_client_uri(&registration.client_uri).map_err(OAuthError::invalid_metadata)
}

pub async fn register(State(state): State<AppState>, body: Bytes) -> Result<Response, OAuthError> {
    let mut registration: Registration =
        serde_json::from_slice(&body).map_err(|_| OAuthError::invalid_metadata("the body is not a JSON object"))?;
    validate_registration(&mut registration)?;
    let client_id = Uuid::new_v4().to_string();
    let created: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO oauth_clients (client_id, name, client_uri, redirect_uris) VALUES ($1, $2, $3, $4)
         RETURNING created_at",
    )
    .bind(&client_id)
    .bind(&registration.client_name)
    .bind(&registration.client_uri)
    .bind(&registration.redirect_uris)
    .fetch_one(&state.db)
    .await?;
    let mut answer = json!({
        "client_id": client_id,
        "client_id_issued_at": created.timestamp(),
        "client_name": registration.client_name,
        "redirect_uris": registration.redirect_uris,
        "grant_types": registration.grant_types,
        "response_types": registration.response_types,
        "token_endpoint_auth_method": registration.token_endpoint_auth_method,
    });
    if !registration.client_uri.is_empty() {
        answer["client_uri"] = json!(registration.client_uri);
    }
    Ok((StatusCode::CREATED, no_store(), Json(answer)).into_response())
}

/// A client_id that is a document's URL rather than an id we made: https, a host and a path.
pub fn is_document_id(client_id: &str) -> bool {
    if client_id.len() > MAX_URI || !client_id.starts_with("https://") {
        return false;
    }
    let Ok(url) = Url::parse(client_id) else { return false };
    let clean = url.host_str().is_some() && url.username().is_empty() && url.password().is_none();
    let dots = client_id.split(['/', '?']).any(|segment| segment == "." || segment == "..");
    clean && url.fragment().is_none() && url.path() != "/" && !dots
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Document {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub client_uri: String,
    pub token_endpoint_auth_method: String,
    /// How a client that would rather authenticate (ChatGPT) says it works as a public one too.
    pub token_endpoint_auth_methods_supported: Vec<String>,
}

pub fn validate_document(client_id: &str, document: Document) -> Result<Client, OAuthError> {
    if document.client_id != client_id {
        return Err(OAuthError::invalid_client("the client's document names another client_id"));
    }
    let name: String = match document.client_name.trim() {
        "" => host_of(client_id),
        name => name.chars().take(MAX_CLIENT_NAME).collect(),
    };
    if document.redirect_uris.is_empty() {
        return Err(OAuthError::invalid_client("the client's document has no redirect_uris"));
    }
    if let Some(error) = document.redirect_uris.iter().find_map(|uri| validate_redirect(uri).err()) {
        return Err(OAuthError::invalid_client(format!("the client's document redirect_uris: {error}")));
    }
    let public = ["", "none"].contains(&document.token_endpoint_auth_method.as_str())
        || document.token_endpoint_auth_methods_supported.iter().any(|method| method == "none");
    if !public {
        return Err(OAuthError::invalid_client("only public clients (token_endpoint_auth_method none) are supported"));
    }
    validate_client_uri(&document.client_uri).map_err(OAuthError::invalid_client)?;
    Ok(Client {
        client_id: client_id.to_string(),
        name,
        kind: ClientKind::Document,
        redirect_uris: document.redirect_uris,
    })
}

/// The document a client publishes. Its URL comes from whoever starts a sign-in, so it is only
/// fetched from public addresses, without redirects.
async fn fetch_document(client_id: &str) -> Result<Client, OAuthError> {
    let unreachable = || OAuthError::invalid_client("the client's metadata document could not be fetched");
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .dns_resolver(unsubscribe::PublicOnly)
        .timeout(DOCUMENT_TIMEOUT)
        .user_agent("wonnet-oauth")
        .build()
        .map_err(|_| OAuthError::server_error())?;
    let url = Url::parse(client_id).map_err(|_| unreachable())?;
    let literal = url.host_str().unwrap_or_default().trim_matches(['[', ']']).parse::<std::net::IpAddr>();
    if literal.is_ok_and(|ip| !unsubscribe::public(ip)) {
        return Err(unreachable());
    }
    let mut response = http.get(url).header("accept", "application/json").send().await.map_err(|_| unreachable())?;
    if response.status() != StatusCode::OK {
        return Err(OAuthError::invalid_client(format!(
            "the client's metadata document answered {}",
            response.status()
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
        body.extend_from_slice(&chunk);
        if body.len() > DOCUMENT_LIMIT {
            return Err(OAuthError::invalid_client("the client's metadata document is too large"));
        }
    }
    let document: Document = serde_json::from_slice(&body)
        .map_err(|_| OAuthError::invalid_client("the client's metadata document is not valid JSON"))?;
    validate_document(client_id, document)
}

#[derive(sqlx::FromRow)]
struct ClientRow {
    client_id: String,
    name: String,
    redirect_uris: Vec<String>,
}

pub async fn resolve_client(db: &PgPool, client_id: &str) -> Result<Client, OAuthError> {
    if client_id.is_empty() {
        return Err(OAuthError::invalid_request("client_id is required"));
    }
    if is_document_id(client_id) {
        return fetch_document(client_id).await;
    }
    let row: Option<ClientRow> = sqlx::query_as(
        "UPDATE oauth_clients SET last_used_at = now() WHERE client_id = $1
         RETURNING client_id, name, redirect_uris",
    )
    .bind(client_id)
    .fetch_optional(db)
    .await?;
    let row = row.ok_or_else(|| OAuthError::invalid_client("unknown client"))?;
    Ok(Client {
        client_id: row.client_id,
        name: row.name,
        kind: ClientKind::Registered,
        redirect_uris: row.redirect_uris,
    })
}

// ---------- authorization ----------

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub resource: String,
    pub scope: String,
}

/// Why an authorization request can't go on: told on the page when the client or its redirect
/// can't be trusted, otherwise sent back to the client.
#[derive(Debug)]
pub enum Refusal {
    Here(String),
    Back(String),
}

impl From<sqlx::Error> for Refusal {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!("oauth: {error}");
        Refusal::Here("Something went wrong. Try again in a moment.".into())
    }
}

/// Checks an authorization request. An absent resource is this server's MCP endpoint.
pub async fn validate(state: &AppState, request: &mut AuthorizeRequest) -> Result<Client, Refusal> {
    let client =
        resolve_client(&state.db, &request.client_id).await.map_err(|error| Refusal::Here(error.description))?;
    if request.redirect_uri.is_empty() {
        return Err(Refusal::Here("The application didn't say where to go back to.".into()));
    }
    if !client.allows(&request.redirect_uri) {
        return Err(Refusal::Here("The application asked to go back to an address it didn't register.".into()));
    }
    let resource = resource(&state.config);
    if request.resource.is_empty() {
        request.resource = resource.clone();
    }
    let back = |code, description: &str| Err(Refusal::Back(error_redirect(state, request, code, description)));
    if request.response_type != "code" {
        return back("unsupported_response_type", "only response_type=code is supported");
    }
    if request.code_challenge_method != "S256" {
        return back("invalid_request", "code_challenge_method must be S256");
    }
    if !valid_pkce(&request.code_challenge) {
        return back("invalid_request", "code_challenge is required and must be 43 to 128 characters");
    }
    if request.state.len() > MAX_STATE {
        return back("invalid_request", "state is too long");
    }
    if request.scope.len() > MAX_SCOPE {
        return back("invalid_request", "scope is too long");
    }
    if request.resource != resource {
        return back("invalid_target", &format!("resource must be {resource}"));
    }
    Ok(client)
}

pub fn error_redirect(state: &AppState, request: &AuthorizeRequest, code: &str, description: &str) -> String {
    let mut parameters = vec![("error", code), ("error_description", description)];
    if !request.state.is_empty() {
        parameters.push(("state", &request.state));
    }
    redirect_with(&request.redirect_uri, &parameters, &issuer(&state.config))
}

/// The redirect URI with `parameters` and `iss` added to the query it registered.
fn redirect_with(redirect_uri: &str, parameters: &[(&str, &str)], issuer: &str) -> String {
    let Ok(mut url) = Url::parse(redirect_uri) else { return redirect_uri.to_string() };
    url.query_pairs_mut().extend_pairs(parameters).append_pair("iss", issuer);
    url.to_string()
}

/// Records the consent as a short-lived code and returns where the browser goes with it. The
/// request is checked again, so a form can't approve what the authorize step refused.
pub async fn approve(
    state: &AppState,
    user_id: Uuid,
    request: &mut AuthorizeRequest,
    access: &Access,
) -> Result<String, Refusal> {
    let client = validate(state, request).await?;
    let code = random_token();
    sqlx::query(
        "INSERT INTO oauth_codes (code_hash, user_id, client_id, client_name, client_kind, redirect_uri, code_challenge,
             access, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(sha256_hex(code.as_bytes()))
    .bind(user_id)
    .bind(&client.client_id)
    .bind(&client.name)
    .bind(client.kind.as_str())
    .bind(&request.redirect_uri)
    .bind(&request.code_challenge)
    .bind(JsonColumn(access))
    .bind(Utc::now() + CODE_LIFETIME)
    .execute(&state.db)
    .await?;
    let mut parameters = vec![("code", code.as_str())];
    if !request.state.is_empty() {
        parameters.push(("state", &request.state));
    }
    Ok(redirect_with(&request.redirect_uri, &parameters, &issuer(&state.config)))
}

/// RFC 7636's unreserved alphabet at 43 to 128 characters: a verifier or an S256 challenge.
pub fn valid_pkce(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value.chars().all(|character| character.is_ascii_alphanumeric() || "-._~".contains(character))
}

pub fn verify_s256(verifier: &str, challenge: &str) -> bool {
    valid_pkce(verifier) && URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) == challenge
}

// ---------- tokens ----------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: String,
    pub code: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub refresh_token: String,
    pub resource: String,
}

pub async fn token(State(state): State<AppState>, Form(request): Form<TokenRequest>) -> Result<Response, OAuthError> {
    let tokens = match request.grant_type.as_str() {
        "authorization_code" => exchange_code(&state, &request).await?,
        "refresh_token" => refresh(&state, &request).await?,
        _ => {
            return Err(OAuthError::new(
                "unsupported_grant_type",
                "only authorization_code and refresh_token are supported",
            ));
        }
    };
    Ok((no_store(), Json(tokens)).into_response())
}

#[derive(sqlx::FromRow)]
struct CodeRow {
    user_id: Uuid,
    client_id: String,
    client_name: String,
    client_kind: String,
    redirect_uri: String,
    code_challenge: String,
    access: JsonColumn<Access>,
    grant_id: Option<Uuid>,
    used_at: Option<DateTime<Utc>>,
    expires_at: DateTime<Utc>,
}

/// Turns a code into a grant and its first tokens. A code presented twice revokes the grant it
/// made (OAuth 2.1 §4.1.2).
async fn exchange_code(state: &AppState, request: &TokenRequest) -> Result<Value, OAuthError> {
    let given = [&request.client_id, &request.code, &request.redirect_uri, &request.code_verifier];
    if given.iter().any(|value| value.is_empty()) {
        return Err(OAuthError::invalid_request("client_id, code, redirect_uri and code_verifier are required"));
    }
    let mut tx = state.db.begin().await?;
    let code: Option<CodeRow> = sqlx::query_as(
        "SELECT user_id, client_id, client_name, client_kind, redirect_uri, code_challenge, access, grant_id, used_at,
             expires_at
         FROM oauth_codes WHERE code_hash = $1 FOR UPDATE",
    )
    .bind(sha256_hex(request.code.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let code = code.ok_or_else(|| OAuthError::invalid_grant("unknown authorization code"))?;
    if code.used_at.is_some() {
        if let Some(grant_id) = code.grant_id {
            revoke_grant(&mut tx, grant_id).await?;
        }
        tx.commit().await?;
        return Err(OAuthError::invalid_grant("authorization code was already used"));
    }
    if code.expires_at <= Utc::now() {
        return Err(OAuthError::invalid_grant("authorization code has expired"));
    }
    if code.client_id != request.client_id {
        return Err(OAuthError::invalid_grant("authorization code was issued to another client"));
    }
    if code.redirect_uri != request.redirect_uri {
        return Err(OAuthError::invalid_grant("redirect_uri does not match the authorization request"));
    }
    if !verify_s256(&request.code_verifier, &code.code_challenge) {
        return Err(OAuthError::invalid_grant("code_verifier does not match"));
    }
    let resource = resource(&state.config);
    if !request.resource.is_empty() && request.resource != resource {
        return Err(OAuthError::new("invalid_target", format!("resource must be {resource}")));
    }
    let grant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO oauth_grants (id, user_id, client_id, client_name, client_kind, redirect_uri, access)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(grant_id)
    .bind(code.user_id)
    .bind(&code.client_id)
    .bind(&code.client_name)
    .bind(&code.client_kind)
    .bind(&code.redirect_uri)
    .bind(&code.access)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE oauth_codes SET used_at = now(), grant_id = $2 WHERE code_hash = $1")
        .bind(sha256_hex(request.code.as_bytes()))
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
    let tokens = mint(&mut tx, grant_id).await?;
    tx.commit().await?;
    Ok(tokens)
}

#[derive(sqlx::FromRow)]
struct RefreshRow {
    grant_id: Uuid,
    kind: String,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    client_id: String,
    revoked_at: Option<DateTime<Utc>>,
}

/// Swaps a refresh token for new tokens. One presented again after that is stolen or replayed,
/// so the whole grant is revoked.
async fn refresh(state: &AppState, request: &TokenRequest) -> Result<Value, OAuthError> {
    if request.client_id.is_empty() || request.refresh_token.is_empty() {
        return Err(OAuthError::invalid_request("client_id and refresh_token are required"));
    }
    let mut tx = state.db.begin().await?;
    let token: Option<RefreshRow> = sqlx::query_as(
        "SELECT token.grant_id, token.kind, token.expires_at, token.used_at, grant_.client_id, grant_.revoked_at
         FROM oauth_tokens token JOIN oauth_grants grant_ ON grant_.id = token.grant_id
         WHERE token.token_hash = $1 FOR UPDATE OF token",
    )
    .bind(sha256_hex(request.refresh_token.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let token = token.filter(|token| token.kind == "refresh");
    let token = token.ok_or_else(|| OAuthError::invalid_grant("unknown refresh token"))?;
    if token.used_at.is_some() {
        revoke_grant(&mut tx, token.grant_id).await?;
        tx.commit().await?;
        return Err(OAuthError::invalid_grant("refresh token was already used"));
    }
    if token.revoked_at.is_some() {
        return Err(OAuthError::invalid_grant("access was revoked"));
    }
    if token.expires_at <= Utc::now() {
        return Err(OAuthError::invalid_grant("refresh token has expired"));
    }
    if token.client_id != request.client_id {
        return Err(OAuthError::invalid_grant("refresh token was issued to another client"));
    }
    sqlx::query("UPDATE oauth_tokens SET used_at = now() WHERE token_hash = $1")
        .bind(sha256_hex(request.refresh_token.as_bytes()))
        .execute(&mut *tx)
        .await?;
    let tokens = mint(&mut tx, token.grant_id).await?;
    tx.commit().await?;
    Ok(tokens)
}

async fn mint(tx: &mut sqlx::PgConnection, grant_id: Uuid) -> sqlx::Result<Value> {
    let access = format!("{ACCESS_PREFIX}{}", random_token());
    let refresh = format!("{REFRESH_PREFIX}{}", random_token());
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO oauth_tokens (token_hash, grant_id, kind, expires_at)
         VALUES ($1, $3, 'access', $4), ($2, $3, 'refresh', $5)",
    )
    .bind(sha256_hex(access.as_bytes()))
    .bind(sha256_hex(refresh.as_bytes()))
    .bind(grant_id)
    .bind(now + ACCESS_LIFETIME)
    .bind(now + REFRESH_LIFETIME)
    .execute(&mut *tx)
    .await?;
    Ok(json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": ACCESS_LIFETIME.num_seconds(),
        "refresh_token": refresh,
    }))
}

async fn revoke_grant(tx: &mut sqlx::PgConnection, grant_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE oauth_grants SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM oauth_tokens WHERE grant_id = $1").bind(grant_id).execute(&mut *tx).await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct CallerRow {
    user_id: Uuid,
    grant_id: Uuid,
    client_name: String,
    access: JsonColumn<Access>,
}

/// Who an access token is for, while it lasts and its grant stands.
pub async fn caller(db: &PgPool, token: &str) -> sqlx::Result<Option<Caller>> {
    if !token.starts_with(ACCESS_PREFIX) {
        return Ok(None);
    }
    let row: Option<CallerRow> = sqlx::query_as(
        "SELECT grant_.user_id, grant_.id AS grant_id, grant_.client_name, grant_.access
         FROM oauth_tokens token JOIN oauth_grants grant_ ON grant_.id = token.grant_id
         WHERE token.token_hash = $1 AND token.kind = 'access' AND token.expires_at > now()
             AND grant_.revoked_at IS NULL",
    )
    .bind(sha256_hex(token.as_bytes()))
    .fetch_optional(db)
    .await?;
    let Some(row) = row else { return Ok(None) };
    sqlx::query(
        "UPDATE oauth_grants SET last_used_at = now()
         WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')",
    )
    .bind(row.grant_id)
    .execute(db)
    .await?;
    Ok(Some(Caller {
        user_id: row.user_id,
        grant_id: row.grant_id,
        client_name: row.client_name,
        access: row.access.0,
    }))
}

// ---------- connected apps ----------

#[derive(sqlx::FromRow)]
pub struct GrantRow {
    pub id: Uuid,
    pub client_id: String,
    pub client_name: String,
    pub client_kind: String,
    pub redirect_uri: String,
    pub access: JsonColumn<Access>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl GrantRow {
    pub fn client(&self) -> Client {
        Client {
            client_id: self.client_id.clone(),
            name: self.client_name.clone(),
            kind: ClientKind::parse(&self.client_kind),
            redirect_uris: vec![self.redirect_uri.clone()],
        }
    }
}

pub async fn grants(db: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<GrantRow>> {
    sqlx::query_as(
        "SELECT id, client_id, client_name, client_kind, redirect_uri, access, created_at, last_used_at
         FROM oauth_grants WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// Ends a connected app. Only its user can; to anyone else it isn't there.
pub async fn revoke(db: &PgPool, user_id: Uuid, grant_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let owned: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM oauth_grants WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL")
            .bind(grant_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if owned.is_none() {
        return Ok(false);
    }
    revoke_grant(&mut tx, grant_id).await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn cleanup(db: &PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM oauth_codes WHERE expires_at < now() - interval '1 day'").execute(db).await?;
    sqlx::query("DELETE FROM oauth_tokens WHERE expires_at < now()").execute(db).await?;
    sqlx::query(
        "DELETE FROM oauth_clients WHERE last_used_at < now() - interval '30 days'
             AND NOT EXISTS (SELECT 1 FROM oauth_grants WHERE oauth_grants.client_id = oauth_clients.client_id
                 AND oauth_grants.revoked_at IS NULL)",
    )
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_go_to_https_or_this_device() {
        for good in [
            "https://claude.ai/api/mcp/auth_callback",
            "http://localhost:3118/callback",
            "http://127.0.0.1/cb",
            "cursor://anysphere.cursor-mcp/oauth/callback",
        ] {
            assert_eq!(validate_redirect(good), Ok(()), "{good}");
        }
        for bad in [
            "http://example.com/cb",
            "javascript://alert(1)",
            "https://x.com/cb#frag",
            "https://user:pw@x.com/cb",
            "/cb",
            "",
        ] {
            assert!(validate_redirect(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn loopback_redirects_match_any_port_and_nothing_else_does() {
        assert!(redirect_matches("http://localhost/callback", "http://localhost:51234/callback"));
        assert!(redirect_matches("http://127.0.0.1:1/cb", "http://127.0.0.1:2/cb"));
        assert!(!redirect_matches("http://localhost/callback", "http://127.0.0.1:51234/callback"));
        assert!(!redirect_matches("http://localhost/callback", "http://localhost:5/other"));
        assert!(!redirect_matches("https://a.com/cb", "https://a.com:8443/cb"));
    }

    #[test]
    fn registration_fills_in_a_public_client() {
        let registration = || Registration {
            client_name: " Claude Code ".into(),
            redirect_uris: vec!["http://localhost/cb".into()],
            ..Default::default()
        };
        let mut public = registration();
        validate_registration(&mut public).unwrap();
        assert_eq!(public.client_name, "Claude Code");
        assert_eq!(public.grant_types, ["authorization_code", "refresh_token"]);
        assert_eq!(public.token_endpoint_auth_method, "none");

        let mut secret = Registration { token_endpoint_auth_method: "client_secret_basic".into(), ..registration() };
        assert_eq!(validate_registration(&mut secret).unwrap_err().code, "invalid_client_metadata");
        let mut nowhere = Registration { redirect_uris: vec!["http://example.com/cb".into()], ..registration() };
        assert_eq!(validate_registration(&mut nowhere).unwrap_err().code, "invalid_redirect_uri");
    }

    #[test]
    fn pkce_takes_s256_only() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert!(verify_s256(verifier, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
        assert!(!verify_s256(verifier, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cX"));
        assert!(!verify_s256("short", "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
    }

    #[test]
    fn documents_are_vouched_for_only_on_their_own_host() {
        let document = |redirects: &[&str]| Document {
            client_id: "https://claude.ai/oauth/claude-code-client-metadata".into(),
            client_name: "Claude".into(),
            redirect_uris: redirects.iter().map(|uri| uri.to_string()).collect(),
            ..Default::default()
        };
        let id = "https://claude.ai/oauth/claude-code-client-metadata";
        let own = validate_document(id, document(&["https://claude.ai/api/mcp/auth_callback", "http://localhost/cb"]))
            .unwrap();
        assert_eq!(own.verified(), Some("Claude"));
        let elsewhere = validate_document(id, document(&["https://evil.example/cb"])).unwrap();
        assert_eq!(elsewhere.verified(), None);
        assert!(validate_document("https://claude.ai/other", document(&["http://localhost/cb"])).is_err());
    }

    #[test]
    fn document_ids_are_https_urls_with_a_path() {
        assert!(is_document_id("https://claude.ai/oauth/metadata"));
        assert!(!is_document_id("https://claude.ai/"));
        assert!(!is_document_id("http://claude.ai/oauth"));
        assert!(!is_document_id("https://claude.ai/a/../b"));
        assert!(!is_document_id("6f1c0c2e-6c8e-4f19-9a7d-1a6c1f3b2d10"));
    }
}
