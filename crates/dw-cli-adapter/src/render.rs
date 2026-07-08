use dw_ado_commands::auth::{
    AuthLoginMode, AuthLoginReport, AuthLogoutReport, AuthStatusReport, AuthTokenExpiration,
};
use dw_agent::{
    AgentContextReport,
    command::{AgentDoctorCheck, AgentDoctorReport},
};
use dw_app::{
    AdoActionResult, AgentActionResult, AppActionResult, ConfigActionResult, DbActionResult,
    DwActionResult, SecretActionResult, TaskActionResult, UpgradeActionResult,
};
use dw_config::{ConfigDoctorCheck, ConfigDoctorReport, ConfigShow, InitReport, RefreshReport};
use dw_contracts::{
    TaskHandoffValidationDetail, TaskHandoffValidationItem, TaskHandoffValidationReport,
    TaskHandoffValidationStatus, TaskPreflightIssue, TaskPreflightIssueCode,
    TaskPreflightIssueDetail, TaskPreflightReport, TaskPreflightSeverity, TaskPreflightStaleReason,
};
use dw_core::{AdoActionEvent, DbActionEvent, GitOperation, TaskActionEvent, Timestamp};
use dw_db::{QueryResult, SqlGuardResult};
use dw_doctor::{DoctorCheck, DoctorCheckDetail, DoctorCheckKind, DoctorRemediation, DoctorReport};
use dw_secret::command::{SecretDeleteReport, SecretGetReport, SecretSetReport};
use dw_task::open::{TaskListReport, TaskStatusReport};
use dw_ui::TerminalTheme;
use dw_workspace::TaskCurrentItem;
use std::fmt::Display;

const MAX_DB_CELL_WIDTH: usize = 48;

pub fn version_lines(version: &str) -> Vec<String> {
    vec![format!("Dev Workflow {version}")]
}

pub fn action_result_lines(result: &DwActionResult, theme: &TerminalTheme) -> Vec<String> {
    match result {
        DwActionResult::App(AppActionResult::Version { version }) => version_lines(version),
        DwActionResult::App(AppActionResult::Guide { .. }) => {
            vec!["DevWorkflow guide".into()]
        }
        DwActionResult::Doctor(report) => doctor_report_lines(report, theme),
        DwActionResult::Config(result) => match result {
            ConfigActionResult::Show(report) => config_show_lines(report, theme),
            ConfigActionResult::Init(report) => init_report_lines(report),
            ConfigActionResult::Refresh(report) => refresh_report_lines(report),
            ConfigActionResult::Doctor(report) => config_doctor_lines(report, theme),
            ConfigActionResult::SetColor(report) => vec![
                "Configuration updated".into(),
                format!("Color    : {}", report.mode),
            ],
            ConfigActionResult::SetRoot(report) => vec![
                "Configuration updated".into(),
                format!("Root     : {}", report.root),
            ],
        },
        DwActionResult::Agent(result) => match result {
            AgentActionResult::Config { root, agent } => agent_config_lines(root, agent, theme),
            AgentActionResult::SetDefault { root, agent } => {
                agent_config_updated_lines(root, agent, theme)
            }
            AgentActionResult::Doctor(report) => agent_doctor_lines(report, theme),
            AgentActionResult::Context(report) => agent_context_markdown(report)
                .lines()
                .map(str::to_owned)
                .collect(),
        },
        DwActionResult::Db(result) => match result {
            DbActionResult::Guard(report) => db_guard_lines(report, theme),
            DbActionResult::Schema(report) | DbActionResult::Query(report) => {
                db_query_table(report, theme)
                    .lines()
                    .map(str::to_owned)
                    .collect()
            }
            DbActionResult::Describe(Some(report)) => db_query_table(report, theme)
                .lines()
                .map(str::to_owned)
                .collect(),
            DbActionResult::Describe(None) => Vec::new(),
        },
        DwActionResult::Ado(result) => match result {
            AdoActionResult::AuthLogin(report) => auth_login_lines(report),
            AdoActionResult::AuthStatus(report) => auth_status_lines(report),
            AdoActionResult::AuthLogout(report) => auth_logout_lines(report),
            AdoActionResult::Assigned(report) => ado_assigned_lines(report, theme),
            AdoActionResult::Prs(report) => ado_prs_lines(report),
            AdoActionResult::Changelog(report) => ado_changelog_lines(report, theme),
            AdoActionResult::Context(report) => ado_context_lines(report, theme),
            AdoActionResult::AiContext(report) => serde_json::to_string_pretty(&report.items)
                .map(|json| json.lines().map(str::to_owned).collect())
                .unwrap_or_else(|error| vec![format!("JSON render error: {error}")]),
            AdoActionResult::WorkItem(report) => ado_work_item_lines(report, theme),
            AdoActionResult::SetStatePlan(report) => ado_set_state_plan_lines(report),
            AdoActionResult::SetState(report) => ado_set_state_execution_lines(report),
        },
        DwActionResult::Task(result) => match result.as_ref() {
            TaskActionResult::Status(report) => task_status_lines(report),
            TaskActionResult::List(report) => task_list_lines(report),
            TaskActionResult::Current(report) => task_current_lines(report),
            TaskActionResult::Open(plan) => task_open_launch_lines(plan),
            TaskActionResult::StartPlan(report) => task_start_plan_lines(report),
            TaskActionResult::StartExecution(report) => task_start_execution_lines(report),
            TaskActionResult::StartPrPlan(report) => task_start_pr_plan_lines(report),
            TaskActionResult::Preflight(report) => task_preflight_lines(report),
            TaskActionResult::HandoffValidate(report) => task_handoff_validation_lines(report),
            TaskActionResult::Sync(report) => task_sync_lines(report),
            TaskActionResult::RenamePlan(report) => task_rename_plan_lines(report),
            TaskActionResult::RenameExecution(report) => task_rename_execution_lines(report),
            TaskActionResult::RepoLatestPlan(report) => task_repo_latest_plan_lines(report),
            TaskActionResult::RepoLatestExecution { plan, execution } => {
                let mut lines = task_repo_latest_plan_lines(plan);
                lines.extend(task_repo_latest_execution_lines(execution));
                lines
            }
            TaskActionResult::CommitPlan(report) => task_commit_plan_lines(report, false),
            TaskActionResult::CommitExecution { plan, execution } => {
                let mut lines = task_commit_plan_lines(plan, true);
                lines.extend(task_commit_execution_lines(execution));
                lines
            }
            TaskActionResult::AddRepoPlan(report) => task_add_repo_plan_lines(report),
            TaskActionResult::AddRepoExecution { plan, execution } => {
                let mut lines = task_add_repo_plan_lines(plan);
                lines.extend(task_add_repo_execution_lines(execution));
                lines
            }
            TaskActionResult::TeardownPlan {
                plan,
                execute_requested,
            } => task_teardown_plan_lines(plan, *execute_requested),
            TaskActionResult::TeardownExecution(report) => task_teardown_execution_lines(report),
            TaskActionResult::FinishPlan(report) => task_finish_plan_lines(report),
            TaskActionResult::FinishExecution(report) => task_finish_execution_lines(report),
            TaskActionResult::PrunePlan(report) => task_prune_plan_lines(report),
            TaskActionResult::PruneExecution(report) => task_prune_execution_lines(report),
            TaskActionResult::CreateChildTask(report) => task_child_task_lines(report),
            TaskActionResult::WorkItemPlan(report) => task_work_item_plan_lines(report),
            TaskActionResult::WorkItemExecution { plan, execution } => {
                let mut lines = task_work_item_plan_lines(plan);
                if let Some(execution) = execution {
                    lines.extend(task_work_item_execution_lines(execution));
                }
                lines
            }
        },
        DwActionResult::Secret(result) => match result {
            SecretActionResult::Get(report) => secret_get_lines(report),
            SecretActionResult::Set(report) => secret_set_lines(report),
            SecretActionResult::Delete(report) => secret_delete_lines(report),
        },
        DwActionResult::Upgrade(UpgradeActionResult::Report(report)) => {
            upgrade_report_lines(report)
        }
    }
}

pub fn task_open_launch_lines(plan: &dw_core::ExternalLaunchPlan) -> Vec<String> {
    let mut lines = vec![
        "Opening agent".into(),
        format!("Command  : {}", plan.display_command()),
    ];
    if let Some(working_directory) = &plan.working_directory {
        lines.push(format!("Directory: {working_directory}"));
    }
    lines
}

