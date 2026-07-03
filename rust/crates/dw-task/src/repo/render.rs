use dw_git::RepositoryStatus;

pub(super) fn repo_latest_header_lines(workspace: &str, branch_name: &str) -> Vec<String> {
    vec![
        format!("Workspace: {workspace}"),
        format!("Branche: {branch_name}"),
    ]
}

pub(super) fn commit_status_lines(
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
        lines.push("Rien à committer.".into());
    } else {
        lines.push(format!("Message: {commit_message}"));
    }
    lines
}

pub(super) fn add_repo_plan_lines(plan: &dw_workspace::TaskAddRepoPlan) -> Vec<String> {
    vec![
        "Prévisualisation ajout repo:".into(),
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

pub(super) fn teardown_plan_lines(
    workspace: &str,
    steps: &[dw_workspace::WorkspaceTeardownStep],
    execute: bool,
) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace: {workspace}"),
        if execute {
            "Teardown exécuté:".into()
        } else {
            "Prévisualisation teardown:".into()
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

fn repository_status_label(status: &RepositoryStatus) -> &'static str {
    if !status.is_git_repository {
        "Pas un repo Git utilisable."
    } else if status.has_changes {
        "Changements détectés:"
    } else if status.has_unpushed {
        "Commits non poussés."
    } else {
        "Aucun changement."
    }
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
            "Changements détectés:"
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

        assert_eq!(lines[0], "Prévisualisation ajout repo:");
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

        assert_eq!(dry_run[1], "Prévisualisation teardown:");
        assert_eq!(execute[1], "Teardown exécuté:");
        assert!(dry_run.contains(&"- [front] remove-worktree: /tmp/ws/front".into()));
    }
}
