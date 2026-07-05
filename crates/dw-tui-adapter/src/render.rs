use dw_ado_commands::auth::{
    AuthLoginMode, AuthLoginReport, AuthLogoutReport, AuthStatusReport, expiration_line,
};
use dw_agent::command::{AgentDoctorCheck, AgentDoctorReport};
use dw_config::{ConfigDoctorCheck, ConfigDoctorReport, ConfigShow, InitReport, RefreshReport};
use dw_contracts::{TaskHandoffValidationItem, TaskHandoffValidationReport, TaskPreflightReport};
use dw_db::{QueryResult, SqlGuardResult};
use dw_doctor::{DoctorCheck, DoctorReport};
use dw_secret::command::{SecretDeleteReport, SecretGetReport, SecretSetReport};
use dw_task::open::{TaskListReport, TaskStatusReport};
use dw_ui::TerminalTheme;
use dw_workspace::TaskCurrentItem;

const MAX_DB_CELL_WIDTH: usize = 48;

pub fn auth_login_lines(report: &AuthLoginReport) -> Vec<String> {
    if report.uses_environment_pat {
        return vec![
            "ADO connection".into(),
            "Mode      : PAT from environment".into(),
            "Next      : set DW_ADO_TOKEN or AZURE_DEVOPS_EXT_PAT.".into(),
            "Security  : no secret is read or stored by this action.".into(),
        ];
    }

    vec![
        "ADO connection".into(),
        "Status    : connected".into(),
        format!("Mode      : {}", auth_login_mode_label(report.mode)),
        format!(
            "Source    : {}",
            report.source.as_deref().unwrap_or("unknown source")
        ),
        expiration_line(report.expires_on.as_deref()),
    ]
}

pub fn auth_status_lines(report: &AuthStatusReport) -> Vec<String> {
    if report.connected {
        vec![
            "ADO connection".into(),
            "Status    : connected".into(),
            format!(
                "Source    : {}",
                report.source.as_deref().unwrap_or("unknown source")
            ),
            expiration_line(report.expires_on.as_deref()),
        ]
    } else {
        vec![
            "ADO connection".into(),
            "Status    : not connected".into(),
            "Next      : start ADO authentication or set DW_ADO_TOKEN.".into(),
        ]
    }
}

pub fn auth_logout_lines(report: &AuthLogoutReport) -> Vec<String> {
    vec![
        "ADO connection".into(),
        format!(
            "Sessions  : {}",
            if report.removed_local_session {
                "local session removed"
            } else {
                "no local session"
            }
        ),
        "PAT       : DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT remain managed by the environment.".into(),
    ]
}

fn auth_login_mode_label(mode: AuthLoginMode) -> &'static str {
    match mode {
        AuthLoginMode::Browser => "browser",
        AuthLoginMode::DeviceCode => "device code",
        AuthLoginMode::EnvironmentPat => "environment PAT",
    }
}

pub fn config_show_lines(report: &ConfigShow, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command("Configuration DevWorkflow"),
        format!("Root      : {}", theme.path(&report.root)),
        format!("Color     : {}", theme.bold(&report.color)),
        format!("Settings  : {}", theme.path(&report.settings_path)),
        String::new(),
        "Files".into(),
        config_file_line(
            theme,
            "projects",
            &report.projects_path,
            report.projects_exists,
        ),
        config_file_line(
            theme,
            "workflow",
            &report.workflow_path,
            report.workflow_exists,
        ),
        config_file_line(
            theme,
            "databases",
            &report.databases_path,
            report.databases_exists,
        ),
    ]
}

pub fn config_doctor_lines(report: &ConfigDoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let mut lines = vec![
        theme.command("Diagnostic configuration"),
        format!(
            "Status    : {}",
            if report.passed {
                theme.success("valid")
            } else {
                theme.warning("needs fixes")
            }
        ),
        format!("Root      : {}", theme.path(&report.root)),
        String::new(),
        "Checks".into(),
    ];
    for check in &report.checks {
        lines.push(config_check_line(theme, check));
        if let Some(message) = &check.message {
            lines.push(format!("  Detail  : {message}"));
        }
    }
    lines.push(String::new());
    lines.push(if report.passed {
        format!("Result    : {}", theme.success("Configuration is valid."))
    } else {
        format!(
            "Result    : {}",
            theme.warning(
                "Configuration is incomplete. Fix reported points, then run diagnostics again."
            )
        )
    });
    lines
}

pub fn init_report_lines(report: &InitReport) -> Vec<String> {
    if report.dry_run {
        let mut lines = vec![
            format!("DevWorkflow init preview: {}", report.root),
            format!("Profile: {}", report.profile),
        ];
        lines.extend(
            report
                .planned_paths
                .iter()
                .map(|path| format!("  + create/update: {path}")),
        );
        lines.push(if report.no_save {
            "  - user settings unchanged.".into()
        } else {
            format!("  + save user root: {}", report.root)
        });
        return lines;
    }

    let mut lines = vec![
        format!("DevWorkflow root initialized: {}", report.root),
        format!("Profile: {}", report.profile),
    ];
    if report.no_save {
        lines.push("User settings unchanged.".into());
    }
    lines.push("Suggested next step: global diagnostics".into());
    lines
}

pub fn refresh_report_lines(report: &RefreshReport) -> Vec<String> {
    vec![
        format!("Root refreshed: {}", report.root),
        format!("Profile: {}", report.profile),
        "Schemas and agent contexts regenerated.".into(),
        "User files preserved: projects.json, workflow.json, databases.json, plan.md.".into(),
    ]
}

pub fn agent_doctor_lines(report: &AgentDoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let available_count = report.available_count();
    let total_count = report.total_count();
    let mut lines = vec![
        theme.command("Agent doctor"),
        format!(
            "{} {available_count}/{total_count} agents available",
            if available_count == total_count {
                theme.success("✓")
            } else {
                theme.warning("!")
            }
        ),
        String::new(),
    ];
    for check in &report.checks {
        lines.extend(agent_check_lines(check, theme));
    }
    lines
}

pub fn agent_config_lines(root: &str, agent: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command("Agent config"),
        format!("Default agent: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
}

pub fn agent_config_updated_lines(root: &str, agent: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.success("✓ Agent config updated"),
        format!("Default agent: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
}

pub fn ado_prs_lines(report: &dw_ado_commands::commands::prs::PrsReport) -> Vec<String> {
    if report.items.is_empty() {
        return vec![format!("No active PR for {}.", report.project)];
    }

    let mut lines = vec![format!("Active PRs · {}", report.project)];
    for item in &report.items {
        let work_items = if item.work_item_ids.is_empty() {
            "-".into()
        } else {
            item.work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let draft = if item.is_draft { " draft" } else { "" };
        lines.push(format!(
            "#{:<7} {:<24} {:<8} {} -> {}{}",
            item.pull_request_id,
            item.repository,
            work_items,
            trim_ref(item.source_ref_name.as_deref()),
            trim_ref(item.target_ref_name.as_deref()),
            draft
        ));
        if let Some(title) = item
            .title
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(format!("          {title}"));
        }
    }
    lines
}

pub fn ado_assigned_lines(
    report: &dw_ado_commands::commands::assigned::AssignedReport,
    theme: &TerminalTheme,
) -> Vec<String> {
    if report.items.is_empty() {
        return vec![
            theme.warning(dw_ado_commands::commands::assigned::empty_assigned_message(
                report.include_final_states,
            )),
        ];
    }
    if report.group_by_parent {
        return ado_assigned_group_lines(report, theme);
    }

    let mut lines = vec![
        theme.success("ADO assigned"),
        format!("Items     : {}", report.items.len()),
    ];
    for item in &report.items {
        lines.push(String::new());
        lines.push(format!(
            "Item      : {}",
            ado_work_item_summary(item, theme)
        ));
        lines.push(ado_start_action_line(&item.id, &report.project, theme));
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines)
}

pub fn ado_set_state_execution_lines(
    report: &dw_ado_commands::commands::set_state::SetStateExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "ADO update".into(),
        format!("Project   : {}", report.plan.project),
        format!("State     : {}", report.plan.state),
        format!(
            "Work items: {}",
            report
                .plan
                .ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "{} work item{} moved to `{}`.",
            report.updated.len(),
            if report.updated.len() == 1 { "" } else { "s" },
            report.plan.state
        ),
    ];
    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push("Events".into());
        lines.extend(report.events.iter().map(|event| format!("- {event}")));
    }
    lines
}

pub fn ado_work_item_lines(
    report: &dw_ado_commands::commands::work_item::WorkItemReport,
    theme: &TerminalTheme,
) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, item) in report.items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("---".into());
            lines.push(String::new());
        }
        lines.push("ADO work item".into());
        lines.push(format!(
            "Item      : {}",
            theme.success(&format!("#{}", item.id))
        ));
        lines.push(format!(
            "Type      : {}",
            item.kind.as_deref().unwrap_or("unknown type")
        ));
        lines.push(format!(
            "State     : {}",
            item.state.as_deref().unwrap_or("unknown state")
        ));
        lines.push(format!(
            "Title     : {}",
            item.title.as_deref().unwrap_or("(untitled)")
        ));
        lines.push(String::new());
        lines.push(format!(
            "Context   : action available for #{} ({})",
            item.id, report.project
        ));
    }
    lines
}