pub fn guide_lines(version: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command(&format!("Dev Workflow {version}")),
        "Step-by-step getting started guide".into(),
        String::new(),
        "1. Check the installation".into(),
        format!("   {}", theme.command("dw version")),
        format!("   {}", theme.command("dw doctor")),
        "   Fix reported prerequisites before creating workspaces.".into(),
        String::new(),
        "2. Initialize the DevWorkflow root".into(),
        format!("   {}", theme.command("dw init")),
        format!("   {}", theme.command("dw config show")),
        "   The root contains config, schemas, cache, projects, workspaces, and agent contexts.".into(),
        "   To choose an explicit path:".into(),
        format!("   {}", theme.command("dw init --root ~/dev/dw")),
        String::new(),
        "3. Connect Azure DevOps".into(),
        format!("   {}", theme.command("dw auth login")),
        format!("   {}", theme.command("dw auth status")),
        format!("   {}", theme.command("dw ado assigned")),
        "   Without --project, dw offers configured projects when the terminal is interactive.".into(),
        String::new(),
        "4. Create a task workspace".into(),
        format!("   {}", theme.command("dw task start <work-item-id>")),
        "   Without --execute, dw shows the plan: branch, repositories, worktrees, handoffs.".into(),
        format!("   {}", theme.command("dw task start <work-item-id> --execute")),
        format!("   {}", theme.command("dw task open --continue")),
        "   The configured agent opens in the workspace with DevWorkflow context.".into(),
        String::new(),
        "5. Daily loop".into(),
        format!("   {}", theme.command("dw task status")),
        format!("   {}", theme.command("dw task list")),
        format!("   {}", theme.command("dw task current")),
        format!("   {}", theme.command("dw task preflight --continue")),
        format!("   {}", theme.command("dw task sync --continue")),
        "   Use preflight before implementation and sync to refresh task.json from ADO.".into(),
        String::new(),
        "6. Manage workspace contents".into(),
        format!("   {}", theme.command("dw task add-work-item --continue")),
        format!("   {}", theme.command("dw task remove-work-item --continue")),
        format!("   {}", theme.command("dw task add-repo --continue")),
        format!("   {}", theme.command("dw task repo-latest --continue")),
        "   Interactive commands offer local values when available.".into(),
        String::new(),
        "7. Prepare task completion".into(),
        format!("   {}", theme.command("dw task handoff-validate --continue")),
        format!("   {}", theme.command("dw task commit --continue")),
        format!("   {}", theme.command("dw task finish --continue")),
        "   These commands preview by default. Add --execute only after reading the plan.".into(),
        format!("   {}", theme.command("dw task finish --continue --execute")),
        String::new(),
        "8. Clean up".into(),
        format!("   {}", theme.command("dw task teardown --continue")),
        format!("   {}", theme.command("dw task prune")),
        "   teardown and prune remove only with --execute, and ask for confirmation interactively.".into(),
        String::new(),
        "9. ADO, DB, and AI context".into(),
        format!("   {}", theme.command("dw ado work-item <id>")),
        format!("   {}", theme.command("dw ado context <id>")),
        format!("   {}", theme.command("dw ado changelog <ids>")),
        format!("   {}", theme.command("dw db schema")),
        format!("   {}", theme.command("dw db describe <table>")),
        format!("   {}", theme.command("dw db query --sql \"select top 20 * from ...\"")),
        format!("   {}", theme.command("dw agent context")),
        "   DB access is protected by the read-only guard.".into(),
        String::new(),
        "10. Shell productivity".into(),
        format!("   {}", theme.command("dw completion show")),
        format!("   {}", theme.command("dw completion install fish")),
        format!("   {}", theme.command("dw completion install powershell")),
        "   Completions suggest options, projects, repositories, workspaces, databases, and descriptions.".into(),
        String::new(),
        "Quick diagnostics".into(),
        format!("   {}", theme.command("dw doctor --fix")),
        format!("   {}", theme.command("dw config doctor")),
        format!("   {}", theme.command("dw refresh")),
        "   refresh regenerates schemas and agent contexts without overwriting user files.".into(),
    ]
}

pub fn auth_login_lines(report: &AuthLoginReport) -> Vec<String> {
    if report.uses_environment_pat {
        return vec![
            "ADO connection".into(),
            "Mode     : PAT via environment".into(),
            "To do    : set DW_ADO_TOKEN or AZURE_DEVOPS_EXT_PAT.".into(),
            "Security : no secret is entered or stored by this command.".into(),
        ];
    }

    vec![
        "ADO connection".into(),
        "Status   : connected".into(),
        format!("Mode     : {}", auth_login_mode_label(report.mode)),
        format!(
            "Source   : {}",
            report
                .source
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown source".into())
        ),
        expiration_line(report.expires_on.as_ref()),
    ]
}

pub fn auth_status_lines(report: &AuthStatusReport) -> Vec<String> {
    if report.connected {
        vec![
            "ADO connection".into(),
            "Status   : connected".into(),
            format!(
                "Source   : {}",
                report
                    .source
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown source".into())
            ),
            expiration_line(report.expires_on.as_ref()),
        ]
    } else {
        vec![
            "ADO connection".into(),
            "Status   : disconnected".into(),
            "To do    : dw auth login or set DW_ADO_TOKEN.".into(),
        ]
    }
}

pub fn auth_logout_lines(report: &AuthLogoutReport) -> Vec<String> {
    vec![
        "ADO connection".into(),
        format!(
            "Sessions : {}",
            if report.removed_local_session {
                "local session removed"
            } else {
                "no local session"
            }
        ),
        "PAT      : DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT variables remain managed by the environment."
            .into(),
    ]
}

fn auth_login_mode_label(mode: AuthLoginMode) -> &'static str {
    match mode {
        AuthLoginMode::Browser => "browser",
        AuthLoginMode::DeviceCode => "device code",
        AuthLoginMode::EnvironmentPat => "environment PAT",
    }
}

fn expiration_line(expires_on: Option<&AuthTokenExpiration>) -> String {
    format!(
        "Expires  : {}",
        expires_on
            .map(AuthTokenExpiration::as_str)
            .unwrap_or("unknown expiration")
    )
}

