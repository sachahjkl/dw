mod ai_context;
pub mod auth;
mod auth_browser;
mod changelog;
mod http;
mod json;
mod state;
mod urls;
mod wiql;

use crate::auth::{AdoAuthScheme, AdoToken};
use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
use dw_contracts::{AdoAiContextComment, AdoAiContextItem};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

pub use ai_context::map_ai_context_item;
pub use changelog::{
    ChangelogFormat, RELATION_HIERARCHY_FORWARD, RELATION_HIERARCHY_REVERSE, WorkItemGroup,
    extract_work_item_ids_from_commit_messages, get_work_item_ids_from_pull_requests,
    group_work_items_by_parent, load_changelog_items, parse_changelog_format, parse_id_set,
    render_flat_changelog, render_grouped_changelog,
};
use http::{
    get_json, get_json_authenticated, get_json_authenticated_optional_404,
    patch_json_authenticated_with_content_type, patch_json_with_content_type, post_json,
    post_json_authenticated, post_json_authenticated_with_content_type,
    post_json_with_content_type,
};
use json::{clean_text, element_text, field_text, identity_text, work_item_id_from_relation_url};
pub use state::is_final_state;
use urls::organization_name;
pub use urls::{
    active_pull_requests_url, create_work_item_url, expanded_work_item_url,
    pull_request_work_items_url, pull_requests_url, work_item_api_url, work_item_comments_url,
    work_item_url, work_item_web_url, work_items_batch_url,
};

