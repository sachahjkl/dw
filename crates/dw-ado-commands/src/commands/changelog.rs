use crate::{load_auth_options, resolve_ado_options};
use anyhow::{Context, Result};
use dw_ado::{
    auth::require_token, extract_work_item_ids_from_commit_messages,
    get_work_item_ids_from_pull_requests, group_work_items_by_parent, load_changelog_items,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::{
    AdoActionEvent, AdoRepositoryName, DevWorkflowRoot, ProjectKey, PullRequestId, WorkItemId,
};
use dw_git::{GitRevisionRange, commit_messages_in_range};
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct ChangelogArgs {
    pub source: ChangelogSource,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub repo: Option<AdoRepositoryName>,
    pub group_by_parent: bool,
    pub format: ChangelogOutputFormat,
    pub table: bool,
    pub ids_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangelogSource {
    WorkItems(Vec<WorkItemId>),
    PullRequests(Vec<PullRequestId>),
    GitRange(GitRevisionRange),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    #[serde(rename = "fromPr")]
    pub from_pr: bool,
    #[serde(rename = "fromGit")]
    pub from_git: bool,
    #[serde(rename = "groupByParent")]
    pub group_by_parent: bool,
    pub format: ChangelogOutputFormat,
    pub table: bool,
    pub options: dw_ado::AzureDevOpsOptions,
    #[serde(rename = "idsOnly")]
    pub ids_only: bool,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<WorkItemId>,
    pub items: Vec<dw_ado::WorkItemSnapshot>,
    pub groups: Vec<dw_ado::WorkItemGroup>,
    #[serde(rename = "sourceEmpty")]
    pub source_empty: bool,
    #[serde(rename = "resolvedEmpty")]
    pub resolved_empty: bool,
    pub events: Vec<AdoActionEvent>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ChangelogOutputFormat {
    #[default]
    Raw,
    Markdown,
    Html,
}

impl ChangelogOutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Markdown => "markdown",
            Self::Html => "html",
        }
    }
}

impl FromStr for ChangelogOutputFormat {
    type Err = ChangelogOutputFormatParseError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "raw" => Ok(Self::Raw),
            "markdown" => Ok(Self::Markdown),
            "html" => Ok(Self::Html),
            _ => Err(ChangelogOutputFormatParseError {
                value: value.into(),
            }),
        }
    }
}

impl fmt::Display for ChangelogOutputFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogOutputFormatParseError {
    value: String,
}

impl fmt::Display for ChangelogOutputFormatParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Format de changelog inconnu: {}", self.value)
    }
}

impl std::error::Error for ChangelogOutputFormatParseError {}