pub fn config_show_lines(report: &ConfigShow, theme: &TerminalTheme) -> Vec<String> {
    let color = report.color.to_string();
    vec![
        theme.command("DevWorkflow configuration"),
        format!("Root     : {}", theme.path(&report.root)),
        format!("Color    : {}", theme.bold(&color)),
        format!("Settings : {}", theme.path(&report.settings_path)),
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
        theme.command("Configuration diagnostics"),
        format!(
            "Status   : {}",
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
            lines.push(format!("  Detail : {message}"));
        }
    }
    lines.push(String::new());
    lines.push(if report.passed {
        format!("Result   : {}", theme.success("Configuration is valid."))
    } else {
        format!(
            "Result   : {}",
            theme.warning(
                "Configuration is incomplete. Fix the reported issues and rerun `dw config doctor`."
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
            "  - user settings unchanged (--no-save).".into()
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
        lines.push("User settings unchanged (--no-save).".into());
    }
    lines.push("Recommended next step: dw doctor".into());
    lines
}

pub fn refresh_report_lines(report: &RefreshReport) -> Vec<String> {
    vec![
        format!("Refreshed root: {}", report.root),
        format!("Profile: {}", report.profile),
        "Schemas and agent contexts regenerated.".into(),
        "User files preserved: projects.json, workflow.json, databases.json, plan.md.".into(),
    ]
}

pub fn agent_doctor_lines(report: &AgentDoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let available_count = report.available_count();
    let total_count = report.total_count();
    let mut lines = vec![
        theme.command("Agent diagnostics"),
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

pub fn agent_config_lines(
    root: &impl std::fmt::Display,
    agent: &impl std::fmt::Display,
    theme: &TerminalTheme,
) -> Vec<String> {
    vec![
        theme.command("Agent config"),
        format!("Default agent: {}", theme.bold(&agent.to_string())),
        format!("DevWorkflow root: {}", theme.path(&root.to_string())),
    ]
}

pub fn agent_config_updated_lines(
    root: &impl std::fmt::Display,
    agent: &impl std::fmt::Display,
    theme: &TerminalTheme,
) -> Vec<String> {
    vec![
        theme.success("✓ Agent config updated"),
        format!("Default agent: {}", theme.bold(&agent.to_string())),
        format!("DevWorkflow root: {}", theme.path(&root.to_string())),
    ]
}

pub fn agent_context_markdown(report: &AgentContextReport) -> String {
    format!(
        r#"# Contexte agent DevWorkflow

Tu travailles dans un environnement géré par DevWorkflow.

Utilise les actions DevWorkflow pour les opérations du workflow:

- Diagnostic local vérifie les prérequis.
- Authentification Azure DevOps connecte l'environnement quand la connexion silencieuse est indisponible.
- Liste ADO assignée affiche les work items assignés pour le projet courant.
- Lecture work item ADO charge le résumé d'un work item.
- Contexte IA ADO lit le contexte work item structuré et déterministe pour usage IA.
- Workspace courant affiche le workspace task actif et la branche.
- Synchronisation task rafraîchit `task.json` depuis ADO quand le contexte local peut être obsolète.
- Préflight task vérifie les blocages et alertes déterministes avant implémentation ou découpage en child tasks.
- Validation handoff vérifie les contrats handoff avant finalisation task ou exécution de sub-agents.
- Ouverture task ouvre ou reprend une session agent pour un workspace.
- Création child task crée des child tasks ADO après rédaction du plan.
- Commit task crée un commit intermédiaire sans push ni PR.
- Finalisation task est le flow commit/push/PR attendu en fin de travail.
- Actions DB schema, describe et query sont les points d'entrée SQL et restent read-only par défaut.

Root configuré courant:

```text
{}
```

Règles importantes:

1. Les work items Azure DevOps sont la source de vérité.
2. Utiliser les actions DevWorkflow pour toute opération ADO, nommage Git, PR et worktree.
3. Ne pas utiliser les outils MCP Azure DevOps.
4. Lire le work item ADO avant de coder, puis charger le contexte IA ADO avant d'agir sur le contexte ADO.
5. Avant de travailler, vérifier que le setup initial requis par l'environnement est fait: `pnpm install`, validation des builds pnpm si nécessaire, `npm install` en fallback, ou `dotnet restore` selon le projet.
6. Mettre à jour `plan.md` avant d'implémenter.
7. Écrire tout texte utilisateur/projet en français.
8. Ne pas normaliser les labels métier ni le vocabulaire de domaine issus d'ADO, des screenshots, mockups ou textes projet.
9. Traiter les screenshots, mockups et attachments comme sources factuelles.
10. Les branches, commits et titres de PR sont créés par DevWorkflow; ne pas les créer manuellement.
"#,
        report.root
    )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoActionJsonProjection {
    Assigned,
    PullRequests,
    SetState,
    WorkItems,
    ContextExpanded,
    AiContextItems,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoActionRenderedOutput {
    Lines(Vec<String>),
    Json(String),
}

pub fn ado_action_output(
    result: &AdoActionResult,
    json_projection: Option<AdoActionJsonProjection>,
    theme: &TerminalTheme,
) -> serde_json::Result<AdoActionRenderedOutput> {
    if let Some(projection) = json_projection {
        return match (result, projection) {
            (AdoActionResult::Assigned(report), AdoActionJsonProjection::Assigned) => {
                if report.group_by_parent {
                    serde_json::to_string_pretty(&report.groups).map(AdoActionRenderedOutput::Json)
                } else {
                    serde_json::to_string_pretty(&report.items).map(AdoActionRenderedOutput::Json)
                }
            }
            (AdoActionResult::Prs(report), AdoActionJsonProjection::PullRequests) => {
                serde_json::to_string_pretty(&report.items).map(AdoActionRenderedOutput::Json)
            }
            (AdoActionResult::SetState(report), AdoActionJsonProjection::SetState) => {
                serde_json::to_string_pretty(report).map(AdoActionRenderedOutput::Json)
            }
            (AdoActionResult::WorkItem(report), AdoActionJsonProjection::WorkItems) => {
                serde_json::to_string_pretty(&report.items).map(AdoActionRenderedOutput::Json)
            }
            (AdoActionResult::Context(report), AdoActionJsonProjection::ContextExpanded) => {
                serde_json::to_string_pretty(&report.expanded).map(AdoActionRenderedOutput::Json)
            }
            (AdoActionResult::AiContext(report), AdoActionJsonProjection::AiContextItems) => {
                serde_json::to_string_pretty(&report.items).map(AdoActionRenderedOutput::Json)
            }
            _ => unreachable!("ADO JSON projection is incompatible with the action result"),
        };
    }

    Ok(AdoActionRenderedOutput::Lines(match result {
        AdoActionResult::AuthLogin(report) => auth_login_lines(report),
        AdoActionResult::AuthStatus(report) => auth_status_lines(report),
        AdoActionResult::AuthLogout(report) => auth_logout_lines(report),
        AdoActionResult::Assigned(report) => {
            let mut report = report.clone();
            report.events.clear();
            ado_assigned_lines(&report, theme)
        }
        AdoActionResult::Prs(report) => ado_prs_lines(report),
        AdoActionResult::Changelog(report) => {
            let mut report = report.clone();
            report.events.clear();
            ado_changelog_lines(&report, theme)
        }
        AdoActionResult::Context(report) => {
            let mut report = report.clone();
            report.events.clear();
            ado_context_lines(&report, theme)
        }
        AdoActionResult::AiContext(report) => serde_json::to_string_pretty(&report.items)
            .map(|json| json.lines().map(str::to_owned).collect())
            .unwrap_or_else(|error| vec![format!("JSON render error: {error}")]),
        AdoActionResult::WorkItem(report) => {
            let mut report = report.clone();
            report.events.clear();
            ado_work_item_lines(&report, theme)
        }
        AdoActionResult::SetStatePlan(report) => ado_set_state_plan_lines(report),
        AdoActionResult::SetState(report) => {
            let mut report = report.clone();
            report.events.clear();
            ado_set_state_execution_lines(&report)
        }
    }))
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
        format!("Items    : {}", report.items.len()),
    ];
    for (index, item) in report.items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.push(ado_assigned_field_line(
            "Item",
            ado_work_item_summary(item, theme),
        ));
        let ids = [item.id.clone()];
        lines.push(ado_start_command_line(&ids, &report.project, theme));
    }
    trim_trailing_blank_line(lines)
}

pub fn ado_set_state_execution_lines(
    report: &dw_ado_commands::commands::set_state::SetStateExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "ADO update".into(),
        format!("Project  : {}", report.plan.project),
        format!("State    : {}", report.plan.state),
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
        lines.extend(
            report
                .events
                .iter()
                .map(|event| format!("- {}", ado_action_event_line(event))),
        );
    }
    lines
}

pub fn ado_set_state_plan_lines(
    report: &dw_ado_commands::commands::set_state::SetStatePlanReport,
) -> Vec<String> {
    vec![
        "ADO update plan".into(),
        format!("Project  : {}", report.project),
        format!("State    : {}", report.state),
        format!("Work items: {}", format_ids(&report.ids)),
    ]
}

pub fn diagnostic_log_event_line(event: &dw_core::DiagnosticLogEvent) -> String {
    dw_ui::diagnostic_log_event_line(event)
}

pub fn ado_action_event_line(event: &AdoActionEvent) -> String {
    dw_ui::ado_action_event_line(event)
}

pub fn task_action_event_line(event: &TaskActionEvent) -> String {
    dw_ui::task_action_event_line(event)
}

pub fn db_action_event_line(event: &DbActionEvent) -> String {
    dw_ui::db_action_event_line(event)
}

fn format_ids<T: Display>(ids: &[T]) -> String {
    if ids.is_empty() {
        "none".into()
    } else {
        ids.iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn git_operation_label(operation: GitOperation) -> &'static str {
    match operation {
        GitOperation::CommitAndPush => "commit + push",
        GitOperation::Push => "push",
    }
}

fn join_display<T: Display>(items: &[T]) -> String {
    join_display_with_separator(items, ", ")
}

fn join_display_with_separator<T: Display>(items: &[T], separator: &str) -> String {
    items
        .iter()
        .map(|item| format!("{item}"))
        .collect::<Vec<_>>()
        .join(separator)
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
            "Context   : {}",
            theme.command(&format!(
                "dw ado context {} --project {}",
                item.id, report.project
            ))
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
            lines.push(format!("Metadata  : {metadata}"));
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
            lines.push(format!("Directory : {}", item.attachments.directory_hint));
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
            "AI context: {}",
            theme.command(&format!(
                "dw ado ai-context {} --project {}",
                item.work_item.id, report.project
            ))
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
            "No work item detected in git range commit messages."
        } else {
            "No work item detected for the given pull requests."
        })];
    }
    if report.ids_only {
        return vec![join_display_with_separator(&report.work_item_ids, " ")];
    }
    if report.resolved_empty {
        return vec![theme.warning("No work item resolved in Azure DevOps.")];
    }
    let document = dw_ui::render_ado_changelog_document(report);
    if report.format == dw_ado_commands::commands::changelog::ChangelogOutputFormat::Raw {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbQueryRenderedOutput {
    Table(String),
    Tsv(String),
}

impl DbQueryRenderedOutput {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Table(text) | Self::Tsv(text) => text,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbActionRenderedOutput {
    Lines(Vec<String>),
    Query(DbQueryRenderedOutput),
    Json(String),
    Empty,
}

pub fn db_action_output(
    result: &DbActionResult,
    json: bool,
    stdout_is_terminal: bool,
    theme: &TerminalTheme,
) -> serde_json::Result<DbActionRenderedOutput> {
    if json {
        return match result {
            DbActionResult::Guard(report) => {
                serde_json::to_string_pretty(report).map(DbActionRenderedOutput::Json)
            }
            DbActionResult::Schema(report) | DbActionResult::Query(report) => {
                serde_json::to_string_pretty(report).map(DbActionRenderedOutput::Json)
            }
            DbActionResult::Describe(Some(report)) => {
                serde_json::to_string_pretty(report).map(DbActionRenderedOutput::Json)
            }
            DbActionResult::Describe(None) => Ok(DbActionRenderedOutput::Empty),
        };
    }

    Ok(match result {
        DbActionResult::Guard(report) => {
            DbActionRenderedOutput::Lines(db_guard_lines(report, theme))
        }
        DbActionResult::Schema(report) | DbActionResult::Query(report) => {
            DbActionRenderedOutput::Query(db_query_output(report, stdout_is_terminal, theme))
        }
        DbActionResult::Describe(Some(report)) => {
            DbActionRenderedOutput::Query(db_query_output(report, stdout_is_terminal, theme))
        }
        DbActionResult::Describe(None) => DbActionRenderedOutput::Empty,
    })
}

pub fn db_query_output(
    result: &QueryResult,
    stdout_is_terminal: bool,
    theme: &TerminalTheme,
) -> DbQueryRenderedOutput {
    if stdout_is_terminal {
        DbQueryRenderedOutput::Table(db_query_table(result, theme))
    } else {
        DbQueryRenderedOutput::Tsv(db_query_tsv(result))
    }
}

pub fn db_query_tsv(result: &QueryResult) -> String {
    let mut lines = vec![
        result
            .columns
            .iter()
            .map(|column| column.as_str())
            .collect::<Vec<_>>()
            .join("\t"),
    ];
    lines.extend(result.rows.iter().map(|row| {
        row.iter()
            .map(|value| value.as_ref().map(|value| value.as_str()).unwrap_or("NULL"))
            .collect::<Vec<_>>()
            .join("\t")
    }));
    lines.push(if result.truncated {
        format!("-- {} rows (truncated)", result.rows.len())
    } else {
        format!("-- {} rows", result.rows.len())
    });
    lines.join("\n")
}

pub fn upgrade_report_lines(report: &dw_upgrade::UpgradeReport) -> Vec<String> {
    match report {
        dw_upgrade::UpgradeReport::Check(report) => {
            let mut lines = vec![
                "Upgrade available".into(),
                format!("  Release : {}", report.release_tag),
                format!("  Version : {}+{}", report.version, report.commit),
                String::new(),
                "Path".into(),
                "  ✓ GitHub release resolved".into(),
                "  ✓ Manifest read".into(),
                format!("  ✓ {} compatible artifact(s)", report.assets.len()),
            ];
            lines.extend(report.assets.iter().map(|asset| {
                format!(
                    "    • {:14} {} {}",
                    asset.rid, asset.file_name, asset.sha256
                )
            }));
            lines
        }
        dw_upgrade::UpgradeReport::Installed(report) => {
            let mut lines = upgrade_install_summary_header(
                "Upgrade ready",
                &report.version,
                &report.commit,
                &report.executable_path,
            );
            lines.extend([
                String::new(),
                "Path".into(),
                "  ✓ GitHub release resolved".into(),
                "  ✓ Artifact selected".into(),
                "  ✓ Binary downloaded".into(),
                "  ✓ SHA256 verified".into(),
                "  ✓ Executable prepared".into(),
            ]);
            if report.deferred_windows_replacement {
                lines.push("  → Replacement scheduled after dw exits".into());
            } else {
                lines.push("  ✓ Active binary replaced".into());
            }
            lines
        }
    }
}

fn upgrade_install_summary_header(
    title: &str,
    version: &dw_core::SemanticVersion,
    commit: &dw_core::GitCommitSha,
    executable_path: &dw_core::ExecutablePath,
) -> Vec<String> {
    vec![
        title.into(),
        format!("  Version : {version}+{commit}"),
        format!("  Binary  : {executable_path}"),
    ]
}

pub fn upgrade_event_line(event: &dw_core::UpgradeActionEvent) -> String {
    dw_ui::upgrade_action_event_line(event)
}

pub fn upgrade_event_is_transient(event: &dw_core::UpgradeActionEvent) -> bool {
    matches!(
        event,
        dw_core::UpgradeActionEvent::DownloadedAssetBytes { .. }
    )
}

pub fn upgrade_download_progress_line(
    file_name: &dw_core::UpgradeFileName,
    received: dw_core::ByteCount,
    total: Option<dw_core::ByteCount>,
    theme: &TerminalTheme,
) -> String {
    let received_bytes = received.as_u64();
    let size = match total {
        Some(total) => format!(
            "{} / {}",
            human_bytes(received_bytes),
            human_bytes(total.as_u64())
        ),
        None => human_bytes(received_bytes),
    };
    let bar = upgrade_download_progress_bar(received, total, theme);
    match total.and_then(|total| progress_percent(received, total)) {
        Some(percent) => {
            format!(
                "{} {bar} {percent:>3}% {size} {file_name}",
                dw_core::UpgradeActionEvent::DOWNLOADED_ASSET_BYTES_ACTION_ID
            )
        }
        None => format!(
            "{} {bar} {size} {file_name}",
            dw_core::UpgradeActionEvent::DOWNLOADED_ASSET_BYTES_ACTION_ID
        ),
    }
}

fn upgrade_download_progress_bar(
    received: dw_core::ByteCount,
    total: Option<dw_core::ByteCount>,
    theme: &TerminalTheme,
) -> String {
    const WIDTH: usize = 28;
    let filled = total
        .and_then(|total| {
            let total = total.as_u64();
            (total > 0).then(|| {
                ((received.as_u64().min(total) as f64 / total as f64) * WIDTH as f64).round()
                    as usize
            })
        })
        .unwrap_or(0)
        .min(WIDTH);
    let empty = WIDTH - filled;
    let raw = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));
    if total.is_some() {
        theme.success(&raw)
    } else {
        theme.cyan("[····························]")
    }
}

fn progress_percent(received: dw_core::ByteCount, total: dw_core::ByteCount) -> Option<u64> {
    let total = total.as_u64();
    (received.as_u64().min(total) * 100).checked_div(total)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = UNITS[0];
    for &next in UNITS.iter().skip(1) {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if unit == "B" {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {unit}")
    }
}

pub fn task_status_lines(report: &TaskStatusReport) -> Vec<String> {
    let mut lines = vec![
        "Task workspaces".into(),
        format!("Root      : {}", report.root),
        format!("Detected : {}", report.items.len()),
    ];

    if report.items.is_empty() {
        lines.push("No task workspace found.".into());
        return lines;
    }

    lines.push("Details".into());
    for item in &report.items {
        lines.push(format!(
            "- {} {} {}",
            item.project,
            item.kind,
            format_current_work_items(&item.work_items)
        ));
        lines.push(format!("  Branch      : {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!(
                "  Repositories: {}",
                join_display(&item.repositories)
            ));
        }
        lines.push(format!("  Path        : {}", item.path));
    }
    lines
}

pub fn task_list_lines(report: &TaskListReport) -> Vec<String> {
    if report.items.is_empty() {
        return vec!["No task workspace found.".into()];
    }

    let mut lines = vec![
        format!("Task workspaces: {}", report.items.len()),
        "Project Created    Type   Work items".into(),
    ];

    for item in &report.items {
        lines.push(format!(
            "{:<7} {}  {:<6} {}",
            item.project,
            created_date(&item.created_at),
            item.kind,
            format_current_work_items(&item.work_items)
        ));
        lines.push(format!("  Branch: {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!(
                "  Repositories: {}",
                join_display(&item.repositories)
            ));
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

    lines.push(format!(
        "Repositories: {}",
        join_display(&item.repositories)
    ));
    lines
}

pub fn task_preflight_lines(report: &TaskPreflightReport) -> Vec<String> {
    let mut lines = vec![
        "Task preflight".into(),
        format!(
            "Status   : {}",
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
        lines.push("✓ No warnings or blockers detected.".into());
        return lines;
    }

    let blocking_count = report
        .issues
        .iter()
        .filter(|issue| issue.severity.is_blocking())
        .count();
    let warning_count = report
        .issues
        .iter()
        .filter(|issue| issue.severity.is_warning())
        .count();
    let other_count = report
        .issues
        .len()
        .saturating_sub(blocking_count + warning_count);
    lines.push(format!("Blockers : {blocking_count}"));
    lines.push(format!("Warnings : {warning_count}"));
    lines.push(format!("Infos    : {other_count}"));
    lines.push(String::new());
    push_preflight_issue_group(&mut lines, "Blockers", report, |severity| {
        severity.is_blocking()
    });
    push_preflight_issue_group(&mut lines, "Warnings", report, |severity| {
        severity.is_warning()
    });
    push_preflight_issue_group(&mut lines, "Infos", report, |severity| {
        !severity.is_blocking() && !severity.is_warning()
    });

    if report.has_blocking_issues {
        lines.push(String::new());
        lines.push(
            "Blockers detected: ask for user confirmation before forcing implementation.".into(),
        );
    }

    lines
}

pub fn task_handoff_validation_lines(report: &TaskHandoffValidationReport) -> Vec<String> {
    let mut lines = vec![
        "Validation handoff".into(),
        format!("Status   : {}", validation_status_label(report.is_valid)),
        format!("Workspace : {}", report.workspace),
        format!("Project   : {}", report.project),
        format!(
            "Handoffs : {}/{} valid",
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
        "Mode     : preview".into(),
        format!("Root      : {}", report.root),
        format!("Candidates: {}", report.candidates.len()),
        "To do    : dw task prune --execute".into(),
        "Non-TTY  : add --yes to delete everything without interactive selection".into(),
    ];

    if !report.sync.is_empty() {
        lines.push(String::new());
        lines.push("ADO synchronization".into());
        for item in &report.sync {
            lines.push(format!(
                "- {} [{}] {}",
                item.workspace,
                prune_sync_status_label(&item.status),
                prune_sync_detail_label(&item.detail)
            ));
        }
    }

    if report.candidates.is_empty() {
        lines.push(String::new());
        lines.push("No workspace eligible for prune.".into());
        return lines;
    }

    for candidate in &report.candidates {
        lines.push(String::new());
        lines.push(format!("Workspace : {}", candidate.path));
        lines.push(format!(
            "Items    : {}",
            dw_task::prune::prune_candidate_label(candidate)
        ));
        lines.push(format!(
            "Repositories: {}",
            candidate
                .manifest
                .repositories
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines
}

pub fn task_prune_execution_lines(report: &dw_task::prune::PruneExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace cleanup".into(),
        "Mode     : execution".into(),
        format!("Root      : {}", report.root),
        format!("Deleted  : {}", report.deleted.len()),
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
        format!("Synchronized: {}", report.updated.len()),
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
        lines.extend(repository_status_detail_lines_fr(&item.status.detail));
    }

    lines.push(String::new());
    if nothing_to_commit {
        lines.push("Nothing to commit.".into());
    } else {
        lines.push(format!("Message   : {}", report.message));
        if !execute {
            lines.push("To do    : dw task commit --execute".into());
        }
    }
    lines
}

pub fn task_commit_execution_lines(report: &dw_task::repo::CommitExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Repository commit".into(),
        "Mode     : execution".into(),
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
        format!("To do    : dw task add-repo {} --execute", plan.repository),
    ]
}

pub fn task_add_repo_execution_lines(
    report: &dw_task::repo::AddRepoExecutionReport,
) -> Vec<String> {
    vec![
        "Add repository".into(),
        "Mode     : execution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Repository: {}", report.worktree.repository),
        format!("Status    : {}", report.worktree.status),
        format!(
            "Detail    : {}",
            worktree_prepare_detail_fr(&report.worktree.detail)
        ),
    ]
}

fn worktree_prepare_detail_fr(detail: &dw_git::WorktreePrepareDetail) -> String {
    match detail {
        dw_git::WorktreePrepareDetail::MissingRemoteUrl => {
            "Remote URL missing in projects.json.".into()
        }
        dw_git::WorktreePrepareDetail::AlreadyPresent => "Worktree already present.".into(),
        dw_git::WorktreePrepareDetail::CreatedFromExistingBranch { branch } => {
            format!("Worktree created from existing branch {branch}.")
        }
        dw_git::WorktreePrepareDetail::CreatedFromBaseReference { reference } => {
            format!("Worktree created from {reference}.")
        }
    }
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
            "Actions applied".into()
        } else {
            "Planned actions".into()
        },
    ];
    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.subject,
            step.action,
            step.target_path()
        ));
    }
    if !execute {
        lines.push(String::new());
        lines.push("To do    : dw task teardown --execute".into());
        lines.push("Non-TTY  : add --yes to confirm without a prompt".into());
    }
    lines
}