pub fn ado_context_lines(
    report: &dw_ado_commands::commands::context::ContextReport,
    theme: &TerminalTheme,
) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, item) in report.items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("---".into());
            lines.push(String::new());
        }

        lines.push("ADO context".into());
        lines.extend(ado_context_header(item, theme));
        lines.push(format!(
            "Assigned  : {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("unassigned")
        ));
        if let Some(metadata) = ado_context_metadata(item)
            && !metadata.is_empty()
        {
            lines.push(format!("Metadata : {metadata}"));
        }

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            lines.push(String::new());
            lines.push(theme.bold("Description"));
            lines.push(description.trim().into());
        }

        if let Some(acceptance_criteria) = &item.content.acceptance_criteria
            && !acceptance_criteria.trim().is_empty()
        {
            lines.push(String::new());
            lines.push(theme.bold("Acceptance criteria"));
            lines.push(acceptance_criteria.trim().into());
        }

        if !item.attachments.items.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold(&format!("Attachments ({})", item.attachments.items.len())));
            lines.push(format!("Folder    : {}", item.attachments.directory_hint));
            for attachment in &item.attachments.items {
                lines.push(format!(
                    "- {}",
                    attachment
                        .name
                        .as_deref()
                        .or(attachment.url.as_deref())
                        .unwrap_or("unnamed attachment")
                ));
            }
        }

        if !item.relations.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Relations"));
            for relation in &item.relations {
                lines.push(format!("- {}", ado_relation_display(relation)));
            }
        }

        if report.comments != 0 && !item.comments.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Comments"));
            for comment in item.comments.iter().take(report.comments.max(0) as usize) {
                lines.push(format!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("unknown"),
                    comment.text.as_deref().unwrap_or("").trim()
                ));
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "AI context: action available for #{} ({})",
            item.work_item.id, report.project
        ));
    }
    lines
}

pub fn ado_changelog_lines(
    report: &dw_ado_commands::commands::changelog::ChangelogReport,
    theme: &TerminalTheme,
) -> Vec<String> {
    if report.source_empty {
        return vec![theme.warning(if report.from_git {
            "No work item detected in commit messages from the git range."
        } else {
            "No work item detected for the given pull requests."
        })];
    }
    if report.ids_only {
        return vec![report.work_item_ids.join(" ")];
    }
    if report.resolved_empty {
        return vec![theme.warning("No work item resolved in Azure DevOps.")];
    }
    let format = changelog_format_from_name(&report.format);
    let document = if report.group_by_parent {
        dw_ado::render_grouped_changelog(&report.groups, format, &report.options, report.table)
    } else {
        dw_ado::render_flat_changelog(&report.items, format, &report.options, report.table)
    };
    if report.format == "raw" {
        document
            .lines()
            .map(|line| render_raw_changelog_line(line, theme))
            .collect()
    } else {
        document.lines().map(str::to_owned).collect()
    }
}

pub fn db_guard_lines(result: &SqlGuardResult, theme: &TerminalTheme) -> Vec<String> {
    render_sql_guard(result, theme)
        .lines()
        .map(str::to_owned)
        .collect()
}

pub fn db_query_table(result: &QueryResult, theme: &TerminalTheme) -> String {
    render_query_result_table(result, theme)
}

pub fn upgrade_report_lines(report: &dw_upgrade::UpgradeReport) -> Vec<String> {
    match report {
        dw_upgrade::UpgradeReport::Check(report) => {
            let mut lines = vec![
                "Upgrade".into(),
                format!("Release  : {}", report.release_tag),
                format!("Version  : {}+{}", report.version, report.commit),
                format!("Artifacts: {}", report.assets.len()),
            ];
            lines.extend(
                report.assets.iter().map(|asset| {
                    format!("- {:14} {} {}", asset.rid, asset.file_name, asset.sha256)
                }),
            );
            lines
        }
        dw_upgrade::UpgradeReport::Installed(report) => {
            let status = if report.deferred_windows_replacement {
                "replacement scheduled"
            } else {
                "done"
            };
            vec![
                "Upgrade".into(),
                format!("Status   : {status}"),
                format!("Version  : {}+{}", report.version, report.commit),
                format!("Binary   : {}", report.executable_path),
            ]
        }
    }
}

pub fn task_status_lines(report: &TaskStatusReport) -> Vec<String> {
    let mut lines = vec![
        "Workspaces task".into(),
        format!("Root      : {}", report.root),
        format!("Detected  : {}", report.items.len()),
    ];

    if report.items.is_empty() {
        lines.push("No task workspace found.".into());
        return lines;
    }

    lines.push("Details".into());
    for item in &report.items {
        lines.push(format!(
            "- {} {} {}",
            item.project, item.kind, item.display_work_items
        ));
        lines.push(format!("  Branch      : {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!("  Repositories: {}", item.repositories.join(", ")));
        }
        lines.push(format!("  Path        : {}", item.path));
    }
    lines
}

pub fn task_list_lines(report: &TaskListReport) -> Vec<String> {
    let mut lines = vec![
        format!("Workspaces task: {}", report.items.len()),
        "Project Created     Type   Work items".into(),
    ];

    for item in &report.items {
        lines.push(format!(
            "{:<7} {}  {:<6} {}",
            item.project,
            created_date(&item.created_at),
            item.kind,
            item.display_work_items
        ));
        lines.push(format!("  Branch: {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!("  Repositories: {}", item.repositories.join(", ")));
        }
        lines.push(format!("  Path: {}", item.path));
    }

    lines
}

pub fn task_current_lines(item: &TaskCurrentItem) -> Vec<String> {
    let mut lines = vec![
        "Current workspace".into(),
        format!("Workspace : {}", item.workspace),
        format!("Project   : {}", item.project),
        format!("Branch    : {}", item.branch),
        format!(
            "Work items: {}",
            format_current_work_items(&item.work_items)
        ),
    ];

    if !item.child_tasks.is_empty() || !item.child_task_ids.is_empty() {
        lines.push(format!("Child tasks: {}", format_child_tasks(item)));
    }

    lines.push(format!("Repositories: {}", item.repositories.join(", ")));
    lines
}

pub fn task_preflight_lines(report: &TaskPreflightReport) -> Vec<String> {
    let mut lines = vec![
        "Task preflight".into(),
        format!(
            "Status    : {}",
            validation_status_label(!report.has_blocking_issues)
        ),
        format!("Workspace : {}", report.workspace),
        format!("Project   : {}", report.project),
        format!(
            "Work items: {}",
            report
                .work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
    ];

    if report.issues.is_empty() {
        lines.push("✓ No warning or blocker detected.".into());
        return lines;
    }

    let blocking_count = report
        .issues
        .iter()
        .filter(|issue| is_blocking_severity(&issue.severity))
        .count();
    let warning_count = report
        .issues
        .iter()
        .filter(|issue| is_warning_severity(&issue.severity))
        .count();
    let other_count = report
        .issues
        .len()
        .saturating_sub(blocking_count + warning_count);
    lines.push(format!("Blockers  : {blocking_count}"));
    lines.push(format!("Warnings  : {warning_count}"));
    lines.push(format!("Infos     : {other_count}"));
    lines.push(String::new());
    push_preflight_issue_group(&mut lines, "Blockers", report, is_blocking_severity);
    push_preflight_issue_group(&mut lines, "Warnings", report, is_warning_severity);
    push_preflight_issue_group(&mut lines, "Infos", report, |severity| {
        !is_blocking_severity(severity) && !is_warning_severity(severity)
    });

    if report.has_blocking_issues {
        lines.push(String::new());
        lines.push(
            "Blockers detected: require user confirmation before forcing implementation.".into(),
        );
    }

    lines
}

pub fn task_handoff_validation_lines(report: &TaskHandoffValidationReport) -> Vec<String> {
    let mut lines = vec![
        "Validation handoff".into(),
        format!("Status    : {}", validation_status_label(report.is_valid)),
        format!("Workspace : {}", report.workspace),
        format!("Project   : {}", report.project),
        format!(
            "Handoffs  : {}/{} valid",
            report.items.iter().filter(|item| item.valid).count(),
            report.items.len()
        ),
        String::new(),
    ];

    push_handoff_group(&mut lines, "Needs fixes", report, |item| !item.valid);
    push_handoff_group(&mut lines, "Valid", report, |item| item.valid);

    if !report.is_valid {
        lines.push(String::new());
        lines.push("Handoff validation failed: complete/fix handoffs before task finish.".into());
    }

    lines
}

pub fn task_prune_plan_lines(report: &dw_task::prune::PrunePlanReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace cleanup".into(),
        "Mode      : preview".into(),
        format!("Root      : {}", report.root),
        format!("Candidates: {}", report.candidates.len()),
        "Action    : enable execution to delete candidates".into(),
        "Confirmation: handled by the TUI before deletion".into(),
    ];

    if !report.sync.is_empty() {
        lines.push(String::new());
        lines.push("ADO synchronization".into());
        for item in &report.sync {
            lines.push(format!(
                "- {} [{}] {}",
                item.workspace,
                prune_sync_status_label(&item.status),
                item.message
            ));
        }
    }

    if report.candidates.is_empty() {
        lines.push(String::new());
        lines.push("No workspace eligible for pruning.".into());
        return lines;
    }

    for candidate in &report.candidates {
        lines.push(String::new());
        lines.push(format!("Workspace : {}", candidate.path));
        lines.push(format!(
            "Items     : {}",
            dw_task::prune::prune_candidate_label(candidate)
        ));
        lines.push(format!(
            "Repositories: {}",
            candidate.manifest.repositories.join(", ")
        ));
    }
    lines
}

pub fn task_prune_execution_lines(report: &dw_task::prune::PruneExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace cleanup".into(),
        "Mode      : execution".into(),
        format!("Root      : {}", report.root),
        format!("Deleted   : {}", report.deleted.len()),
    ];
    for path in &report.deleted {
        lines.push(format!("- {path}"));
    }
    lines
}

pub fn task_repo_latest_plan_lines(report: &dw_task::repo::RepoLatestPlanReport) -> Vec<String> {
    vec![
        "Repository update".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branch    : {}", report.branch_name),
        format!("Repositories: {}", report.targets.len()),
    ]
}

pub fn task_repo_latest_execution_lines(
    report: &dw_task::repo::RepoLatestExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Repository update".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branch    : {}", report.branch_name),
        format!("Synced    : {}", report.updated.len()),
    ];
    for item in &report.updated {
        lines.push(format!(
            "- {} from {} ({})",
            item.repository, item.default_branch, item.path
        ));
    }
    lines
}

