//! Microsoft's OAuth with PKCE, through the `common` endpoint so work, school and personal
//! accounts can all sign in. The server is the OAuth client: it keeps the refresh token, sealed,
//! and reads Outlook mail through Microsoft Graph with it.

use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::config::Config;
use crate::google::pkce_challenge;

pub const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
pub const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
pub const SCOPES: &str = "openid email offline_access User.Read Mail.ReadWrite Mail.Send MailboxSettings.ReadWrite";
const MAIL_SCOPE: &str = "mail.readwrite";

pub struct Grant {
    pub email: String,
    pub refresh_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    scope: String,
}

fn redirect_uri(config: &Config) -> String {
    format!("{}/auth/microsoft/callback", config.public_url)
}

pub fn authorize_url(config: &Config, state: &str, nonce: &str, verifier: &str) -> String {
    let mut url = Url::parse(&config.microsoft_authorize_url).expect("a valid authorize URL");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("response_mode", "query")
        .append_pair("client_id", &config.microsoft_client_id)
        .append_pair("redirect_uri", &redirect_uri(config))
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", &pkce_challenge(verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    url.into()
}

/// Graph answers scopes either bare or behind its own URL.
fn granted(scopes: &str, wanted: &str) -> bool {
    scopes.split(' ').any(|scope| scope.rsplit('/').next().is_some_and(|scope| scope.eq_ignore_ascii_case(wanted)))
}

pub async fn exchange(http: &reqwest::Client, config: &Config, code: &str, verifier: &str) -> Result<Grant, String> {
    let redirect_uri = redirect_uri(config);
    let response = http
        .post(&config.microsoft_token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &config.microsoft_client_id),
            ("client_secret", &config.microsoft_client_secret),
            ("code_verifier", verifier),
            ("scope", SCOPES),
        ])
        .send()
        .await
        .map_err(|err| format!("token request failed: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("token endpoint answered {status}: {body}"));
    }
    let tokens: TokenResponse = response.json().await.map_err(|err| format!("bad token response: {err}"))?;
    if !granted(&tokens.scope, MAIL_SCOPE) {
        return Err("mail access wasn't granted".into());
    }
    let Some(refresh_token) = tokens.refresh_token else {
        return Err("Microsoft sent no refresh token".into());
    };
    let email = address(http, config, &tokens.access_token).await?;
    Ok(Grant { email, refresh_token })
}

/// The mailbox's address, from Graph with the token just given: personal accounts have no `mail`,
/// only their sign-in name.
async fn address(http: &reqwest::Client, config: &Config, access_token: &str) -> Result<String, String> {
    let response = http
        .get(format!("{}/me", config.graph_api_url))
        .query(&[("$select", "mail,userPrincipalName")])
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| format!("Graph request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("Graph answered /me with {}", response.status()));
    }
    let me: Value = response.json().await.map_err(|err| format!("bad /me response: {err}"))?;
    let email = [&me["mail"], &me["userPrincipalName"]]
        .into_iter()
        .filter_map(Value::as_str)
        .find(|email| email.contains('@'))
        .ok_or("Microsoft gave no address for this account")?;
    Ok(email.to_lowercase())
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_scopes_bare_or_behind_graphs_url() {
        assert!(super::granted("openid https://graph.microsoft.com/Mail.ReadWrite", "mail.readwrite"));
        assert!(super::granted("Mail.ReadWrite Mail.Send", "mail.readwrite"));
        assert!(!super::granted("Mail.Read Mail.Send", "mail.readwrite"));
    }
}