pub fn task_teardown_execution_lines(
    report: &dw_task::repo::TeardownExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Workspace removal".into(),
        "Mode     : execution".into(),
        format!("Workspace : {}", report.workspace),
        format!("Actions   : {}", report.steps.len()),
    ];
    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.subject,
            step.action,
            step.target_path()
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
        lines.push("ADO work items".into());
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
        "Mode     : preview".into(),
        format!("Slug      : {} -> {}", plan.old_slug, plan.new_slug),
        format!("Branch    : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        "To do    : dw task rename <slug> --execute".into(),
    ]
}

pub fn task_rename_execution_lines(
    report: &dw_task::lifecycle::RenameExecutionReport,
) -> Vec<String> {
    vec![
        "Workspace rename".into(),
        "Mode     : execution".into(),
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
        "Status   : saved in the workspace".into(),
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
            "Mode     : preview".into(),
            format!("Action   : {}", work_item_action_label(report.action)),
            format!("Workspace : {}", report.workspace),
            "Status   : no change".into(),
            "All requested work items are already present in the workspace.".into(),
        ];
    };
    vec![
        "Work items workspace".into(),
        "Mode     : preview".into(),
        format!("Action   : {}", work_item_action_label(report.action)),
        format!("Branch    : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        format!(
            "Items    : {}",
            plan.work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "To do    : dw task {} --execute",
            work_item_action_command(report.action)
        ),
    ]
}