pub fn task_commit_plan_lines(
    report: &dw_task::repo::CommitPlanReport,
    execute: bool,
) -> Vec<String> {
    let nothing_to_commit = dw_task::repo::changed_commit_targets(report).is_empty();
    let mut lines = vec![
        "Repository commit".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branch    : {}", report.branch_name),
    ];

    for item in &report.targets {
        lines.push(String::new());
        lines.push(format!("Repository: {}", item.target.repository));
        lines.push(format!("Path      : {}", item.status.path));
        lines.push(format!(
            "Status    : {}",
            repository_status_label(&item.status)
        ));
        if !item.status.detail.trim().is_empty() {
            lines.push(item.status.detail.clone());
        }
    }

    lines.push(String::new());
    if nothing_to_commit {
        lines.push("Nothing to commit.".into());
    } else {
        lines.push(format!("Message   : {}", report.message));
        if !execute {
            lines.push("Action    : enable execution to create commits".into());
        }
    }
    lines
}

pub fn task_commit_execution_lines(report: &dw_task::repo::CommitExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Repository commit".into(),
        "Mode      : execution".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branch    : {}", report.branch_name),
        format!("Message   : {}", report.message),
        format!("Commits   : {}", report.committed.len()),
    ];
    for repository in &report.committed {
        lines.push(format!("- {repository}"));
    }
    if report.committed.is_empty() {
        lines.push("Nothing to commit.".into());
    }
    lines
}

pub fn task_add_repo_plan_lines(report: &dw_task::repo::AddRepoPlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Add repository (preview)".into(),
        format!("Workspace : {}", plan.workspace),
        format!("Repository: {}", plan.repository),
        format!("Worktree  : {}", plan.worktree_path),
        format!("Branch    : {}", plan.branch_name),
        format!(
            "Anchor    : {}/repositories/{}",
            plan.project_root, plan.anchor_name
        ),
        format!("Action    : enable execution to add {}", plan.repository),
    ]
}

pub fn task_add_repo_execution_lines(
    report: &dw_task::repo::AddRepoExecutionReport,
) -> Vec<String> {
    vec![
        "Add repository".into(),
        "Mode      : execution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Repository: {}", report.worktree.repository),
        format!("Status    : {}", report.worktree.status),
        format!("Detail    : {}", report.worktree.message),
    ]
}

pub fn task_teardown_plan_lines(
    report: &dw_task::repo::TeardownPlanReport,
    execute: bool,
) -> Vec<String> {
    let Some(workspace) = &report.workspace else {
        return vec!["No task workspace found.".into()];
    };
    let mut lines = vec![
        if execute {
            "Workspace removal executed".into()
        } else {
            "Workspace removal (preview)".into()
        },
        format!("Workspace : {workspace}"),
        format!("Actions   : {}", report.steps.len()),
        if execute {
            "Applied actions".into()
        } else {
            "Planned actions".into()
        },
    ];
    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.repository, step.action, step.target
        ));
    }
    if !execute {
        lines.push(String::new());
        lines.push("Action    : enable execution to remove the workspace".into());
        lines.push("Confirmation: handled by the TUI before execution".into());
    }
    lines
}

pub fn task_teardown_execution_lines(
    report: &dw_task::repo::TeardownExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Workspace removal".into(),
        "Mode      : execution".into(),
        format!("Workspace : {}", report.workspace),
        format!("Actions   : {}", report.steps.len()),
    ];
    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.repository, step.action, step.target
        ));
    }
    lines.push(format!("Workspace removed: {}", report.workspace));
    lines
}

pub fn task_sync_lines(report: &dw_task::lifecycle::SyncReport) -> Vec<String> {
    let items = report.manifest.parent_work_items();
    let mut lines = vec![
        "Task synchronization".into(),
        format!("Workspace : {}", report.workspace),
        format!("Items     : {}", items.len()),
    ];
    if !items.is_empty() {
        lines.push(String::new());
        lines.push("Work items ADO".into());
    }
    for item in &items {
        lines.push(work_item_line(item));
    }
    lines
}

pub fn task_rename_plan_lines(report: &dw_task::lifecycle::RenamePlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Workspace rename".into(),
        "Mode      : preview".into(),
        format!("Slug      : {} -> {}", plan.old_slug, plan.new_slug),
        format!("Branch    : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        "Action    : enable execution to rename the workspace".into(),
    ]
}

pub fn task_rename_execution_lines(
    report: &dw_task::lifecycle::RenameExecutionReport,
) -> Vec<String> {
    vec![
        "Workspace rename".into(),
        "Mode      : execution".into(),
        format!(
            "Slug      : {} -> {}",
            report.plan.old_slug, report.plan.new_slug
        ),
        format!(
            "Branch    : {} -> {}",
            report.plan.old_branch, report.plan.new_branch
        ),
        format!(
            "Workspace : {} -> {}",
            report.plan.workspace, report.plan.new_workspace
        ),
        format!("Workspace renamed: {}", report.plan.new_workspace),
    ]
}

pub fn task_child_task_lines(report: &dw_task::lifecycle::CreateChildTaskReport) -> Vec<String> {
    vec![
        "ADO child task".into(),
        "Status    : saved in workspace".into(),
        format!("Workspace : {}", report.workspace),
        format!("Repository: {}", report.repository),
        format!("Item      : #{}", report.created.id),
        format!("Title     : {}", report.created.title),
    ]
}

pub fn task_work_item_plan_lines(
    report: &dw_task::work_item::WorkItemUpdatePlanReport,
) -> Vec<String> {
    let Some(plan) = &report.plan else {
        return vec![
            "Work items workspace".into(),
            "Mode      : preview".into(),
            format!("Action    : {}", work_item_action_label(report.action)),
            format!("Workspace : {}", report.workspace),
            "Status    : no change".into(),
            "All requested work items are already present in the workspace.".into(),
        ];
    };
    vec![
        "Work items workspace".into(),
        "Mode      : preview".into(),
        format!("Action    : {}", work_item_action_label(report.action)),
        format!("Branch    : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        format!(
            "Items     : {}",
            plan.work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "Action    : enable execution for {}",
            work_item_action_label(report.action)
        ),
    ]
}

pub fn task_work_item_execution_lines(
    report: &dw_task::work_item::WorkItemUpdateExecutionReport,
) -> Vec<String> {
    vec![
        "Work items workspace".into(),
        "Mode      : execution".into(),
        format!("Action    : {}", work_item_action_label(report.action)),
        format!(
            "Branch    : {} -> {}",
            report.plan.old_branch, report.plan.new_branch
        ),
        format!(
            "Workspace : {} -> {}",
            report.plan.workspace, report.plan.new_workspace
        ),
        format!(
            "Items     : {}",
            report
                .plan
                .work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!("Workspace updated: {}", report.new_workspace),
    ]
}

pub fn task_start_plan_lines(report: &dw_task::start::StartPlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Plan task start".into(),
        format!("Project: {}", plan.project),
        format!("Work items: {}", plan.work_item_ids.join(", ")),
        format!("Slug: {}", plan.slug),
        format!("Target branch: {}", plan.branch_name),
        format!("Target workspace: {}", plan.workspace),
        format!("Repositories: {}", plan.repositories.join(", ")),
        "Action    : enable execution to create the workspace.".into(),
    ]
}

pub fn task_start_execution_lines(report: &dw_task::start::StartExecutionReport) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace created: {}", report.plan.workspace),
        format!("Target branch: {}", report.plan.branch_name),
        format!("Repositories: {}", report.plan.repositories.join(", ")),
    ];
    for task in &report.child_tasks {
        lines.push(format!(
            "ADO task created [{}]: #{} {}",
            task.repository,
            task.id,
            task.title.as_deref().unwrap_or("(untitled)")
        ));
    }
    for update in &report.state_updates {
        if update.changed {
            lines.push(format!(
                "ADO item {}: state -> {}",
                update.label, update.target_state
            ));
        }
    }
    lines.push("Suggested next step: open the workspace or launch the agent.".into());
    lines
}

pub fn task_start_pr_plan_lines(report: &dw_task::start::StartPrPlanReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "PR resolution: #{} in {}",
            report.pull_request_id,
            if report.repositories.is_empty() {
                "no repository".into()
            } else {
                report.repositories.join(", ")
            }
        ),
        dw_task::start::start_pr_resolved_line(&report.work_item_ids),
        String::new(),
    ];
    lines.extend(task_start_plan_lines(&report.start));
    lines
}

pub fn task_finish_plan_lines(report: &dw_task::finish::FinishPlanReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace finish".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branch    : {}", report.manifest.branch_name),
    ];

    for item in &report.targets {
        lines.extend(finish_repository_status_lines(
            &item.target.repository,
            &item.status,
        ));
    }

    lines.push(String::new());
    lines.push("Validation handoff".into());
    lines.push(format!(
        "Status    : {}",
        if report.handoff.is_valid { "OK" } else { "KO" }
    ));
    for item in &report.handoff.items {
        lines.push(format!(
            "- [{}] {} - {}",
            item.status, item.repository, item.message
        ));
    }
    for summary in &report.handoff_summaries {
        lines.extend(finish_handoff_summary_lines(summary));
    }
    if !report.changed_repositories.is_empty() {
        lines.push(String::new());
        lines.push("Commit to create".into());
        lines.push(format!("Message   : {}", report.commit_message));
    }
    if report.create_pr {
        lines.push(String::new());
        lines.push("Pull requests to create".into());
        if report.pull_request_candidates.is_empty() {
            lines.push("No candidate repository detected.".into());
        } else {
            for candidate in &report.pull_request_candidates {
                lines.push(format!(
                    "- {} -> {}",
                    candidate.repository, candidate.target_branch
                ));
            }
        }
    }
    if dw_task::finish::finish_has_work(report) {
        lines.push(String::new());
        lines.push("Action    : enable execution to finish the workspace".into());
        lines.push("Confirmation: handled by the TUI before execution".into());
    }

    lines
}

