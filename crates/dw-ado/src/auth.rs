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
const KEYRING_CHUNK_PREFIX: &str = "dw-refresh-token-v1";
const KEYRING_CHUNK_UTF16_UNITS: usize = 1_000;

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
    let previous_manifest = read_keyring_password(KEYRING_USER)?
        .as_deref()
        .and_then(parse_chunk_manifest);
    let chunks = split_keyring_chunks(refresh_token);

    if chunks.len() == 1 {
        keyring_entry()?
            .set_password(refresh_token)
            .map_err(|error| AdoAuthError::Keyring(error.to_string()))?;
    } else {
        let generation = format!("{:016x}", rand::random::<u64>());
        for (index, chunk) in chunks.iter().enumerate() {
            keyring_entry_for(&chunk_user(&generation, index))?
                .set_password(chunk)
                .map_err(|error| AdoAuthError::Keyring(error.to_string()))?;
        }
        keyring_entry()?
            .set_password(&chunk_manifest(&generation, chunks.len()))
            .map_err(|error| AdoAuthError::Keyring(error.to_string()))?;
    }

    if let Some((generation, count)) = previous_manifest {
        delete_keyring_chunks(&generation, count)?;
    }
    Ok(())
}

fn read_refresh_token() -> Result<Option<String>, AdoAuthError> {
    let Some(stored) = read_keyring_password(KEYRING_USER)? else {
        return Ok(None);
    };
    let Some((generation, count)) = parse_chunk_manifest(&stored) else {
        return Ok(Some(stored));
    };

    let mut refresh_token = String::new();
    for index in 0..count {
        let user = chunk_user(&generation, index);
        let chunk = read_keyring_password(&user)?.ok_or_else(|| {
            AdoAuthError::Keyring(format!("Stored refresh token chunk {index} is missing."))
        })?;
        refresh_token.push_str(&chunk);
    }
    Ok(Some(refresh_token))
}

fn read_keyring_password(user: &str) -> Result<Option<String>, AdoAuthError> {
    match keyring_entry_for(user)?.get_password() {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if is_missing_keyring_entry(&error) => Ok(None),
        Err(error) => Err(AdoAuthError::Keyring(error.to_string())),
    }
}

fn delete_stored_refresh_token() -> Result<bool, AdoAuthError> {
    let manifest = read_keyring_password(KEYRING_USER)?
        .as_deref()
        .and_then(parse_chunk_manifest);
    let deleted = delete_keyring_credential(KEYRING_USER)?;
    if let Some((generation, count)) = manifest {
        delete_keyring_chunks(&generation, count)?;
    }
    Ok(deleted)
}

fn split_keyring_chunks(value: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut utf16_units = 0;
    for character in value.chars() {
        let character_units = character.len_utf16();
        if utf16_units + character_units > KEYRING_CHUNK_UTF16_UNITS && !chunk.is_empty() {
            chunks.push(std::mem::take(&mut chunk));
            utf16_units = 0;
        }
        chunk.push(character);
        utf16_units += character_units;
    }
    chunks.push(chunk);
    chunks
}

fn chunk_manifest(generation: &str, count: usize) -> String {
    format!("{KEYRING_CHUNK_PREFIX}:{generation}:{count}")
}

fn parse_chunk_manifest(value: &str) -> Option<(String, usize)> {
    let mut parts = value.split(':');
    if parts.next()? != KEYRING_CHUNK_PREFIX {
        return None;
    }
    let generation = parts.next()?;
    let count = parts.next()?.parse().ok()?;
    if generation.is_empty() || count < 2 || parts.next().is_some() {
        return None;
    }
    Some((generation.to_string(), count))
}

fn chunk_user(generation: &str, index: usize) -> String {
    format!("{KEYRING_USER}.{generation}.{index}")
}

fn delete_keyring_chunks(generation: &str, count: usize) -> Result<(), AdoAuthError> {
    for index in 0..count {
        delete_keyring_credential(&chunk_user(generation, index))?;
    }
    Ok(())
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
    fn keyring_chunks_respect_utf16_limit_and_roundtrip() {
        let value = format!("{}😀{}", "a".repeat(999), "b".repeat(1_200));

        let chunks = split_keyring_chunks(&value);

        assert_eq!(chunks.concat(), value);
        assert!(chunks.len() >= 3);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.encode_utf16().count() <= KEYRING_CHUNK_UTF16_UNITS)
        );
    }

    #[test]
    fn keyring_chunk_manifest_roundtrips() {
        let manifest = chunk_manifest("0123456789abcdef", 3);

        assert_eq!(
            parse_chunk_manifest(&manifest),
            Some(("0123456789abcdef".into(), 3))
        );
        assert_eq!(parse_chunk_manifest("plain-refresh-token"), None);
        assert_eq!(
            parse_chunk_manifest("dw-refresh-token-v1:generation:1"),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "writes to the real Windows Credential Manager; run explicitly when validating auth login storage"]
    fn windows_auth_refresh_token_roundtrips_in_keyring() {
        let original = read_refresh_token().expect("existing refresh token should be readable");
        let token = test_refresh_token(512);

        let result = (|| {
            store_refresh_token(&token)?;
            assert_eq!(read_refresh_token()?, Some(token));
            Ok::<_, AdoAuthError>(())
        })();

        let _ = delete_stored_refresh_token();
        if let Some(original) = original {
            let _ = store_refresh_token(&original);
        }

        result.expect("refresh token should roundtrip through Windows Credential Manager");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "writes to the real Windows Credential Manager; run explicitly when validating auth login storage"]
    fn windows_auth_refresh_token_chunks_values_above_platform_limit() {
        let original = read_refresh_token().expect("existing refresh token should be readable");
        let token = test_refresh_token(2048);

        let result = (|| {
            store_refresh_token(&token)?;
            assert_eq!(read_refresh_token()?, Some(token));
            Ok::<_, AdoAuthError>(())
        })();

        let _ = delete_stored_refresh_token();
        if let Some(original) = original {
            let _ = store_refresh_token(&original);
        }

        result.expect("long refresh token should roundtrip through chunked credentials");
    }

    #[cfg(windows)]
    fn test_refresh_token(length: usize) -> String {
        (0..length)
            .map(|index| char::from(b'a' + (index % 26) as u8))
            .collect()
    }
}