pub fn task_work_item_execution_lines(
    report: &dw_task::work_item::WorkItemUpdateExecutionReport,
) -> Vec<String> {
    vec![
        "Work items workspace".into(),
        "Mode     : execution".into(),
        format!("Action   : {}", work_item_action_label(report.action)),
        format!(
            "Branch    : {} -> {}",
            report.plan.old_branch, report.plan.new_branch
        ),
        format!(
            "Workspace : {} -> {}",
            report.plan.workspace, report.plan.new_workspace
        ),
        format!(
            "Items    : {}",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskStartCreateCommandOptions {
    pub skip_ado: bool,
    pub with_active_children: bool,
    pub create_child_tasks: bool,
}

pub fn task_start_create_command(
    report: &dw_task::start::StartPlanReport,
    options: TaskStartCreateCommandOptions,
) -> String {
    let plan = &report.plan;
    let mut parts = vec!["dw".into(), "task".into(), "start".into()];
    parts.push(shell_arg(&join_display_with_separator(
        &plan.work_item_ids,
        ",",
    )));
    parts.push("--project".into());
    parts.push(shell_arg(plan.project.as_str()));
    if let Some(task) = &plan.task_id {
        parts.push("--task".into());
        parts.push(shell_arg(task.as_str()));
    }
    parts.push("--type".into());
    parts.push(shell_arg(plan.kind.as_str()));
    if !plan.repositories.is_empty() {
        parts.push("--only".into());
        parts.push(shell_arg(&join_display_with_separator(
            &plan.repositories,
            ",",
        )));
    }
    parts.push("--slug".into());
    parts.push(shell_arg(plan.slug.as_str()));
    if options.skip_ado {
        parts.push("--skip-ado".into());
    }
    if options.with_active_children {
        parts.push("--with-active-children".into());
    }
    if options.create_child_tasks {
        parts.push("--create-child-tasks".into());
    }
    parts.push("--execute".into());
    parts.join(" ")
}

pub fn task_start_open_command(workspace: &dw_core::WorkspacePath) -> String {
    format!("dw task open --workspace {}", shell_arg(workspace.as_str()))
}

fn shell_arg(value: &str) -> String {
    if value.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | '-' | '/' | ':' | ',' | '#')
    }) {
        return value.into();
    }
    format!("\"{}\"", value.replace('"', "\\\""))
}

pub fn task_start_plan_lines(report: &dw_task::start::StartPlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Task start plan".into(),
        format!("Project: {}", plan.project),
        format!("Work items: {}", join_display(&plan.work_item_ids)),
        format!("Slug: {}", plan.slug),
        format!("Target branch: {}", plan.branch_name),
        format!("Target workspace: {}", plan.workspace),
        format!("Repositories: {}", join_display(&plan.repositories)),
        "Action: preview only; answer yes when prompted or run the command below.".into(),
    ]
}

pub fn task_start_execution_lines(report: &dw_task::start::StartExecutionReport) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace created: {}", report.plan.workspace),
        format!("Target branch: {}", report.plan.branch_name),
        format!("Repositories: {}", join_display(&report.plan.repositories)),
    ];
    for task in &report.child_tasks {
        lines.push(format!(
            "ADO task created [{}]: #{} {}",
            task.repository,
            task.id,
            task.title
                .as_ref()
                .map(|title| title.as_str())
                .unwrap_or("(untitled)")
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
    lines.push(format!(
        "Open command: {}",
        task_start_open_command(&report.plan.workspace)
    ));
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
                join_display(&report.repositories)
            }
        ),
        task_start_pr_resolved_line(&report.work_item_ids),
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
            item.target.repository.as_str(),
            &item.status,
        ));
    }

    lines.push(String::new());
    lines.push("Handoff validation".into());
    lines.push(format!(
        "Status   : {}",
        if report.handoff.is_valid { "OK" } else { "KO" }
    ));
    for item in &report.handoff.items {
        lines.push(format!(
            "- [{}] {} - {}",
            item.status,
            item.repository,
            handoff_validation_message(item)
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
        lines.push("To do    : dw task finish --execute".into());
        lines.push("Non-TTY  : add --yes to confirm without a prompt".into());
    }

    lines
}

pub fn task_finish_execution_lines(report: &dw_task::finish::FinishExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Workspace finish".into(),
        "Mode     : execution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Branch    : {}", report.plan.manifest.branch_name),
    ];
    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push("Events".into());
        lines.extend(
            report
                .events
                .iter()
                .map(|event| format!("- {}", task_action_event_line(event))),
        );
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
                action.repository,
                git_operation_label(action.operation),
                action.path
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
        lines.push("ADO work items".into());
        for update in &report.work_item_updates {
            lines.push(finish_work_item_update_line(update));
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

fn task_start_pr_resolved_line<T: Display>(work_item_ids: &[T]) -> String {
    match work_item_ids.len() {
        0 => "No work item linked to the PR.".into(),
        1 => format!("PR linked to work item #{}.", work_item_ids[0]),
        count => format!(
            "PR linked to {count} work items: {}.",
            work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn finish_work_item_update_line(update: &dw_task::finish::FinishWorkItemStateUpdate) -> String {
    match update.outcome {
        dw_task::finish::FinishWorkItemStateOutcome::UnsupportedWorkItemType => {
            format!("ADO item {}: state unchanged for this type", update.label)
        }
        dw_task::finish::FinishWorkItemStateOutcome::AlreadyInTargetState => format!(
            "ADO item {}: already in state {}",
            update.label,
            update
                .target_state
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "target".into())
        ),
        dw_task::finish::FinishWorkItemStateOutcome::Updated => format!(
            "ADO item {}: state -> {}",
            update.label,
            update
                .target_state
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "target".into())
        ),
    }
}

pub fn task_finish_dry_run_hint(no_changes: bool, create_pr: bool) -> &'static str {
    if create_pr {
        "Preview only. Rerun with --execute to push/create PR."
    } else if no_changes {
        "Preview only. Rerun with --execute --skip-ado to push."
    } else {
        "Preview only. Rerun with --execute --skip-ado to commit/push."
    }
}

pub fn doctor_report_lines(report: &DoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let passed_count = report.passed_count();
    let total_count = report.checks.len();
    let failed_count = report.failed_count();
    let mut lines = vec![
        theme.command("Dev Workflow diagnostics"),
        format!(
            "{} {passed_count}/{total_count} checks passed",
            if failed_count == 0 {
                theme.success("✓")
            } else {
                theme.warning("!")
            }
        ),
        format!(
            "Status   : {}",
            if failed_count == 0 {
                "healthy"
            } else {
                "needs fixes"
            }
        ),
        format!("Blockers : {failed_count}"),
        String::new(),
    ];
    lines.extend(render_doctor_check_group(
        "Needs fixes",
        report.checks.iter().filter(|check| !check.passed).collect(),
        theme,
    ));
    lines.extend(render_doctor_check_group(
        "Checks",
        report.checks.iter().filter(|check| check.passed).collect(),
        theme,
    ));
    lines
}

fn created_date(value: &Timestamp) -> &str {
    value.as_str().get(..10).unwrap_or_else(|| value.as_str())
}

fn format_current_work_items(items: &[dw_workspace::WorkspaceWorkItem]) -> String {
    items
        .iter()
        .map(|item| {
            let title = item
                .title
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(untitled)".into());
            let metadata = [
                item.kind.as_ref().map(|kind| kind.as_str()),
                item.state.as_ref().map(|state| state.as_str()),
            ]
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
    lines.extend(repository_status_detail_lines_fr(&status.detail));
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

fn push_finish_summary_list(
    lines: &mut Vec<String>,
    label: &str,
    items: &[dw_workspace::HandoffSummaryEntry],
) {
    if !items.is_empty() {
        lines.push(format!(
            "{label}: {}",
            join_display_with_separator(items, " | ")
        ));
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
            finish_pull_request_skip_reason_label(result.skip_reason)
        ),
    }
}

fn finish_pull_request_skip_reason_label(
    reason: Option<dw_task::finish::FinishPullRequestSkipReason>,
) -> &'static str {
    match reason {
        Some(dw_task::finish::FinishPullRequestSkipReason::MissingAdoRepository) => {
            "missing azureDevOpsRepository"
        }
        None => "unknown reason",
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
        lines.push(ado_assigned_field_line(
            "Parent",
            ado_work_item_summary(&group.parent, theme),
        ));
        if !group.items.is_empty() {
            lines.push(ado_start_command_line(
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
                "  {}",
                ado_assigned_field_line("Child", ado_work_item_summary(item, theme))
            ));
        }
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines)
}

fn ado_start_command_line(
    ids: &[dw_core::WorkItemId],
    project: &impl std::fmt::Display,
    theme: &TerminalTheme,
) -> String {
    let ids = join_display_with_separator(ids, ",");
    ado_assigned_field_line(
        "Start",
        theme.command(&format!("dw task start {ids} --project {project}")),
    )
}

fn ado_assigned_field_line(label: &str, value: String) -> String {
    format!("{label:<5}: {value}")
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
    let target = relation
        .work_item_id
        .as_ref()
        .map(|id| format!("#{id}"))
        .or_else(|| relation.name.clone())
        .or_else(|| relation.url.clone())
        .unwrap_or_default();
    if target.is_empty() {
        relation.kind.clone()
    } else {
        format!("{} {}", relation.kind, target)
    }
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

fn push_preflight_issue_group(
    lines: &mut Vec<String>,
    title: &str,
    report: &TaskPreflightReport,
    predicate: impl Fn(TaskPreflightSeverity) -> bool,
) {
    let issues = report
        .issues
        .iter()
        .filter(|issue| predicate(issue.severity))
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
            preflight_issue_message(issue)
        ));
        if let Some(details) = preflight_issue_detail(issue) {
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

fn preflight_issue_message(issue: &TaskPreflightIssue) -> String {
    match issue.code {
        TaskPreflightIssueCode::WorkspaceAdoContextStale => format!(
            "The workspace local ADO context appears stale for #{}.",
            issue.work_item_id
        ),
        TaskPreflightIssueCode::AdoAttachmentsPresent => format!(
            "Work item #{} has attachments to treat as factual sources.",
            issue.work_item_id
        ),
    }
}

fn preflight_issue_detail(issue: &TaskPreflightIssue) -> Option<String> {
    match &issue.detail {
        TaskPreflightIssueDetail::WorkspaceAdoContextStale { reasons } => (!reasons.is_empty())
            .then(|| {
                reasons
                    .iter()
                    .map(preflight_stale_reason_label)
                    .collect::<Vec<_>>()
                    .join("; ")
            }),
        TaskPreflightIssueDetail::AdoAttachmentsPresent {
            directory_hint,
            names,
        } => Some(if names.is_empty() {
            format!("Attachments present. Expected directory: {directory_hint}")
        } else {
            format!(
                "Attachments present: {}. Expected directory: {directory_hint}",
                names.join(", ")
            )
        }),
    }
}

fn preflight_stale_reason_label(reason: &TaskPreflightStaleReason) -> &'static str {
    match reason {
        TaskPreflightStaleReason::Title => "local title differs from ADO",
        TaskPreflightStaleReason::State => "local state differs from ADO",
        TaskPreflightStaleReason::Kind => "local type differs from ADO",
    }
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
        lines.push(format!("  Message: {}", handoff_validation_message(item)));
        if !item.path.as_str().trim().is_empty() {
            lines.push(format!("  File   : {}", item.path));
        }
        if item.valid {
            lines.push(format!(
                "  Summary: done={} decisions={} risks={} blockers={} follow_up={}",
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

fn handoff_validation_message(item: &TaskHandoffValidationItem) -> String {
    match &item.detail {
        TaskHandoffValidationDetail::MissingFile => "Missing handoff file.".into(),
        TaskHandoffValidationDetail::Valid => "Handoff is valid.".into(),
        TaskHandoffValidationDetail::NotFinishReady => format!(
            "Handoff is parseable but not ready for finish (status: {}).",
            item.status
        ),
        TaskHandoffValidationDetail::InvalidFile { reason } => reason.to_string(),
    }
}

fn validation_status_label(valid: bool) -> &'static str {
    if valid { "✓ OK" } else { "✕ Needs fixes" }
}

fn severity_icon(severity: &TaskPreflightSeverity) -> &'static str {
    match severity {
        TaskPreflightSeverity::Blocking => "✕",
        TaskPreflightSeverity::Warning => "!",
        TaskPreflightSeverity::Info => "-",
    }
}

fn severity_label(severity: &TaskPreflightSeverity) -> &'static str {
    match severity {
        TaskPreflightSeverity::Blocking => "[blocker]",
        TaskPreflightSeverity::Warning => "[warning]",
        TaskPreflightSeverity::Info => "[info]",
    }
}

fn handoff_status_label(status: &TaskHandoffValidationStatus) -> &'static str {
    match status {
        TaskHandoffValidationStatus::Missing => "missing",
        TaskHandoffValidationStatus::Invalid => "invalid",
        TaskHandoffValidationStatus::Blocked => "blocked",
        TaskHandoffValidationStatus::Todo => "todo",
        TaskHandoffValidationStatus::InProgress => "in_progress",
        TaskHandoffValidationStatus::Valid => "valid",
    }
}

fn handoff_status_icon(status: &TaskHandoffValidationStatus, valid: bool) -> &'static str {
    if valid {
        return "✓";
    }
    match status {
        TaskHandoffValidationStatus::Missing
        | TaskHandoffValidationStatus::Invalid
        | TaskHandoffValidationStatus::Blocked => "✕",
        TaskHandoffValidationStatus::Todo | TaskHandoffValidationStatus::InProgress => "!",
        TaskHandoffValidationStatus::Valid => "-",
    }
}

fn prune_sync_status_label(status: &dw_task::prune::PruneSyncStatus) -> &'static str {
    match status {
        dw_task::prune::PruneSyncStatus::Skipped => "skipped",
        dw_task::prune::PruneSyncStatus::Synced => "synchronized",
    }
}