pub fn task_finish_execution_lines(report: &dw_task::finish::FinishExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace finish".into(),
        "Mode      : execution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Branch    : {}", report.plan.manifest.branch_name),
    ];
    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push("Events".into());
        lines.extend(report.events.iter().map(|event| format!("- {event}")));
    }
    if !report.verification_results.is_empty() {
        lines.push(String::new());
        lines.push("Verification".into());
        for result in &report.verification_results {
            lines.push(format!(
                "- [{}] {} ({})",
                result.repository, result.command, result.exit_code
            ));
        }
    }
    if !report.git_actions.is_empty() {
        lines.push(String::new());
        lines.push("Git".into());
        for action in &report.git_actions {
            lines.push(format!(
                "- {}: {} ({})",
                action.repository, action.action, action.path
            ));
        }
    }
    if !report.pull_requests.is_empty() {
        lines.push(String::new());
        lines.push("Pull requests".into());
        for result in &report.pull_requests {
            lines.push(finish_pull_request_line(result));
        }
    }
    if !report.work_item_updates.is_empty() {
        lines.push(String::new());
        lines.push("Work items ADO".into());
        for update in &report.work_item_updates {
            lines.push(format!("ADO item {}: {}", update.label, update.message));
        }
    }
    if report.git_actions.is_empty()
        && report.pull_requests.is_empty()
        && report.work_item_updates.is_empty()
    {
        lines.push(String::new());
        lines.push("Nothing to finish.".into());
    }
    lines
}

pub fn task_finish_dry_run_hint(no_changes: bool, create_pr: bool) -> &'static str {
    if create_pr {
        "Preview only. Enable execution to push and create the PR."
    } else if no_changes {
        "Preview only. Enable execution to push without ADO update."
    } else {
        "Preview only. Enable execution to commit and push without ADO update."
    }
}

pub fn doctor_report_lines(report: &DoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let passed_count = report.passed_count();
    let total_count = report.checks.len();
    let failed_count = report.failed_count();
    let mut lines = vec![
        theme.command("Diagnostic Dev Workflow"),
        format!(
            "{} {passed_count}/{total_count} checks OK",
            if failed_count == 0 {
                theme.success("✓")
            } else {
                theme.warning("!")
            }
        ),
        format!(
            "Status    : {}",
            if failed_count == 0 {
                "OK"
            } else {
                "needs fixes"
            }
        ),
        format!("Blockers  : {failed_count}"),
        String::new(),
    ];
    lines.extend(render_doctor_check_group(
        "Needs fixes",
        report.checks.iter().filter(|check| !check.passed).collect(),
        theme,
    ));
    lines.extend(render_doctor_check_group(
        "OK",
        report.checks.iter().filter(|check| check.passed).collect(),
        theme,
    ));
    lines
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

fn format_current_work_items(items: &[dw_workspace::WorkspaceWorkItem]) -> String {
    items
        .iter()
        .map(|item| {
            let title = item.title.clone().unwrap_or_else(|| "(untitled)".into());
            let metadata = [item.kind.as_deref(), item.state.as_deref()]
                .into_iter()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>();
            if metadata.is_empty() {
                format!("#{} {}", item.id, title)
            } else {
                format!("#{} {} [{}]", item.id, title, metadata.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_child_tasks(item: &TaskCurrentItem) -> String {
    if !item.child_tasks.is_empty() {
        return item
            .child_tasks
            .iter()
            .map(|task| {
                let title = task.title.clone().unwrap_or_else(|| "(untitled)".into());
                format!("#{} {} ({})", task.id, title, task.repository)
            })
            .collect::<Vec<_>>()
            .join(", ");
    }

    item.child_task_ids
        .iter()
        .map(|(repository, id)| format!("#{id} ({repository})"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn trim_ref(value: Option<&str>) -> &str {
    value
        .unwrap_or("-")
        .strip_prefix("refs/heads/")
        .unwrap_or_else(|| value.unwrap_or("-"))
}

fn finish_repository_status_lines(
    repository: &str,
    status: &dw_git::RepositoryStatus,
) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("Repository: {repository}"),
        format!("Path      : {}", status.path),
        format!("Status    : {}", repository_status_label(status)),
    ];
    if !status.detail.trim().is_empty() {
        lines.push(status.detail.clone());
    }
    lines
}

fn finish_handoff_summary_lines(summary: &dw_workspace::WorkspaceHandoffSummary) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("Handoff {}", summary.repository),
        format!("Status    : {}", summary.status),
    ];
    push_finish_summary_list(&mut lines, "Done      ", &summary.done);
    push_finish_summary_list(&mut lines, "Decisions ", &summary.decisions);
    push_finish_summary_list(&mut lines, "Risks     ", &summary.risks);
    push_finish_summary_list(&mut lines, "Blockers  ", &summary.blockers);
    push_finish_summary_list(&mut lines, "Next      ", &summary.follow_up);
    lines
}

fn push_finish_summary_list(lines: &mut Vec<String>, label: &str, items: &[String]) {
    if !items.is_empty() {
        lines.push(format!("{label}: {}", items.join(" | ")));
    }
}

fn finish_pull_request_line(result: &dw_task::finish::FinishPullRequestResult) -> String {
    let url = result.url.as_deref().unwrap_or("(url not returned)");
    match result.action {
        dw_task::finish::FinishPullRequestAction::Created => {
            format!("PR created for {}: {url}", result.repository)
        }
        dw_task::finish::FinishPullRequestAction::Existing => {
            format!("PR already open for {}: {url}", result.repository)
        }
        dw_task::finish::FinishPullRequestAction::Skipped => format!(
            "PR skipped for {}: {}",
            result.repository,
            result.message.as_deref().unwrap_or("unknown reason")
        ),
    }
}

fn ado_assigned_group_lines(
    report: &dw_ado_commands::commands::assigned::AssignedReport,
    theme: &TerminalTheme,
) -> Vec<String> {
    let total_items = report
        .groups
        .iter()
        .map(|group| 1 + group.items.len())
        .sum::<usize>();
    let mut lines = vec![theme.success(&format!(
        "Assigned work items: {} group(s), {} item(s)",
        report.groups.len(),
        total_items
    ))];
    for group in &report.groups {
        lines.push(String::new());
        lines.push(format!(
            "Parent    : {}",
            ado_work_item_summary(&group.parent, theme)
        ));
        if !group.items.is_empty() {
            lines.push(ado_start_action_line(
                &dw_ado_commands::commands::assigned::suggested_start_ids(
                    &group.parent,
                    &group.items,
                ),
                &report.project,
                theme,
            ));
        }
        for item in &group.items {
            lines.push(format!(
                "  Child   : {}",
                ado_work_item_summary(item, theme)
            ));
        }
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines)
}

fn ado_start_action_line(ids: &str, project: &str, theme: &TerminalTheme) -> String {
    format!(
        "Start     : {}",
        theme.command(&format!("start action for {ids} ({project})"))
    )
}

fn ado_work_item_summary(item: &dw_ado::WorkItemSnapshot, theme: &TerminalTheme) -> String {
    format!(
        "{} {} {}",
        theme.success(&format!("#{}", item.id)),
        theme.dim(&format!(
            "[{} / {}]",
            item.kind.as_deref().unwrap_or("unknown type"),
            item.state.as_deref().unwrap_or("unknown state")
        )),
        item.title.as_deref().unwrap_or("(untitled)")
    )
}

fn trim_trailing_blank_line(mut lines: Vec<String>) -> Vec<String> {
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn ado_context_header(item: &dw_contracts::AdoAiContextItem, theme: &TerminalTheme) -> Vec<String> {
    vec![
        format!(
            "Item      : {}",
            theme.success(&format!("#{}", item.work_item.id))
        ),
        format!(
            "Type      : {}",
            item.work_item.kind.as_deref().unwrap_or("unknown type")
        ),
        format!(
            "State     : {}",
            item.work_item.state.as_deref().unwrap_or("unknown state")
        ),
        format!(
            "Title     : {}",
            item.work_item.title.as_deref().unwrap_or("(untitled)")
        ),
    ]
}

fn ado_context_metadata(item: &dw_contracts::AdoAiContextItem) -> Option<String> {
    let mut values = Vec::new();
    if let Some(area) = item
        .work_item
        .area_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        values.push(format!("area={area}"));
    }
    if let Some(iteration) = item
        .work_item
        .iteration_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        values.push(format!("iteration={iteration}"));
    }
    if !item.work_item.tags.is_empty() {
        values.push(format!("tags={}", item.work_item.tags.join(", ")));
    }
    (!values.is_empty()).then(|| values.join(" | "))
}

fn ado_relation_display(relation: &dw_contracts::AdoAiContextRelation) -> String {
    if !relation.display.trim().is_empty() {
        return relation.display.clone();
    }
    format!(
        "{} {}",
        relation.kind,
        relation
            .work_item_id
            .as_deref()
            .or(relation.url.as_deref())
            .unwrap_or("")
    )
}

fn render_raw_changelog_line(line: &str, theme: &TerminalTheme) -> String {
    let Some(hash_index) = line.find('#') else {
        return theme.style_line(line, false);
    };
    let id_end = line[hash_index + 1..]
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_digit()).then_some(hash_index + 1 + index)
        })
        .unwrap_or(line.len());

    if id_end == hash_index + 1 {
        return theme.style_line(line, false);
    }

    let prefix = &line[..hash_index];
    let id = &line[hash_index..id_end];
    let suffix = &line[id_end..];
    format!("{prefix}{}{}", theme.success(id), suffix)
}

fn changelog_format_from_name(name: &str) -> dw_ado::ChangelogFormat {
    match name {
        "markdown" => dw_ado::ChangelogFormat::Markdown,
        "html" => dw_ado::ChangelogFormat::Html,
        _ => dw_ado::ChangelogFormat::Raw,
    }
}

fn push_preflight_issue_group(
    lines: &mut Vec<String>,
    title: &str,
    report: &TaskPreflightReport,
    predicate: impl Fn(&str) -> bool,
) {
    let issues = report
        .issues
        .iter()
        .filter(|issue| predicate(&issue.severity))
        .collect::<Vec<_>>();
    if issues.is_empty() {
        return;
    }

    lines.push(format!("Preflight details - {title}"));
    for issue in issues {
        lines.push(format!(
            "{} {} #{} {} - {}",
            severity_icon(&issue.severity),
            severity_label(&issue.severity),
            issue.work_item_id,
            issue.code,
            issue.message
        ));
        if let Some(details) = &issue.details {
            lines.push(format!("  Detail : {details}"));
        }
        if !issue.related_ids.is_empty() {
            lines.push(format!(
                "  Related: {}",
                issue
                    .related_ids
                    .iter()
                    .map(|id| format!("#{id}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    lines.push(String::new());
}

fn push_handoff_group(
    lines: &mut Vec<String>,
    title: &str,
    report: &TaskHandoffValidationReport,
    predicate: impl Fn(&TaskHandoffValidationItem) -> bool,
) {
    let items = report
        .items
        .iter()
        .filter(|item| predicate(item))
        .collect::<Vec<_>>();
    if items.is_empty() {
        return;
    }

    lines.push(format!("Handoff details - {title}"));
    for item in items {
        lines.push(format!(
            "{} {} [{}]",
            handoff_status_icon(&item.status, item.valid),
            item.repository,
            handoff_status_label(&item.status)
        ));
        lines.push(format!("  Message : {}", item.message));
        if !item.path.trim().is_empty() {
            lines.push(format!("  File    : {}", item.path));
        }
        if item.valid {
            lines.push(format!(
                "  Summary : done={} decisions={} risks={} blockers={} follow_up={}",
                item.done_count,
                item.decision_count,
                item.risk_count,
                item.blocker_count,
                item.follow_up_count
            ));
        }
    }
    lines.push(String::new());
}

fn validation_status_label(valid: bool) -> &'static str {
    if valid { "✓ OK" } else { "✕ Needs fixes" }
}

fn severity_icon(severity: &str) -> &'static str {
    if is_blocking_severity(severity) {
        "✕"
    } else if is_warning_severity(severity) {
        "!"
    } else {
        "-"
    }
}

fn severity_label(severity: &str) -> &'static str {
    if is_blocking_severity(severity) {
        "[blocker]"
    } else if is_warning_severity(severity) {
        "[warning]"
    } else {
        "[info]"
    }
}

fn handoff_status_label(status: &str) -> &str {
    match status {
        "missing" => "missing",
        "invalid" => "invalid",
        "blocked" => "blocked",
        "todo" => "todo",
        "in_progress" => "in_progress",
        "done" => "done",
        other => other,
    }
}

fn is_blocking_severity(severity: &str) -> bool {
    matches!(severity.to_ascii_lowercase().as_str(), "blocking" | "error")
}

fn is_warning_severity(severity: &str) -> bool {
    matches!(severity.to_ascii_lowercase().as_str(), "warning" | "warn")
}

fn handoff_status_icon(status: &str, valid: bool) -> &'static str {
    if valid {
        return "✓";
    }
    match status.to_ascii_lowercase().as_str() {
        "missing" | "invalid" | "blocked" => "✕",
        "todo" | "in_progress" => "!",
        _ => "-",
    }
}

fn prune_sync_status_label(status: &dw_task::prune::PruneSyncStatus) -> &'static str {
    match status {
        dw_task::prune::PruneSyncStatus::Skipped => "skipped",
        dw_task::prune::PruneSyncStatus::Synced => "synced",
    }
}

fn repository_status_label(status: &dw_git::RepositoryStatus) -> &'static str {
    if !status.is_git_repository {
        "Not a usable Git repo."
    } else if status.has_changes {
        "Changes detected:"
    } else if status.has_unpushed {
        "Unpushed commits."
    } else {
        "No change."
    }
}

fn work_item_line(item: &dw_workspace::WorkspaceWorkItem) -> String {
    format!(
        "#{} [{} / {}] {}",
        item.id,
        item.kind.as_deref().unwrap_or("unknown type"),
        item.state.as_deref().unwrap_or("unknown state"),
        item.title.as_deref().unwrap_or("(untitled)")
    )
}

fn work_item_action_label(action: dw_task::work_item::WorkItemUpdateAction) -> &'static str {
    match action {
        dw_task::work_item::WorkItemUpdateAction::Add => "add",
        dw_task::work_item::WorkItemUpdateAction::Remove => "remove",
    }
}

