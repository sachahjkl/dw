use crate::auth_browser;
use chrono::{DateTime, Utc};
use keyring::Entry;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{Duration, Instant, sleep};

pub const DEFAULT_TENANT_ID: &str = "organizations";
pub const DEFAULT_PUBLIC_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
pub const ADO_RESOURCE_ID: &str = "499b84ac-1321-427f-aa17-267ca6975798";
pub const DEFAULT_ADO_SCOPE: &str = "499b84ac-1321-427f-aa17-267ca6975798/.default";

const KEYRING_SERVICE: &str = "dw.azure-devops";
const KEYRING_USER: &str = "oauth-refresh-token";
const KEYRING_CHUNK_PREFIX: &str = "oauth-refresh-token.";
const KEYRING_SPLIT_PREFIX: &str = "dw-split:v1:";
const KEYRING_REFRESH_TOKEN_CHUNK_BYTES: usize = 1800;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAuthOptions {
    #[serde(default, rename = "tenantId")]
    pub tenant_id: Option<String>,
    #[serde(default, rename = "clientId")]
    pub client_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdoAuthScheme {
    Bearer,
    Basic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoToken {
    pub access_token: String,
    pub source: String,
    pub scheme: AdoAuthScheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAuthStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceLoginInstructions {
    pub verification_uri: String,
    pub user_code: String,
    pub expires_in_seconds: u32,
    pub poll_interval_seconds: u32,
}

#[derive(Debug, Error)]
pub enum AdoAuthError {
    #[error("ADO auth is not configured. Add auth to workflow.json or set DW_ADO_TOKEN.")]
    MissingConfig,
    #[error("ADO token unavailable. Run the auth login action or set DW_ADO_TOKEN.")]
    MissingToken,
    #[error("Azure DevOps OAuth failed: {0}")]
    OAuth(String),
    #[error("OS credential storage unavailable: {0}")]
    Keyring(String),
    #[error("ADO login expired before browser validation.")]
    LoginExpired,
    #[error("Browser login failed: {0}")]
    BrowserLogin(String),
}

pub fn environment_token() -> Option<AdoToken> {
    std::env::var("DW_ADO_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("AZURE_DEVOPS_EXT_PAT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .map(|access_token| AdoToken {
            access_token,
            source: "environment PAT".into(),
            scheme: AdoAuthScheme::Basic,
            expires_on: None,
        })
}

pub async fn login_browser_interactive(
    auth: Option<AdoAuthOptions>,
) -> Result<AdoToken, AdoAuthError> {
    let auth = auth.ok_or(AdoAuthError::MissingConfig)?;
    let token = auth_browser::login(&auth).await?;
    let refresh_token = token.refresh_token.as_deref().ok_or_else(|| {
        AdoAuthError::BrowserLogin("Microsoft did not return a refresh_token.".into())
    })?;
    store_refresh_token(refresh_token)?;
    Ok(oauth_token_result(token, "browser"))
}

pub async fn login_device_code(
    auth: Option<AdoAuthOptions>,
    mut on_instructions: impl FnMut(DeviceLoginInstructions),
) -> Result<AdoToken, AdoAuthError> {
    let auth = auth.ok_or(AdoAuthError::MissingConfig)?;
    let scopes = scopes(&auth);
    let flow = initiate_device_flow(&auth, &scopes).await?;
    open_browser(&flow.verification_uri);
    on_instructions(DeviceLoginInstructions {
        verification_uri: flow.verification_uri.clone(),
        user_code: flow.user_code.clone(),
        expires_in_seconds: flow.expires_in,
        poll_interval_seconds: flow.interval.unwrap_or(5).max(1),
    });

    let token = acquire_device_token_polling(&auth, flow).await?;
    if let Some(refresh_token) = token.refresh_token.as_deref() {
        store_refresh_token(refresh_token)?;
    }
    Ok(oauth_token_result(token, "device code"))
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u32,
    interval: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    error_description: Option<String>,
}

async fn initiate_device_flow(
    auth: &AdoAuthOptions,
    scopes: &[String],
) -> Result<DeviceAuthorizationResponse, AdoAuthError> {
    let tenant = auth.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID);
    let client_id = auth
        .client_id
        .as_deref()
        .unwrap_or(DEFAULT_PUBLIC_CLIENT_ID);
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode");
    let scope = scopes.join(" ");
    post_oauth_form(&url, &[("client_id", client_id), ("scope", scope.as_str())]).await
}

async fn acquire_device_token_polling(
    auth: &AdoAuthOptions,
    flow: DeviceAuthorizationResponse,
) -> Result<auth_browser::OAuthTokenResponse, AdoAuthError> {
    let mut interval = Duration::from_secs(flow.interval.unwrap_or(5).max(1).into());
    let deadline = Instant::now() + Duration::from_secs(flow.expires_in.into());
    let tenant = auth.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID);
    let client_id = auth
        .client_id
        .as_deref()
        .unwrap_or(DEFAULT_PUBLIC_CLIENT_ID);
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");

    loop {
        match post_oauth_form::<auth_browser::OAuthTokenResponse>(
            &url,
            &[
                ("client_id", client_id),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", flow.device_code.as_str()),
            ],
        )
        .await
        {
            Ok(token) => return Ok(token),
            Err(AdoAuthError::OAuth(error)) if is_pending_device_auth(&error) => {
                if Instant::now() + interval >= deadline {
                    return Err(AdoAuthError::LoginExpired);
                }
                sleep(interval).await;
            }
            Err(AdoAuthError::OAuth(error)) if is_slow_down_device_auth(&error) => {
                interval += Duration::from_secs(5);
                if Instant::now() + interval >= deadline {
                    return Err(AdoAuthError::LoginExpired);
                }
                sleep(interval).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_pending_device_auth(error: &str) -> bool {
    error.contains("authorization_pending") || error.contains("AADSTS70016")
}

fn is_slow_down_device_auth(error: &str) -> bool {
    error.contains("slow_down")
}

fn open_browser(url: &str) {
    if webbrowser::open(url).is_err() {
        // The explicit URL and code are printed below; browser opening is a UX improvement only.
    }
}

pub async fn token_silent_or_environment(
    auth: Option<AdoAuthOptions>,
) -> Result<Option<AdoToken>, AdoAuthError> {
    if let Some(token) = environment_token() {
        return Ok(Some(token));
    }

    let auth = match auth {
        Some(auth) => auth,
        None => return Ok(None),
    };
    let refresh_token = match read_refresh_token()? {
        Some(token) => token,
        None => return Ok(None),
    };

    let scopes = scopes(&auth);
    let token = refresh_access_token(&auth, &scopes, &refresh_token).await?;
    if let Some(refresh_token) = token.refresh_token.as_deref() {
        store_refresh_token(refresh_token)?;
    }
    Ok(Some(oauth_token_result(token, "keyring")))
}

pub async fn require_token(auth: Option<AdoAuthOptions>) -> Result<AdoToken, AdoAuthError> {
    token_silent_or_environment(auth)
        .await?
        .ok_or(AdoAuthError::MissingToken)
}

pub async fn status(auth: Option<AdoAuthOptions>) -> Result<AdoAuthStatus, AdoAuthError> {
    Ok(match token_silent_or_environment(auth).await? {
        Some(token) => AdoAuthStatus {
            connected: true,
            source: Some(token.source),
            expires_on: token.expires_on,
        },
        None => AdoAuthStatus {
            connected: false,
            source: None,
            expires_on: None,
        },
    })
}

pub fn logout() -> Result<bool, AdoAuthError> {
    delete_stored_refresh_token()
}

async fn refresh_access_token(
    auth: &AdoAuthOptions,
    scopes: &[String],
    refresh_token: &str,
) -> Result<auth_browser::OAuthTokenResponse, AdoAuthError> {
    let tenant = auth.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID);
    let client_id = auth
        .client_id
        .as_deref()
        .unwrap_or(DEFAULT_PUBLIC_CLIENT_ID);
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let scope = scopes.join(" ");
    post_oauth_form(
        &url,
        &[
            ("client_id", client_id),
            ("scope", scope.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
    )
    .await
}

fn scopes(auth: &AdoAuthOptions) -> Vec<String> {
    if auth.scopes.is_empty() {
        vec![DEFAULT_ADO_SCOPE.into()]
    } else {
        auth.scopes.clone()
    }
}

fn oauth_token_result(token: auth_browser::OAuthTokenResponse, source: &str) -> AdoToken {
    let expires_on = Utc::now() + chrono::Duration::seconds(token.expires_in.into());
    AdoToken {
        access_token: token.access_token,
        source: source.into(),
        scheme: AdoAuthScheme::Bearer,
        expires_on: Some(format_rfc3339(expires_on)),
    }
}

fn format_rfc3339(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

async fn post_oauth_form<T: for<'de> Deserialize<'de>>(
    url: &str,
    form: &[(&str, &str)],
) -> Result<T, AdoAuthError> {
    let response = reqwest::Client::new()
        .post(url)
        .form(form)
        .send()
        .await
        .map_err(|error| AdoAuthError::OAuth(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| AdoAuthError::OAuth(error.to_string()))?;

    if status != StatusCode::OK {
        return Err(AdoAuthError::OAuth(oauth_error_message(&body)));
    }

    serde_json::from_str::<T>(&body).map_err(|error| {
        AdoAuthError::OAuth(format!("Invalid OAuth response: {error}. Body: {body}"))
    })
}

fn oauth_error_message(body: &str) -> String {
    serde_json::from_str::<OAuthErrorResponse>(body)
        .map(|error| {
            error
                .error_description
                .map(|description| format!("{}: {description}", error.error))
                .unwrap_or(error.error)
        })
        .unwrap_or_else(|_| body.to_string())
}

fn store_refresh_token(refresh_token: &str) -> Result<(), AdoAuthError> {
    let chunks = split_refresh_token(refresh_token);
    delete_stored_refresh_token()?;
    for (index, chunk) in chunks.iter().enumerate() {
        keyring_entry_for(&chunk_key(index))?
            .set_password(chunk)
            .map_err(|error| AdoAuthError::Keyring(error.to_string()))?;
    }
    keyring_entry()?
        .set_password(&format!("{KEYRING_SPLIT_PREFIX}{}", chunks.len()))
        .map_err(|error| AdoAuthError::Keyring(error.to_string()))
}

fn read_refresh_token() -> Result<Option<String>, AdoAuthError> {
    let Some(value) = read_keyring_password(KEYRING_USER)? else {
        return Ok(None);
    };
    if let Some(count) = split_refresh_token_count(&value)? {
        return read_split_refresh_token(count);
    }
    Ok(Some(value))
}

fn split_refresh_token(refresh_token: &str) -> Vec<String> {
    if refresh_token.is_empty() {
        return vec![String::new()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    for (index, _) in refresh_token.char_indices() {
        if index - start >= KEYRING_REFRESH_TOKEN_CHUNK_BYTES {
            chunks.push(refresh_token[start..index].to_string());
            start = index;
        }
    }
    chunks.push(refresh_token[start..].to_string());
    chunks
}

fn split_refresh_token_count(value: &str) -> Result<Option<usize>, AdoAuthError> {
    let Some(count) = value.strip_prefix(KEYRING_SPLIT_PREFIX) else {
        return Ok(None);
    };
    count.parse::<usize>().map(Some).map_err(|error| {
        AdoAuthError::Keyring(format!("invalid split refresh token marker: {error}"))
    })
}

fn read_split_refresh_token(count: usize) -> Result<Option<String>, AdoAuthError> {
    if count == 0 {
        return Ok(None);
    }
    let mut token = String::new();
    for index in 0..count {
        let Some(chunk) = read_keyring_password(&chunk_key(index))? else {
            return Ok(None);
        };
        token.push_str(&chunk);
    }
    Ok(Some(token))
}

fn delete_stored_refresh_token() -> Result<bool, AdoAuthError> {
    let marker = read_keyring_password(KEYRING_USER)?;
    let mut deleted = delete_keyring_credential(KEYRING_USER)?;
    if let Some(value) = marker
        && let Some(count) = split_refresh_token_count(&value)?
    {
        for index in 0..count {
            deleted |= delete_keyring_credential(&chunk_key(index))?;
        }
    }
    Ok(deleted)
}

fn chunk_key(index: usize) -> String {
    format!("{KEYRING_CHUNK_PREFIX}{index}")
}

#[cfg(test)]
fn join_refresh_token_chunks(chunks: &[String]) -> String {
    chunks.concat()
}

fn read_keyring_password(user: &str) -> Result<Option<String>, AdoAuthError> {
    match keyring_entry_for(user)?.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if is_missing_keyring_entry(&error) => Ok(None),
        Err(error) => Err(AdoAuthError::Keyring(error.to_string())),
    }
}

fn keyring_entry() -> Result<Entry, AdoAuthError> {
    keyring_entry_for(KEYRING_USER)
}

fn keyring_entry_for(user: &str) -> Result<Entry, AdoAuthError> {
    Entry::new(KEYRING_SERVICE, user).map_err(|error| AdoAuthError::Keyring(error.to_string()))
}

fn delete_keyring_credential(user: &str) -> Result<bool, AdoAuthError> {
    match keyring_entry_for(user)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(error) if is_missing_keyring_entry(&error) => Ok(false),
        Err(error) => Err(AdoAuthError::Keyring(error.to_string())),
    }
}

fn is_missing_keyring_entry(error: &keyring::Error) -> bool {
    matches!(error, keyring::Error::NoEntry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scope_is_ado_default() {
        assert_eq!(
            scopes(&AdoAuthOptions {
                tenant_id: None,
                client_id: None,
                scopes: vec![]
            }),
            vec![DEFAULT_ADO_SCOPE]
        );
    }

    #[test]
    fn explicit_scopes_are_preserved() {
        let auth = AdoAuthOptions {
            tenant_id: None,
            client_id: None,
            scopes: vec!["scope-a".into(), "scope-b".into()],
        };
        assert_eq!(scopes(&auth), vec!["scope-a", "scope-b"]);
    }

    #[test]
    fn pending_device_auth_detects_error_name_or_code() {
        assert!(is_pending_device_auth("authorization_pending"));
        assert!(is_pending_device_auth(
            "AADSTS70016: Authorization is pending"
        ));
        assert!(!is_pending_device_auth("authorization_declined"));
    }

    #[test]
    fn refresh_token_storage_chunks_long_values_below_windows_limit() {
        let token = "a".repeat(KEYRING_REFRESH_TOKEN_CHUNK_BYTES * 2 + 17);
        let chunks = split_refresh_token(&token);

        assert_eq!(chunks.len(), 3);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= KEYRING_REFRESH_TOKEN_CHUNK_BYTES)
        );
        assert_eq!(join_refresh_token_chunks(&chunks), token);
    }

    #[test]
    fn refresh_token_storage_chunks_on_char_boundaries() {
        let token = "é".repeat(KEYRING_REFRESH_TOKEN_CHUNK_BYTES);
        let chunks = split_refresh_token(&token);

        assert_eq!(join_refresh_token_chunks(&chunks), token);
    }

    #[test]
    fn split_refresh_token_marker_parses_chunk_count() {
        assert_eq!(
            split_refresh_token_count("dw-split:v1:3").expect("marker should parse"),
            Some(3)
        );
        assert_eq!(
            split_refresh_token_count("plain-refresh-token").expect("plain token should pass"),
            None
        );
    }
}
