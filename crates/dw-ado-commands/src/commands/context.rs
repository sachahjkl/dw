use crate::commands::project::{resolve_ado_options, resolve_cli_ado_options};
use crate::commands::work_item::ado_work_item_id_values;
use crate::load_auth_options;
use anyhow::{Context, Result};
use dw_ado::auth::{AdoToken, require_token};
use dw_ado::{get_ai_context, get_work_item_expanded};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{AdoActionEvent, DevWorkflowRoot, ProjectKey, WorkItemId};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ContextArgs {
    pub ids: Vec<WorkItemId>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub summary: bool,
    pub comments: i32,
    pub mode: ContextMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMode {
    AiContext,
    Expanded,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContextReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<WorkItemId>,
    pub summary: bool,
    pub comments: i32,
    pub expanded: Vec<Value>,
    pub items: Vec<dw_contracts::AdoAiContextItem>,
    pub events: Vec<AdoActionEvent>,
}

#[derive(Debug, Clone)]
pub struct AiContextArgs {
    pub root: Option<DevWorkflowRoot>,
    pub organization: Option<String>,
    pub project: Option<ProjectKey>,
    pub ids: Vec<WorkItemId>,
    pub summary: bool,
    pub comments: i32,
    pub include_comments: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AiContextReport {
    pub root: DevWorkflowRoot,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<WorkItemId>,
    pub summary: bool,
    pub comments: i32,
    #[serde(rename = "includeComments")]
    pub include_comments: bool,
    pub items: Vec<dw_contracts::AdoAiContextItem>,
    pub events: Vec<AdoActionEvent>,
}

pub async fn context_report(args: ContextArgs) -> Result<ContextReport> {
    context_report_with_events(args, |_| {}).await
}

pub async fn context_report_with_events(
    args: ContextArgs,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<ContextReport> {
    let ContextArgs {
        ids,
        root,
        project,
        summary,
        comments,
        mode,
    } = args;
    let root = DevWorkflowRoot::from(resolve_root(root.as_ref().map(DevWorkflowRoot::as_str)));
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado context requires a configured project."))?;
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating {
            project: Some(project_key.clone()),
        },
    );
    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    if ids.is_empty() {
        return Err(anyhow::anyhow!("At least one work item is required."));
    }
    let id_values = ado_work_item_id_values(&ids);
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::LoadingWorkItems { ids: ids.clone() },
    );
    if mode == ContextMode::Expanded {
        let options = options.clone();
        let expanded_ids = id_values.clone();
        let expanded = tokio::task::spawn_blocking(move || {
            load_expanded_context_items(&options, &expanded_ids, &token)
        })
        .await
        .context("loading expanded ADO context was interrupted")??;
        Ok(ContextReport {
            root,
            project: project_key,
            requested_ids: ids,
            summary,
            comments,
            expanded,
            items: Vec::new(),
            events,
        })
    } else {
        let options = options.clone();
        let context_ids = id_values.clone();
        let items = tokio::task::spawn_blocking(move || {
            load_ai_context_items(&options, &context_ids, summary, comments, &token)
        })
        .await
        .context("loading ADO AI context was interrupted")??;
        Ok(ContextReport {
            root,
            project: project_key,
            requested_ids: ids,
            summary,
            comments,
            expanded: Vec::new(),
            items,
            events,
        })
    }
}

pub async fn ai_context_report(args: AiContextArgs) -> Result<AiContextReport> {
    ai_context_report_with_events(args, |_| {}).await
}

pub async fn ai_context_report_with_events(
    args: AiContextArgs,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<AiContextReport> {
    let AiContextArgs {
        root,
        organization,
        project,
        ids,
        summary,
        comments,
        include_comments,
    } = args;
    let root = DevWorkflowRoot::from(resolve_root(root.as_ref().map(DevWorkflowRoot::as_str)));
    let options = resolve_cli_ado_options(root.as_str(), organization, project)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating { project: None },
    );
    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    if ids.is_empty() {
        return Err(anyhow::anyhow!("At least one work item is required."));
    }
    let id_values = ado_work_item_id_values(&ids);
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::LoadingWorkItems { ids: ids.clone() },
    );
    let context_ids = id_values.clone();
    let context_comments = if include_comments { comments } else { 0 };
    let items = tokio::task::spawn_blocking(move || {
        load_ai_context_items(&options, &context_ids, summary, context_comments, &token)
    })
    .await
    .context("loading ADO AI context was interrupted")?
    .map(|items| {
        if include_comments {
            items
        } else {
            items
                .into_iter()
                .map(|context| dw_contracts::AdoAiContextItem {
                    comments: vec![],
                    ..context
                })
                .collect()
        }
    })?;
    Ok(AiContextReport {
        root,
        requested_ids: ids,
        summary,
        comments,
        include_comments,
        items,
        events,
    })
}

fn load_expanded_context_items(
    options: &dw_ado::AzureDevOpsOptions,
    ids: &[String],
    token: &AdoToken,
) -> Result<Vec<Value>> {
    Ok(ids
        .iter()
        .map(|item_id| get_work_item_expanded(options, item_id, token))
        .collect::<Result<Vec<_>, _>>()?)
}

fn load_ai_context_items(
    options: &dw_ado::AzureDevOpsOptions,
    ids: &[String],
    summary: bool,
    comments: i32,
    token: &AdoToken,
) -> Result<Vec<dw_contracts::AdoAiContextItem>> {
    Ok(ids
        .iter()
        .map(|item_id| get_ai_context(options, item_id, summary, comments, token))
        .collect::<Result<Vec<_>, _>>()?)
}

fn push_event(
    events: &mut Vec<AdoActionEvent>,
    emit: &mut impl FnMut(AdoActionEvent),
    event: AdoActionEvent,
) {
    emit(event.clone());
    events.push(event);
}

pub fn context_fetch_line(count: usize, include_comments: bool, comments: i32) -> String {
    let comment_part = if include_comments && comments == 1 {
        " with 1 comment".to_string()
    } else if include_comments && comments > 1 {
        format!(" with {comments} comments")
    } else {
        String::new()
    };
    match count {
        0 => format!("Loading ADO context: no work items{comment_part}."),
        1 => format!("Loading ADO context: 1 work item{comment_part}..."),
        count => format!("Loading ADO context: {count} work items{comment_part}..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_fetch_line_handles_counts_and_comments() {
        assert_eq!(
            context_fetch_line(0, true, 5),
            "Loading ADO context: no work items with 5 comments."
        );
        assert_eq!(
            context_fetch_line(1, false, 5),
            "Loading ADO context: 1 work item..."
        );
        assert_eq!(
            context_fetch_line(3, true, 2),
            "Loading ADO context: 3 work items with 2 comments..."
        );
    }
}
