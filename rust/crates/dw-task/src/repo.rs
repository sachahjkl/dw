use crate::write_workspace_agent_configs;
use anyhow::Result;
use dw_config::{load_projects_config, resolve_project, resolve_root};
use dw_git::{
    WorktreePrepareRequest, commit_repository, prepare_worktree, repository_status,
    update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    build_commit_message, execute_task_add_repo, execute_task_teardown, plan_task_add_repo,
    plan_task_commit, plan_task_repo_latest, plan_task_teardown, resolve_workspace,
    resolve_workspace_for_workspace_command,
};

use self::render::{
    add_repo_plan_lines, commit_status_lines, repo_latest_header_lines, teardown_plan_lines,
};
use crate::render::{print_styled, print_styled_lines};
use dw_ui::{confirm_or_require_flag, is_stdin_interactive, select_optional};

mod render;

#[derive(Debug, Clone)]
pub struct RepoLatestArgs {
    pub workspace: Option<String>,
    pub r#continue: bool,
    pub only: Option<String>,
    pub root: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct CommitArgs {
    pub workspace: Option<String>,
    pub r#continue: bool,
    pub root: Option<String>,
    pub execute: bool,
    pub message: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct AddRepoArgs {
    pub repo: Option<String>,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub execute: bool,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct TeardownArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub execute: bool,
    pub yes: bool,
    pub json: bool,
}

pub fn repo_latest(args: RepoLatestArgs) -> Result<()> {
    let RepoLatestArgs {
        workspace,
        r#continue,
        only,
        root,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        workspace.as_deref(),
        r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (manifest, targets) = plan_task_repo_latest(&root, &projects, &workspace, only.as_deref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&targets)?);
    } else {
        print_styled_lines(&repo_latest_header_lines(&workspace, &manifest.branch_name));
        for target in &targets {
            print_styled(&format!(
                "Repository {}: synchronisation depuis la branche par défaut...",
                target.repository
            ));
            update_repository(&target.repository_path, &target.default_branch)?;
        }
        print_styled("Repos synchronisés avec la remote.");
    }
    Ok(())
}

pub fn commit(args: CommitArgs) -> Result<()> {
    let CommitArgs {
        workspace,
        r#continue,
        root,
        execute,
        message,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        workspace.as_deref(),
        r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (manifest, targets) = plan_task_commit(&projects, &workspace)?;
    let statuses = targets
        .iter()
        .map(|target| (target, repository_status(&target.path)))
        .collect::<Vec<_>>();
    let changed = statuses
        .iter()
        .filter(|(_, status)| status.is_git_repository && status.has_changes)
        .collect::<Vec<_>>();
    let commit_message = build_commit_message(&manifest, message.as_deref());

    if json {
        let report = serde_json::json!({
            "workspace": workspace,
            "branch": manifest.branch_name,
            "message": commit_message,
            "targets": statuses.iter().map(|(target, status)| serde_json::json!({
                "repository": target.repository,
                "path": status.path,
                "isGitRepository": status.is_git_repository,
                "hasChanges": status.has_changes,
                "hasUnpushed": status.has_unpushed,
                "detail": status.detail,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_styled_lines(&commit_status_lines(
            &workspace,
            &manifest.branch_name,
            &statuses,
            &commit_message,
            changed.is_empty(),
            execute,
        ));
    }

    if changed.is_empty() || !execute {
        return Ok(());
    }

    for (target, _) in changed {
        commit_repository(&target.path, &commit_message)?;
    }
    if !json {
        print_styled("Commits terminés. Aucun push ni PR créée.");
    }
    Ok(())
}

pub fn add_repo(args: AddRepoArgs) -> Result<()> {
    let AddRepoArgs {
        repo,
        workspace,
        root,
        execute,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        workspace.as_deref(),
        false,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let repo = resolve_add_repo_selection(repo, &projects, &workspace)?;
    let (manifest, plan) = plan_task_add_repo(&root, &projects, &workspace, &repo)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_styled_lines(&add_repo_plan_lines(&plan));
    }

    if !execute {
        return Ok(());
    }

    let result = prepare_worktree(&WorktreePrepareRequest {
        project_root: plan.project_root.clone(),
        repository: plan.repository.clone(),
        url: plan.url.clone(),
        default_branch: plan.default_branch.clone(),
        anchor_name: plan.anchor_name.clone(),
        branch_name: plan.branch_name.clone(),
        worktree_path: plan.worktree_path.clone(),
    })?;
    let updated = execute_task_add_repo(&manifest, &plan)?;
    write_workspace_agent_configs(&workspace, &updated)?;
    if !json {
        print_styled(&format!(
            "Repository ajouté: {} - {} - {}",
            result.repository, result.status, result.message
        ));
    }
    Ok(())
}

fn resolve_add_repo_selection(
    repo: Option<String>,
    projects: &dw_config::ProjectsConfig,
    workspace: &str,
) -> Result<String> {
    if let Some(repo) = repo.filter(|value| !value.trim().is_empty()) {
        return Ok(repo);
    }
    if !is_stdin_interactive() {
        return Err(anyhow::anyhow!(
            "Repository manquant. Fournir `dw task add-repo <repo>`."
        ));
    }

    let manifest = dw_workspace::read_manifest_path(&format!("{workspace}/task.json"))?;
    let choices = add_repo_choices(projects, &manifest);
    select_optional("Repository à ajouter", choices)?
        .ok_or_else(|| anyhow::anyhow!("Aucun repository configuré à ajouter."))
}

fn add_repo_choices(
    projects: &dw_config::ProjectsConfig,
    manifest: &dw_workspace::WorkspaceManifest,
) -> Vec<String> {
    resolve_project(projects, &manifest.project)
        .map(|project| {
            project
                .repositories
                .keys()
                .filter(|repository| {
                    !manifest
                        .repositories
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(repository))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn teardown(args: TeardownArgs) -> Result<()> {
    let TeardownArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
        positional_work_item,
        execute,
        yes,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace(
        &root,
        workspace.as_deref(),
        project.as_deref(),
        work_item.as_deref(),
        positional_work_item.as_deref(),
        r#continue,
    )?;
    let projects = load_projects_config(&root);
    let (_manifest, steps) = plan_task_teardown(&root, &projects, &workspace)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&steps)?);
    } else {
        print_styled_lines(&teardown_plan_lines(&workspace, &steps, execute));
    }

    if !execute {
        return Ok(());
    }

    if !yes
        && !confirm_or_require_flag(
            "--yes",
            &format!("Supprimer ce workspace et ses worktrees ?\n{workspace}"),
        )?
    {
        print_styled("Suppression annulée.");
        return Ok(());
    }

    execute_task_teardown(&workspace, &steps, |git_dir, args| match args {
        ["worktree", "remove", "--force", target] => {
            worktree_remove(git_dir, target).map_err(|error| error.to_string())
        }
        ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
        _ => Err(format!("commande git non supportée: {}", args.join(" "))),
    })?;
    if !json {
        print_styled(&format!("Workspace supprimé: {workspace}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::add_repo_choices;

    #[test]
    fn add_repo_choices_hide_repositories_already_in_workspace() {
        let projects: dw_config::ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": { "url": "", "defaultBranch": "main" },
        "back": { "url": "", "defaultBranch": "main" },
        "db": { "url": "", "defaultBranch": "main" }
      }
    }
  }
}"#,
        )
        .expect("projects config should parse");
        let manifest = dw_workspace::WorkspaceManifest {
            schema: 1,
            work_item_id: "42".into(),
            task_id: None,
            project: "ha".into(),
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/42-demo".into(),
            created_at: "2026-07-03T10:00:00Z".into(),
            repositories: vec!["front".into()],
            status: "created".into(),
            work_item_type: None,
            work_item_title: None,
            work_item_state: None,
            child_task_ids: None,
            child_tasks: None,
            work_items: None,
        };

        assert_eq!(add_repo_choices(&projects, &manifest), vec!["back", "db"]);
    }
}
