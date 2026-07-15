use anyhow::Result;
use dw_ado::auth::{
    AdoAuthStatus, AdoToken, DeviceLoginInstructions, login_browser_interactive, login_device_code,
    logout, status,
};
use dw_core::{AdoActionEvent, AdoDeviceUserCode, AdoDeviceVerificationUri, DevWorkflowRoot};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::load_auth_options;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthLoginMode {
    Browser,
    DeviceCode,
    EnvironmentPat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLoginChoice {
    pub label: &'static str,
    pub mode: AuthLoginMode,
}

impl std::fmt::Display for AuthLoginChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthLoginReport {
    pub mode: AuthLoginMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AuthTokenSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_on: Option<AuthTokenExpiration>,
    pub uses_environment_pat: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthStatusReport {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AuthTokenSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_on: Option<AuthTokenExpiration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthLogoutReport {
    pub removed_local_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct AuthTokenSource(String);

impl AuthTokenSource {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AuthTokenSource {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for AuthTokenSource {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for AuthTokenSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct AuthTokenExpiration(String);

impl AuthTokenExpiration {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AuthTokenExpiration {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for AuthTokenExpiration {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for AuthTokenExpiration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub fn auth_login_choices() -> Vec<AuthLoginChoice> {
    vec![
        AuthLoginChoice {
            label: "Automatic browser (recommended)",
            mode: AuthLoginMode::Browser,
        },
        AuthLoginChoice {
            label: "Manual device code",
            mode: AuthLoginMode::DeviceCode,
        },
        AuthLoginChoice {
            label: "PAT from environment variable",
            mode: AuthLoginMode::EnvironmentPat,
        },
    ]
}

pub async fn login_report(
    root: Option<DevWorkflowRoot>,
    mode: AuthLoginMode,
) -> Result<AuthLoginReport> {
    login_report_with_events(root, mode, |_| {}).await
}

pub async fn login_report_with_events(
    root: Option<DevWorkflowRoot>,
    mode: AuthLoginMode,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<AuthLoginReport> {
    let auth = load_auth_options(root.as_ref().map(DevWorkflowRoot::as_str))?;
    match mode {
        AuthLoginMode::Browser => Ok(token_login_report(
            mode,
            login_browser_interactive(auth).await?,
        )),
        AuthLoginMode::DeviceCode => Ok(token_login_report(
            mode,
            login_device_code(auth, |instructions| {
                emit(device_login_required_event(instructions))
            })
            .await?,
        )),
        AuthLoginMode::EnvironmentPat => Ok(AuthLoginReport {
            mode,
            source: None,
            expires_on: None,
            uses_environment_pat: true,
        }),
    }
}

fn device_login_required_event(instructions: DeviceLoginInstructions) -> AdoActionEvent {
    AdoActionEvent::DeviceLoginRequired {
        verification_uri: AdoDeviceVerificationUri::from(instructions.verification_uri),
        user_code: AdoDeviceUserCode::from(instructions.user_code),
        expires_in_seconds: instructions.expires_in_seconds,
        poll_interval_seconds: instructions.poll_interval_seconds,
    }
}

pub async fn status_report(root: Option<DevWorkflowRoot>) -> Result<AuthStatusReport> {
    let auth = load_auth_options(root.as_ref().map(DevWorkflowRoot::as_str))?;
    Ok(status(auth).await?.into())
}

pub fn logout_report(root: Option<DevWorkflowRoot>) -> Result<AuthLogoutReport> {
    let _ = load_auth_options(root.as_ref().map(DevWorkflowRoot::as_str))?;
    Ok(AuthLogoutReport {
        removed_local_session: logout()?,
    })
}

fn token_login_report(mode: AuthLoginMode, token: AdoToken) -> AuthLoginReport {
    AuthLoginReport {
        mode,
        source: Some(AuthTokenSource::from(token.source)),
        expires_on: token.expires_on.map(AuthTokenExpiration::from),
        uses_environment_pat: false,
    }
}

impl From<AdoAuthStatus> for AuthStatusReport {
    fn from(status: AdoAuthStatus) -> Self {
        Self {
            connected: status.connected,
            source: status.source.map(AuthTokenSource::from),
            expires_on: status.expires_on.map(AuthTokenExpiration::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_ado::auth::AdoAuthScheme;

    #[test]
    fn auth_login_choices_keep_browser_first() {
        let choices = auth_login_choices();

        assert_eq!(choices[0].label, "Automatic browser (recommended)");
        assert!(matches!(choices[0].mode, AuthLoginMode::Browser));
    }

    #[test]
    fn login_report_omits_secret_token_value() {
        let report = token_login_report(
            AuthLoginMode::Browser,
            AdoToken {
                access_token: "secret".into(),
                source: "keyring".into(),
                scheme: AdoAuthScheme::Bearer,
                expires_on: Some("2026-07-03T12:14:07Z".into()),
            },
        );

        assert_eq!(
            report.source.as_ref().map(AuthTokenSource::as_str),
            Some("keyring")
        );
        assert_eq!(
            report.expires_on.as_ref().map(AuthTokenExpiration::as_str),
            Some("2026-07-03T12:14:07Z")
        );
        assert!(!serde_json::to_string(&report).unwrap().contains("secret"));
    }

    #[test]
    fn auth_status_report_keeps_disconnected_shape() {
        let report = AuthStatusReport::from(AdoAuthStatus {
            connected: false,
            source: None,
            expires_on: None,
        });

        assert!(!report.connected);
        assert!(report.source.is_none());
    }
}
