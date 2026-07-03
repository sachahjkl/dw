use crate::ado::resolve_ado_options;
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    extract_work_item_ids_from_commit_messages, get_work_item_ids_from_pull_requests,
    group_work_items_by_parent, load_changelog_items, parse_changelog_format,
    render_flat_changelog, render_grouped_changelog,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use std::process::Command as ProcessCommand;

#[derive(Debug, Clone)]
pub(crate) struct ChangelogArgs {
    pub(crate) ids: String,
    pub(crate) root: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) from_pr: bool,
    pub(crate) from_git: bool,
    pub(crate) repo: Option<String>,
    pub(crate) group_by_parent: bool,
    pub(crate) format: Option<String>,
    pub(crate) table: bool,
    pub(crate) ids_only: bool,
    pub(crate) git_to: Option<String>,
}

pub(crate) fn handle(args: ChangelogArgs) -> Result<()> {
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
            "Choisir soit --from-pr, soit --from-git, pas les deux."
        ));
    }
    let output_format = parse_changelog_format(format.as_deref())?;
    if table && output_format != dw_ado::ChangelogFormat::Markdown {
        return Err(anyhow::anyhow!(
            "L'option --table est uniquement disponible avec --format markdown."
        ));
    }
    if ids_only && table {
        return Err(anyhow::anyhow!(
            "Les options --ids-only et --table ne peuvent pas etre combinees."
        ));
    }

    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado changelog requiert --project configure."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let token = require_token(load_auth_options(Some(&root))?)?;

    let work_item_ids = if from_git {
        extract_work_item_ids_from_git_range(&ids, git_to.as_deref())?
    } else {
        let project_config = resolve_project(&projects, &project_key);
        let repositories = resolve_ado_repositories(project_config.as_ref(), repo.as_deref());
        get_work_item_ids_from_pull_requests(&options, &repositories, &ids, &token)?
    };

    if work_item_ids.is_empty() {
        println!(
            "{}",
            if from_git {
                "Aucun work item detecte dans les messages de commit de la plage git."
            } else {
                "Aucun work item detecte pour les pull requests donnees."
            }
        );
        return Ok(());
    }

    if ids_only {
        println!("{}", work_item_ids.join(" "));
        return Ok(());
    }

    let mut items = load_changelog_items(&options, &work_item_ids, &token)?;
    if items.is_empty() {
        println!("Aucun work item resolu dans Azure DevOps.");
        return Ok(());
    }

    if group_by_parent {
        let groups = group_work_items_by_parent(&options, &items, &token)?;
        println!(
            "{}",
            render_grouped_changelog(&groups, output_format, &options, table)
        );
    } else {
        items.sort_by(|left, right| left.id.cmp(&right.id));
        println!(
            "{}",
            render_flat_changelog(&items, output_format, &options, table)
        );
    }
    Ok(())
}

fn resolve_ado_repositories(
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

fn resolve_ado_repository(
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
    let to = to.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
        anyhow::anyhow!("Le mode --from-git attend 2 refs git: source et target.")
    })?;
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
        return Err(anyhow::anyhow!("git log a echoue: {message}"));
    }
    Ok(extract_work_item_ids_from_commit_messages(
        &String::from_utf8_lossy(&output.stdout),
    ))
}
