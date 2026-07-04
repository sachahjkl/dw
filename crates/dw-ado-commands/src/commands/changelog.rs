use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    ChangelogFormat, extract_work_item_ids_from_commit_messages,
    get_work_item_ids_from_pull_requests, group_work_items_by_parent, load_changelog_items,
    parse_changelog_format,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::ActionEvent;
use serde::Serialize;
use std::process::Command as ProcessCommand;

#[derive(Debug, Clone)]
pub struct ChangelogArgs {
    pub ids: String,
    pub root: Option<String>,
    pub project: Option<String>,
    pub from_pr: bool,
    pub from_git: bool,
    pub repo: Option<String>,
    pub group_by_parent: bool,
    pub format: Option<String>,
    pub table: bool,
    pub ids_only: bool,
    pub git_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogReport {
    pub root: String,
    pub project: String,
    #[serde(rename = "fromPr")]
    pub from_pr: bool,
    #[serde(rename = "fromGit")]
    pub from_git: bool,
    #[serde(rename = "groupByParent")]
    pub group_by_parent: bool,
    pub format: String,
    pub table: bool,
    pub options: dw_ado::AzureDevOpsOptions,
    #[serde(rename = "idsOnly")]
    pub ids_only: bool,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<String>,
    pub items: Vec<dw_ado::WorkItemSnapshot>,
    pub groups: Vec<dw_ado::WorkItemGroup>,
    #[serde(rename = "sourceEmpty")]
    pub source_empty: bool,
    #[serde(rename = "resolvedEmpty")]
    pub resolved_empty: bool,
    pub events: Vec<String>,
}

pub async fn report(args: ChangelogArgs) -> Result<ChangelogReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: ChangelogArgs,
    mut emit: impl FnMut(ActionEvent),
) -> Result<ChangelogReport> {
    let ChangelogArgs {
        ids,
        root,
        project,
        from_pr,
        from_git,
        repo,
        group_by_parent,
        format,
        table,
        ids_only,
        git_to,
    } = args;
    if from_pr && from_git {
        return Err(anyhow::anyhow!(
            "Choisir soit le mode PR, soit le mode git, pas les deux."
        ));
    }
    let output_format = parse_changelog_format(format.as_deref())?;
    if table && output_format != ChangelogFormat::Markdown {
        return Err(anyhow::anyhow!(
            "La sortie tableau est uniquement disponible avec le format markdown."
        ));
    }
    if ids_only && table {
        return Err(anyhow::anyhow!(
            "La sortie IDs seuls et la sortie tableau ne peuvent pas être combinées."
        ));
    }

    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado changelog requiert un projet configuré."))?;
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

    let work_item_ids = if from_git {
        push_event(
            &mut events,
            &mut emit,
            changelog_git_extract_line(git_to.as_deref()),
        );
        extract_work_item_ids_from_git_range(&ids, git_to.as_deref())?
    } else {
        let project_config = resolve_project(&projects, &project_key);
        let repositories = resolve_ado_repositories(project_config.as_ref(), repo.as_deref());
        push_event(
            &mut events,
            &mut emit,
            changelog_pr_fetch_line(&repositories),
        );
        get_work_item_ids_from_pull_requests(&options, &repositories, &ids, &token)?
    };

    if work_item_ids.is_empty() {
        return Ok(ChangelogReport {
            root,
            project: project_key,
            from_pr,
            from_git,
            group_by_parent,
            format: changelog_format_name(output_format).into(),
            table,
            options,
            ids_only,
            work_item_ids,
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
            format: changelog_format_name(output_format).into(),
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
        changelog_items_fetch_line(work_item_ids.len()),
    );
    let mut items = load_changelog_items(&options, &work_item_ids, &token)?;
    if items.is_empty() {
        return Ok(ChangelogReport {
            root,
            project: project_key,
            from_pr,
            from_git,
            group_by_parent,
            format: changelog_format_name(output_format).into(),
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
            "Groupement par parent ADO...".into(),
        );
        group_work_items_by_parent(&options, &items, &token)?
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
        format: changelog_format_name(output_format).into(),
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

fn push_event(events: &mut Vec<String>, emit: &mut impl FnMut(ActionEvent), message: String) {
    emit(ActionEvent::info(message.clone()));
    events.push(message);
}

pub fn changelog_git_extract_line(git_to: Option<&str>) -> String {
    match git_to.filter(|value| !value.trim().is_empty()) {
        Some(target) => format!("Extraction des work items depuis git jusqu'à {target}..."),
        None => "Extraction des work items depuis git...".into(),
    }
}

pub fn changelog_pr_fetch_line(repositories: &[String]) -> String {
    match repositories.len() {
        0 => "Chargement PR ADO: aucun repository configuré.".into(),
        1 => format!("Chargement PR ADO sur {}...", repositories[0]),
        count => format!("Chargement PR ADO sur {count} repositories..."),
    }
}

pub fn changelog_items_fetch_line(count: usize) -> String {
    match count {
        0 => "Chargement changelog ADO: aucun work item à résoudre.".into(),
        1 => "Chargement changelog ADO: résolution de 1 work item...".into(),
        count => format!("Chargement changelog ADO: résolution de {count} work items..."),
    }
}

pub fn resolve_ado_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&str>,
) -> Vec<String> {
    if let Some(repository) = repository.filter(|value| !value.trim().is_empty()) {
        return repository
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|repo| resolve_ado_repository(project_config, repo))
            .fold(Vec::new(), |mut repos, repo| {
                if !repos
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&repo))
                {
                    repos.push(repo);
                }
                repos
            });
    }

    project_config
        .map(|project| {
            project
                .repositories
                .keys()
                .filter_map(|key| dw_config::repository_config(project, key))
                .filter_map(|repo| repo.azure_dev_ops_repository)
                .filter(|repo| !repo.trim().is_empty())
                .fold(Vec::new(), |mut repos, repo| {
                    if !repos
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(&repo))
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
) -> String {
    project_config
        .and_then(|project| dw_config::repository_config(project, repository))
        .and_then(|repo| repo.azure_dev_ops_repository)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repository.to_string())
}

fn extract_work_item_ids_from_git_range(from: &str, to: Option<&str>) -> Result<Vec<String>> {
    let to = to
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Le mode git attend 2 refs git: source et target."))?;
    let output = ProcessCommand::new("git")
        .args(["log", "--format=%B%x1e", &format!("{from}..{to}")])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or("erreur inconnue");
        return Err(anyhow::anyhow!("git log a échoué: {message}"));
    }
    Ok(extract_work_item_ids_from_commit_messages(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

pub fn changelog_format_name(format: ChangelogFormat) -> &'static str {
    match format {
        ChangelogFormat::Raw => "raw",
        ChangelogFormat::Markdown => "markdown",
        ChangelogFormat::Html => "html",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_progress_lines_handle_counts() {
        assert_eq!(
            changelog_pr_fetch_line(&[]),
            "Chargement PR ADO: aucun repository configuré."
        );
        assert_eq!(
            changelog_pr_fetch_line(&["front".into()]),
            "Chargement PR ADO sur front..."
        );
        assert_eq!(
            changelog_pr_fetch_line(&["front".into(), "back".into()]),
            "Chargement PR ADO sur 2 repositories..."
        );
        assert_eq!(
            changelog_items_fetch_line(0),
            "Chargement changelog ADO: aucun work item à résoudre."
        );
        assert_eq!(
            changelog_items_fetch_line(1),
            "Chargement changelog ADO: résolution de 1 work item..."
        );
        assert_eq!(
            changelog_items_fetch_line(3),
            "Chargement changelog ADO: résolution de 3 work items..."
        );
    }

    #[test]
    fn changelog_git_extract_line_mentions_target_when_present() {
        assert_eq!(
            changelog_git_extract_line(None),
            "Extraction des work items depuis git..."
        );
        assert_eq!(
            changelog_git_extract_line(Some("develop")),
            "Extraction des work items depuis git jusqu'à develop..."
        );
    }
}
