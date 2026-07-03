use crate::write_workspace_agent_configs;
use anyhow::Result;
use dw_config::{load_projects_config, resolve_root};
use dw_git::{
    WorktreePrepareRequest, commit_repository, prepare_worktree, repository_status,
    update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    build_commit_message, execute_task_add_repo, execute_task_teardown, plan_task_add_repo,
    plan_task_commit, plan_task_repo_latest, plan_task_teardown, resolve_workspace,
    resolve_workspace_for_workspace_command,
};

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
    pub repo: String,
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
        println!("Workspace: {}", workspace);
        println!("Branche: {}", manifest.branch_name);
        for target in &targets {
            println!("Repo {}: sync latest...", target.repository);
            update_repository(&target.repository_path, &target.default_branch)?;
        }
        println!("Repos synchronises avec la remote.");
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
        println!("Workspace: {workspace}");
        println!("Branche: {}", manifest.branch_name);
        for (target, status) in &statuses {
            println!();
            println!("[{}] {}", target.repository, status.path);
            if !status.is_git_repository {
                println!("Pas un repo Git utilisable.");
            } else if status.has_changes {
                println!("Changements detectes:");
            } else if status.has_unpushed {
                println!("Commits non pousses.");
            } else {
                println!("Aucun changement.");
            }
            if !status.detail.trim().is_empty() {
                println!("{}", status.detail);
            }
        }
        if changed.is_empty() {
            println!();
            println!("Rien a committer.");
        } else {
            println!();
            println!("Message: {commit_message}");
        }
    }

    if changed.is_empty() || !execute {
        if !changed.is_empty() && !json {
            println!("Dry-run uniquement. Relancer avec --execute pour committer.");
        }
        return Ok(());
    }

    for (target, _) in changed {
        commit_repository(&target.path, &commit_message)?;
    }
    if !json {
        println!("Commits termines. Aucun push ni PR creee.");
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
    let (manifest, plan) = plan_task_add_repo(&root, &projects, &workspace, &repo)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("Add repo dry-run:");
        println!("- workspace: {}", plan.workspace);
        println!("- repo: {}", plan.repository);
        println!("- worktree: {}", plan.worktree_path);
        println!("- branche: {}", plan.branch_name);
        println!(
            "- anchor: {}/repositories/{}",
            plan.project_root, plan.anchor_name
        );
    }

    if !execute {
        if !json {
            println!("Relancer avec --execute pour appliquer.");
        }
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
        println!(
            "Repo ajoute: {} - {} - {}",
            result.repository, result.status, result.message
        );
    }
    Ok(())
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
        println!("Workspace: {workspace}");
        println!(
            "{}",
            if execute {
                "Teardown execute:"
            } else {
                "Teardown dry-run:"
            }
        );
        for step in &steps {
            println!("- [{}] {}: {}", step.repository, step.action, step.target);
        }
    }

    if !execute {
        if !json {
            println!();
            println!(
                "Dry-run uniquement. Relancer avec --execute --yes pour supprimer les worktrees et le workspace."
            );
        }
        return Ok(());
    }

    if !yes {
        return Err(anyhow::anyhow!(
            "Suppression destructive refusee: ajouter --yes avec --execute."
        ));
    }

    execute_task_teardown(&workspace, &steps, |git_dir, args| match args {
        ["worktree", "remove", "--force", target] => {
            worktree_remove(git_dir, target).map_err(|error| error.to_string())
        }
        ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
        _ => Err(format!("commande git non supportee: {}", args.join(" "))),
    })?;
    if !json {
        println!("Workspace supprime: {workspace}");
    }
    Ok(())
}
