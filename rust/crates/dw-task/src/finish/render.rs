pub(super) struct FinishSummary<'a> {
    pub(super) workspace: &'a str,
    pub(super) branch_name: &'a str,
    pub(super) statuses: &'a [(&'a dw_workspace::TaskCommitTarget, dw_git::RepositoryStatus)],
    pub(super) handoff: &'a dw_contracts::TaskHandoffValidationReport,
    pub(super) handoff_summaries: &'a [dw_workspace::WorkspaceHandoffSummary],
    pub(super) commit_message: &'a str,
    pub(super) has_changes: bool,
    pub(super) create_pr: bool,
    pub(super) pull_request_candidates: &'a [dw_workspace::PullRequestCandidate],
}

pub(super) fn finish_summary_lines(summary: FinishSummary<'_>) -> Vec<String> {
    let mut lines = vec![
        "Finish workspace".into(),
        format!("Workspace : {}", summary.workspace),
        format!("Branche   : {}", summary.branch_name),
    ];

    for (target, status) in summary.statuses {
        lines.extend(repository_status_lines(&target.repository, status));
    }

    lines.push(String::new());
    lines.push("Handoff validation".into());
    lines.push(format!(
        "Statut    : {}",
        if summary.handoff.is_valid { "OK" } else { "KO" }
    ));
    for item in &summary.handoff.items {
        lines.push(format!(
            "- [{}] {} - {}",
            item.status, item.repository, item.message
        ));
    }
    for handoff_summary in summary.handoff_summaries {
        lines.extend(handoff_summary_lines(handoff_summary));
    }
    if summary.has_changes {
        lines.push(String::new());
        lines.push("Commit à créer".into());
        lines.push(format!("Message   : {}", summary.commit_message));
    }
    if summary.create_pr {
        lines.push(String::new());
        lines.push("Pull requests".into());
        if summary.pull_request_candidates.is_empty() {
            lines.push("Aucun dépôt candidat détecté.".into());
        } else {
            for candidate in summary.pull_request_candidates {
                lines.push(format!(
                    "- {} -> {}",
                    candidate.repository, candidate.target_branch
                ));
            }
        }
    }

    lines
}

fn handoff_summary_lines(summary: &dw_workspace::WorkspaceHandoffSummary) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("Handoff {}", summary.repository),
        format!("Statut    : {}", summary.status),
    ];
    push_summary_list(&mut lines, "Fait      ", &summary.done);
    push_summary_list(&mut lines, "Décisions ", &summary.decisions);
    push_summary_list(&mut lines, "Risques   ", &summary.risks);
    push_summary_list(&mut lines, "Blockers  ", &summary.blockers);
    push_summary_list(&mut lines, "Follow-up ", &summary.follow_up);
    lines
}

fn push_summary_list(lines: &mut Vec<String>, label: &str, items: &[String]) {
    if !items.is_empty() {
        lines.push(format!("{label}: {}", items.join(" | ")));
    }
}

pub(super) fn finish_dry_run_hint(no_changes: bool, create_pr: bool) -> &'static str {
    if create_pr {
        "Prévisualisation uniquement. Relancer avec --execute pour pousser/créer PR."
    } else if no_changes {
        "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour pousser."
    } else {
        "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour committer/pousser."
    }
}

fn repository_status_lines(repository: &str, status: &dw_git::RepositoryStatus) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("Repo      : {repository}"),
        format!("Path      : {}", status.path),
        format!("Statut    : {}", repository_status_label(status)),
    ];
    if !status.detail.trim().is_empty() {
        lines.push(status.detail.clone());
    }
    lines
}

