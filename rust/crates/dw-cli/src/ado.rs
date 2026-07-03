use crate::cli::AdoCommand;
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    AzureDevOpsOptions, default_api_version, extract_work_item_ids_from_commit_messages,
    get_ai_context, get_work_item_ids_from_pull_requests, group_work_items_by_parent,
    load_changelog_items, parse_changelog_format, query_assigned_work_items,
    query_work_item_snapshots, render_flat_changelog, render_grouped_changelog,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use inquire::Select;
use std::io::IsTerminal;
use std::process::Command as ProcessCommand;

pub(crate) fn handle_ado(command: AdoCommand) -> Result<()> {
    match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let projects = load_projects_config(&root);
            let project_key = resolve_project_key_or_prompt(project, &projects, "ado assigned")?;
            let workflow = load_workflow_config(&root);
            let options = resolve_ado_options(&projects, &workflow, &project_key)?;
            let token = require_token(load_auth_options(Some(&root))?)?;
            let runtime = tokio::runtime::Runtime::new()?;
            let items = runtime.block_on(query_assigned_work_items(
                &options,
                top.try_into().unwrap_or(20),
                &token,
            ))?;
            let items = items
                .into_iter()
                .filter(|item| {
                    all || !dw_workspace::is_final_state(
                        item.kind.as_deref(),
                        item.state.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            if group_by_parent {
                print_assigned_items_grouped(&options, &items, &token, &project_key, all, json)?;
            } else {
                print_assigned_items(&items, &project_key, all, json)?;
            }
        }
        AdoCommand::Changelog {
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
        } => {
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
            let project_key = project
                .ok_or_else(|| anyhow::anyhow!("ado changelog requiert --project configure."))?;
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let options = resolve_ado_options(&projects, &workflow, &project_key)?;
            let token = require_token(load_auth_options(Some(&root))?)?;

            let work_item_ids = if from_git {
                extract_work_item_ids_from_git_range(&ids, git_to.as_deref())?
            } else {
                let project_config = resolve_project(&projects, &project_key);
                let repositories =
                    resolve_ado_repositories(project_config.as_ref(), repo.as_deref());
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
        }
        AdoCommand::WorkItem {
            id,
            root,
            project,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let project_key = project
                .ok_or_else(|| anyhow::anyhow!("ado work-item requiert --project configure."))?;
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let options = resolve_ado_options(&projects, &workflow, &project_key)?;
            let token = require_token(load_auth_options(Some(&root))?)?;
            let ids = parse_work_item_ids(&id)?;
            let runtime = tokio::runtime::Runtime::new()?;
            let items = runtime.block_on(query_work_item_snapshots(&options, &ids, &token))?;
            print_work_item_snapshots(&items, &project_key, json)?;
        }
        AdoCommand::Context {
            id,
            root,
            project,
            summary,
            comments,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let project_key = project
                .ok_or_else(|| anyhow::anyhow!("ado context requiert --project configure."))?;
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let options = resolve_ado_options(&projects, &workflow, &project_key)?;
            let token = require_token(load_auth_options(Some(&root))?)?;
            let ids = parse_work_item_ids_as_strings(&id)?;
            if json {
                let payloads = ids
                    .iter()
                    .map(|item_id| dw_ado::get_work_item_expanded(&options, item_id, &token))
                    .collect::<Result<Vec<_>, _>>()?;
                println!("{}", serde_json::to_string_pretty(&payloads)?);
            } else {
                let items = ids
                    .iter()
                    .map(|item_id| dw_ado::get_ai_context(&options, item_id, summary, &token))
                    .collect::<Result<Vec<_>, _>>()?;
                print_context_items(&items, comments, &project_key);
            }
        }
        AdoCommand::AiContext {
            root,
            organization,
            project,
            id,
            summary,
            comments: _,
            include_comments,
        } => {
            let root = resolve_root(root.as_deref());
            let options = match (organization, project) {
                (Some(organization), Some(project)) => AzureDevOpsOptions {
                    organization,
                    project,
                    api_version: default_api_version(),
                },
                (None, Some(project)) => {
                    let projects = load_projects_config(&root);
                    let workflow = load_workflow_config(&root);
                    resolve_ado_options(&projects, &workflow, &project)?
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "ado ai-context requiert --project configure ou --organization avec --project."
                    ));
                }
            };
            let token = require_token(load_auth_options(Some(&root))?)?;
            let contexts = parse_work_item_ids_as_strings(&id)?
                .iter()
                .map(|item_id| get_ai_context(&options, item_id, summary, &token))
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
            println!("{}", serde_json::to_string_pretty(&contexts)?);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectChoice {
    pub(crate) key: String,
    pub(crate) label: String,
}

impl std::fmt::Display for ProjectChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub(crate) fn resolve_ado_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project_key: &str,
) -> Result<AzureDevOpsOptions> {
    let workflow_options = workflow
        .azure_dev_ops
        .clone()
        .and_then(|value| serde_json::from_value::<AzureDevOpsOptions>(value).ok());
    let project_options =
        resolve_project(projects, project_key).and_then(|project| project.azure_dev_ops);

    match (workflow_options, project_options) {
        (Some(workflow), Some(project)) => Ok(AzureDevOpsOptions {
            organization: if project.organization.trim().is_empty() {
                workflow.organization
            } else {
                project.organization
            },
            project: if project.project.trim().is_empty() {
                workflow.project
            } else {
                project.project
            },
            api_version: if project.api_version.trim().is_empty() {
                workflow.api_version
            } else {
                project.api_version
            },
        }),
        (Some(options), None) | (None, Some(options)) => Ok(options),
        (None, None) => Err(anyhow::anyhow!(
            "Configuration azureDevOps manquante pour {}.",
            project_key
        )),
    }
}

pub(crate) fn resolve_project_key_or_prompt(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
    command_name: &str,
) -> Result<String> {
    if let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        return Ok(project);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "{command_name} requiert --project configure en mode non-interactif."
        ));
    }

    let choices = project_choices(projects);
    if choices.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun projet configure dans projects.json. Executer dw init ou completer config/projects.json."
        ));
    }

    let selected = Select::new("Projet Azure DevOps", choices).prompt()?;
    Ok(selected.key)
}