fn prune_sync_detail_label(detail: &dw_task::prune::PruneSyncDetail) -> String {
    match detail {
        dw_task::prune::PruneSyncDetail::AuthUnavailable { error } => {
            format!("auth unavailable: {error}")
        }
        dw_task::prune::PruneSyncDetail::SyncFailed { error } => error.clone(),
        dw_task::prune::PruneSyncDetail::Synced { work_items } => {
            format_current_work_items(work_items)
        }
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
        "No changes."
    }
}

fn repository_status_detail_lines_fr(detail: &dw_git::RepositoryStatusDetail) -> Vec<String> {
    match detail {
        dw_git::RepositoryStatusDetail::MissingDirectory => vec!["Missing directory.".into()],
        dw_git::RepositoryStatusDetail::OpenFailed { detail }
        | dw_git::RepositoryStatusDetail::StatusFailed { detail } => vec![detail.to_string()],
        dw_git::RepositoryStatusDetail::Changed { paths } => {
            paths.iter().map(ToString::to_string).collect()
        }
        dw_git::RepositoryStatusDetail::Unpushed { ahead } => {
            vec![format!("{ahead} unpushed commit(s).")]
        }
        dw_git::RepositoryStatusDetail::Clean => Vec::new(),
    }
}

fn work_item_line(item: &dw_workspace::WorkspaceWorkItem) -> String {
    format!(
        "#{} [{} / {}] {}",
        item.id,
        item.kind
            .as_ref()
            .map(|kind| kind.as_str())
            .unwrap_or("unknown type"),
        item.state
            .as_ref()
            .map(|state| state.as_str())
            .unwrap_or("unknown state"),
        item.title
            .as_ref()
            .map(|title| title.as_str())
            .unwrap_or("(untitled)")
    )
}

fn work_item_action_label(action: dw_task::work_item::WorkItemUpdateAction) -> &'static str {
    match action {
        dw_task::work_item::WorkItemUpdateAction::Add => "add",
        dw_task::work_item::WorkItemUpdateAction::Remove => "remove",
    }
}

fn work_item_action_command(action: dw_task::work_item::WorkItemUpdateAction) -> &'static str {
    match action {
        dw_task::work_item::WorkItemUpdateAction::Add => "add-work-item",
        dw_task::work_item::WorkItemUpdateAction::Remove => "remove-work-item",
    }
}

pub fn secret_set_lines(report: &SecretSetReport) -> Vec<String> {
    vec![
        "Secret".into(),
        "Status   : saved".into(),
        format!("Key      : {}", report.key),
        format!("Storage  : {}", report.storage),
        "Value    : hidden".into(),
    ]
}

pub fn secret_get_lines(report: &SecretGetReport) -> Vec<String> {
    vec![
        "Secret".into(),
        format!(
            "Status   : {}",
            if report.exists {
                "present"
            } else {
                "not found"
            }
        ),
        format!("Key      : {}", report.key),
        "Value    : hidden".into(),
    ]
}

pub fn secret_delete_lines(report: &SecretDeleteReport) -> Vec<String> {
    vec![
        "Secret".into(),
        "Status   : deleted if present".into(),
        format!("Key      : {}", report.key),
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
        status, check.agent, check.command
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
            theme.success("✓")
        } else {
            theme.error("! Needs fixes")
        };
        lines.push(format!("{:<12} {}", status, doctor_check_label(check.kind)));
        if let Some(detail) = doctor_check_detail(check.detail.as_ref()) {
            lines.push(format!("         {}", theme.path(&detail)));
        }
        if !check.passed {
            lines.push(format!(
                "         {}",
                theme.command(&doctor_remediation_label(&check.remediation))
            ));
        }
    }
    lines.push(String::new());
    lines
}

fn doctor_check_label(kind: DoctorCheckKind) -> &'static str {
    match kind {
        DoctorCheckKind::DevWorkflowRoot => "DevWorkflow root",
        DoctorCheckKind::UserConfiguration => "User configuration",
        DoctorCheckKind::DefaultAgent => "Default agent",
        DoctorCheckKind::Git => "Git",
        DoctorCheckKind::NodePackageManager => "pnpm/npm",
        DoctorCheckKind::OpenCode => "OpenCode",
    }
}

fn doctor_check_detail(detail: Option<&DoctorCheckDetail>) -> Option<String> {
    match detail? {
        DoctorCheckDetail::Path { path } => Some(path.to_string()),
        DoctorCheckDetail::Agent { agent } => Some(agent.to_string()),
        DoctorCheckDetail::ProcessOutput { line } => Some(line.to_string()),
        DoctorCheckDetail::PackageManagerVersion { manager, version } => {
            Some(format!("{manager} {version}"))
        }
    }
    .filter(|value| !value.trim().is_empty())
}

fn doctor_remediation_label(remediation: &DoctorRemediation) -> String {
    match remediation {
        DoctorRemediation::InitRoot { root } => {
            format!("Initialize the DevWorkflow root: {root}")
        }
        DoctorRemediation::RunInit => "Run: dw init".into(),
        DoctorRemediation::ConfigureDefaultAgent { agent } => {
            format!("Configure: dw agent config set-default {agent}")
        }
        DoctorRemediation::InstallGit => "Install Git, then rerun dw doctor".into(),
        DoctorRemediation::InstallNodePackageManager => {
            "Install pnpm, or Node.js/npm if pnpm is unavailable.".into()
        }
        DoctorRemediation::InstallOpenCode => {
            "Install OpenCode using the team procedure, then check PATH".into()
        }
    }
}

