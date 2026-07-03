use crate::auth_browser;
use chrono::{DateTime, Utc};
use keyring::Entry;
use msal::PublicClientApplication;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{Duration, Instant, sleep};

pub const DEFAULT_TENANT_ID: &str = "organizations";
pub const DEFAULT_PUBLIC_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
pub const ADO_RESOURCE_ID: &str = "499b84ac-1321-427f-aa17-267ca6975798";
pub const DEFAULT_ADO_SCOPE: &str = "499b84ac-1321-427f-aa17-267ca6975798/.default";

const KEYRING_SERVICE: &str = "dw.azure-devops";
const KEYRING_USER: &str = "msal-refresh-token";

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

#[derive(Debug, Error)]
pub enum AdoAuthError {
    #[error("Auth ADO non configuree. Renseigner auth dans workflow.json ou definir DW_ADO_TOKEN.")]
    MissingConfig,
    #[error("Token ADO indisponible. Executer dw auth login ou definir DW_ADO_TOKEN.")]
    MissingToken,
    #[error("MSAL a echoue: {0}")]
    Msal(String),
    #[error("Stockage credentials OS indisponible: {0}")]
    Keyring(String),
    #[error("Runtime async indisponible: {0}")]
    Runtime(String),
    #[error("Connexion ADO expiree avant validation dans le navigateur.")]
    LoginExpired,
    #[error("Login navigateur impossible: {0}")]
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

pub fn login_browser_interactive(auth: Option<AdoAuthOptions>) -> Result<AdoToken, AdoAuthError> {
    let auth = auth.ok_or(AdoAuthError::MissingConfig)?;
    let token = auth_browser::login(&auth)?;
    let refresh_token = token.refresh_token.as_deref().ok_or_else(|| {
        AdoAuthError::BrowserLogin("Microsoft n'a pas renvoye de refresh_token.".into())
    })?;
    store_refresh_token(refresh_token)?;
    Ok(oauth_token_result(token, "MSAL interactive"))
}

pub fn login_device_code(auth: Option<AdoAuthOptions>) -> Result<AdoToken, AdoAuthError> {
    let auth = auth.ok_or(AdoAuthError::MissingConfig)?;
    block_on(async move {
        let app = public_client(&auth)?;
        let scopes = scopes(&auth);
        let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
        let flow = app
            .initiate_device_flow(scope_refs.clone())
            .await
            .map_err(|error| AdoAuthError::Msal(error.to_string()))?;
        open_browser(&flow.verification_uri);
        if let Some(message) = &flow.message {
            println!("{message}");
        } else {
            println!(
                "Ouvrir {} et entrer le code {}.",
                flow.verification_uri, flow.user_code
            );
        }

        let token = acquire_device_token_polling(&app, flow).await?;
        store_refresh_token(&token.refresh_token)?;
        Ok(token_result(token, "MSAL device code"))
    })?
}

async fn acquire_device_token_polling(
    app: &PublicClientApplication,
    flow: msal::DeviceAuthorizationResponse,
) -> Result<msal::UserToken, AdoAuthError> {
    let mut interval = Duration::from_secs(flow.interval.unwrap_or(5).max(1).into());
    let deadline = Instant::now() + Duration::from_secs(flow.expires_in.into());

    loop {
        match app.acquire_token_by_device_flow(flow.clone()).await {
            Ok(token) => return Ok(token),
            Err(error) if is_pending_device_auth(&error.to_string()) => {
                if Instant::now() + interval >= deadline {
                    return Err(AdoAuthError::LoginExpired);
                }
                sleep(interval).await;
            }
            Err(error) if is_slow_down_device_auth(&error.to_string()) => {
                interval += Duration::from_secs(5);
                if Instant::now() + interval >= deadline {
                    return Err(AdoAuthError::LoginExpired);
                }
                sleep(interval).await;
            }
            Err(error) => return Err(AdoAuthError::Msal(error.to_string())),
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

pub fn token_silent_or_environment(
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

    block_on(async move {
        let app = public_client(&auth)?;
        let scopes = scopes(&auth);
        let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
        let token = app
            .acquire_token_by_refresh_token(&refresh_token, scope_refs)
            .await
            .map_err(|error| AdoAuthError::Msal(error.to_string()))?;
        store_refresh_token(&token.refresh_token)?;
        Ok(Some(token_result(token, "MSAL keyring")))
    })?
}

pub fn require_token(auth: Option<AdoAuthOptions>) -> Result<AdoToken, AdoAuthError> {
    token_silent_or_environment(auth)?.ok_or(AdoAuthError::MissingToken)
}

pub fn status(auth: Option<AdoAuthOptions>) -> Result<AdoAuthStatus, AdoAuthError> {
    Ok(match token_silent_or_environment(auth)? {
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
    let entry = keyring_entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(error) if is_missing_keyring_entry(&error) => Ok(false),
        Err(error) => Err(AdoAuthError::Keyring(error.to_string())),
    }
}

fn public_client(auth: &AdoAuthOptions) -> Result<PublicClientApplication, AdoAuthError> {
    let tenant = auth.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID);
    let client_id = auth
        .client_id
        .as_deref()
        .unwrap_or(DEFAULT_PUBLIC_CLIENT_ID);
    let authority = format!("https://login.microsoftonline.com/{tenant}");
    PublicClientApplication::new(client_id, Some(&authority))
        .map_err(|error| AdoAuthError::Msal(error.to_string()))
}

fn scopes(auth: &AdoAuthOptions) -> Vec<String> {
    if auth.scopes.is_empty() {
        vec![DEFAULT_ADO_SCOPE.into()]
    } else {
        auth.scopes.clone()
    }
}

fn token_result(token: msal::UserToken, source: &str) -> AdoToken {
    let expires_on = Utc::now() + chrono::Duration::seconds(token.expires_in.into());
    AdoToken {
        access_token: token.access_token.clone().unwrap_or_default(),
        source: source.into(),
        scheme: AdoAuthScheme::Bearer,
        expires_on: Some(format_rfc3339(expires_on)),
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

fn store_refresh_token(refresh_token: &str) -> Result<(), AdoAuthError> {
    keyring_entry()?
        .set_password(refresh_token)
        .map_err(|error| AdoAuthError::Keyring(error.to_string()))
}

fn read_refresh_token() -> Result<Option<String>, AdoAuthError> {
    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if is_missing_keyring_entry(&error) => Ok(None),
        Err(error) => Err(AdoAuthError::Keyring(error.to_string())),
    }
}

fn keyring_entry() -> Result<Entry, AdoAuthError> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|error| AdoAuthError::Keyring(error.to_string()))
}

fn is_missing_keyring_entry(error: &keyring::Error) -> bool {
    matches!(error, keyring::Error::NoEntry)
}

pub(crate) fn block_on<T>(future: impl std::future::Future<Output = T>) -> Result<T, AdoAuthError> {
    tokio::runtime::Runtime::new()
        .map_err(|error| AdoAuthError::Runtime(error.to_string()))?
        .block_on(async { Ok(future.await) })
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
}