pub fn secret_set_lines(report: &SecretSetReport) -> Vec<String> {
    vec![
        "Secret".into(),
        "Status    : saved".into(),
        format!("Key       : {}", report.key),
        format!("Stockage  : {}", report.storage),
        "Value     : masked".into(),
    ]
}

pub fn secret_get_lines(report: &SecretGetReport) -> Vec<String> {
    vec![
        "Secret".into(),
        format!(
            "Status    : {}",
            if report.exists {
                "present"
            } else {
                "not found"
            }
        ),
        format!("Key       : {}", report.key),
        "Value     : masked".into(),
    ]
}

pub fn secret_delete_lines(report: &SecretDeleteReport) -> Vec<String> {
    vec![
        "Secret".into(),
        "Status    : deleted if present".into(),
        format!("Key       : {}", report.key),
    ]
}

fn config_file_line(theme: &TerminalTheme, label: &str, path: &str, exists: bool) -> String {
    let status = if exists {
        theme.success("✓")
    } else {
        theme.warning("!")
    };
    format!("{status} {label:9}: {}", theme.path(path))
}

fn config_check_line(theme: &TerminalTheme, check: &ConfigDoctorCheck) -> String {
    let status = if check.passed {
        theme.success("✓")
    } else {
        theme.warning("!")
    };
    format!("{status} {}", theme.path(&check.path))
}

fn agent_check_lines(check: &AgentDoctorCheck, theme: &TerminalTheme) -> Vec<String> {
    let status = if check.available {
        theme.success("✓ OK")
    } else {
        theme.warning("! missing")
    };
    let mut lines = vec![format!(
        "{:<10} {} via {}",
        status, check.agent_name, check.command
    )];
    if !check.available {
        lines.push(format!(
            "           {}",
            theme.command(&format!("Install `{}` or check PATH", check.command))
        ));
    }
    lines
}

fn render_doctor_check_group(
    title: &str,
    checks: Vec<&DoctorCheck>,
    theme: &TerminalTheme,
) -> Vec<String> {
    if checks.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    lines.push(theme.cyan(title));
    for check in checks {
        let status = if check.passed {
            theme.success("✓ OK")
        } else {
            theme.error("! Needs fixes")
        };
        lines.push(format!("{:<8} {}", status, check.name));
        if let Some(detail) = check
            .detail
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(format!("         {}", theme.path(detail)));
        }
        if !check.passed {
            lines.push(format!("         {}", theme.command(&check.remediation)));
        }
    }
    lines.push(String::new());
    lines
}

fn render_query_result_table(result: &QueryResult, theme: &TerminalTheme) -> String {
    let columns = if result.columns.is_empty() {
        vec!["Result".to_string()]
    } else {
        result.columns.clone()
    };
    let rows = if result.columns.is_empty() && result.rows.is_empty() {
        Vec::new()
    } else {
        result.rows.clone()
    };
    let widths = db_column_widths(&columns, &rows);
    let mut lines = Vec::new();

    lines.push(theme.bold(&theme.cyan("DB query")));
    lines.push(format!(
        "Result    : {}",
        theme.bold(&db_row_count_label(result))
    ));
    lines.push(db_separator(&widths));
    lines.push(db_row(
        &columns
            .iter()
            .map(|column| Some(column.as_str()))
            .collect::<Vec<_>>(),
        &widths,
        Some(theme),
    ));
    lines.push(db_separator(&widths));
    lines.extend(rows.iter().map(|row| {
        let cells = row
            .iter()
            .map(|value| value.as_deref())
            .collect::<Vec<Option<&str>>>();
        db_row(&cells, &widths, None)
    }));
    lines.push(db_separator(&widths));
    if result.truncated {
        lines.push(theme.warning(&format!(
            "Result truncated after {} row(s). Increase the row limit to expand.",
            result.rows.len()
        )));
    }
    lines.join("\n")
}

fn render_sql_guard(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    let mut lines = vec![theme.bold(&theme.cyan("SQL guard"))];
    lines.push(format!(
        "Status    : {}",
        db_guard_status_label(result, theme)
    ));
    if result.is_allowed {
        lines.push(format!("Decision  : {}", theme.success("✓")));
        lines.push("Message   : Read-only query allowed.".into());
        lines.push(format!(
            "Detail    : {}",
            theme.dim("No execution was started by this action.")
        ));
    } else {
        lines.push(format!("Decision  : {}", theme.error("!")));
        lines.push("Message   : Query blocked before execution.".into());
        lines.push(format!(
            "Raison    : {}",
            result.reason.as_deref().unwrap_or("unknown reason")
        ));
        lines.push(format!(
            "Next      : {}",
            theme.warning("Use only SELECT/WITH or introspection actions.")
        ));
    }
    lines.join("\n")
}