fn render_query_result_table(result: &QueryResult, theme: &TerminalTheme) -> String {
    let columns = if result.columns.is_empty() {
        vec!["Result"]
    } else {
        result
            .columns
            .iter()
            .map(|column| column.as_str())
            .collect()
    };
    let rows = if result.columns.is_empty() && result.rows.is_empty() {
        Vec::new()
    } else {
        result
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| value.as_ref().map(|value| value.as_str()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let widths = db_column_widths(&columns, &rows);
    let mut lines = Vec::new();

    lines.push(theme.bold(&theme.cyan("DB query")));
    lines.push(format!(
        "Result   : {}",
        theme.bold(&db_row_count_label(result))
    ));
    lines.push(db_separator(&widths));
    lines.push(db_row(
        &columns
            .iter()
            .map(|column| Some(*column))
            .collect::<Vec<_>>(),
        &widths,
        Some(theme),
    ));
    lines.push(db_separator(&widths));
    lines.extend(rows.iter().map(|row| db_row(row, &widths, None)));
    lines.push(db_separator(&widths));
    if result.truncated {
        lines.push(theme.warning(&format!(
            "Result truncated after {} row(s). Rerun with --max-rows to expand.",
            result.rows.len()
        )));
    }
    lines.join("\n")
}

fn render_sql_guard(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    let mut lines = vec![theme.bold(&theme.cyan("SQL guard"))];
    lines.push(format!(
        "Status   : {}",
        db_guard_status_label(result, theme)
    ));
    if result.is_allowed {
        lines.push(format!("Decision : {}", theme.success("✓")));
        lines.push("Message  : Query allowed as read-only.".into());
        lines.push(format!(
            "Detail   : {}",
            theme.dim("This command did not start any execution.")
        ));
    } else {
        lines.push(format!("Decision : {}", theme.error("!")));
        lines.push("Message  : Query blocked before execution.".into());
        lines.push(format!(
            "Reason   : {}",
            result
                .reason
                .as_ref()
                .map(|reason| reason.as_str())
                .unwrap_or("unknown reason")
        ));
        lines.push(format!(
            "To do    : {}",
            theme.warning("Use only SELECT/WITH or introspection commands.")
        ));
    }
    lines.join("\n")
}

fn db_row_count_label(result: &QueryResult) -> String {
    let suffix = if result.rows.len() > 1 { "s" } else { "" };
    if result.truncated {
        format!("{} row{suffix} shown, result truncated", result.rows.len())
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

fn db_column_widths(columns: &[&str], rows: &[Vec<Option<&str>>]) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let row_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| db_display_cell(*value).chars().count())
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
        TaskPreflightIssue, TaskPreflightIssueCode, TaskPreflightIssueDetail,
    };

    #[test]
    fn upgrade_event_line_renders_one_step_per_action() {
        let event = dw_core::UpgradeActionEvent::DownloadingAsset {
            file_name: dw_core::UpgradeFileName::from("dw-linux-x64.tar.gz"),
        };
        let action_id = event.action_id();
        let line = upgrade_event_line(&event);

        assert!(line.contains(action_id.as_str()));
        assert!(line.contains("file=dw-linux-x64.tar.gz"));
        assert!(!line.contains("download/checksum"));
    }

    #[test]
    fn task_list_lines_render_table_and_paths() {
        let report = TaskListReport {
            root: dw_core::DevWorkflowRoot::from("/tmp/dw"),
            project: None,
            work_item_ids: Vec::new(),
            items: vec![dw_workspace::TaskListItem {
                path: "/tmp/ws".into(),
                project: "ha".into(),
                work_item_id: "42".into(),
                work_items: vec![dw_workspace::WorkspaceWorkItem {
                    id: "42".into(),
                    kind: Some("User Story".into()),
                    title: Some("Titre".into()),
                    state: Some("Actif".into()),
                }],
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

        assert_eq!(lines[0], "Task workspaces: 1");
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
    fn ado_assigned_lines_render_start_command() {
        let report = dw_ado_commands::commands::assigned::AssignedReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            top: 20,
            include_final_states: false,
            group_by_parent: false,
            items: vec![
                dw_ado::WorkItemSnapshot {
                    id: "42".into(),
                    kind: Some("Bug".into()),
                    state: Some("En developpement".into()),
                    title: Some("Corriger".into()),
                    url: None,
                },
                dw_ado::WorkItemSnapshot {
                    id: "43".into(),
                    kind: Some("Task".into()),
                    state: Some("Actif".into()),
                    title: Some("Verifier".into()),
                    url: None,
                },
            ],
            groups: Vec::new(),
            events: Vec::new(),
        };

        let lines = ado_assigned_lines(&report, &TerminalTheme::plain());

        assert!(lines.contains(&"ADO assigned".into()));
        assert!(lines.contains(&"Items    : 2".into()));
        assert!(lines.contains(&"Item : #42 [Bug / En developpement] Corriger".into()));
        assert!(lines.contains(&"Start: dw task start 42 --project ha".into()));
        assert!(lines.contains(&"Item : #43 [Task / Actif] Verifier".into()));
        assert!(lines.contains(&"Start: dw task start 43 --project ha".into()));
        assert!(!lines.windows(2).any(|pair| pair == ["", ""]));
    }

    #[test]
    fn ado_assigned_group_lines_render_parent_children_and_start_command() {
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
        assert!(lines.contains(&"Parent: #42 [User Story / Actif] Parent".into()));
        assert!(lines.contains(&"Start: dw task start 42,43 --project ha".into()));
        assert!(lines.contains(&"  Child: #43 [Task / Actif] Enfant".into()));
    }

    #[test]
    fn ado_set_state_execution_lines_include_ids_and_state() {
        let report = dw_ado_commands::commands::set_state::SetStateExecutionReport {
            plan: dw_ado_commands::commands::set_state::SetStatePlanReport {
                root: "/tmp/dw".into(),
                project: "ha".into(),
                ids: vec![
                    dw_core::WorkItemId::from("42"),
                    dw_core::WorkItemId::from("43"),
                ],
                state: "Actif".into(),
                history: "dw ado set-state".into(),
            },
            events: vec![dw_core::AdoActionEvent::UpdatedWorkItemState {
                id: dw_core::WorkItemId::from("42"),
                state: "Actif".into(),
            }],
            updated: vec![
                dw_ado_commands::commands::set_state::SetStateUpdate {
                    id: dw_core::WorkItemId::from("42"),
                    state: "Actif".into(),
                },
                dw_ado_commands::commands::set_state::SetStateUpdate {
                    id: dw_core::WorkItemId::from("43"),
                    state: "Actif".into(),
                },
            ],
        };

        let lines = ado_set_state_execution_lines(&report);

        assert_eq!(lines[0], "ADO update");
        assert!(lines.contains(&"Project  : ha".into()));
        assert!(lines.contains(&"State    : Actif".into()));
        assert!(lines.contains(&"Work items: #42, #43".into()));
        assert!(lines.contains(&"2 work items moved to `Actif`.".into()));
    }

    #[test]
    fn ado_work_item_lines_keep_context_command() {
        let report = dw_ado_commands::commands::work_item::WorkItemReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            requested_ids: vec![dw_core::WorkItemId::from("7")],
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
        assert!(lines.contains(&"Context   : dw ado context 7 --project ha".into()));
    }

    #[test]
    fn ado_context_lines_include_relations_comments_and_ai_context_command() {
        let report = dw_ado_commands::commands::context::ContextReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            requested_ids: vec![dw_core::WorkItemId::from("42")],
            summary: false,
            comments: 10,
            expanded: Vec::new(),
            items: vec![dw_contracts::AdoAiContextItem {
                schema_version: dw_contracts::AI_CONTEXT_VERSION.into(),
                work_item: dw_contracts::AdoAiContextWorkItem {
                    id: dw_core::WorkItemId::from("42"),
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
                    work_item_id: Some(dw_core::WorkItemId::from("1")),
                    name: None,
                    url: None,
                    comment: None,
                    artifact: None,
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
            output.contains("Metadata  : area=Produit\\Backlog | iteration=Sprint 1 | tags=urgent")
        );
        assert!(output.contains("Description courte"));
        assert!(output.contains("Acceptance criteria"));
        assert!(output.contains("Critère A"));
        assert!(output.contains("Attachments (1)"));
        assert!(output.contains("Directory : attachments/ado/42"));
        assert!(output.contains("capture.png"));
        assert!(output.contains("- Parent #1"));
        assert!(output.contains("- Bob: OK"));
        assert!(output.contains("AI context: dw ado ai-context 42 --project ha"));
    }

    #[test]
    fn ado_changelog_lines_render_ids_and_empty_messages() {
        let ids_only = dw_ado_commands::commands::changelog::ChangelogReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            from_pr: true,
            from_git: false,
            group_by_parent: false,
            format: dw_ado_commands::commands::changelog::ChangelogOutputFormat::Raw,
            table: false,
            options: ado_options(),
            ids_only: true,
            work_item_ids: vec![
                dw_core::WorkItemId::from("42"),
                dw_core::WorkItemId::from("43"),
            ],
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
            vec!["No work item detected in git range commit messages.".to_string()]
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
            format: dw_ado_commands::commands::changelog::ChangelogOutputFormat::Raw,
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
            format: dw_ado_commands::commands::changelog::ChangelogOutputFormat::Markdown,
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
            project: dw_core::ProjectKey::from("ha"),
            work_item_ids: vec![dw_core::WorkItemId::from("42")],
            has_blocking_issues: true,
            issues: vec![TaskPreflightIssue {
                code: TaskPreflightIssueCode::AdoAttachmentsPresent,
                severity: TaskPreflightSeverity::Blocking,
                work_item_id: dw_core::WorkItemId::from("42"),
                detail: TaskPreflightIssueDetail::AdoAttachmentsPresent {
                    directory_hint: "attachments/ado/42".into(),
                    names: vec!["screenshot.png".into()],
                },
                related_ids: vec![],
            }],
        };

        let lines = task_preflight_lines(&report);

        assert_eq!(lines[0], "Task preflight");
        assert!(lines.contains(&"Status   : ✕ Needs fixes".into()));
        assert!(lines.contains(&"Blockers : 1".into()));
        assert!(lines.contains(&"✕ [blocker] #42 ado.attachments.present - Work item #42 has attachments to treat as factual sources.".into()));
        assert!(lines.contains(&"  Detail : Attachments present: screenshot.png. Expected directory: attachments/ado/42".into()));
    }

    #[test]
    fn task_handoff_validation_lines_include_counts_and_failure() {
        let report = TaskHandoffValidationReport {
            schema_version: HANDOFF_VALIDATION_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: dw_core::ProjectKey::from("ha"),
            is_valid: false,
            items: vec![TaskHandoffValidationItem {
                repository: "front".into(),
                path: "/tmp/ws/front/handoff-front.md".into(),
                status: TaskHandoffValidationStatus::Valid,
                valid: true,
                detail: TaskHandoffValidationDetail::Valid,
                done_count: 2,
                decision_count: 1,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 1,
            }],
        };

        let lines = task_handoff_validation_lines(&report);

        assert_eq!(lines[0], "Validation handoff");
        assert!(lines.contains(&"Status   : ✕ Needs fixes".into()));
        assert!(lines.contains(&"Handoffs : 1/1 valid".into()));
        assert!(lines.contains(&"✓ front [valid]".into()));
        assert!(
            lines.contains(&"  Summary: done=2 decisions=1 risks=0 blockers=0 follow_up=1".into())
        );
    }

    #[test]
    fn task_prune_plan_lines_render_preview_summary() {
        let report = dw_task::prune::PrunePlanReport {
            root: "/tmp/dw".into(),
            project: Some("ha".into()),
            work_item_ids: Vec::new(),
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
                    status: dw_workspace::WorkspaceManifestStatus::Created,
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
        assert_eq!(lines[1], "Mode     : preview");
        assert!(lines.contains(&"Candidates: 1".into()));
        assert!(lines.contains(&"To do    : dw task prune --execute".into()));
        assert!(lines.contains(&"Workspace : /tmp/dw/projects/ha/workspaces/feat-1-done".into()));
        assert!(lines.contains(&"Items    : ha / #1 Done [Valide]".into()));
        assert!(lines.contains(&"Repositories: front, back".into()));
    }

    #[test]
    fn task_commit_plan_lines_render_statuses_and_execute_hint() {
        let report = dw_task::repo::CommitPlanReport {
            workspace: dw_core::WorkspacePath::from("/tmp/ws"),
            branch_name: dw_core::BranchName::from("feat/42-demo"),
            message: dw_core::CommitMessage::from("feat(42): demo"),
            targets: vec![dw_task::repo::CommitTargetStatus {
                target: dw_workspace::TaskCommitTarget {
                    repository: dw_core::WorkspaceRepositoryName::from("front"),
                    path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                },
                status: dw_git::RepositoryStatus {
                    path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                    is_git_repository: true,
                    has_changes: true,
                    has_unpushed: false,
                    detail: dw_git::RepositoryStatusDetail::Changed {
                        paths: vec![dw_git::RepositoryStatusPath::from("src/lib.rs")],
                    },
                },
            }],
        };

        let lines = task_commit_plan_lines(&report, false);

        assert_eq!(lines[0], "Repository commit");
        assert!(lines.contains(&"Repository: front".into()));
        assert!(lines.contains(&"Status    : Changes detected:".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"To do    : dw task commit --execute".into()));
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
                    path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                },
                status: dw_git::RepositoryStatus {
                    path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                    is_git_repository: true,
                    has_changes: true,
                    has_unpushed: false,
                    detail: dw_git::RepositoryStatusDetail::Changed {
                        paths: vec![dw_git::RepositoryStatusPath::from("src/lib.rs")],
                    },
                },
            }],
            handoff: TaskHandoffValidationReport {
                schema_version: HANDOFF_VALIDATION_VERSION.into(),
                workspace: "/tmp/ws".into(),
                project: dw_core::ProjectKey::from("ha"),
                is_valid: true,
                items: vec![TaskHandoffValidationItem {
                    repository: "front".into(),
                    path: "/tmp/ws/handoff-front.md".into(),
                    status: TaskHandoffValidationStatus::Valid,
                    valid: true,
                    detail: TaskHandoffValidationDetail::Valid,
                    done_count: 1,
                    decision_count: 0,
                    risk_count: 0,
                    blocker_count: 0,
                    follow_up_count: 0,
                }],
            },
            handoff_summaries: vec![dw_workspace::WorkspaceHandoffSummary {
                repository: dw_core::WorkspaceRepositoryName::from("front"),
                status: dw_workspace::WorkspaceHandoffStatus::Done,
                done: vec![dw_workspace::HandoffSummaryEntry::from("UI ajustée")],
                decisions: vec![dw_workspace::HandoffSummaryEntry::from(
                    "Conserver le contrat JSON",
                )],
                risks: Vec::new(),
                blockers: Vec::new(),
                follow_up: vec![dw_workspace::HandoffSummaryEntry::from(
                    "Valider en recette",
                )],
            }],
            commit_message: dw_core::CommitMessage::from("feat(42): demo"),
            create_pr: true,
            ready: false,
            skip_ado: false,
            changed_repositories: vec!["front".into()],
            unpushed_repositories: Vec::new(),
            actionable_repositories: vec!["front".into()],
            pull_request_candidates: vec![dw_workspace::PullRequestCandidate {
                repository: dw_core::WorkspaceRepositoryName::from("front"),
                path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                ado_repository: Some(dw_core::AdoRepositoryName::from("front")),
                target_branch: dw_core::BranchName::from("develop"),
            }],
        };

        let lines = task_finish_plan_lines(&report);

        assert_eq!(lines[0], "Workspace finish");
        assert!(lines.contains(&"Status   : OK".into()));
        assert!(lines.contains(&"- [valid] front - Handoff is valid.".into()));
        assert!(lines.contains(&"Commit to create".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"Handoff front".into()));
        assert!(lines.contains(&"Done      : UI ajustée".into()));
        assert!(lines.contains(&"- front -> develop".into()));
        assert!(lines.contains(&"To do    : dw task finish --execute".into()));
        assert!(lines.contains(&"Non-TTY  : add --yes to confirm without a prompt".into()));
    }

    #[test]
    fn task_add_repo_plan_lines_include_anchor() {
        let report = dw_task::repo::AddRepoPlanReport {
            plan: dw_workspace::TaskAddRepoPlan {
                workspace: dw_core::WorkspacePath::from("/tmp/ws"),
                repository: dw_core::WorkspaceRepositoryName::from("front"),
                project_root: dw_core::ProjectRootPath::from("/tmp/project"),
                worktree_path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                http_url: dw_core::GitRemoteUrl::from("https://example.invalid/front.git"),
                ssh_url: None,
                default_branch: dw_core::BranchName::from("main"),
                anchor_name: dw_core::GitAnchorName::from("front-anchor"),
                git_credential_secret: None,
                branch_name: dw_core::BranchName::from("feat/42-demo"),
                repositories: vec![dw_core::WorkspaceRepositoryName::from("front")],
            },
        };

        let lines = task_add_repo_plan_lines(&report);

        assert_eq!(lines[0], "Add repository (preview)");
        assert!(lines.contains(&"Anchor    : /tmp/project/repositories/front-anchor".into()));
        assert!(lines.contains(&"To do    : dw task add-repo front --execute".into()));
    }

    #[test]
    fn task_teardown_plan_lines_switch_title_for_execute() {
        let report = dw_task::repo::TeardownPlanReport {
            workspace: Some(dw_core::WorkspacePath::from("/tmp/ws")),
            steps: vec![dw_workspace::WorkspaceTeardownStep {
                subject: dw_workspace::WorkspaceTeardownSubject::Repository {
                    repository: dw_core::WorkspaceRepositoryName::from("front"),
                },
                action: dw_workspace::WorkspaceTeardownAction::WorktreeRemove {
                    worktree_path: dw_core::RepositoryPath::from("/tmp/ws/front"),
                    git_dir: dw_core::RepositoryPath::from("/tmp/project/repositories/front/.git"),
                },
            }],
        };

        let dry_run = task_teardown_plan_lines(&report, false);
        let execute = task_teardown_plan_lines(&report, true);

        assert_eq!(dry_run[0], "Workspace removal (preview)");
        assert_eq!(dry_run[2], "Actions   : 1");
        assert_eq!(dry_run[3], "Planned actions");
        assert_eq!(execute[0], "Workspace removal executed");
        assert_eq!(execute[2], "Actions   : 1");
        assert_eq!(execute[3], "Actions applied");
        assert!(dry_run.contains(&"- [front] worktree remove: /tmp/ws/front".into()));
        assert!(dry_run.contains(&"To do    : dw task teardown --execute".into()));
        assert!(dry_run.contains(&"Non-TTY  : add --yes to confirm without a prompt".into()));
        assert!(!execute.contains(&"To do    : dw task teardown --execute".into()));
    }

    #[test]
    fn task_sync_lines_render_missing_ado_fields_as_unknown() {
        let report = dw_task::lifecycle::SyncReport {
            workspace: "/tmp/ws".into(),
            requested_ids: vec![dw_core::WorkItemId::from("42")],
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
        assert_eq!(lines[4], "ADO work items");
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
        assert!(lines.contains(&"Mode     : preview".into()));
        assert!(lines.contains(&"Slug      : old -> new".into()));
        assert!(lines.contains(&"Branch    : feat/1-old -> feat/1-new".into()));
        assert!(lines.contains(&"To do    : dw task rename <slug> --execute".into()));
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
        assert_eq!(lines[1], "Status   : saved in the workspace");
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
            requested_ids: vec![
                dw_core::WorkItemId::from("1"),
                dw_core::WorkItemId::from("2"),
            ],
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
        assert_eq!(lines[1], "Mode     : preview");
        assert_eq!(lines[2], "Action   : add");
        assert!(lines.contains(&"Branch    : feat/1-old -> feat/1-2-new".into()));
        assert!(lines.contains(&"Items    : #1, #2".into()));
        assert!(lines.contains(&"To do    : dw task add-work-item --execute".into()));
    }

    #[test]
    fn task_start_plan_lines_include_execute_hint() {
        let report = dw_task::start::StartPlanReport {
            root: "/tmp/dw".into(),
            plan: dw_workspace::TaskStartPlan {
                project: dw_core::ProjectKey::from("ha"),
                work_item_ids: vec![dw_core::WorkItemId::from("42")],
                primary_work_item_id: dw_core::WorkItemId::from("42"),
                task_id: None,
                kind: dw_core::WorkItemTypeName::from("feat"),
                slug: dw_core::TaskSlug::from("titre"),
                branch_name: dw_core::BranchName::from("feat/42-titre"),
                subject_name: dw_core::TaskSubjectName::from("42-titre"),
                workspace: dw_core::WorkspacePath::from("/tmp/dw/ha/42-titre"),
                repositories: vec![
                    dw_core::WorkspaceRepositoryName::from("front"),
                    dw_core::WorkspaceRepositoryName::from("back"),
                ],
                repository_folders: Default::default(),
                repository_worktrees: Vec::new(),
            },
            work_items: Vec::new(),
            child_tasks: Vec::new(),
        };

        let lines = task_start_plan_lines(&report);

        assert_eq!(lines[0], "Task start plan");
        assert!(lines.contains(
            &"Action: preview only; answer yes when prompted or run the command below.".into()
        ));
        assert_eq!(
            task_start_create_command(
                &report,
                TaskStartCreateCommandOptions {
                    skip_ado: false,
                    with_active_children: false,
                    create_child_tasks: false,
                }
            ),
            "dw task start 42 --project ha --type feat --only front,back --slug titre --execute"
        );
    }

    #[test]
    fn guide_lines_render_version_and_next_steps() {
        let lines = guide_lines("2026.07.02.3+54011f0", &TerminalTheme::plain());

        assert_eq!(lines[0], "Dev Workflow 2026.07.02.3+54011f0");
        assert!(lines.contains(&"Step-by-step getting started guide".into()));
        assert!(lines.iter().any(|line| line.contains("dw init")));
        assert!(lines.iter().any(|line| line.contains("dw doctor")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task start <work-item-id>"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task finish --continue"))
        );
        assert!(lines.iter().any(|line| line.contains("dw db query")));
        assert!(lines.iter().any(|line| line.contains("dw ado assigned")));
        assert!(lines.iter().any(|line| line.contains("dw completion show")));
    }

    #[test]
    fn db_query_tsv_renders_null_and_truncation() {
        let result = QueryResult {
            columns: vec!["Id".into(), "Name".into()],
            rows: vec![vec![Some("1".into()), None]],
            truncated: true,
        };

        assert_eq!(
            db_query_tsv(&result),
            "Id\tName\n1\tNULL\n-- 1 rows (truncated)"
        );
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
            status: dw_workspace::WorkspaceManifestStatus::Created,
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
