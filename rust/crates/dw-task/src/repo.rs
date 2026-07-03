use crate::write_workspace_agent_configs;
use anyhow::Result;
use dw_config::{load_projects_config, resolve_root};
use dw_git::{
    RepositoryStatus, WorktreePrepareRequest, commit_repository, prepare_worktree,
    repository_status, update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    build_commit_message, execute_task_add_repo, execute_task_teardown, plan_task_add_repo,
    plan_task_commit, plan_task_repo_latest, plan_task_teardown, resolve_workspace,
    resolve_workspace_for_workspace_command,
};

use crate::render::{print_styled, print_styled_lines};

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
        print_styled_lines(&repo_latest_header_lines(&workspace, &manifest.branch_name));
        for target in &targets {
            print_styled(&format!("Repo {}: sync latest...", target.repository));
            update_repository(&target.repository_path, &target.default_branch)?;
        }
        print_styled("Repos synchronises avec la remote.");
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
        ));
    }

    if changed.is_empty() || !execute {
        if !changed.is_empty() && !json {
            print_styled("Dry-run uniquement. Relancer avec --execute pour committer.");
        }
        return Ok(());
    }

    for (target, _) in changed {
        commit_repository(&target.path, &commit_message)?;
    }
    if !json {
        print_styled("Commits termines. Aucun push ni PR creee.");
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
        print_styled_lines(&add_repo_plan_lines(&plan));
    }

    if !execute {
        if !json {
            print_styled("Relancer avec --execute pour appliquer.");
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
        print_styled(&format!(
            "Repo ajoute: {} - {} - {}",
            result.repository, result.status, result.message
        ));
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
        print_styled_lines(&teardown_plan_lines(&workspace, &steps, execute));
    }

    if !execute {
        if !json {
            print_styled("");
            print_styled(
                "Dry-run uniquement. Relancer avec --execute --yes pour supprimer les worktrees et le workspace.",
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
        print_styled(&format!("Workspace supprime: {workspace}"));
    }
    Ok(())
}

fn repo_latest_header_lines(workspace: &str, branch_name: &str) -> Vec<String> {
    vec![
        format!("Workspace: {workspace}"),
        format!("Branche: {branch_name}"),
    ]
}

fn commit_status_lines(
    workspace: &str,
    branch_name: &str,
    statuses: &[(&dw_workspace::TaskCommitTarget, RepositoryStatus)],
    commit_message: &str,
    nothing_to_commit: bool,
) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace: {workspace}"),
        format!("Branche: {branch_name}"),
    ];

    for (target, status) in statuses {
        lines.push(String::new());
        lines.push(format!("[{}] {}", target.repository, status.path));
        lines.push(repository_status_label(status).into());
        if !status.detail.trim().is_empty() {
            lines.push(status.detail.clone());
        }
    }

    lines.push(String::new());
    if nothing_to_commit {
        lines.push("Rien a committer.".into());
    } else {
        lines.push(format!("Message: {commit_message}"));
    }
    lines
}

fn repository_status_label(status: &RepositoryStatus) -> &'static str {
    if !status.is_git_repository {
        "Pas un repo Git utilisable."
    } else if status.has_changes {
        "Changements detectes:"
    } else if status.has_unpushed {
        "Commits non pousses."
    } else {
        "Aucun changement."
    }
}

fn add_repo_plan_lines(plan: &dw_workspace::TaskAddRepoPlan) -> Vec<String> {
    vec![
        "Add repo dry-run:".into(),
        format!("- workspace: {}", plan.workspace),
        format!("- repo: {}", plan.repository),
        format!("- worktree: {}", plan.worktree_path),
        format!("- branche: {}", plan.branch_name),
        format!(
            "- anchor: {}/repositories/{}",
            plan.project_root, plan.anchor_name
        ),
    ]
}

fn teardown_plan_lines(
    workspace: &str,
    steps: &[dw_workspace::WorkspaceTeardownStep],
    execute: bool,
) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace: {workspace}"),
        if execute {
            "Teardown execute:".into()
        } else {
            "Teardown dry-run:".into()
        },
    ];
    for step in steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.repository, step.action, step.target
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_status_label_prioritizes_git_state() {
        assert_eq!(
            repository_status_label(&RepositoryStatus {
                path: "/tmp/not-git".into(),
                is_git_repository: false,
                has_changes: true,
                has_unpushed: true,
                detail: String::new(),
            }),
            "Pas un repo Git utilisable."
        );
        assert_eq!(
            repository_status_label(&RepositoryStatus {
                path: "/tmp/repo".into(),
                is_git_repository: true,
                has_changes: true,
                has_unpushed: false,
                detail: String::new(),
            }),
            "Changements detectes:"
        );
    }

    #[test]
    fn add_repo_plan_lines_include_anchor() {
        let plan = dw_workspace::TaskAddRepoPlan {
            workspace: "/tmp/ws".into(),
            repository: "front".into(),
            project_root: "/tmp/project".into(),
            worktree_path: "/tmp/ws/front".into(),
            url: "https://example.invalid/front.git".into(),
            default_branch: "main".into(),
            anchor_name: "front-anchor".into(),
            branch_name: "feat/42-demo".into(),
            repositories: vec!["front".into()],
        };

        let lines = add_repo_plan_lines(&plan);

        assert_eq!(lines[0], "Add repo dry-run:");
        assert!(lines.contains(&"- anchor: /tmp/project/repositories/front-anchor".into()));
    }

    #[test]
    fn teardown_plan_lines_switch_title_for_execute() {
        let steps = vec![dw_workspace::WorkspaceTeardownStep {
            repository: "front".into(),
            action: "remove-worktree".into(),
            target: "/tmp/ws/front".into(),
            git_dir: Some("/tmp/project/repositories/front/.git".into()),
        }];

        let dry_run = teardown_plan_lines("/tmp/ws", &steps, false);
        let execute = teardown_plan_lines("/tmp/ws", &steps, true);

        assert_eq!(dry_run[1], "Teardown dry-run:");
        assert_eq!(execute[1], "Teardown execute:");
        assert!(dry_run.contains(&"- [front] remove-worktree: /tmp/ws/front".into()));
    }
}
