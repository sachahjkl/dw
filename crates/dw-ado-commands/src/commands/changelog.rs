use crate::{load_auth_options, resolve_ado_options};
use anyhow::{Context, Result};
use dw_ado::{
    auth::require_token, extract_work_item_ids_from_commit_messages,
    get_work_item_ids_from_pull_requests, group_work_items_by_parent, load_changelog_items,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::{
    AdoActionEvent, AdoRepositoryName, DevWorkflowRoot, ProjectKey, PullRequestId, RepositoryPath,
    WorkItemId, WorkspaceRepositoryName,
};
use dw_git::{GitRevisionRange, commit_messages_in_range_at};
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};
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
    pub sections: Vec<ChangelogSection>,
    pub events: Vec<AdoActionEvent>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogSection {
    pub repository: Option<WorkspaceRepositoryName>,
    #[serde(rename = "repositoryPath")]
    pub repository_path: Option<RepositoryPath>,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<WorkItemId>,
    pub items: Vec<dw_ado::WorkItemSnapshot>,
    pub groups: Vec<dw_ado::WorkItemGroup>,
    #[serde(rename = "sourceEmpty")]
    pub source_empty: bool,
    #[serde(rename = "resolvedEmpty")]
    pub resolved_empty: bool,
    pub warnings: Vec<ChangelogWarning>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogWarning {
    pub detail: String,
}

impl ChangelogSection {
    fn new(
        repository: Option<WorkspaceRepositoryName>,
        repository_path: Option<RepositoryPath>,
        work_item_ids: Vec<WorkItemId>,
    ) -> Self {
        let source_empty = work_item_ids.is_empty();
        Self {
            repository,
            repository_path,
            work_item_ids,
            items: Vec::new(),
            groups: Vec::new(),
            source_empty,
            resolved_empty: false,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitChangelogTarget {
    repository: WorkspaceRepositoryName,
    path: RepositoryPath,
    warnings: Vec<ChangelogWarning>,
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
    if table && format == ChangelogOutputFormat::Raw {
        return Err(anyhow::anyhow!(
            "Table output is only available with markdown or html format."
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

    let mut sections = match source {
        ChangelogSource::WorkItems(ids) => vec![ChangelogSection::new(None, None, ids)],
        ChangelogSource::GitRange(range) => {
            push_event(
                &mut events,
                &mut emit,
                AdoActionEvent::ExtractingGitWorkItems {
                    git_to: range.to.clone(),
                },
            );
            let project_config = resolve_project(&projects, project_key.as_str());
            resolve_git_changelog_targets(
                root.as_str(),
                &project_key,
                project_config.as_ref(),
                repo.as_ref(),
            )?
            .into_iter()
            .map(|target| extract_git_changelog_section(target, &range))
            .collect()
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
            let work_item_ids = tokio::task::spawn_blocking(move || {
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
            .collect();
            vec![ChangelogSection::new(None, None, work_item_ids)]
        }
    };

    let work_item_ids = distinct_work_item_ids(
        sections
            .iter()
            .flat_map(|section| section.work_item_ids.iter()),
    );

    if work_item_ids.is_empty() || ids_only {
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
            sections,
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
    let items = {
        let options = options.clone();
        let token = token.clone();
        let work_item_ids = work_item_ids.clone();
        tokio::task::spawn_blocking(move || load_changelog_items(&options, &work_item_ids, &token))
            .await
            .context("loading changelog work items was interrupted")??
    };
    for section in &mut sections {
        section.items = items
            .iter()
            .filter(|item| section.work_item_ids.iter().any(|id| id == &item.id))
            .cloned()
            .collect();
        section.items.sort_by(|left, right| left.id.cmp(&right.id));
        section.resolved_empty = !section.work_item_ids.is_empty() && section.items.is_empty();
    }

    if group_by_parent {
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::GroupingAssignedWorkItems {
                project: project_key.clone(),
            },
        );
        for section in sections
            .iter_mut()
            .filter(|section| !section.items.is_empty())
        {
            let options = options.clone();
            let token = token.clone();
            let items_for_grouping = section.items.clone();
            match tokio::task::spawn_blocking(move || {
                group_work_items_by_parent(&options, &items_for_grouping, &token)
            })
            .await
            .context("grouping changelog work items was interrupted")?
            {
                Ok(groups) => section.groups = groups,
                Err(error) => section.warnings.push(ChangelogWarning {
                    detail: format!("Could not group work items by parent: {error}"),
                }),
            }
        }
    }
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
        sections,
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

fn resolve_git_changelog_targets(
    root: &str,
    project_key: &ProjectKey,
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&AdoRepositoryName>,
) -> Result<Vec<GitChangelogTarget>> {
    let current_directory = std::env::current_dir().context("resolving the current directory")?;
    resolve_git_changelog_targets_from(
        root,
        project_key,
        project_config,
        repository,
        &current_directory,
    )
}

fn resolve_git_changelog_targets_from(
    root: &str,
    project_key: &ProjectKey,
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&AdoRepositoryName>,
    current_directory: &Path,
) -> Result<Vec<GitChangelogTarget>> {
    if let Some(repository) = repository {
        if let Some((key, config)) = find_configured_repository(project_config, repository.as_str())
        {
            return Ok(vec![configured_git_target(
                root,
                project_key,
                &key,
                config,
                current_directory,
            )]);
        }

        let path = absolute_path(current_directory, repository.as_str());
        return Ok(vec![GitChangelogTarget {
            repository: WorkspaceRepositoryName::from(repository_label(&path, repository.as_str())),
            path: RepositoryPath::from(path.display().to_string()),
            warnings: Vec::new(),
        }]);
    }

    let project_config = project_config.ok_or_else(|| {
        anyhow::anyhow!(
            "Project '{}' is not configured; specify --repo with a local repository path.",
            project_key
        )
    })?;
    if project_config.repositories.is_empty() {
        return Err(anyhow::anyhow!(
            "Project '{}' has no configured repositories.",
            project_key
        ));
    }

    Ok(project_config
        .repositories
        .keys()
        .map(|key| {
            let config = dw_config::repository_config(project_config, key);
            configured_git_target(root, project_key, key, config, current_directory)
        })
        .collect())
}

fn find_configured_repository(
    project_config: Option<&dw_config::ProjectConfig>,
    requested: &str,
) -> Option<(String, Option<dw_config::RepositoryConfig>)> {
    let project = project_config?;
    project.repositories.keys().find_map(|key| {
        let config = dw_config::repository_config(project, key);
        let matches_key = key.eq_ignore_ascii_case(requested);
        let matches_ado_name = config
            .as_ref()
            .and_then(|config| config.azure_dev_ops_repository.as_deref())
            .is_some_and(|name| name.eq_ignore_ascii_case(requested));
        (matches_key || matches_ado_name).then(|| (key.clone(), config))
    })
}

fn configured_git_target(
    root: &str,
    project_key: &ProjectKey,
    key: &str,
    config: Option<dw_config::RepositoryConfig>,
    current_directory: &Path,
) -> GitChangelogTarget {
    let mut warnings = Vec::new();
    if config.is_none() {
        warnings.push(ChangelogWarning {
            detail: "The configured repository entry is invalid; using its default anchor path."
                .into(),
        });
    }
    let path =
        configured_repository_path(root, project_key, key, config.as_ref(), current_directory);
    GitChangelogTarget {
        repository: WorkspaceRepositoryName::from(key),
        path: RepositoryPath::from(path.display().to_string()),
        warnings,
    }
}

fn configured_repository_path(
    root: &str,
    project_key: &ProjectKey,
    key: &str,
    config: Option<&dw_config::RepositoryConfig>,
    current_directory: &Path,
) -> PathBuf {
    let current_directory_text = current_directory.display().to_string();
    if let Some(workspace) = dw_workspace::find_workspace_path(&current_directory_text) {
        let manifest_path = Path::new(&workspace).join("task.json");
        if let Ok(manifest) = dw_workspace::read_manifest_path(&manifest_path.display().to_string())
            && manifest.project == *project_key
            && manifest
                .repositories
                .iter()
                .any(|repository| repository.as_str().eq_ignore_ascii_case(key))
        {
            let folder = config
                .and_then(|config| config.folder.as_deref())
                .filter(|folder| !folder.trim().is_empty())
                .unwrap_or(key);
            return Path::new(&workspace).join(folder);
        }
    }

    let anchor = config
        .and_then(|config| config.anchor_name.as_deref())
        .filter(|anchor| !anchor.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{key}.git"));
    Path::new(root)
        .join("projects")
        .join(project_key.as_str())
        .join("repositories")
        .join(anchor)
}

fn absolute_path(current_directory: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        current_directory.join(path)
    }
}

fn repository_label(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.strip_suffix(".git").unwrap_or(name).to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn extract_git_changelog_section(
    target: GitChangelogTarget,
    range: &GitRevisionRange,
) -> ChangelogSection {
    let mut section = ChangelogSection::new(
        Some(target.repository),
        Some(target.path.clone()),
        Vec::new(),
    );
    section.warnings = target.warnings;
    match commit_messages_in_range_at(&target.path, range) {
        Ok(messages) => {
            section.work_item_ids = extract_work_item_ids_from_commit_messages(messages.as_str());
            section.source_empty = section.work_item_ids.is_empty();
        }
        Err(error) => {
            section.source_empty = false;
            section.warnings.push(ChangelogWarning {
                detail: format!("Could not read git range from '{}': {error}", target.path),
            });
        }
    }
    section
}

fn distinct_work_item_ids<'a>(values: impl IntoIterator<Item = &'a WorkItemId>) -> Vec<WorkItemId> {
    values.into_iter().fold(Vec::new(), |mut ids, id| {
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.clone());
        }
        ids
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn git_targets_default_to_every_configured_anchor_in_order() {
        let project = project_config();

        let targets = resolve_git_changelog_targets_from(
            "/dw",
            &ProjectKey::from("acme"),
            Some(&project),
            None,
            Path::new("/outside"),
        )
        .expect("targets");

        assert_eq!(
            targets
                .iter()
                .map(|target| target.repository.as_str())
                .collect::<Vec<_>>(),
            vec!["front", "back"]
        );
        assert!(
            Path::new(targets[0].path.as_str())
                .ends_with(Path::new("projects/acme/repositories/acme-front.git"))
        );
        assert!(
            Path::new(targets[1].path.as_str())
                .ends_with(Path::new("projects/acme/repositories/acme-back.git"))
        );
    }

    #[test]
    fn git_target_accepts_configured_key_ado_name_or_local_path() {
        let project = project_config();
        let configured_key = resolve_git_changelog_targets_from(
            "/dw",
            &ProjectKey::from("acme"),
            Some(&project),
            Some(&AdoRepositoryName::from("FRONT")),
            Path::new("/outside"),
        )
        .expect("configured key target");
        assert_eq!(configured_key[0].repository.as_str(), "front");

        let configured_name = resolve_git_changelog_targets_from(
            "/dw",
            &ProjectKey::from("acme"),
            Some(&project),
            Some(&AdoRepositoryName::from("ACME-BACK")),
            Path::new("/outside"),
        )
        .expect("configured ADO target");
        assert_eq!(configured_name[0].repository.as_str(), "back");

        let local = resolve_git_changelog_targets_from(
            "/dw",
            &ProjectKey::from("acme"),
            Some(&project),
            Some(&AdoRepositoryName::from("local/custom.git")),
            Path::new("workspace"),
        )
        .expect("local target");
        assert_eq!(local[0].repository.as_str(), "custom");
        assert_eq!(
            Path::new(local[0].path.as_str()),
            Path::new("workspace/local/custom.git")
        );

        let mut project_with_invalid_entry = project;
        project_with_invalid_entry
            .repositories
            .insert("broken".into(), json!({ "invalid": true }));
        let invalid = resolve_git_changelog_targets_from(
            "/dw",
            &ProjectKey::from("acme"),
            Some(&project_with_invalid_entry),
            Some(&AdoRepositoryName::from("BROKEN")),
            Path::new("/outside"),
        )
        .expect("invalid configured target");
        assert_eq!(invalid[0].repository.as_str(), "broken");
        assert!(
            Path::new(invalid[0].path.as_str())
                .ends_with(Path::new("projects/acme/repositories/broken.git"))
        );
        assert_eq!(invalid[0].warnings.len(), 1);
    }

    #[test]
    fn git_range_failure_is_kept_as_a_repository_warning() {
        let missing = std::env::temp_dir().join(format!(
            "dw-missing-changelog-repository-{}",
            std::process::id()
        ));
        let section = extract_git_changelog_section(
            GitChangelogTarget {
                repository: "missing".into(),
                path: missing.display().to_string().into(),
                warnings: Vec::new(),
            },
            &GitRevisionRange::new("from".into(), "to".into()),
        );

        assert_eq!(
            section.repository.as_ref().map(|value| value.as_str()),
            Some("missing")
        );
        assert!(!section.source_empty);
        assert!(section.work_item_ids.is_empty());
        assert_eq!(section.warnings.len(), 1);
        assert!(
            section.warnings[0]
                .detail
                .contains("Could not read git range")
        );
    }

    fn project_config() -> dw_config::ProjectConfig {
        serde_json::from_value(json!({
            "displayName": "Acme",
            "repositories": {
                "front": {
                    "url": "https://example.test/front.git",
                    "defaultBranch": "develop",
                    "azureDevOpsRepository": "acme-front",
                    "anchorName": "acme-front.git",
                    "folder": "front"
                },
                "back": {
                    "url": "https://example.test/back.git",
                    "defaultBranch": "develop",
                    "azureDevOpsRepository": "acme-back",
                    "anchorName": "acme-back.git",
                    "folder": "back"
                }
            }
        }))
        .expect("project config")
    }
}