fn db_row_count_label(result: &QueryResult) -> String {
    let suffix = if result.rows.len() > 1 { "s" } else { "" };
    if result.truncated {
        format!(
            "{} row{suffix} displayed, result truncated",
            result.rows.len()
        )
    } else {
        format!("{} row{suffix}", result.rows.len())
    }
}

fn db_guard_status_label(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    if result.is_allowed {
        theme.success("allowed")
    } else {
        theme.error("blocked")
    }
}

fn db_column_widths(columns: &[String], rows: &[Vec<Option<String>>]) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let row_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| db_display_cell(value.as_deref()).chars().count())
                .max()
                .unwrap_or(0);
            column
                .chars()
                .count()
                .max(row_width)
                .clamp(1, MAX_DB_CELL_WIDTH)
        })
        .collect()
}

fn db_separator(widths: &[usize]) -> String {
    let cells = widths
        .iter()
        .map(|width| "-".repeat(width + 2))
        .collect::<Vec<_>>();
    format!("+{}+", cells.join("+"))
}

fn db_row(cells: &[Option<&str>], widths: &[usize], theme: Option<&TerminalTheme>) -> String {
    let rendered = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let value = db_display_cell(cells.get(index).copied().flatten());
            let value = db_truncate_cell(&value, *width);
            let padded = format!(" {value:<width$} ");
            if let Some(theme) = theme {
                theme.bold(&padded)
            } else if cells.get(index).copied().flatten().is_none() {
                TerminalTheme::plain().dim(&padded)
            } else {
                padded
            }
        })
        .collect::<Vec<_>>();
    format!("|{}|", rendered.join("|"))
}

fn db_display_cell(value: Option<&str>) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or("NULL")
        .replace(['\n', '\r'], " ")
}