fn repository_status_label(status: &dw_git::RepositoryStatus) -> &'static str {
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
    use dw_contracts::{
        HANDOFF_VALIDATION_VERSION, TaskHandoffValidationItem, TaskHandoffValidationReport,
    };

    #[test]
    fn finish_dry_run_hint_matches_action() {
        assert_eq!(
            finish_dry_run_hint(false, false),
            "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour committer/pousser."
        );
        assert_eq!(
            finish_dry_run_hint(true, false),
            "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour pousser."
        );
        assert_eq!(
            finish_dry_run_hint(false, true),
            "Prévisualisation uniquement. Relancer avec --execute pour pousser/créer PR."
        );
    }

    #[test]
    fn repository_status_lines_include_detail_when_present() {
        let status = dw_git::RepositoryStatus {
            path: "/tmp/repo".into(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: " M src/lib.rs".into(),
        };

        let lines = repository_status_lines("front", &status);

        assert!(lines.contains(&"Repo      : front".into()));
        assert!(lines.contains(&"Path      : /tmp/repo".into()));
        assert!(lines.contains(&"Statut    : Changements détectés:".into()));
        assert!(lines.contains(&" M src/lib.rs".into()));
    }

    #[test]
    fn finish_summary_lines_include_handoff_and_pr_candidates() {
        let target = dw_workspace::TaskCommitTarget {
            repository: "front".into(),
            path: "/tmp/repo".into(),
        };
        let status = dw_git::RepositoryStatus {
            path: "/tmp/repo".into(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: String::new(),
        };
        let statuses = vec![(&target, status)];
        let handoff = TaskHandoffValidationReport {
            schema_version: HANDOFF_VALIDATION_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            is_valid: true,
            items: vec![TaskHandoffValidationItem {
                repository: "front".into(),
                path: "/tmp/ws/handoff-front.md".into(),
                status: "done".into(),
                valid: true,
                message: "OK".into(),
                done_count: 1,
                decision_count: 0,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 0,
            }],
        };
        let pull_request_candidates = vec![dw_workspace::PullRequestCandidate {
            repository: "front".into(),
            path: "/tmp/repo".into(),
            ado_repository: Some("front".into()),
            target_branch: "develop".into(),
        }];

        let lines = finish_summary_lines(FinishSummary {
            workspace: "/tmp/ws",
            branch_name: "feat/42-demo",
            statuses: &statuses,
            handoff: &handoff,
            handoff_summaries: &[],
            commit_message: "feat(42): demo",
            has_changes: true,
            create_pr: true,
            pull_request_candidates: &pull_request_candidates,
        });

        assert_eq!(lines[0], "Finish workspace");
        assert!(lines.contains(&"Statut    : OK".into()));
        assert!(lines.contains(&"- [done] front - OK".into()));
        assert!(lines.contains(&"Commit à créer".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"- front -> develop".into()));
    }

    #[test]
    fn finish_summary_lines_include_handoff_summary_details() {
        let handoff = TaskHandoffValidationReport {
            schema_version: HANDOFF_VALIDATION_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            is_valid: true,
            items: Vec::new(),
        };
        let summaries = vec![dw_workspace::WorkspaceHandoffSummary {
            repository: "front".into(),
            status: "done".into(),
            done: vec!["UI ajustée".into()],
            decisions: vec!["Conserver le contrat JSON".into()],
            risks: Vec::new(),
            blockers: Vec::new(),
            follow_up: vec!["Valider en recette".into()],
        }];

        let lines = finish_summary_lines(FinishSummary {
            workspace: "/tmp/ws",
            branch_name: "feat/42-demo",
            statuses: &[],
            handoff: &handoff,
            handoff_summaries: &summaries,
            commit_message: "feat(42): demo",
            has_changes: false,
            create_pr: false,
            pull_request_candidates: &[],
        });

        assert!(lines.contains(&"Handoff front".into()));
        assert!(lines.contains(&"Statut    : done".into()));
        assert!(lines.contains(&"Fait      : UI ajustée".into()));
        assert!(lines.contains(&"Décisions : Conserver le contrat JSON".into()));
        assert!(lines.contains(&"Follow-up : Valider en recette".into()));
    }
}