pub async fn report(args: ChangelogArgs) -> Result<ChangelogReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: ChangelogArgs,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<ChangelogReport> {
    let ChangelogArgs {
        source,
        root,
        project,
        repo,
        group_by_parent,
        format,
        table,
        ids_only,
    } = args;
    let from_pr = matches!(source, ChangelogSource::PullRequests(_));
    let from_git = matches!(source, ChangelogSource::GitRange(_));
    if table && format != ChangelogOutputFormat::Markdown {
        return Err(anyhow::anyhow!(
            "Table output is only available with markdown format."
        ));
    }
    if ids_only && table {
        return Err(anyhow::anyhow!(
            "IDs-only output and table output cannot be combined."
        ));
    }

    let root = DevWorkflowRoot::from(resolve_root(root.as_ref().map(DevWorkflowRoot::as_str)));
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado changelog requires a configured project."))?;
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

    let work_item_ids = match source {
        ChangelogSource::WorkItems(ids) => ids,
        ChangelogSource::GitRange(range) => {
            push_event(
                &mut events,
                &mut emit,
                AdoActionEvent::ExtractingGitWorkItems {
                    git_to: range.to.clone(),
                },
            );
            extract_work_item_ids_from_git_range(&range)?
        }
        ChangelogSource::PullRequests(pull_request_ids) => {
            let project_config = resolve_project(&projects, project_key.as_str());
            let repositories = resolve_ado_repositories(project_config.as_ref(), repo.as_ref());
            push_event(
                &mut events,
                &mut emit,
                AdoActionEvent::ResolvingPullRequestWorkItems {
                    repositories: repositories.clone(),
                },
            );
            let options = options.clone();
            let token = token.clone();
            let ado_repositories = repositories
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            tokio::task::spawn_blocking(move || {
                get_work_item_ids_from_pull_requests(
                    &options,
                    &ado_repositories,
                    &pull_request_ids,
                    &token,
                )
            })
            .await
            .context("resolving work items from pull requests was interrupted")??
            .into_iter()
            .collect()
        }
    };

    if work_item_ids.is_empty() {
        return Ok(ChangelogReport {
            root,
            project: project_key,
            from_pr,
            from_git,
            group_by_parent,
            format,
            table,
            options,
            ids_only,
            work_item_ids: Vec::new(),
            items: Vec::new(),
            groups: Vec::new(),
            source_empty: true,
            resolved_empty: false,
            events,
        });
    }

    if ids_only {
        return Ok(ChangelogReport {
            root,
            project: project_key,
            from_pr,
            from_git,
            group_by_parent,
            format,
            table,
            options,
            ids_only,
            work_item_ids,
            items: Vec::new(),
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: false,
            events,
        });
    }

    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::LoadingChangelogItems {
            ids: work_item_ids.clone(),
        },
    );
    let mut items = {
        let options = options.clone();
        let token = token.clone();
        let work_item_ids = work_item_ids.clone();
        tokio::task::spawn_blocking(move || load_changelog_items(&options, &work_item_ids, &token))
            .await
            .context("loading changelog work items was interrupted")??
    };
    if items.is_empty() {
        return Ok(ChangelogReport {
            root,
            project: project_key,
            from_pr,
            from_git,
            group_by_parent,
            format,
            table,
            options,
            ids_only,
            work_item_ids,
            items,
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: true,
            events,
        });
    }

    let groups = if group_by_parent {
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::GroupingAssignedWorkItems {
                project: project_key.clone(),
            },
        );
        let options = options.clone();
        let token = token.clone();
        let items_for_grouping = items.clone();
        tokio::task::spawn_blocking(move || {
            group_work_items_by_parent(&options, &items_for_grouping, &token)
        })
        .await
        .context("grouping changelog work items was interrupted")??
    } else {
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Vec::new()
    };
    Ok(ChangelogReport {
        root,
        project: project_key,
        from_pr,
        from_git,
        group_by_parent,
        format,
        table,
        options,
        ids_only,
        work_item_ids,
        items,
        groups,
        source_empty: false,
        resolved_empty: false,
        events,
    })
}

fn push_event(
    events: &mut Vec<AdoActionEvent>,
    emit: &mut impl FnMut(AdoActionEvent),
    event: AdoActionEvent,
) {
    emit(event.clone());
    events.push(event);
}

pub fn resolve_ado_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&AdoRepositoryName>,
) -> Vec<AdoRepositoryName> {
    if let Some(repository) = repository {
        return std::iter::once(resolve_ado_repository(project_config, repository.as_str())).fold(
            Vec::<AdoRepositoryName>::new(),
            |mut repos, repo| {
                if !repos
                    .iter()
                    .any(|existing| existing.as_str().eq_ignore_ascii_case(repo.as_str()))
                {
                    repos.push(repo);
                }
                repos
            },
        );
    }

    project_config
        .map(|project| {
            project
                .repositories
                .keys()
                .filter_map(|key| dw_config::repository_config(project, key))
                .filter_map(|repo| repo.azure_dev_ops_repository)
                .filter(|repo| !repo.trim().is_empty())
                .map(AdoRepositoryName::from)
                .fold(Vec::<AdoRepositoryName>::new(), |mut repos, repo| {
                    if !repos
                        .iter()
                        .any(|existing| existing.as_str().eq_ignore_ascii_case(repo.as_str()))
                    {
                        repos.push(repo);
                    }
                    repos
                })
        })
        .unwrap_or_default()
}

pub fn resolve_ado_repository(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: &str,
) -> AdoRepositoryName {
    AdoRepositoryName::from(
        project_config
            .and_then(|project| dw_config::repository_config(project, repository))
            .and_then(|repo| repo.azure_dev_ops_repository)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| repository.to_string()),
    )
}

fn extract_work_item_ids_from_git_range(range: &GitRevisionRange) -> Result<Vec<WorkItemId>> {
    let messages = commit_messages_in_range(range)?;
    Ok(extract_work_item_ids_from_commit_messages(
        messages.as_str(),
    ))
}
