use crate::commands::project::{resolve_ado_options, resolve_cli_ado_options};
use crate::commands::work_item::parse_work_item_ids_as_strings;
use crate::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{get_ai_context, get_work_item_expanded};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::ActionEvent;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ContextArgs {
    pub id: String,
    pub root: Option<String>,
    pub project: Option<String>,
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
    pub root: String,
    pub project: String,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<String>,
    pub summary: bool,
    pub comments: i32,
    pub expanded: Vec<Value>,
    pub items: Vec<dw_contracts::AdoAiContextItem>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AiContextArgs {
    pub root: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub id: String,
    pub summary: bool,
    pub comments: i32,
    pub include_comments: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AiContextReport {
    pub root: String,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<String>,
    pub summary: bool,
    pub comments: i32,
    #[serde(rename = "includeComments")]
    pub include_comments: bool,
    pub items: Vec<dw_contracts::AdoAiContextItem>,
    pub events: Vec<String>,
}

pub async fn context_report(args: ContextArgs) -> Result<ContextReport> {
    context_report_with_events(args, |_| {}).await
}

pub async fn context_report_with_events(
    args: ContextArgs,
    mut emit: impl FnMut(ActionEvent),
) -> Result<ContextReport> {
    let ContextArgs {
        id,
        root,
        project,
        summary,
        comments,
        mode,
    } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado context requiert un projet configuré."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        format!("Connexion Azure DevOps pour le projet {project_key}..."),
    );
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let ids = parse_work_item_ids_as_strings(&id)?;
    push_event(
        &mut events,
        &mut emit,
        context_fetch_line(ids.len(), mode == ContextMode::AiContext, comments),
    );
    if mode == ContextMode::Expanded {
        let expanded = ids
            .iter()
            .map(|item_id| get_work_item_expanded(&options, item_id, &token))
            .collect::<Result<Vec<_>, _>>()?;
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
        let items = ids
            .iter()
            .map(|item_id| get_ai_context(&options, item_id, summary, comments, &token))
            .collect::<Result<Vec<_>, _>>()?;
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
    mut emit: impl FnMut(ActionEvent),
) -> Result<AiContextReport> {
    let AiContextArgs {
        root,
        organization,
        project,
        id,
        summary,
        comments,
        include_comments,
    } = args;
    let root = resolve_root(root.as_deref());
    let options = resolve_cli_ado_options(&root, organization, project)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        "Connexion Azure DevOps pour le contexte IA...".into(),
    );
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let ids = parse_work_item_ids_as_strings(&id)?;
    push_event(
        &mut events,
        &mut emit,
        context_fetch_line(ids.len(), include_comments, comments),
    );
    let items = ids
        .iter()
        .map(|item_id| {
            get_ai_context(
                &options,
                item_id,
                summary,
                if include_comments { comments } else { 0 },
                &token,
            )
        })
        .map(|context| {
            context.map(|context| {
                if include_comments {
                    context
                } else {
                    dw_contracts::AdoAiContextItem {
                        comments: vec![],
                        ..context
                    }
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
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

fn push_event(events: &mut Vec<String>, emit: &mut impl FnMut(ActionEvent), message: String) {
    emit(ActionEvent::info(message.clone()));
    events.push(message);
}

pub fn context_fetch_line(count: usize, include_comments: bool, comments: i32) -> String {
    let comment_part = if include_comments && comments > 0 {
        format!(" avec {comments} commentaire(s)")
    } else {
        String::new()
    };
    match count {
        0 => format!("Chargement contexte ADO: aucun work item{comment_part}."),
        1 => format!("Chargement contexte ADO: 1 work item{comment_part}..."),
        count => format!("Chargement contexte ADO: {count} work items{comment_part}..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_fetch_line_handles_counts_and_comments() {
        assert_eq!(
            context_fetch_line(0, true, 5),
            "Chargement contexte ADO: aucun work item avec 5 commentaire(s)."
        );
        assert_eq!(
            context_fetch_line(1, false, 5),
            "Chargement contexte ADO: 1 work item..."
        );
        assert_eq!(
            context_fetch_line(3, true, 2),
            "Chargement contexte ADO: 3 work items avec 2 commentaire(s)..."
        );
    }
}
