use dw_git::RepositoryStatus;

pub(super) fn repo_latest_header_lines(workspace: &str, branch_name: &str) -> Vec<String> {
    vec![
        "Repo latest".into(),
        format!("Workspace : {workspace}"),
        format!("Branche   : {branch_name}"),
    ]
}

pub(super) fn commit_status_lines(
    workspace: &str,
    branch_name: &str,
    statuses: &[(&dw_workspace::TaskCommitTarget, RepositoryStatus)],
    commit_message: &str,
    nothing_to_commit: bool,
    execute: bool,
) -> Vec<String> {
    let mut lines = vec![
        "Commit workspace".into(),
        format!("Workspace : {workspace}"),
        format!("Branche   : {branch_name}"),
    ];

    for (target, status) in statuses {
        lines.push(String::new());
        lines.push(format!("Repo      : {}", target.repository));
        lines.push(format!("Path      : {}", status.path));
        lines.push(format!("Statut    : {}", repository_status_label(status)));
        if !status.detail.trim().is_empty() {
            lines.push(status.detail.clone());
        }
    }

    lines.push(String::new());
    if nothing_to_commit {
        lines.push("Rien à committer.".into());
    } else {
        lines.push(format!("Message   : {commit_message}"));
        if !execute {
            lines.push("À faire   : dw task commit --execute".into());
        }
    }
    lines
}

pub(super) fn add_repo_plan_lines(plan: &dw_workspace::TaskAddRepoPlan) -> Vec<String> {
    vec![
        "Ajout repo (prévisualisation)".into(),
        format!("Workspace : {}", plan.workspace),
        format!("Repo      : {}", plan.repository),
        format!("Worktree  : {}", plan.worktree_path),
        format!("Branche   : {}", plan.branch_name),
        format!(
            "Anchor    : {}/repositories/{}",
            plan.project_root, plan.anchor_name
        ),
        format!("À faire   : dw task add-repo {} --execute", plan.repository),
    ]
}

pub(super) fn teardown_plan_lines(
    workspace: &str,
    steps: &[dw_workspace::WorkspaceTeardownStep],
    execute: bool,
) -> Vec<String> {
    let mut lines = vec![
        if execute {
            "Teardown exécuté".into()
        } else {
            "Teardown (prévisualisation)".into()
        },
        format!("Workspace : {workspace}"),
        if execute {
            "Actions appliquées".into()
        } else {
            "Actions prévues".into()
        },
    ];
    for step in steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.repository, step.action, step.target
        ));
    }
    if !execute {
        lines.push(String::new());
        lines.push("À faire   : dw task teardown --execute --yes".into());
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

        assert_eq!(lines[0], "Ajout repo (prévisualisation)");
        assert!(lines.contains(&"Anchor    : /tmp/project/repositories/front-anchor".into()));
        assert!(lines.contains(&"À faire   : dw task add-repo front --execute".into()));
    }

    #[test]
    fn commit_status_lines_render_aligned_summary() {
        let target = dw_workspace::TaskCommitTarget {
            repository: "front".into(),
            path: "/tmp/repo".into(),
        };
        let status = RepositoryStatus {
            path: "/tmp/repo".into(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: " M src/lib.rs".into(),
        };
        let statuses = vec![(&target, status)];

        let lines = commit_status_lines(
            "/tmp/ws",
            "feat/42-demo",
            &statuses,
            "feat(42): demo",
            false,
            false,
        );

        assert_eq!(lines[0], "Commit workspace");
        assert!(lines.contains(&"Workspace : /tmp/ws".into()));
        assert!(lines.contains(&"Repo      : front".into()));
        assert!(lines.contains(&"Path      : /tmp/repo".into()));
        assert!(lines.contains(&"Statut    : Changements détectés:".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"À faire   : dw task commit --execute".into()));
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

        assert_eq!(dry_run[0], "Teardown (prévisualisation)");
        assert_eq!(dry_run[2], "Actions prévues");
        assert_eq!(execute[0], "Teardown exécuté");
        assert_eq!(execute[2], "Actions appliquées");
        assert!(dry_run.contains(&"- [front] remove-worktree: /tmp/ws/front".into()));
        assert!(dry_run.contains(&"À faire   : dw task teardown --execute --yes".into()));
        assert!(!execute.contains(&"À faire   : dw task teardown --execute --yes".into()));
    }
}
