use base64::Engine;
use rand::RngCore;
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Response, Server};
use url::Url;

use crate::auth::{
    AdoAuthError, AdoAuthOptions, DEFAULT_ADO_SCOPE, DEFAULT_PUBLIC_CLIENT_ID, DEFAULT_TENANT_ID,
};

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthTokenResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_in: u32,
}

pub(crate) fn login(auth: &AdoAuthOptions) -> Result<OAuthTokenResponse, AdoAuthError> {
    let port = reserve_loopback_port()?;
    let redirect_uri = format!("http://localhost:{port}");
    let state = random_url_token(32);
    let verifier = random_url_token(64);
    let challenge = pkce_challenge(&verifier);
    let scopes = interactive_scopes(auth);
    let tenant = auth.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID);
    let client_id = auth
        .client_id
        .as_deref()
        .unwrap_or(DEFAULT_PUBLIC_CLIENT_ID);
    let auth_url = authorization_url(
        tenant,
        client_id,
        &redirect_uri,
        &scopes,
        &state,
        &challenge,
    )?;

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = run_callback_server(port, &state, sender);
    });

    webbrowser::open(auth_url.as_str())
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    let callback = receiver
        .recv_timeout(Duration::from_secs(180))
        .map_err(|_| AdoAuthError::LoginExpired)??;

    crate::auth::block_on(exchange_authorization_code(
        tenant,
        client_id,
        &redirect_uri,
        &scopes,
        &callback.code,
        &verifier,
    ))?
}

#[derive(Debug)]
struct BrowserCallback {
    code: String,
}

fn reserve_loopback_port() -> Result<u16, AdoAuthError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

fn run_callback_server(
    port: u16,
    expected_state: &str,
    sender: mpsc::Sender<Result<BrowserCallback, AdoAuthError>>,
) -> Result<(), AdoAuthError> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    let request = server
        .recv_timeout(Duration::from_secs(180))
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?
        .ok_or(AdoAuthError::LoginExpired)?;
    let result = parse_callback_url(request.url(), expected_state);
    let html = match &result {
        Ok(_) => success_page(),
        Err(error) => error_page(&error.to_string()),
    };
    let _ = request.respond(html_response(html));
    let _ = sender.send(result);
    Ok(())
}

fn parse_callback_url(url: &str, expected_state: &str) -> Result<BrowserCallback, AdoAuthError> {
    let parsed = Url::parse(&format!("http://localhost{url}"))
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    let query = parsed.query_pairs().collect::<HashMap<_, _>>();
    if let Some(error) = query.get("error") {
        let description = query
            .get("error_description")
            .map(|value| value.to_string())
            .unwrap_or_else(|| error.to_string());
        return Err(AdoAuthError::BrowserLogin(description));
    }
    let state = query
        .get("state")
        .ok_or_else(|| AdoAuthError::BrowserLogin("Callback OAuth sans state.".into()))?;
    if state.as_ref() != expected_state {
        return Err(AdoAuthError::BrowserLogin(
            "Callback OAuth state invalide.".into(),
        ));
    }
    let code = query
        .get("code")
        .ok_or_else(|| AdoAuthError::BrowserLogin("Callback OAuth sans code.".into()))?;
    Ok(BrowserCallback {
        code: code.to_string(),
    })
}

fn authorization_url(
    tenant: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
    state: &str,
    code_challenge: &str,
) -> Result<Url, AdoAuthError> {
    let mut url = Url::parse(&format!(
        "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
    ))
    .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

async fn exchange_authorization_code(
    tenant: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
    code: &str,
    code_verifier: &str,
) -> Result<OAuthTokenResponse, AdoAuthError> {
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let scope = scopes.join(" ");
    let response = reqwest::Client::new()
        .post(url)
        .form(&[
            ("client_id", client_id),
            ("scope", scope.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;

    if response.status() != StatusCode::OK {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Token endpoint error".into());
        return Err(AdoAuthError::BrowserLogin(message));
    }

    let body = response
        .text()
        .await
        .map_err(|error| AdoAuthError::BrowserLogin(error.to_string()))?;
    serde_json::from_str::<OAuthTokenResponse>(&body).map_err(|error| {
        AdoAuthError::BrowserLogin(format!(
            "Réponse token OAuth invalide: {error}. Body: {body}"
        ))
    })
}

fn interactive_scopes(auth: &AdoAuthOptions) -> Vec<String> {
    let mut values = if auth.scopes.is_empty() {
        vec![DEFAULT_ADO_SCOPE.into()]
    } else {
        auth.scopes.clone()
    };
    if !values.iter().any(|scope| scope == "offline_access") {
        values.push("offline_access".into());
    }
    values
}

fn random_url_token(bytes: usize) -> String {
    let mut data = vec![0_u8; bytes];
    rand::rng().fill_bytes(&mut data);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn html_response(html: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let response = Response::from_string(html);
    match Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]) {
        Ok(header) => response.with_header(header),
        Err(_) => response,
    }
}

fn success_page() -> String {
    r#"<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>DevWorkflow connecté</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #fff; color: #111; }
    main { width: min(90vw, 520px); padding: 36px; border: 1px solid #111; background: #fff; }
    .top { display: flex; justify-content: space-between; gap: 24px; padding-bottom: 14px; border-bottom: 1px solid #111; font-size: .78rem; letter-spacing: .1em; text-transform: uppercase; }
    h1 { margin: 42px 0 10px; font-size: clamp(2.4rem, 8vw, 4.7rem); line-height: .92; letter-spacing: -.06em; font-weight: 650; }
    p { margin: 0; color: #333; line-height: 1.6; }
  </style>
</head>
<body>
  <main>
    <div class="top"><span>DevWorkflow</span><span>Connecté</span></div>
    <h1>Success.</h1>
    <p>Vous pouvez fermer cet onglet.</p>
  </main>
</body>
</html>"#
        .into()
}

fn error_page(message: &str) -> String {
    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DevWorkflow - erreur</title><style>body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#fff;color:#111;font-family:system-ui,sans-serif}}main{{width:min(90vw,520px);padding:36px;border:1px solid #111;background:#fff}}.top{{display:flex;justify-content:space-between;border-bottom:1px solid #111;padding-bottom:14px;text-transform:uppercase;letter-spacing:.1em;font-size:.78rem}}h1{{font-size:clamp(2.4rem,8vw,4.7rem);line-height:.92;letter-spacing:-.06em;margin:42px 0 10px;font-weight:650}}p{{color:#333;line-height:1.6}}</style></head><body><main><div class="top"><span>DevWorkflow</span><span>Erreur</span></div><h1>Failed.</h1><p>{}</p></main></body></html>"#,
        html_escape(message)
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