pub const DEFAULT_API_VERSION: &str = "7.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AzureDevOpsOptions {
    #[serde(alias = "organizationUrl")]
    pub organization: String,
    #[serde(default)]
    pub project: String,
    #[serde(default = "default_api_version", rename = "apiVersion")]
    pub api_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    EnvironmentPat,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    pub source: AuthSource,
    pub variable_name: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum AdoError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("Azure DevOps auth indisponible. Définir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT.")]
    MissingAuth,
    #[error("Azure DevOps HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Azure DevOps requête échouée: {0}")]
    Request(String),
    #[error("Azure DevOps réponse JSON invalide: {0}")]
    Json(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemSnapshot {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub state: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceChildTaskCreateResult {
    pub repository: String,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestSummary {
    #[serde(rename = "pullRequestId")]
    pub pull_request_id: i64,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestCreateResult {
    #[serde(rename = "pullRequestId")]
    pub pull_request_id: Option<i64>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreatePullRequestInput {
    pub repository: String,
    #[serde(rename = "sourceRefName")]
    pub source_ref_name: String,
    #[serde(rename = "targetRefName")]
    pub target_ref_name: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "isDraft")]
    pub is_draft: bool,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JsonPatchOperation {
    op: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
}

pub fn default_api_version() -> String {
    DEFAULT_API_VERSION.into()
}

pub fn detect_env_auth() -> AuthStatus {
    if std::env::var("DW_ADO_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
    {
        return AuthStatus {
            source: AuthSource::EnvironmentPat,
            variable_name: Some("DW_ADO_TOKEN"),
        };
    }

    if std::env::var("AZURE_DEVOPS_EXT_PAT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
    {
        return AuthStatus {
            source: AuthSource::EnvironmentPat,
            variable_name: Some("AZURE_DEVOPS_EXT_PAT"),
        };
    }

    AuthStatus {
        source: AuthSource::Missing,
        variable_name: None,
    }
}

pub fn env_pat() -> Result<String, AdoError> {
    std::env::var("DW_ADO_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("AZURE_DEVOPS_EXT_PAT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or(AdoError::MissingAuth)
}

pub fn get_work_item_expanded(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    token: &AdoToken,
) -> Result<Value, AdoError> {
    get_json_authenticated(&expanded_work_item_url(options, work_item_id), token)
}

pub fn get_work_item_comments(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    limit: i32,
    token: &AdoToken,
) -> Result<Vec<AdoAiContextComment>, AdoError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let root = get_json_authenticated(
        &work_item_comments_url(options, work_item_id, limit as u32),
        token,
    )?;
    Ok(root
        .get("comments")
        .or_else(|| root.get("value"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|comment| AdoAiContextComment {
            author: identity_text(comment.get("createdBy").or_else(|| comment.get("author"))),
            created_date: field_text(&comment, "createdDate"),
            text: clean_text(
                field_text(&comment, "renderedText").or_else(|| field_text(&comment, "text")),
            ),
        })
        .collect())
}

pub fn get_ai_context(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    summary_only: bool,
    comment_limit: i32,
    token: &AdoToken,
) -> Result<AdoAiContextItem, AdoError> {
    let expanded = get_work_item_expanded(options, work_item_id, token)?;
    let comments =
        get_work_item_comments(options, work_item_id, comment_limit, token).unwrap_or_default();
    Ok(map_ai_context_item(
        &expanded,
        options,
        summary_only,
        comments,
    ))
}

pub fn get_work_item_snapshot(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    token: &str,
) -> Result<WorkItemSnapshot, AdoError> {
    let root = get_json(&work_item_url(options, work_item_id), token)?;
    Ok(snapshot_from_value(&root))
}

pub fn get_work_item_snapshots(
    options: &AzureDevOpsOptions,
    work_item_ids: &[String],
    token: &str,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    let ids = work_item_ids
        .iter()
        .filter_map(|id| id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let body = serde_json::json!({
        "ids": ids,
        "fields": ["System.Id", "System.WorkItemType", "System.State", "System.Title"]
    });
    let root = post_json(&work_items_batch_url(options), token, &body)?;
    let snapshots = root
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|value| snapshot_from_value(&value))
        .collect::<Vec<_>>();
    Ok(work_item_ids
        .iter()
        .filter_map(|id| snapshots.iter().find(|item| item.id == *id).cloned())
        .collect())
}

pub fn get_work_item_snapshot_authenticated(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    token: &AdoToken,
) -> Result<WorkItemSnapshot, AdoError> {
    let root = get_json_authenticated(&work_item_url(options, work_item_id), token)?;
    Ok(snapshot_from_value(&root))
}

pub fn get_work_item_snapshots_authenticated(
    options: &AzureDevOpsOptions,
    work_item_ids: &[String],
    token: &AdoToken,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    let ids = work_item_ids
        .iter()
        .filter_map(|id| id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let body = serde_json::json!({
        "ids": ids,
        "fields": ["System.Id", "System.WorkItemType", "System.State", "System.Title"]
    });
    let root = post_json_authenticated(&work_items_batch_url(options), token, &body)?;
    let snapshots = root
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|value| snapshot_from_value(&value))
        .collect::<Vec<_>>();
    Ok(work_item_ids
        .iter()
        .filter_map(|id| snapshots.iter().find(|item| item.id == *id).cloned())
        .collect())
}

pub fn get_related_work_item_ids(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    relation: &str,
    token: &AdoToken,
) -> Result<Vec<String>, AdoError> {
    let root = get_work_item_expanded(options, work_item_id, token)?;
    Ok(root
        .get("relations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            item.get("rel")
                .and_then(Value::as_str)
                .is_some_and(|rel| rel.eq_ignore_ascii_case(relation))
        })
        .filter_map(|item| {
            item.get("url")
                .and_then(Value::as_str)
                .and_then(work_item_id_from_relation_url)
        })
        .collect())
}

pub fn try_get_pull_request_work_item_ids(
    options: &AzureDevOpsOptions,
    repository: &str,
    pull_request_id: i64,
    token: &AdoToken,
) -> Result<Option<Vec<String>>, AdoError> {
    let Some(root) = get_json_authenticated_optional_404(
        &pull_request_work_items_url(options, repository, pull_request_id),
        token,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(
        root.get("value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| element_text(item.get("id")))
            .collect(),
    ))
}

pub async fn query_assigned_work_items(
    options: &AzureDevOpsOptions,
    top: usize,
    token: &AdoToken,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    let organization = organization_name(&options.organization);
    let client = azure_devops_rust_api::wit::Client::builder(sdk_credential(token)).build();
    let result = client
        .wiql_client()
        .query_by_wiql(
            &organization,
            wiql::assigned_work_items(),
            &options.project,
            "",
        )
        .top(top as i32)
        .await
        .map_err(|error| AdoError::Request(error.to_string()))?;
    let ids = result
        .work_items
        .into_iter()
        .filter_map(|item| item.id)
        .take(top)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(vec![]);
    }

    query_work_item_snapshots(options, &ids, token).await
}

pub async fn query_work_item_snapshots(
    options: &AzureDevOpsOptions,
    ids: &[i32],
    token: &AdoToken,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let organization = organization_name(&options.organization);
    let client = azure_devops_rust_api::wit::Client::builder(sdk_credential(token)).build();
    let request = azure_devops_rust_api::wit::models::WorkItemBatchGetRequest {
        ids: ids.to_vec(),
        fields: vec![
            "System.Id".into(),
            "System.WorkItemType".into(),
            "System.State".into(),
            "System.Title".into(),
        ],
        ..azure_devops_rust_api::wit::models::WorkItemBatchGetRequest::new()
    };
    let items = client
        .work_items_client()
        .get_work_items_batch(&organization, request, &options.project)
        .await
        .map_err(|error| AdoError::Request(error.to_string()))?;

    Ok(items
        .value
        .into_iter()
        .map(|item| snapshot_from_sdk_work_item(&item))
        .collect())
}

#[derive(Debug)]
struct StaticBearerCredential {
    token: String,
}

#[async_trait::async_trait]
impl TokenCredential for StaticBearerCredential {
    async fn get_token(
        &self,
        _scopes: &[&str],
        _options: Option<TokenRequestOptions<'_>>,
    ) -> azure_core::Result<AccessToken> {
        Ok(AccessToken::new(
            self.token.clone(),
            time::OffsetDateTime::now_utc() + time::Duration::hours(1),
        ))
    }
}

fn sdk_credential(token: &AdoToken) -> azure_devops_rust_api::Credential {
    match token.scheme {
        AdoAuthScheme::Basic => azure_devops_rust_api::Credential::from_pat(&token.access_token),
        AdoAuthScheme::Bearer => azure_devops_rust_api::Credential::from_token_credential(
            Arc::new(StaticBearerCredential {
                token: token.access_token.clone(),
            }),
        ),
    }
}

pub fn create_child_task(
    options: &AzureDevOpsOptions,
    parent: &WorkItemSnapshot,
    repository: &str,
    title: &str,
    source: &str,
    token: &str,
) -> Result<WorkspaceChildTaskCreateResult, AdoError> {
    let body = create_child_task_body(options, parent, repository, title, source)?;
    let root = post_json_with_content_type(
        &create_work_item_url(options, "Task"),
        token,
        &body,
        "application/json-patch+json",
    )?;
    Ok(child_task_result_from_root(root, repository, title))
}

pub fn create_child_task_authenticated(
    options: &AzureDevOpsOptions,
    parent: &WorkItemSnapshot,
    repository: &str,
    title: &str,
    source: &str,
    token: &AdoToken,
) -> Result<WorkspaceChildTaskCreateResult, AdoError> {
    let body = create_child_task_body(options, parent, repository, title, source)?;
    let root = post_json_authenticated_with_content_type(
        &create_work_item_url(options, "Task"),
        token,
        &body,
        "application/json-patch+json",
    )?;
    Ok(child_task_result_from_root(root, repository, title))
}

fn create_child_task_body(
    options: &AzureDevOpsOptions,
    parent: &WorkItemSnapshot,
    repository: &str,
    title: &str,
    source: &str,
) -> Result<Value, AdoError> {
    let trace = format!(
        "Créé automatiquement par Dev Workflow Rust via {source}. Parent #{}. Repository: {repository}.",
        parent.id
    );
    let body = vec![
        patch_add("/fields/System.Title", Value::String(title.into())),
        patch_add("/fields/System.AssignedTo", Value::String("@Me".into())),
        patch_add("/fields/System.History", Value::String(trace)),
        patch_add(
            "/relations/-",
            serde_json::json!({
                "rel": "System.LinkTypes.Hierarchy-Reverse",
                "url": work_item_api_url(options, &parent.id),
                "attributes": { "comment": format!("creation {source}") }
            }),
        ),
    ];
    serde_json::to_value(body).map_err(|error| AdoError::Json(error.to_string()))
}

fn child_task_result_from_root(
    root: Value,
    repository: &str,
    title: &str,
) -> WorkspaceChildTaskCreateResult {
    WorkspaceChildTaskCreateResult {
        repository: repository.into(),
        id: element_text(root.get("id")).unwrap_or_default(),
        title: title.into(),
    }
}

pub fn update_work_item_state(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    state: &str,
    history: &str,
    token: &str,
) -> Result<(), AdoError> {
    let body = vec![
        patch_add("/fields/System.History", Value::String(history.into())),
        patch_add("/fields/System.State", Value::String(state.into())),
    ];
    let _ = patch_json_with_content_type(
        &work_item_url(options, work_item_id),
        token,
        &serde_json::to_value(body).map_err(|error| AdoError::Json(error.to_string()))?,
        "application/json-patch+json",
    )?;
    Ok(())
}

pub fn update_work_item_state_authenticated(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    state: &str,
    history: &str,
    token: &AdoToken,
) -> Result<(), AdoError> {
    let body = vec![
        patch_add("/fields/System.History", Value::String(history.into())),
        patch_add("/fields/System.State", Value::String(state.into())),
    ];
    let _ = patch_json_authenticated_with_content_type(
        &work_item_url(options, work_item_id),
        token,
        &serde_json::to_value(body).map_err(|error| AdoError::Json(error.to_string()))?,
        "application/json-patch+json",
    )?;
    Ok(())
}

pub fn try_find_active_pull_request(
    options: &AzureDevOpsOptions,
    repository: &str,
    source_ref: &str,
    token: &str,
) -> Result<Option<PullRequestSummary>, AdoError> {
    let root = get_json(
        &active_pull_requests_url(options, repository, source_ref),
        token,
    )?;
    let values = root
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(values.into_iter().find_map(|item| {
        let id = item.get("pullRequestId").and_then(Value::as_i64)?;
        let source = item
            .get("sourceRefName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        (source.eq_ignore_ascii_case(source_ref)).then(|| PullRequestSummary {
            pull_request_id: id,
            url: field_text(&item, "url"),
        })
    }))
}

pub fn try_find_active_pull_request_authenticated(
    options: &AzureDevOpsOptions,
    repository: &str,
    source_ref: &str,
    token: &AdoToken,
) -> Result<Option<PullRequestSummary>, AdoError> {
    let root = get_json_authenticated(
        &active_pull_requests_url(options, repository, source_ref),
        token,
    )?;
    pull_request_summary_from_response(root, source_ref)
}

pub fn create_pull_request(
    options: &AzureDevOpsOptions,
    input: &CreatePullRequestInput,
    token: &str,
) -> Result<PullRequestCreateResult, AdoError> {
    let refs = input
        .work_item_ids
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect::<Vec<_>>();
    let root = post_json(
        &pull_requests_url(options, &input.repository),
        token,
        &serde_json::json!({
            "sourceRefName": input.source_ref_name,
            "targetRefName": input.target_ref_name,
            "title": input.title,
            "description": input.description,
            "isDraft": input.is_draft,
            "workItemRefs": refs,
        }),
    )?;
    Ok(PullRequestCreateResult {
        pull_request_id: root.get("pullRequestId").and_then(Value::as_i64),
        url: field_text(&root, "url"),
    })
}

pub fn create_pull_request_authenticated(
    options: &AzureDevOpsOptions,
    input: &CreatePullRequestInput,
    token: &AdoToken,
) -> Result<PullRequestCreateResult, AdoError> {
    let refs = input
        .work_item_ids
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect::<Vec<_>>();
    let root = post_json_authenticated(
        &pull_requests_url(options, &input.repository),
        token,
        &serde_json::json!({
            "sourceRefName": input.source_ref_name,
            "targetRefName": input.target_ref_name,
            "title": input.title,
            "description": input.description,
            "isDraft": input.is_draft,
            "workItemRefs": refs,
        }),
    )?;
    Ok(PullRequestCreateResult {
        pull_request_id: root.get("pullRequestId").and_then(Value::as_i64),
        url: field_text(&root, "url"),
    })
}

pub fn link_work_item_to_pull_request(
    options: &AzureDevOpsOptions,
    repository: &str,
    pull_request_id: i64,
    work_item_id: &str,
    token: &str,
) -> Result<(), AdoError> {
    let _ = patch_json_with_content_type(
        &pull_request_work_items_url(options, repository, pull_request_id),
        token,
        &serde_json::json!([{ "id": work_item_id }]),
        "application/json",
    )?;
    Ok(())
}

pub fn link_work_item_to_pull_request_authenticated(
    options: &AzureDevOpsOptions,
    repository: &str,
    pull_request_id: i64,
    work_item_id: &str,
    token: &AdoToken,
) -> Result<(), AdoError> {
    let _ = patch_json_authenticated_with_content_type(
        &pull_request_work_items_url(options, repository, pull_request_id),
        token,
        &serde_json::json!([{ "id": work_item_id }]),
        "application/json",
    )?;
    Ok(())
}

fn pull_request_summary_from_response(
    root: Value,
    source_ref: &str,
) -> Result<Option<PullRequestSummary>, AdoError> {
    let values = root
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(values.into_iter().find_map(|item| {
        let id = item.get("pullRequestId").and_then(Value::as_i64)?;
        let source = item
            .get("sourceRefName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        (source.eq_ignore_ascii_case(source_ref)).then(|| PullRequestSummary {
            pull_request_id: id,
            url: field_text(&item, "url"),
        })
    }))
}

fn snapshot_from_value(value: &Value) -> WorkItemSnapshot {
    let fields = value.get("fields").cloned().unwrap_or(Value::Null);
    WorkItemSnapshot {
        id: element_text(value.get("id")).unwrap_or_default(),
        kind: field_text(&fields, "System.WorkItemType"),
        state: field_text(&fields, "System.State"),
        title: field_text(&fields, "System.Title"),
        url: field_text(value, "url"),
    }
}

fn snapshot_from_sdk_work_item(
    item: &azure_devops_rust_api::wit::models::WorkItem,
) -> WorkItemSnapshot {
    WorkItemSnapshot {
        id: item.id.to_string(),
        kind: field_text(&item.fields, "System.WorkItemType"),
        state: field_text(&item.fields, "System.State"),
        title: field_text(&item.fields, "System.Title"),
        url: Some(
            item.work_item_tracking_resource
                .work_item_tracking_resource_reference
                .url
                .clone(),
        ),
    }
}

fn patch_add(path: &str, value: Value) -> JsonPatchOperation {
    JsonPatchOperation {
        op: "add".into(),
        path: path.into(),
        value: Some(value),
        from: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_url_matches_dotnet_shape() {
        let options = AzureDevOpsOptions {
            organization: "https://dev.azure.com/org".into(),
            project: "Project X".into(),
            api_version: default_api_version(),
        };

        assert_eq!(
            expanded_work_item_url(&options, "12345"),
            "https://dev.azure.com/org/Project X/_apis/wit/workitems/12345?$expand=all&api-version=7.1"
        );
    }

    #[test]
    fn comments_url_uses_requested_limit() {
        let options = AzureDevOpsOptions {
            organization: "https://dev.azure.com/org/".into(),
            project: "Project X".into(),
            api_version: default_api_version(),
        };

        assert_eq!(
            work_item_comments_url(&options, "12345", 7),
            "https://dev.azure.com/org/Project X/_apis/wit/workItems/12345/comments?$top=7&api-version=7.1"
        );
    }

    #[test]
    fn map_ai_context_item_matches_expected_contract_shape() {
        let root: Value = serde_json::from_str(
            r#"{
  "id": 55201,
  "fields": {
    "System.Title": "Demande transport SOMOTHA",
    "System.WorkItemType": "User Story",
    "System.State": "En realisation",
    "System.AssignedTo": { "displayName": "Alice Martin" },
    "System.AreaPath": "HA\\Transport",
    "System.IterationPath": "HA\\Sprint 42",
    "System.Tags": "transport; somotha",
    "System.Description": "<p>Verifier la maquette</p>",
    "System.CreatedBy": { "displayName": "Bob" },
    "System.CreatedDate": "2026-07-01T10:00:00Z",
    "System.ChangedBy": { "displayName": "Claire" },
    "System.ChangedDate": "2026-07-02T09:00:00Z",
    "Microsoft.VSTS.Common.Priority": 1,
    "Microsoft.VSTS.Common.ValueArea": "Business",
    "Microsoft.VSTS.Common.AcceptanceCriteria": "<div>Respecter le libelle SOMOTHA</div>",
    "Custom.ProductContext": "<div>Ecran existant</div>"
  },
  "relations": [
    { "rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/54000" },
    { "rel": "System.LinkTypes.Hierarchy-Forward", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/55202" },
    { "rel": "System.LinkTypes.Dependency-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/55199" },
    { "rel": "AttachedFile", "url": "https://dev.azure.com/org/_apis/wit/attachments/123", "attributes": { "name": "maquette transport somotha.png", "comment": "<p>Source ecran</p>" } }
  ]
}"#,
        )
        .expect("json should parse");

        let context = map_ai_context_item(
            &root,
            &AzureDevOpsOptions {
                organization: "https://dev.azure.com/org".into(),
                project: "Project".into(),
                api_version: default_api_version(),
            },
            false,
            vec![AdoAiContextComment {
                author: Some("Alice Martin".into()),
                created_date: Some("2026-07-02T08:00:00Z".into()),
                text: Some("Verifier le screenshot".into()),
            }],
        );

        assert_eq!(context.schema_version, dw_contracts::AI_CONTEXT_VERSION);
        assert_eq!(context.work_item.id, "55201");
        assert_eq!(
            context.work_item.title.as_deref(),
            Some("Demande transport SOMOTHA")
        );
        assert_eq!(context.work_item.tags, vec!["transport", "somotha"]);
        assert_eq!(
            context.content.description.as_deref(),
            Some("Verifier la maquette")
        );
        assert_eq!(
            context.content.acceptance_criteria.as_deref(),
            Some("Respecter le libelle SOMOTHA")
        );
        assert_eq!(
            context
                .content
                .product_context
                .get("ProductContext")
                .map(String::as_str),
            Some("Ecran existant")
        );
        assert_eq!(context.links.parent_ids, vec!["54000"]);
        assert_eq!(context.links.child_ids, vec!["55202"]);
        assert_eq!(context.links.predecessor_ids, vec!["55199"]);
        assert_eq!(context.attachments.directory_hint, "attachments/ado/55201");
        assert_eq!(
            context.attachments.items[0].name.as_deref(),
            Some("maquette transport somotha.png")
        );
        assert_eq!(
            context.attachments.items[0].comment.as_deref(),
            Some("Source ecran")
        );
        assert_eq!(
            context.comments[0].text.as_deref(),
            Some("Verifier le screenshot")
        );
        assert!(
            context
                .relations
                .iter()
                .any(|relation| relation.kind == "attachment")
        );
    }

    #[test]
    fn map_ai_context_summary_mode_keeps_links_and_hides_relations() {
        let root: Value = serde_json::from_str(
            r#"{
  "id": 55201,
  "fields": {
    "System.Title": "Titre",
    "System.WorkItemType": "Task",
    "System.State": "New"
  },
  "relations": [
    { "rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/54000" },
    { "rel": "AttachedFile", "url": "https://dev.azure.com/org/_apis/wit/attachments/123", "attributes": { "name": "mockup.png" } }
  ]
}"#,
        )
        .expect("json should parse");

        let context = map_ai_context_item(
            &root,
            &AzureDevOpsOptions {
                organization: "https://dev.azure.com/org".into(),
                project: "Project".into(),
                api_version: default_api_version(),
            },
            true,
            vec![],
        );

        assert_eq!(context.links.parent_ids, vec!["54000"]);
        assert_eq!(context.attachments.items.len(), 1);
        assert!(context.relations.is_empty());
    }
}
