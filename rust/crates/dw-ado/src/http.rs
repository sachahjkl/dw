use crate::AdoError;
use crate::auth::{AdoAuthScheme, AdoToken};
use serde_json::Value;

pub(crate) fn get_json(url: &str, token: &str) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header("Accept", "application/json")
        .header("Authorization", basic_auth_header(token))
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn get_json_authenticated(url: &str, token: &AdoToken) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header("Accept", "application/json")
        .header("Authorization", auth_header(token))
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn get_json_authenticated_optional_404(
    url: &str,
    token: &AdoToken,
) -> Result<Option<Value>, AdoError> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header("Accept", "application/json")
        .header("Authorization", auth_header(token))
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    read_json_response(response).map(Some)
}

pub(crate) fn post_json(url: &str, token: &str, body: &Value) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("Accept", "application/json")
        .header("Authorization", basic_auth_header(token))
        .json(body)
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn post_json_authenticated(
    url: &str,
    token: &AdoToken,
    body: &Value,
) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("Accept", "application/json")
        .header("Authorization", auth_header(token))
        .json(body)
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn patch_json_with_content_type(
    url: &str,
    token: &str,
    body: &Value,
    content_type: &str,
) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .patch(url)
        .header("Accept", "application/json")
        .header("Authorization", basic_auth_header(token))
        .header("Content-Type", content_type)
        .body(body.to_string())
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn patch_json_authenticated_with_content_type(
    url: &str,
    token: &AdoToken,
    body: &Value,
    content_type: &str,
) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .patch(url)
        .header("Accept", "application/json")
        .header("Authorization", auth_header(token))
        .header("Content-Type", content_type)
        .body(body.to_string())
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

pub(crate) fn post_json_with_content_type(
    url: &str,
    token: &str,
    body: &Value,
    content_type: &str,
) -> Result<Value, AdoError> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("Accept", "application/json")
        .header("Authorization", basic_auth_header(token))
        .header("Content-Type", content_type)
        .body(body.to_string())
        .send()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    read_json_response(response)
}

fn read_json_response(response: reqwest::blocking::Response) -> Result<Value, AdoError> {
    let status = response.status().as_u16();
    let body = response
        .text()
        .map_err(|error| AdoError::Request(error.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(AdoError::Http { status, body });
    }
    serde_json::from_str(&body).map_err(|error| AdoError::Json(error.to_string()))
}

fn basic_auth_header(token: &str) -> String {
    use base64::Engine;
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!(":{token}"))
    )
}

fn auth_header(token: &AdoToken) -> String {
    match token.scheme {
        AdoAuthScheme::Basic => basic_auth_header(&token.access_token),
        AdoAuthScheme::Bearer => format!("Bearer {}", token.access_token),
    }
}