fn db_truncate_cell(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    let take = width.saturating_sub(1);
    format!("{}…", value.chars().take(take).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_contracts::{
        HANDOFF_VALIDATION_VERSION, PREFLIGHT_VERSION, TaskHandoffValidationItem,
        TaskPreflightIssue,
    };

    #[test]
    fn task_list_lines_render_table_and_paths() {
        let report = TaskListReport {
            root: "/tmp/dw".into(),
            project: None,
            work_item: None,
            items: vec![dw_workspace::TaskListItem {
                path: "/tmp/ws".into(),
                project: "ha".into(),
                work_item_id: "42".into(),
                display_work_items: "#42 Titre [Actif]".into(),
                task_id: None,
                all_known_work_item_ids: vec!["42".into()],
                kind: "feat".into(),
                slug: "titre".into(),
                branch_name: "feature/42-titre".into(),
                created_at: "2026-07-02T10:00:00Z".into(),
                work_item_type: Some("User Story".into()),
                work_item_title: Some("Titre".into()),
                work_item_state: Some("Actif".into()),
                repositories: vec!["front".into(), "api".into()],
            }],
        };

        let lines = task_list_lines(&report);

        assert_eq!(lines[0], "Workspaces task: 1");
        assert!(lines.iter().any(|line| line.contains("ha")));
        assert!(lines.iter().any(|line| line.contains("#42 Titre")));
        assert!(lines.iter().any(|line| line.contains("front, api")));
    }

    #[test]
    fn ado_prs_lines_include_work_items_and_branches() {
        let report = dw_ado_commands::commands::prs::PrsReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            repositories: vec!["front".into()],
            items: vec![dw_ado::PullRequestListItem {
                repository: "front".into(),
                pull_request_id: 42,
                title: Some("Demo".into()),
                status: Some("active".into()),
                source_ref_name: Some("refs/heads/feat/123-demo".into()),
                target_ref_name: Some("refs/heads/develop".into()),
                is_draft: true,
                created_by: Some("Sacha".into()),
                url: None,
                web_url: None,
                work_item_ids: vec!["123".into()],
            }],
        };

        let lines = ado_prs_lines(&report);

        assert_eq!(lines[0], "Active PRs · ha");
        assert!(lines.iter().any(|line| line.contains("#42")));
        assert!(lines.iter().any(|line| line.contains("#123")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("feat/123-demo -> develop draft"))
        );
    }

    #[test]
    fn ado_assigned_lines_render_start_action() {
        let report = dw_ado_commands::commands::assigned::AssignedReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            top: 20,
            include_final_states: false,
            group_by_parent: false,
            items: vec![dw_ado::WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Corriger".into()),
                url: None,
            }],
            groups: Vec::new(),
            events: Vec::new(),
        };

        let lines = ado_assigned_lines(&report, &TerminalTheme::plain());

        assert!(lines.contains(&"ADO assigned".into()));
        assert!(lines.contains(&"Items     : 1".into()));
        assert!(lines.contains(&"Item      : #42 [Bug / En developpement] Corriger".into()));
        assert!(lines.contains(&"Start     : start action for 42 (ha)".into()));
    }

    #[test]
    fn ado_assigned_group_lines_render_parent_children_and_start_action() {
        let parent = dw_ado::WorkItemSnapshot {
            id: "42".into(),
            kind: Some("User Story".into()),
            state: Some("Actif".into()),
            title: Some("Parent".into()),
            url: None,
        };
        let child = dw_ado::WorkItemSnapshot {
            id: "43".into(),
            kind: Some("Task".into()),
            state: Some("Actif".into()),
            title: Some("Enfant".into()),
            url: None,
        };
        let report = dw_ado_commands::commands::assigned::AssignedReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            top: 20,
            include_final_states: false,
            group_by_parent: true,
            items: vec![parent.clone(), child.clone()],
            groups: vec![dw_ado::WorkItemGroup {
                parent,
                items: vec![child],
            }],
            events: Vec::new(),
        };

        let lines = ado_assigned_lines(&report, &TerminalTheme::plain());

        assert!(lines.contains(&"Assigned work items: 1 group(s), 2 item(s)".into()));
        assert!(lines.contains(&"Parent    : #42 [User Story / Actif] Parent".into()));
        assert!(lines.contains(&"Start     : start action for 42,43 (ha)".into()));
        assert!(lines.contains(&"  Child   : #43 [Task / Actif] Enfant".into()));
    }

    #[test]
    fn ado_set_state_execution_lines_include_ids_and_state() {
        let report = dw_ado_commands::commands::set_state::SetStateExecutionReport {
            plan: dw_ado_commands::commands::set_state::SetStatePlanReport {
                root: "/tmp/dw".into(),
                project: "ha".into(),
                ids: vec!["42".into(), "43".into()],
                state: "Actif".into(),
                history: "tui".into(),
            },
            events: vec!["ADO item #42: état -> Actif".into()],
            updated: vec![
                dw_ado_commands::commands::set_state::SetStateUpdate {
                    id: "42".into(),
                    state: "Actif".into(),
                },
                dw_ado_commands::commands::set_state::SetStateUpdate {
                    id: "43".into(),
                    state: "Actif".into(),
                },
            ],
        };

        let lines = ado_set_state_execution_lines(&report);

        assert_eq!(lines[0], "ADO update");
        assert!(lines.contains(&"Project   : ha".into()));
        assert!(lines.contains(&"State     : Actif".into()));
        assert!(lines.contains(&"Work items: #42, #43".into()));
        assert!(lines.contains(&"2 work items moved to `Actif`.".into()));
    }

    #[test]
    fn ado_work_item_lines_keep_context_action() {
        let report = dw_ado_commands::commands::work_item::WorkItemReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            requested_ids: vec![7],
            items: vec![dw_ado::WorkItemSnapshot {
                id: "7".into(),
                kind: None,
                state: None,
                title: None,
                url: None,
            }],
            events: Vec::new(),
        };

        let lines = ado_work_item_lines(&report, &TerminalTheme::plain());

        assert!(lines.contains(&"ADO work item".into()));
        assert!(lines.contains(&"Item      : #7".into()));
        assert!(lines.contains(&"Type      : unknown type".into()));
        assert!(lines.contains(&"State     : unknown state".into()));
        assert!(lines.contains(&"Title     : (untitled)".into()));
        assert!(lines.contains(&"Context   : action available for #7 (ha)".into()));
    }

    #[test]
    fn ado_context_lines_include_relations_comments_and_ai_context_action() {
        let report = dw_ado_commands::commands::context::ContextReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            requested_ids: vec!["42".into()],
            summary: false,
            comments: 10,
            expanded: Vec::new(),
            items: vec![dw_contracts::AdoAiContextItem {
                schema_version: dw_contracts::AI_CONTEXT_VERSION.into(),
                work_item: dw_contracts::AdoAiContextWorkItem {
                    id: "42".into(),
                    url: None,
                    title: Some("Corriger".into()),
                    kind: Some("Bug".into()),
                    state: Some("Actif".into()),
                    assigned_to: Some("Sacha".into()),
                    area_path: Some("Produit\\Backlog".into()),
                    iteration_path: Some("Sprint 1".into()),
                    tags: vec!["urgent".into()],
                },
                core: dw_contracts::AdoAiContextCore {
                    created_by: None,
                    created_date: None,
                    changed_by: None,
                    changed_date: None,
                    priority: None,
                    value_area: None,
                },
                content: dw_contracts::AdoAiContextContent {
                    description: Some("Description courte".into()),
                    acceptance_criteria: Some("Critère A".into()),
                    product_context: Default::default(),
                },
                links: dw_contracts::AdoAiContextLinks {
                    parent_ids: vec![],
                    child_ids: vec![],
                    predecessor_ids: vec![],
                    successor_ids: vec![],
                },
                attachments: dw_contracts::AdoAiContextAttachments {
                    directory_hint: "attachments/ado/42".into(),
                    items: vec![dw_contracts::AdoAiContextAttachment {
                        name: Some("capture.png".into()),
                        url: Some("https://example.invalid/capture.png".into()),
                        comment: None,
                        directory_hint: "attachments/ado/42".into(),
                    }],
                },
                relations: vec![dw_contracts::AdoAiContextRelation {
                    kind: "Parent".into(),
                    rel: None,
                    work_item_id: Some("1".into()),
                    name: None,
                    url: None,
                    comment: None,
                    artifact: None,
                    display: "Parent #1".into(),
                }],
                comments: vec![dw_contracts::AdoAiContextComment {
                    author: Some("Bob".into()),
                    created_date: None,
                    text: Some("OK".into()),
                }],
            }],
            events: Vec::new(),
        };

        let output = ado_context_lines(&report, &TerminalTheme::plain()).join("\n");

        assert!(output.contains("ADO context"));
        assert!(output.contains("Item      : #42"));
        assert!(output.contains("Type      : Bug"));
        assert!(output.contains("State     : Actif"));
        assert!(output.contains("Title     : Corriger"));
        assert!(output.contains("Assigned  : Sacha"));
        assert!(
            output.contains("Metadata : area=Produit\\Backlog | iteration=Sprint 1 | tags=urgent")
        );
        assert!(output.contains("Description courte"));
        assert!(output.contains("Acceptance criteria"));
        assert!(output.contains("Critère A"));
        assert!(output.contains("Attachments (1)"));
        assert!(output.contains("Folder    : attachments/ado/42"));
        assert!(output.contains("capture.png"));
        assert!(output.contains("- Parent #1"));
        assert!(output.contains("- Bob: OK"));
        assert!(output.contains("AI context: action available for #42 (ha)"));
    }

    #[test]
    fn ado_changelog_lines_render_ids_and_empty_messages() {
        let ids_only = dw_ado_commands::commands::changelog::ChangelogReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            from_pr: true,
            from_git: false,
            group_by_parent: false,
            format: "raw".into(),
            table: false,
            options: ado_options(),
            ids_only: true,
            work_item_ids: vec!["42".into(), "43".into()],
            items: Vec::new(),
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: false,
            events: Vec::new(),
        };
        assert_eq!(
            ado_changelog_lines(&ids_only, &TerminalTheme::plain()),
            vec!["42 43".to_string()]
        );

        let source_empty = dw_ado_commands::commands::changelog::ChangelogReport {
            source_empty: true,
            from_git: true,
            ..ids_only
        };
        assert_eq!(
            ado_changelog_lines(&source_empty, &TerminalTheme::plain()),
            vec!["No work item detected in commit messages from the git range.".to_string()]
        );
    }

    #[test]
    fn ado_changelog_lines_style_raw_only() {
        let report = dw_ado_commands::commands::changelog::ChangelogReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            from_pr: true,
            from_git: false,
            group_by_parent: false,
            format: "raw".into(),
            table: false,
            options: ado_options(),
            ids_only: false,
            work_item_ids: vec!["42".into()],
            items: vec![
                dw_ado::WorkItemSnapshot {
                    id: "42".into(),
                    kind: Some("Bug".into()),
                    state: Some("Actif".into()),
                    title: Some("Corriger".into()),
                    url: None,
                },
                dw_ado::WorkItemSnapshot {
                    id: "43".into(),
                    kind: Some("Task".into()),
                    state: Some("Actif".into()),
                    title: Some("Tester".into()),
                    url: None,
                },
            ],
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: false,
            events: Vec::new(),
        };
        let theme = TerminalTheme::new(dw_ui::ColorMode::Always, false, false);
        let output = ado_changelog_lines(&report, &theme).join("\n");

        assert!(output.contains("\u{1b}"));
        assert!(output.contains("[Bug] Actif - Corriger"));
        assert!(output.contains("#43"));

        let markdown = dw_ado_commands::commands::changelog::ChangelogReport {
            format: "markdown".into(),
            ..report
        };
        let markdown_output = ado_changelog_lines(&markdown, &theme).join("\n");
        assert!(markdown_output.contains("# Changelog"));
        assert!(markdown_output.contains("[#42]"));
    }

    #[test]
    fn task_preflight_lines_include_blocking_guidance() {
        let report = TaskPreflightReport {
            schema_version: PREFLIGHT_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_ids: vec!["42".into()],
            has_blocking_issues: true,
            issues: vec![TaskPreflightIssue {
                code: "missing_attachment".into(),
                severity: "blocking".into(),
                work_item_id: "42".into(),
                message: "Piece jointe manquante".into(),
                details: Some("screenshot absent".into()),
                related_ids: vec![],
            }],
        };

        let lines = task_preflight_lines(&report);

        assert_eq!(lines[0], "Task preflight");
        assert!(lines.contains(&"Status    : ✕ Needs fixes".into()));
        assert!(lines.contains(&"Blockers  : 1".into()));
        assert!(
            lines.contains(&"✕ [blocker] #42 missing_attachment - Piece jointe manquante".into())
        );
    }

    #[test]
    fn task_handoff_validation_lines_include_counts_and_failure() {
        let report = TaskHandoffValidationReport {
            schema_version: HANDOFF_VALIDATION_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            is_valid: false,
            items: vec![TaskHandoffValidationItem {
                repository: "front".into(),
                path: "/tmp/ws/front/handoff-front.md".into(),
                status: "done".into(),
                valid: true,
                message: "OK".into(),
                done_count: 2,
                decision_count: 1,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 1,
            }],
        };

        let lines = task_handoff_validation_lines(&report);

        assert_eq!(lines[0], "Validation handoff");
        assert!(lines.contains(&"Status    : ✕ Needs fixes".into()));
        assert!(lines.contains(&"Handoffs  : 1/1 valid".into()));
        assert!(lines.contains(&"✓ front [done]".into()));
        assert!(
            lines.contains(&"  Summary : done=2 decisions=1 risks=0 blockers=0 follow_up=1".into())
        );
    }

    #[test]
    fn task_prune_plan_lines_render_preview_summary() {
        let report = dw_task::prune::PrunePlanReport {
            root: "/tmp/dw".into(),
            project: Some("ha".into()),
            work_item: None,
            sync: Vec::new(),
            candidates: vec![dw_workspace::WorkspaceSummary {
                path: "/tmp/dw/projects/ha/workspaces/feat-1-done".into(),
                manifest: dw_workspace::WorkspaceManifest {
                    schema: 1,
                    work_item_id: "1".into(),
                    task_id: None,
                    project: "ha".into(),
                    kind: "feat".into(),
                    slug: "done".into(),
                    branch_name: "feat/1-done".into(),
                    created_at: "2026-07-02T10:00:00Z".into(),
                    repositories: vec!["front".into(), "back".into()],
                    status: "created".into(),
                    work_item_type: Some("User Story".into()),
                    work_item_title: Some("Done".into()),
                    work_item_state: Some("Valide".into()),
                    child_task_ids: None,
                    child_tasks: None,
                    work_items: None,
                },
            }],
        };

        let lines = task_prune_plan_lines(&report);

        assert_eq!(lines[0], "Workspace cleanup");
        assert_eq!(lines[1], "Mode      : preview");
        assert!(lines.contains(&"Candidates: 1".into()));
        assert!(lines.contains(&"Action    : enable execution to delete candidates".into()));
        assert!(lines.contains(&"Workspace : /tmp/dw/projects/ha/workspaces/feat-1-done".into()));
        assert!(lines.contains(&"Items     : ha / #1 Done [Valide]".into()));
        assert!(lines.contains(&"Repositories: front, back".into()));
    }

    #[test]
    fn task_commit_plan_lines_render_statuses_and_execute_hint() {
        let report = dw_task::repo::CommitPlanReport {
            workspace: "/tmp/ws".into(),
            branch_name: "feat/42-demo".into(),
            message: "feat(42): demo".into(),
            targets: vec![dw_task::repo::CommitTargetStatus {
                target: dw_workspace::TaskCommitTarget {
                    repository: "front".into(),
                    path: "/tmp/ws/front".into(),
                },
                status: dw_git::RepositoryStatus {
                    path: "/tmp/ws/front".into(),
                    is_git_repository: true,
                    has_changes: true,
                    has_unpushed: false,
                    detail: " M src/lib.rs".into(),
                },
            }],
        };

        let lines = task_commit_plan_lines(&report, false);

        assert_eq!(lines[0], "Repository commit");
        assert!(lines.contains(&"Repository: front".into()));
        assert!(lines.contains(&"Status    : Changes detected:".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"Action    : enable execution to create commits".into()));
    }

    #[test]
    fn task_finish_plan_lines_include_handoff_and_pr_candidates() {
        let report = dw_task::finish::FinishPlanReport {
            root: "/tmp/dw".into(),
            workspace: "/tmp/ws".into(),
            manifest: workspace_manifest_with_items(Vec::new()),
            targets: vec![dw_task::finish::FinishTargetStatus {
                target: dw_workspace::TaskCommitTarget {
                    repository: "front".into(),
                    path: "/tmp/ws/front".into(),
                },
                status: dw_git::RepositoryStatus {
                    path: "/tmp/ws/front".into(),
                    is_git_repository: true,
                    has_changes: true,
                    has_unpushed: false,
                    detail: " M src/lib.rs".into(),
                },
            }],
            handoff: TaskHandoffValidationReport {
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
            },
            handoff_summaries: vec![dw_workspace::WorkspaceHandoffSummary {
                repository: "front".into(),
                status: "done".into(),
                done: vec!["UI ajustée".into()],
                decisions: vec!["Conserver le contrat JSON".into()],
                risks: Vec::new(),
                blockers: Vec::new(),
                follow_up: vec!["Valider en recette".into()],
            }],
            commit_message: "feat(42): demo".into(),
            create_pr: true,
            ready: false,
            skip_ado: false,
            changed_repositories: vec!["front".into()],
            unpushed_repositories: Vec::new(),
            actionable_repositories: vec!["front".into()],
            pull_request_candidates: vec![dw_workspace::PullRequestCandidate {
                repository: "front".into(),
                path: "/tmp/ws/front".into(),
                ado_repository: Some("front".into()),
                target_branch: "develop".into(),
            }],
        };

        let lines = task_finish_plan_lines(&report);

        assert_eq!(lines[0], "Workspace finish");
        assert!(lines.contains(&"Status    : OK".into()));
        assert!(lines.contains(&"- [done] front - OK".into()));
        assert!(lines.contains(&"Commit to create".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"Handoff front".into()));
        assert!(lines.contains(&"Done      : UI ajustée".into()));
        assert!(lines.contains(&"- front -> develop".into()));
        assert!(lines.contains(&"Action    : enable execution to finish the workspace".into()));
        assert!(lines.contains(&"Confirmation: handled by the TUI before execution".into()));
    }

    #[test]
    fn task_add_repo_plan_lines_include_anchor() {
        let report = dw_task::repo::AddRepoPlanReport {
            plan: dw_workspace::TaskAddRepoPlan {
                workspace: "/tmp/ws".into(),
                repository: "front".into(),
                project_root: "/tmp/project".into(),
                worktree_path: "/tmp/ws/front".into(),
                url: "https://example.invalid/front.git".into(),
                default_branch: "main".into(),
                anchor_name: "front-anchor".into(),
                branch_name: "feat/42-demo".into(),
                repositories: vec!["front".into()],
            },
        };

        let lines = task_add_repo_plan_lines(&report);

        assert_eq!(lines[0], "Add repository (preview)");
        assert!(lines.contains(&"Anchor    : /tmp/project/repositories/front-anchor".into()));
        assert!(lines.contains(&"Action    : enable execution to add front".into()));
    }

    #[test]
    fn task_teardown_plan_lines_switch_title_for_execute() {
        let report = dw_task::repo::TeardownPlanReport {
            workspace: Some("/tmp/ws".into()),
            steps: vec![dw_workspace::WorkspaceTeardownStep {
                repository: "front".into(),
                action: "remove-worktree".into(),
                target: "/tmp/ws/front".into(),
                git_dir: Some("/tmp/project/repositories/front/.git".into()),
            }],
        };

        let dry_run = task_teardown_plan_lines(&report, false);
        let execute = task_teardown_plan_lines(&report, true);

        assert_eq!(dry_run[0], "Workspace removal (preview)");
        assert_eq!(dry_run[2], "Actions   : 1");
        assert_eq!(dry_run[3], "Planned actions");
        assert_eq!(execute[0], "Workspace removal executed");
        assert_eq!(execute[2], "Actions   : 1");
        assert_eq!(execute[3], "Applied actions");
        assert!(dry_run.contains(&"- [front] remove-worktree: /tmp/ws/front".into()));
        assert!(dry_run.contains(&"Action    : enable execution to remove the workspace".into()));
        assert!(dry_run.contains(&"Confirmation: handled by the TUI before execution".into()));
        assert!(!execute.contains(&"Action    : enable execution to remove the workspace".into()));
    }

    #[test]
    fn task_sync_lines_render_missing_ado_fields_as_unknown() {
        let report = dw_task::lifecycle::SyncReport {
            workspace: "/tmp/ws".into(),
            requested_ids: vec!["42".into()],
            snapshots: Vec::new(),
            manifest: workspace_manifest_with_items(vec![dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: None,
                state: None,
                title: None,
            }]),
        };

        let lines = task_sync_lines(&report);

        assert_eq!(lines[0], "Task synchronization");
        assert_eq!(lines[1], "Workspace : /tmp/ws");
        assert_eq!(lines[2], "Items     : 1");
        assert_eq!(lines[4], "Work items ADO");
        assert_eq!(lines[5], "#42 [unknown type / unknown state] (untitled)");
    }

    #[test]
    fn task_rename_plan_lines_show_slug_branch_and_workspace() {
        let report = dw_task::lifecycle::RenamePlanReport {
            plan: dw_workspace::TaskRenamePlan {
                workspace: "/tmp/old".into(),
                new_workspace: "/tmp/new".into(),
                old_slug: "old".into(),
                new_slug: "new".into(),
                old_branch: "feat/1-old".into(),
                new_branch: "feat/1-new".into(),
            },
        };

        let lines = task_rename_plan_lines(&report);

        assert_eq!(lines[0], "Workspace rename");
        assert!(lines.contains(&"Mode      : preview".into()));
        assert!(lines.contains(&"Slug      : old -> new".into()));
        assert!(lines.contains(&"Branch    : feat/1-old -> feat/1-new".into()));
        assert!(lines.contains(&"Action    : enable execution to rename the workspace".into()));
    }

    #[test]
    fn task_child_task_lines_render_workspace_repo_and_item() {
        let report = dw_task::lifecycle::CreateChildTaskReport {
            workspace: "/tmp/ws".into(),
            repository: "front".into(),
            parent: dw_workspace::WorkspaceWorkItem {
                id: "1".into(),
                kind: Some("User Story".into()),
                title: Some("Parent".into()),
                state: Some("Active".into()),
            },
            requested_title: "[FRONT] Corriger".into(),
            created: dw_ado::WorkspaceChildTaskCreateResult {
                repository: "front".into(),
                id: "42".into(),
                title: "[FRONT] Corriger".into(),
            },
            manifest: workspace_manifest_with_items(Vec::new()),
        };

        let lines = task_child_task_lines(&report);

        assert_eq!(lines[0], "ADO child task");
        assert_eq!(lines[1], "Status    : saved in workspace");
        assert_eq!(lines[2], "Workspace : /tmp/ws");
        assert_eq!(lines[3], "Repository: front");
        assert_eq!(lines[4], "Item      : #42");
        assert_eq!(lines[5], "Title     : [FRONT] Corriger");
    }

    #[test]
    fn task_work_item_plan_lines_render_branch_workspace_and_ids() {
        let report = dw_task::work_item::WorkItemUpdatePlanReport {
            action: dw_task::work_item::WorkItemUpdateAction::Add,
            workspace: "/tmp/old".into(),
            requested_ids: vec!["1".into(), "2".into()],
            skipped_existing_ids: Vec::new(),
            snapshots: Vec::new(),
            plan: Some(dw_workspace::TaskWorkItemUpdatePlan {
                workspace: "/tmp/old".into(),
                new_workspace: "/tmp/new".into(),
                old_branch: "feat/1-old".into(),
                new_branch: "feat/1-2-new".into(),
                work_items: vec![
                    dw_workspace::WorkspaceWorkItem {
                        id: "1".into(),
                        kind: None,
                        title: None,
                        state: None,
                    },
                    dw_workspace::WorkspaceWorkItem {
                        id: "2".into(),
                        kind: None,
                        title: None,
                        state: None,
                    },
                ],
            }),
        };

        let lines = task_work_item_plan_lines(&report);

        assert_eq!(lines[0], "Work items workspace");
        assert_eq!(lines[1], "Mode      : preview");
        assert_eq!(lines[2], "Action    : add");
        assert!(lines.contains(&"Branch    : feat/1-old -> feat/1-2-new".into()));
        assert!(lines.contains(&"Items     : #1, #2".into()));
        assert!(lines.contains(&"Action    : enable execution for add".into()));
    }

    #[test]
    fn task_start_plan_lines_include_execute_hint() {
        let report = dw_task::start::StartPlanReport {
            root: "/tmp/dw".into(),
            plan: dw_workspace::TaskStartPlan {
                project: "ha".into(),
                work_item_ids: vec!["42".into()],
                primary_work_item_id: "42".into(),
                task_id: None,
                kind: "feat".into(),
                slug: "titre".into(),
                branch_name: "feat/42-titre".into(),
                subject_name: "42-titre".into(),
                workspace: "/tmp/dw/ha/42-titre".into(),
                repositories: vec!["front".into(), "back".into()],
                repository_folders: Default::default(),
                repository_worktrees: Vec::new(),
            },
            work_items: Vec::new(),
            child_tasks: Vec::new(),
        };

        let lines = task_start_plan_lines(&report);

        assert_eq!(lines[0], "Plan task start");
        assert!(lines.contains(&"Action    : enable execution to create the workspace.".into()));
    }

    fn workspace_manifest_with_items(
        work_items: Vec<dw_workspace::WorkspaceWorkItem>,
    ) -> dw_workspace::WorkspaceManifest {
        dw_workspace::WorkspaceManifest {
            schema: 1,
            work_item_id: "42".into(),
            task_id: None,
            project: "ha".into(),
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/42-demo".into(),
            created_at: "2026-07-02T10:00:00Z".into(),
            repositories: vec!["front".into()],
            status: "created".into(),
            work_item_type: None,
            work_item_title: None,
            work_item_state: None,
            child_task_ids: None,
            child_tasks: None,
            work_items: Some(work_items),
        }
    }

    fn ado_options() -> dw_ado::AzureDevOpsOptions {
        dw_ado::AzureDevOpsOptions {
            organization: "https://dev.azure.com/acme".into(),
            project: "ha".into(),
            api_version: "7.1".into(),
        }
    }
}
