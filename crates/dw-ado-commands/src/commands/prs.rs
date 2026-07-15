use crate::commands::changelog::resolve_ado_repositories;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{
    AzureDevOpsOptions, PullRequestListItem, auth::AdoToken, auth::require_token,
    list_active_pull_requests_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::{AdoRepositoryName, DevWorkflowRoot, ProjectKey};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct PrsArgs {
    pub root: Option<DevWorkflowRoot>,
    pub project: ProjectKey,
    pub repo: Option<AdoRepositoryName>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrsReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    pub repositories: Vec<AdoRepositoryName>,
    pub items: Vec<PullRequestListItem>,
}

pub async fn report(args: PrsArgs) -> Result<PrsReport> {
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    let options = resolve_ado_options(&projects, &workflow, &args.project)?;
    let project_config = resolve_project(&projects, args.project.as_str());
    let repositories = resolve_ado_repositories(project_config.as_ref(), args.repo.as_ref());
    if repositories.is_empty() {
        return Err(anyhow::anyhow!(
            "ado prs requires an explicit repository, or a project with configured azureDevOpsRepository entries."
        ));
    }

    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    let mut items = load_prs_for_repositories(&options, &token, repositories.clone()).await?;
    sort_prs(&mut items);

    Ok(PrsReport {
        root,
        project: args.project,
        repositories,
        items,
    })
}

async fn load_prs_for_repositories(
    options: &AzureDevOpsOptions,
    token: &AdoToken,
    repositories: Vec<AdoRepositoryName>,
) -> Result<Vec<PullRequestListItem>> {
    let mut jobs = tokio::task::JoinSet::new();
    for repository in repositories {
        let options = options.clone();
        let token = token.clone();
        jobs.spawn_blocking(move || {
            list_active_pull_requests_authenticated(&options, repository.as_str(), &token)
        });
    }

    let mut items = Vec::new();
    while let Some(result) = jobs.join_next().await {
        items.extend(result??);
    }
    Ok(items)
}

fn sort_prs(items: &mut [PullRequestListItem]) {
    items.sort_by(|left, right| {
        left.repository
            .cmp(&right.repository)
            .then(left.pull_request_id.cmp(&right.pull_request_id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_prs_groups_repository_then_id() {
        let mut items = vec![
            pr("back", 20),
            pr("front", 30),
            pr("back", 10),
            pr("front", 5),
        ];

        sort_prs(&mut items);

        assert_eq!(
            items
                .iter()
                .map(|item| format!("{}:{}", item.repository, item.pull_request_id))
                .collect::<Vec<_>>(),
            ["back:10", "back:20", "front:5", "front:30"]
        );
    }

    fn pr(repository: &str, id: i64) -> PullRequestListItem {
        PullRequestListItem {
            repository: repository.into(),
            pull_request_id: id,
            title: None,
            status: Some("active".into()),
            source_ref_name: Some("refs/heads/feature/demo".into()),
            target_ref_name: Some("refs/heads/develop".into()),
            is_draft: false,
            created_by: None,
            url: None,
            web_url: None,
            work_item_ids: Vec::new(),
        }
    }
}