pub(crate) fn project_choices(projects: &dw_config::ProjectsConfig) -> Vec<ProjectChoice> {
    projects
        .projects
        .keys()
        .map(|key| {
            let display_name = resolve_project(projects, key)
                .map(|project| project.display_name)
                .filter(|display_name| !display_name.trim().is_empty());
            ProjectChoice {
                key: key.clone(),
                label: match display_name {
                    Some(display_name) if display_name != *key => format!("{key} - {display_name}"),
                    _ => key.clone(),
                },
            }
        })
        .collect::<Vec<_>>()
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

fn print_assigned_items(
    items: &[dw_ado::WorkItemSnapshot],
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for item in items {
        println!(
            "#{} [{}] {} - {}",
            item.id,
            item.kind.as_deref().unwrap_or("inconnu"),
            item.state.as_deref().unwrap_or("inconnu"),
            item.title.as_deref().unwrap_or("inconnu")
        );
        println!("  Start: dw task start {} --project {}", item.id, project);
    }
    Ok(())
}

fn print_assigned_items_grouped(
    options: &AzureDevOpsOptions,
    items: &[dw_ado::WorkItemSnapshot],
    token: &dw_ado::auth::AdoToken,
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    let groups = group_work_items_by_parent(options, items, token)?;
    if json {
        let payload = groups
            .iter()
            .map(|group| {
                serde_json::json!({
                    "parent": group.parent,
                    "items": group.items,
                    "suggestedStartCommand": format!(
                        "dw task start {} --project {}",
                        suggested_start_ids(&group.parent, &group.items),
                        project
                    )
                })
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string(&payload)?);
        return Ok(());
    }

    for group in groups {
        println!(
            "#{} [{}] {} - {}",
            group.parent.id,
            group.parent.kind.as_deref().unwrap_or("(inconnu)"),
            group.parent.state.as_deref().unwrap_or("(inconnu)"),
            group.parent.title.as_deref().unwrap_or("(sans titre)")
        );
        if !group.items.is_empty() {
            println!(
                "  Start: dw task start {} --project {}",
                suggested_start_ids(&group.parent, &group.items),
                project
            );
        }
        for item in group.items {
            println!(
                "  - #{} [{}] {} - {}",
                item.id,
                item.kind.as_deref().unwrap_or("(inconnu)"),
                item.state.as_deref().unwrap_or("(inconnu)"),
                item.title.as_deref().unwrap_or("(sans titre)")
            );
        }
        println!();
    }
    Ok(())
}

fn suggested_start_ids(
    parent: &dw_ado::WorkItemSnapshot,
    children: &[dw_ado::WorkItemSnapshot],
) -> String {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child.id)) {
            ids.push(child.id.clone());
        }
    }
    ids.join(",")
}

fn parse_work_item_ids(raw: &str) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let id = part
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Work item invalide: {part}"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    Ok(ids)
}

fn parse_work_item_ids_as_strings(raw: &str) -> Result<Vec<String>> {
    Ok(parse_work_item_ids(raw)?
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}

fn print_work_item_snapshots(
    items: &[dw_ado::WorkItemSnapshot],
    project: &str,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
        }
        println!("#{}", item.id);
        println!("Type: {}", item.kind.as_deref().unwrap_or("inconnu"));
        println!("Etat: {}", item.state.as_deref().unwrap_or("inconnu"));
        println!("Titre: {}", item.title.as_deref().unwrap_or("inconnu"));
        println!();
        println!(
            "Contexte complet: dw ado context {} --project {}",
            item.id, project
        );
    }
    Ok(())
}

fn print_context_items(
    items: &[dw_contracts::AdoAiContextItem],
    comment_limit: i32,
    project: &str,
) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
            println!();
        }

        println!("#{}", item.work_item.id);
        println!(
            "Type: {}",
            item.work_item.kind.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Etat: {}",
            item.work_item.state.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Titre: {}",
            item.work_item.title.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Assigne a: {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigne")
        );

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            println!();
            println!("Description:");
            println!("{}", description.trim());
        }

        if !item.relations.is_empty() {
            println!();
            println!("Relations:");
            for relation in &item.relations {
                println!(
                    "- {} {}",
                    relation.kind,
                    relation
                        .work_item_id
                        .as_deref()
                        .or(relation.url.as_deref())
                        .unwrap_or("")
                );
            }
        }

        if comment_limit != 0 && !item.comments.is_empty() {
            println!();
            println!("Commentaires:");
            for comment in item.comments.iter().take(comment_limit.max(0) as usize) {
                println!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                );
            }
        }

        println!();
        println!(
            "AI context: dw ado ai-context {} --project {}",
            item.work_item.id, project
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_choices_keep_config_order_and_include_display_name() {
        let projects: dw_config::ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "zz": { "displayName": "Projet Z", "repositories": {} },
    "ha": { "displayName": "HOMMAGE AGENCE", "repositories": {} }
  }
}"#,
        )
        .expect("projects config should parse");

        let choices = project_choices(&projects);

        assert_eq!(
            choices,
            vec![
                ProjectChoice {
                    key: "zz".into(),
                    label: "zz - Projet Z".into()
                },
                ProjectChoice {
                    key: "ha".into(),
                    label: "ha - HOMMAGE AGENCE".into()
                }
            ]
        );
    }
}
