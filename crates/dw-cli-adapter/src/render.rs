use dw_ado_commands::auth::{
    AuthLoginMode, AuthLoginReport, AuthLogoutReport, AuthStatusReport, expiration_line,
};
use dw_agent::command::{AgentDoctorCheck, AgentDoctorReport};
use dw_config::{ConfigDoctorCheck, ConfigDoctorReport, ConfigShow, InitReport, RefreshReport};
use dw_contracts::{TaskHandoffValidationItem, TaskHandoffValidationReport, TaskPreflightReport};
use dw_core::{AdoActionEvent, GitOperation, TaskActionEvent};
use dw_db::{QueryResult, SqlGuardResult};
use dw_doctor::{DoctorCheck, DoctorReport};
use dw_secret::command::{SecretDeleteReport, SecretGetReport, SecretSetReport};
use dw_task::open::{TaskListReport, TaskStatusReport};
use dw_ui::TerminalTheme;
use dw_workspace::TaskCurrentItem;
use std::fmt::Display;

const MAX_DB_CELL_WIDTH: usize = 48;

pub fn auth_login_lines(report: &AuthLoginReport) -> Vec<String> {
    if report.uses_environment_pat {
        return vec![
            "Connexion ADO".into(),
            "Mode      : PAT via environnement".into(),
            "À faire   : définir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT.".into(),
            "Sécurité : aucun secret n'est saisi ni stocké par cette commande.".into(),
        ];
    }

    vec![
        "Connexion ADO".into(),
        "Statut    : connecté".into(),
        format!("Mode      : {}", auth_login_mode_label(report.mode)),
        format!(
            "Source    : {}",
            report.source.as_deref().unwrap_or("source inconnue")
        ),
        expiration_line(report.expires_on.as_deref()),
    ]
}

pub fn auth_status_lines(report: &AuthStatusReport) -> Vec<String> {
    if report.connected {
        vec![
            "Connexion ADO".into(),
            "Statut    : connecté".into(),
            format!(
                "Source    : {}",
                report.source.as_deref().unwrap_or("source inconnue")
            ),
            expiration_line(report.expires_on.as_deref()),
        ]
    } else {
        vec![
            "Connexion ADO".into(),
            "Statut    : non connecté".into(),
            "À faire   : dw auth login ou définir DW_ADO_TOKEN.".into(),
        ]
    }
}

pub fn auth_logout_lines(report: &AuthLogoutReport) -> Vec<String> {
    vec![
        "Connexion ADO".into(),
        format!(
            "Sessions  : {}",
            if report.removed_local_session {
                "session locale supprimée"
            } else {
                "aucune session locale"
            }
        ),
        "PAT       : les variables DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent gérées par l'environnement.".into(),
    ]
}

fn auth_login_mode_label(mode: AuthLoginMode) -> &'static str {
    match mode {
        AuthLoginMode::Browser => "navigateur",
        AuthLoginMode::DeviceCode => "code appareil",
        AuthLoginMode::EnvironmentPat => "PAT environnement",
    }
}

pub fn config_show_lines(report: &ConfigShow, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command("Configuration DevWorkflow"),
        format!("Root      : {}", theme.path(&report.root)),
        format!("Couleur   : {}", theme.bold(&report.color)),
        format!("Réglages  : {}", theme.path(&report.settings_path)),
        String::new(),
        "Fichiers".into(),
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
            "Statut    : {}",
            if report.passed {
                theme.success("valide")
            } else {
                theme.warning("à corriger")
            }
        ),
        format!("Root      : {}", theme.path(&report.root)),
        String::new(),
        "Vérifications".into(),
    ];
    for check in &report.checks {
        lines.push(config_check_line(theme, check));
        if let Some(message) = &check.message {
            lines.push(format!("  Détail  : {message}"));
        }
    }
    lines.push(String::new());
    lines.push(if report.passed {
        format!("Résultat  : {}", theme.success("Configuration valide."))
    } else {
        format!(
            "Résultat  : {}",
            theme.warning(
                "Configuration incomplète. Corriger les points signalés puis relancer `dw config doctor`."
            )
        )
    });
    lines
}

pub fn init_report_lines(report: &InitReport) -> Vec<String> {
    if report.dry_run {
        let mut lines = vec![
            format!("Prévisualisation init DevWorkflow: {}", report.root),
            format!("Profil: {}", report.profile),
        ];
        lines.extend(
            report
                .planned_paths
                .iter()
                .map(|path| format!("  + créer/mettre à jour: {path}")),
        );
        lines.push(if report.no_save {
            "  - settings utilisateur inchangés (--no-save).".into()
        } else {
            format!("  + enregistrer le root utilisateur: {}", report.root)
        });
        return lines;
    }

    let mut lines = vec![
        format!("Root DevWorkflow initialisé: {}", report.root),
        format!("Profil: {}", report.profile),
    ];
    if report.no_save {
        lines.push("Settings utilisateur non modifiés (--no-save).".into());
    }
    lines.push("Prochaine étape conseillée: dw doctor".into());
    lines
}

pub fn refresh_report_lines(report: &RefreshReport) -> Vec<String> {
    vec![
        format!("Root rafraîchi: {}", report.root),
        format!("Profil: {}", report.profile),
        "Schémas et contextes agents régénérés.".into(),
        "Fichiers utilisateurs préservés: projects.json, workflow.json, databases.json, plan.md."
            .into(),
    ]
}

pub fn agent_doctor_lines(report: &AgentDoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let available_count = report.available_count();
    let total_count = report.total_count();
    let mut lines = vec![
        theme.command("Diagnostic agents"),
        format!(
            "{} {available_count}/{total_count} agents disponibles",
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
        theme.command("Config agent"),
        format!("Agent par défaut: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
}

pub fn agent_config_updated_lines(root: &str, agent: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.success("✓ Config agent mise à jour"),
        format!("Agent par défaut: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
}

pub fn ado_prs_lines(report: &dw_ado_commands::commands::prs::PrsReport) -> Vec<String> {
    if report.items.is_empty() {
        return vec![format!("Aucune PR active pour {}.", report.project)];
    }

    let mut lines = vec![format!("PR actives · {}", report.project)];
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
        theme.success("ADO assignés"),
        format!("Éléments  : {}", report.items.len()),
    ];
    for item in &report.items {
        lines.push(String::new());
        lines.push(format!(
            "Item      : {}",
            ado_work_item_summary(item, theme)
        ));
        lines.push(ado_start_command_line(&item.id, &report.project, theme));
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines)
}

pub fn ado_set_state_execution_lines(
    report: &dw_ado_commands::commands::set_state::SetStateExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Mise à jour ADO".into(),
        format!("Projet    : {}", report.plan.project),
        format!("État      : {}", report.plan.state),
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
            "{} work item{} passé{} en `{}`.",
            report.updated.len(),
            if report.updated.len() == 1 { "" } else { "s" },
            if report.updated.len() == 1 { "" } else { "s" },
            report.plan.state
        ),
    ];
    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push("Événements".into());
        lines.extend(
            report
                .events
                .iter()
                .map(|event| format!("- {}", ado_action_event_line(event))),
        );
    }
    lines
}

pub fn ado_action_event_line(event: &AdoActionEvent) -> String {
    match event {
        AdoActionEvent::Authenticating { project } => format!(
            "ADO auth: {}",
            project
                .as_ref()
                .map(|project| project.to_string())
                .unwrap_or_else(|| "projet résolu".into())
        ),
        AdoActionEvent::LoadingAssignedWorkItems { project, top } => {
            format!("ADO assigned: projet={project} top={top}")
        }
        AdoActionEvent::GroupingAssignedWorkItems { project } => {
            format!("ADO groupement parent: projet={project}")
        }
        AdoActionEvent::LoadingPullRequests { project } => {
            format!("ADO pull requests: projet={project}")
        }
        AdoActionEvent::ResolvingPullRequestWorkItems { repositories } => format!(
            "ADO résolution PR: repos={}",
            repositories
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AdoActionEvent::ExtractingGitWorkItems { git_to } => match git_to {
            Some(git_to) => format!("ADO extraction git: jusqu'à {git_to}"),
            None => "ADO extraction git".into(),
        },
        AdoActionEvent::LoadingWorkItem { id } => format!("ADO work item: #{id}"),
        AdoActionEvent::LoadingWorkItems { ids } => {
            format!("ADO work items: {}", join_display(ids))
        }
        AdoActionEvent::LoadingWorkItemContext { id } => format!("ADO contexte: #{id}"),
        AdoActionEvent::LoadingChangelog { ids } => {
            format!("ADO changelog: {}", join_display(ids))
        }
        AdoActionEvent::LoadingChangelogItems { ids } => {
            format!("ADO items changelog: {}", join_display(ids))
        }
        AdoActionEvent::UpdatingWorkItemState { ids, state } => {
            format!("ADO changement état: {} -> {state}", join_display(ids))
        }
        AdoActionEvent::UpdatedWorkItemState { id, state } => {
            format!("ADO état changé: #{id} -> {state}")
        }
    }
}

pub fn task_action_event_line(event: &TaskActionEvent) -> String {
    match event {
        TaskActionEvent::ResolvingPullRequestWorkItems { pull_request_id } => {
            format!("Task PR: résolution des work items liés à #{pull_request_id}")
        }
        TaskActionEvent::ResolvedPullRequestWorkItems { work_item_ids } => {
            format!("Task PR: work items résolus {}", format_ids(work_item_ids))
        }
        TaskActionEvent::VerifyingFinish {
            pull_request_candidate_count,
        } => format!(
            "Task finish: vérification avant finish pour {pull_request_candidate_count} PR candidate(s)"
        ),
        TaskActionEvent::FinishVerificationCompleted => "Task finish: vérification terminée".into(),
        TaskActionEvent::RunningGitOperation {
            operation,
            repository_count,
        } => format!(
            "Task finish: git {} sur {repository_count} repository(s)",
            git_operation_label(*operation)
        ),
        TaskActionEvent::RunningRepositoryGitOperation {
            repository,
            operation,
        } => format!(
            "Task finish: {} {}",
            repository,
            git_operation_label(*operation)
        ),
        TaskActionEvent::GitOperationCompleted { operation } => {
            format!(
                "Task finish: git {} terminé",
                git_operation_label(*operation)
            )
        }
        TaskActionEvent::SkippingPullRequestCreation => {
            "Task finish: création de PR ignorée".into()
        }
        TaskActionEvent::AuthenticatingAdoForPullRequests {
            pull_request_candidate_count,
        } => format!(
            "Task finish: connexion ADO pour {pull_request_candidate_count} PR candidate(s)"
        ),
        TaskActionEvent::CheckingActivePullRequest { repository } => {
            format!("Task finish: vérification PR active pour {repository}")
        }
        TaskActionEvent::CreatingPullRequest { repository } => {
            format!("Task finish: création PR ADO pour {repository}")
        }
        TaskActionEvent::PullRequestWorkItemLinkSkipped {
            work_item_id,
            error,
        } => format!("Task finish: lien PR/work item #{work_item_id} ignoré ({error})"),
        TaskActionEvent::UpdatingFinishWorkItemStates { work_item_ids } => {
            format!("Task finish: mise à jour ADO {}", format_ids(work_item_ids))
        }
    }
}

fn format_ids<T: Display>(ids: &[T]) -> String {
    if ids.is_empty() {
        "aucun".into()
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
            item.kind.as_deref().unwrap_or("type inconnu")
        ));
        lines.push(format!(
            "État      : {}",
            item.state.as_deref().unwrap_or("état inconnu")
        ));
        lines.push(format!(
            "Titre     : {}",
            item.title.as_deref().unwrap_or("(sans titre)")
        ));
        lines.push(String::new());
        lines.push(format!(
            "Contexte  : {}",
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
            "Assigné   : {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigné")
        ));
        if let Some(metadata) = ado_context_metadata(item)
            && !metadata.is_empty()
        {
            lines.push(format!("Métadonnées: {metadata}"));
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
            lines.push(theme.bold("Critères d'acceptation"));
            lines.push(acceptance_criteria.trim().into());
        }

        if !item.attachments.items.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold(&format!(
                "Pièces jointes ({})",
                item.attachments.items.len()
            )));
            lines.push(format!("Dossier   : {}", item.attachments.directory_hint));
            for attachment in &item.attachments.items {
                lines.push(format!(
                    "- {}",
                    attachment
                        .name
                        .as_deref()
                        .or(attachment.url.as_deref())
                        .unwrap_or("pièce jointe sans nom")
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
            lines.push(theme.bold("Commentaires"));
            for comment in item.comments.iter().take(report.comments.max(0) as usize) {
                lines.push(format!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                ));
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "Contexte IA: {}",
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
            "Aucun work item détecté dans les messages de commit de la plage git."
        } else {
            "Aucun work item détecté pour les pull requests données."
        })];
    }
    if report.ids_only {
        return vec![join_display_with_separator(&report.work_item_ids, " ")];
    }
    if report.resolved_empty {
        return vec![theme.warning("Aucun work item résolu dans Azure DevOps.")];
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
                "Mise à jour".into(),
                format!("Release  : {}", report.release_tag),
                format!("Version  : {}+{}", report.version, report.commit),
                format!("Artefacts : {}", report.assets.len()),
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
                "remplacement programmé"
            } else {
                "terminé"
            };
            vec![
                "Mise à jour".into(),
                format!("Statut   : {status}"),
                format!("Version  : {}+{}", report.version, report.commit),
                format!("Binaire  : {}", report.executable_path),
            ]
        }
    }
}

pub fn upgrade_event_line(event: &dw_upgrade::UpgradeEvent) -> String {
    format!(
        "Upgrade [{:<18}] {}",
        upgrade_step_label(event.step),
        event.message
    )
}

fn upgrade_step_label(step: dw_upgrade::UpgradeStep) -> &'static str {
    match step {
        dw_upgrade::UpgradeStep::CheckHost => "host",
        dw_upgrade::UpgradeStep::ResolveConfig => "config",
        dw_upgrade::UpgradeStep::FetchRelease => "release",
        dw_upgrade::UpgradeStep::FetchManifest => "manifest",
        dw_upgrade::UpgradeStep::SelectAsset => "asset",
        dw_upgrade::UpgradeStep::DownloadAsset => "download",
        dw_upgrade::UpgradeStep::VerifyChecksum => "checksum",
        dw_upgrade::UpgradeStep::PrepareExecutable => "prepare",
        dw_upgrade::UpgradeStep::ReplaceExecutable => "replace",
        dw_upgrade::UpgradeStep::Complete => "done",
    }
}

pub fn task_status_lines(report: &TaskStatusReport) -> Vec<String> {
    let mut lines = vec![
        "Workspaces task".into(),
        format!("Root      : {}", report.root),
        format!("Détectés  : {}", report.items.len()),
    ];

    if report.items.is_empty() {
        lines.push("Aucun workspace task trouvé.".into());
        return lines;
    }

    lines.push("Détails".into());
    for item in &report.items {
        lines.push(format!(
            "- {} {} {}",
            item.project, item.kind, item.display_work_items
        ));
        lines.push(format!("  Branche     : {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!("  Repositories: {}", item.repositories.join(", ")));
        }
        lines.push(format!("  Chemin      : {}", item.path));
    }
    lines
}

pub fn task_list_lines(report: &TaskListReport) -> Vec<String> {
    let mut lines = vec![
        format!("Workspaces task: {}", report.items.len()),
        "Projet  Créé        Type   Work items".into(),
    ];

    for item in &report.items {
        lines.push(format!(
            "{:<7} {}  {:<6} {}",
            item.project,
            created_date(&item.created_at),
            item.kind,
            item.display_work_items
        ));
        lines.push(format!("  Branche: {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!("  Repositories: {}", item.repositories.join(", ")));
        }
        lines.push(format!("  Chemin: {}", item.path));
    }

    lines
}

pub fn task_current_lines(item: &TaskCurrentItem) -> Vec<String> {
    let mut lines = vec![
        "Workspace courant".into(),
        format!("Workspace : {}", item.workspace),
        format!("Projet    : {}", item.project),
        format!("Branche   : {}", item.branch),
        format!(
            "Work items: {}",
            format_current_work_items(&item.work_items)
        ),
    ];

    if !item.child_tasks.is_empty() || !item.child_task_ids.is_empty() {
        lines.push(format!("Tâches enfants: {}", format_child_tasks(item)));
    }

    lines.push(format!("Repositories: {}", item.repositories.join(", ")));
    lines
}

pub fn task_preflight_lines(report: &TaskPreflightReport) -> Vec<String> {
    let mut lines = vec![
        "Préflight task".into(),
        format!(
            "Statut    : {}",
            validation_status_label(!report.has_blocking_issues)
        ),
        format!("Workspace : {}", report.workspace),
        format!("Projet    : {}", report.project),
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
        lines.push("✓ Aucun avertissement ni blocage détecté.".into());
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
    lines.push(format!("Blocages  : {blocking_count}"));
    lines.push(format!("Warnings  : {warning_count}"));
    lines.push(format!("Infos     : {other_count}"));
    lines.push(String::new());
    push_preflight_issue_group(&mut lines, "Blocages", report, is_blocking_severity);
    push_preflight_issue_group(&mut lines, "Warnings", report, is_warning_severity);
    push_preflight_issue_group(&mut lines, "Infos", report, |severity| {
        !is_blocking_severity(severity) && !is_warning_severity(severity)
    });

    if report.has_blocking_issues {
        lines.push(String::new());
        lines.push(
            "Blocages détectés: demander confirmation utilisateur avant de forcer l'implémentation."
                .into(),
        );
    }

    lines
}

pub fn task_handoff_validation_lines(report: &TaskHandoffValidationReport) -> Vec<String> {
    let mut lines = vec![
        "Validation handoff".into(),
        format!("Statut    : {}", validation_status_label(report.is_valid)),
        format!("Workspace : {}", report.workspace),
        format!("Projet    : {}", report.project),
        format!(
            "Handoffs  : {}/{} valides",
            report.items.iter().filter(|item| item.valid).count(),
            report.items.len()
        ),
        String::new(),
    ];

    push_handoff_group(&mut lines, "À corriger", report, |item| !item.valid);
    push_handoff_group(&mut lines, "Valides", report, |item| item.valid);

    if !report.is_valid {
        lines.push(String::new());
        lines.push(
            "Validation handoff échouée: compléter/corriger les handoffs avant task finish.".into(),
        );
    }

    lines
}

pub fn task_prune_plan_lines(report: &dw_task::prune::PrunePlanReport) -> Vec<String> {
    let mut lines = vec![
        "Nettoyage workspaces".into(),
        "Mode      : prévisualisation".into(),
        format!("Root      : {}", report.root),
        format!("Candidats : {}", report.candidates.len()),
        "À faire   : dw task prune --execute".into(),
        "Non-TTY   : ajouter --yes pour tout supprimer sans sélection interactive".into(),
    ];

    if !report.sync.is_empty() {
        lines.push(String::new());
        lines.push("Synchronisation ADO".into());
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
        lines.push("Aucun workspace éligible au prune.".into());
        return lines;
    }

    for candidate in &report.candidates {
        lines.push(String::new());
        lines.push(format!("Workspace : {}", candidate.path));
        lines.push(format!(
            "Éléments  : {}",
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
        "Nettoyage workspaces".into(),
        "Mode      : exécution".into(),
        format!("Root      : {}", report.root),
        format!("Supprimés : {}", report.deleted.len()),
    ];
    for path in &report.deleted {
        lines.push(format!("- {path}"));
    }
    lines
}

pub fn task_repo_latest_plan_lines(report: &dw_task::repo::RepoLatestPlanReport) -> Vec<String> {
    vec![
        "Mise à jour repositories".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branche   : {}", report.branch_name),
        format!("Repositories: {}", report.targets.len()),
    ]
}

pub fn task_repo_latest_execution_lines(
    report: &dw_task::repo::RepoLatestExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Mise à jour repositories".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branche   : {}", report.branch_name),
        format!("Synchronisés: {}", report.updated.len()),
    ];
    for item in &report.updated {
        lines.push(format!(
            "- {} depuis {} ({})",
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
        "Commit des repositories".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branche   : {}", report.branch_name),
    ];

    for item in &report.targets {
        lines.push(String::new());
        lines.push(format!("Repository: {}", item.target.repository));
        lines.push(format!("Chemin    : {}", item.status.path));
        lines.push(format!(
            "Statut    : {}",
            repository_status_label(&item.status)
        ));
        if !item.status.detail.trim().is_empty() {
            lines.push(item.status.detail.clone());
        }
    }

    lines.push(String::new());
    if nothing_to_commit {
        lines.push("Rien à committer.".into());
    } else {
        lines.push(format!("Message   : {}", report.message));
        if !execute {
            lines.push("À faire   : dw task commit --execute".into());
        }
    }
    lines
}

pub fn task_commit_execution_lines(report: &dw_task::repo::CommitExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Commit des repositories".into(),
        "Mode      : exécution".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branche   : {}", report.branch_name),
        format!("Message   : {}", report.message),
        format!("Commits   : {}", report.committed.len()),
    ];
    for repository in &report.committed {
        lines.push(format!("- {repository}"));
    }
    if report.committed.is_empty() {
        lines.push("Rien à committer.".into());
    }
    lines
}

pub fn task_add_repo_plan_lines(report: &dw_task::repo::AddRepoPlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Ajout repository (prévisualisation)".into(),
        format!("Workspace : {}", plan.workspace),
        format!("Repository: {}", plan.repository),
        format!("Worktree  : {}", plan.worktree_path),
        format!("Branche   : {}", plan.branch_name),
        format!(
            "Ancrage   : {}/repositories/{}",
            plan.project_root, plan.anchor_name
        ),
        format!("À faire   : dw task add-repo {} --execute", plan.repository),
    ]
}

pub fn task_add_repo_execution_lines(
    report: &dw_task::repo::AddRepoExecutionReport,
) -> Vec<String> {
    vec![
        "Ajout repository".into(),
        "Mode      : exécution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Repository: {}", report.worktree.repository),
        format!("Statut    : {}", report.worktree.status),
        format!("Détail    : {}", report.worktree.message),
    ]
}

pub fn task_teardown_plan_lines(
    report: &dw_task::repo::TeardownPlanReport,
    execute: bool,
) -> Vec<String> {
    let Some(workspace) = &report.workspace else {
        return vec!["Aucun workspace task trouvé.".into()];
    };
    let mut lines = vec![
        if execute {
            "Suppression workspace exécutée".into()
        } else {
            "Suppression workspace (prévisualisation)".into()
        },
        format!("Workspace : {workspace}"),
        format!("Actions   : {}", report.steps.len()),
        if execute {
            "Actions appliquées".into()
        } else {
            "Actions prévues".into()
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
        lines.push("À faire   : dw task teardown --execute".into());
        lines.push("Non-TTY   : ajouter --yes pour confirmer sans prompt".into());
    }
    lines
}

pub fn task_teardown_execution_lines(
    report: &dw_task::repo::TeardownExecutionReport,
) -> Vec<String> {
    let mut lines = vec![
        "Suppression workspace".into(),
        "Mode      : exécution".into(),
        format!("Workspace : {}", report.workspace),
        format!("Actions   : {}", report.steps.len()),
    ];
    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            step.repository, step.action, step.target
        ));
    }
    lines.push(format!("Workspace supprimé: {}", report.workspace));
    lines
}

pub fn task_sync_lines(report: &dw_task::lifecycle::SyncReport) -> Vec<String> {
    let items = report.manifest.parent_work_items();
    let mut lines = vec![
        "Synchronisation task".into(),
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
        "Renommage workspace".into(),
        "Mode      : prévisualisation".into(),
        format!("Slug      : {} -> {}", plan.old_slug, plan.new_slug),
        format!("Branche   : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        "À faire   : dw task rename <slug> --execute".into(),
    ]
}

pub fn task_rename_execution_lines(
    report: &dw_task::lifecycle::RenameExecutionReport,
) -> Vec<String> {
    vec![
        "Renommage workspace".into(),
        "Mode      : exécution".into(),
        format!(
            "Slug      : {} -> {}",
            report.plan.old_slug, report.plan.new_slug
        ),
        format!(
            "Branche   : {} -> {}",
            report.plan.old_branch, report.plan.new_branch
        ),
        format!(
            "Workspace : {} -> {}",
            report.plan.workspace, report.plan.new_workspace
        ),
        format!("Workspace renommé: {}", report.plan.new_workspace),
    ]
}

pub fn task_child_task_lines(report: &dw_task::lifecycle::CreateChildTaskReport) -> Vec<String> {
    vec![
        "Sous-tâche ADO".into(),
        "Statut    : enregistrée dans le workspace".into(),
        format!("Workspace : {}", report.workspace),
        format!("Repository: {}", report.repository),
        format!("Item      : #{}", report.created.id),
        format!("Titre     : {}", report.created.title),
    ]
}

pub fn task_work_item_plan_lines(
    report: &dw_task::work_item::WorkItemUpdatePlanReport,
) -> Vec<String> {
    let Some(plan) = &report.plan else {
        return vec![
            "Work items workspace".into(),
            "Mode      : prévisualisation".into(),
            format!("Action    : {}", work_item_action_label(report.action)),
            format!("Workspace : {}", report.workspace),
            "Statut    : aucun changement".into(),
            "Tous les work items demandés sont déjà présents dans le workspace.".into(),
        ];
    };
    vec![
        "Work items workspace".into(),
        "Mode      : prévisualisation".into(),
        format!("Action    : {}", work_item_action_label(report.action)),
        format!("Branche   : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        format!(
            "Éléments  : {}",
            plan.work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "À faire   : dw task {} --execute",
            work_item_action_command(report.action)
        ),
    ]
}

pub fn task_work_item_execution_lines(
    report: &dw_task::work_item::WorkItemUpdateExecutionReport,
) -> Vec<String> {
    vec![
        "Work items workspace".into(),
        "Mode      : exécution".into(),
        format!("Action    : {}", work_item_action_label(report.action)),
        format!(
            "Branche   : {} -> {}",
            report.plan.old_branch, report.plan.new_branch
        ),
        format!(
            "Workspace : {} -> {}",
            report.plan.workspace, report.plan.new_workspace
        ),
        format!(
            "Éléments  : {}",
            report
                .plan
                .work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!("Workspace mis à jour: {}", report.new_workspace),
    ]
}

pub fn task_start_plan_lines(report: &dw_task::start::StartPlanReport) -> Vec<String> {
    let plan = &report.plan;
    vec![
        "Plan task start".into(),
        format!("Project: {}", plan.project),
        format!("Work items: {}", join_display(&plan.work_item_ids)),
        format!("Slug: {}", plan.slug),
        format!("Branche cible: {}", plan.branch_name),
        format!("Workspace cible: {}", plan.workspace),
        format!("Repositories: {}", plan.repositories.join(", ")),
        "Relancer avec --execute pour créer le workspace.".into(),
    ]
}

pub fn task_start_execution_lines(report: &dw_task::start::StartExecutionReport) -> Vec<String> {
    let mut lines = vec![
        format!("Workspace créé: {}", report.plan.workspace),
        format!("Branche cible: {}", report.plan.branch_name),
        format!("Repositories: {}", report.plan.repositories.join(", ")),
    ];
    for task in &report.child_tasks {
        lines.push(format!(
            "ADO task créée [{}]: #{} {}",
            task.repository,
            task.id,
            task.title.as_deref().unwrap_or("(sans titre)")
        ));
    }
    for update in &report.state_updates {
        if update.changed {
            lines.push(format!(
                "ADO item {}: état -> {}",
                update.label, update.target_state
            ));
        }
    }
    lines.push("Prochaine étape conseillée: ouvrir le workspace ou lancer l'agent.".into());
    lines
}

pub fn task_start_pr_plan_lines(report: &dw_task::start::StartPrPlanReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Résolution PR: #{} dans {}",
            report.pull_request_id,
            if report.repositories.is_empty() {
                "aucun repository".into()
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
        "Finalisation workspace".into(),
        format!("Workspace : {}", report.workspace),
        format!("Branche   : {}", report.manifest.branch_name),
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
        "Statut    : {}",
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
        lines.push("Commit à créer".into());
        lines.push(format!("Message   : {}", report.commit_message));
    }
    if report.create_pr {
        lines.push(String::new());
        lines.push("Pull requests à créer".into());
        if report.pull_request_candidates.is_empty() {
            lines.push("Aucun repository candidat détecté.".into());
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
        lines.push("À faire   : dw task finish --execute".into());
        lines.push("Non-TTY   : ajouter --yes pour confirmer sans prompt".into());
    }

    lines
}

pub fn task_finish_execution_lines(report: &dw_task::finish::FinishExecutionReport) -> Vec<String> {
    let mut lines = vec![
        "Finalisation workspace".into(),
        "Mode      : exécution".into(),
        format!("Workspace : {}", report.plan.workspace),
        format!("Branche   : {}", report.plan.manifest.branch_name),
    ];
    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push("Événements".into());
        lines.extend(
            report
                .events
                .iter()
                .map(|event| format!("- {}", task_action_event_line(event))),
        );
    }
    if !report.verification_results.is_empty() {
        lines.push(String::new());
        lines.push("Vérification".into());
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
        lines.push("Work items ADO".into());
        for update in &report.work_item_updates {
            lines.push(finish_work_item_update_line(update));
        }
    }
    if report.git_actions.is_empty()
        && report.pull_requests.is_empty()
        && report.work_item_updates.is_empty()
    {
        lines.push(String::new());
        lines.push("Rien à terminer.".into());
    }
    lines
}

fn task_start_pr_resolved_line<T: Display>(work_item_ids: &[T]) -> String {
    match work_item_ids.len() {
        0 => "Aucun work item lié à la PR.".into(),
        1 => format!("PR liée au work item #{}.", work_item_ids[0]),
        count => format!(
            "PR liée à {count} work items: {}.",
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
            format!("ADO item {}: état inchangé pour ce type", update.label)
        }
        dw_task::finish::FinishWorkItemStateOutcome::AlreadyInTargetState => format!(
            "ADO item {}: déjà en état {}",
            update.label,
            update.target_state.as_deref().unwrap_or("cible")
        ),
        dw_task::finish::FinishWorkItemStateOutcome::Updated => format!(
            "ADO item {}: état -> {}",
            update.label,
            update.target_state.as_deref().unwrap_or("cible")
        ),
    }
}

pub fn task_finish_dry_run_hint(no_changes: bool, create_pr: bool) -> &'static str {
    if create_pr {
        "Prévisualisation uniquement. Relancer avec --execute pour pousser/créer PR."
    } else if no_changes {
        "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour pousser."
    } else {
        "Prévisualisation uniquement. Relancer avec --execute --skip-ado pour committer/pousser."
    }
}

pub fn doctor_report_lines(report: &DoctorReport, theme: &TerminalTheme) -> Vec<String> {
    let passed_count = report.passed_count();
    let total_count = report.checks.len();
    let failed_count = report.failed_count();
    let mut lines = vec![
        theme.command("Diagnostic Dev Workflow"),
        format!(
            "{} {passed_count}/{total_count} vérifications OK",
            if failed_count == 0 {
                theme.success("✓")
            } else {
                theme.warning("!")
            }
        ),
        format!(
            "Statut    : {}",
            if failed_count == 0 {
                "OK"
            } else {
                "à corriger"
            }
        ),
        format!("Blocages  : {failed_count}"),
        String::new(),
    ];
    lines.extend(render_doctor_check_group(
        "À corriger",
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
            let title = item.title.clone().unwrap_or_else(|| "(sans titre)".into());
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
                let title = task.title.clone().unwrap_or_else(|| "(sans titre)".into());
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
        format!("Chemin    : {}", status.path),
        format!("Statut    : {}", repository_status_label(status)),
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
        format!("Statut    : {}", summary.status),
    ];
    push_finish_summary_list(&mut lines, "Fait      ", &summary.done);
    push_finish_summary_list(&mut lines, "Décisions ", &summary.decisions);
    push_finish_summary_list(&mut lines, "Risques   ", &summary.risks);
    push_finish_summary_list(&mut lines, "Blocages  ", &summary.blockers);
    push_finish_summary_list(&mut lines, "Suite     ", &summary.follow_up);
    lines
}

fn push_finish_summary_list(lines: &mut Vec<String>, label: &str, items: &[String]) {
    if !items.is_empty() {
        lines.push(format!("{label}: {}", items.join(" | ")));
    }
}

fn finish_pull_request_line(result: &dw_task::finish::FinishPullRequestResult) -> String {
    let url = result.url.as_deref().unwrap_or("(url non retournée)");
    match result.action {
        dw_task::finish::FinishPullRequestAction::Created => {
            format!("PR créée pour {}: {url}", result.repository)
        }
        dw_task::finish::FinishPullRequestAction::Existing => {
            format!("PR déjà ouverte pour {}: {url}", result.repository)
        }
        dw_task::finish::FinishPullRequestAction::Skipped => format!(
            "PR ignorée pour {}: {}",
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
            "azureDevOpsRepository manquant"
        }
        None => "raison inconnue",
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
        "Work items assignés: {} groupe(s), {} item(s)",
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
                "  Enfant  : {}",
                ado_work_item_summary(item, theme)
            ));
        }
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines)
}

fn ado_start_command_line(ids: &str, project: &str, theme: &TerminalTheme) -> String {
    format!(
        "Démarrer  : {}",
        theme.command(&format!("dw task start {ids} --project {project}"))
    )
}

fn ado_work_item_summary(item: &dw_ado::WorkItemSnapshot, theme: &TerminalTheme) -> String {
    format!(
        "{} {} {}",
        theme.success(&format!("#{}", item.id)),
        theme.dim(&format!(
            "[{} / {}]",
            item.kind.as_deref().unwrap_or("type inconnu"),
            item.state.as_deref().unwrap_or("état inconnu")
        )),
        item.title.as_deref().unwrap_or("(sans titre)")
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
            item.work_item.kind.as_deref().unwrap_or("type inconnu")
        ),
        format!(
            "État      : {}",
            item.work_item.state.as_deref().unwrap_or("état inconnu")
        ),
        format!(
            "Titre     : {}",
            item.work_item.title.as_deref().unwrap_or("(sans titre)")
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
            .as_ref()
            .map(|id| id.to_string())
            .or_else(|| relation.url.clone())
            .unwrap_or_default()
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

    lines.push(format!("Détails préflight - {title}"));
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
            lines.push(format!("  Détail : {details}"));
        }
        if !issue.related_ids.is_empty() {
            lines.push(format!(
                "  Liés   : {}",
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

    lines.push(format!("Détails handoff - {title}"));
    for item in items {
        lines.push(format!(
            "{} {} [{}]",
            handoff_status_icon(&item.status, item.valid),
            item.repository,
            handoff_status_label(&item.status)
        ));
        lines.push(format!("  Message : {}", item.message));
        if !item.path.trim().is_empty() {
            lines.push(format!("  Fichier : {}", item.path));
        }
        if item.valid {
            lines.push(format!(
                "  Synthèse: done={} decisions={} risks={} blockers={} follow_up={}",
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
    if valid { "✓ OK" } else { "✕ À corriger" }
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
        "[blocage]"
    } else if is_warning_severity(severity) {
        "[warning]"
    } else {
        "[info]"
    }
}

fn handoff_status_label(status: &str) -> &str {
    match status {
        "missing" => "manquant",
        "invalid" => "invalide",
        "blocked" => "bloqué",
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
        dw_task::prune::PruneSyncStatus::Skipped => "ignoré",
        dw_task::prune::PruneSyncStatus::Synced => "synchronisé",
    }
}

fn prune_sync_detail_label(detail: &dw_task::prune::PruneSyncDetail) -> String {
    match detail {
        dw_task::prune::PruneSyncDetail::AuthUnavailable { error } => {
            format!("auth indisponible: {error}")
        }
        dw_task::prune::PruneSyncDetail::SyncFailed { error } => error.clone(),
        dw_task::prune::PruneSyncDetail::Synced { work_items } => {
            format_current_work_items(work_items)
        }
    }
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

fn work_item_line(item: &dw_workspace::WorkspaceWorkItem) -> String {
    format!(
        "#{} [{} / {}] {}",
        item.id,
        item.kind.as_deref().unwrap_or("type inconnu"),
        item.state.as_deref().unwrap_or("état inconnu"),
        item.title.as_deref().unwrap_or("(sans titre)")
    )
}

fn work_item_action_label(action: dw_task::work_item::WorkItemUpdateAction) -> &'static str {
    match action {
        dw_task::work_item::WorkItemUpdateAction::Add => "ajout",
        dw_task::work_item::WorkItemUpdateAction::Remove => "retrait",
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
        "Statut    : enregistré".into(),
        format!("Clé       : {}", report.key),
        format!("Stockage  : {}", report.storage),
        "Valeur    : masquée".into(),
    ]
}

pub fn secret_get_lines(report: &SecretGetReport) -> Vec<String> {
    vec![
        "Secret".into(),
        format!(
            "Statut    : {}",
            if report.exists {
                "présent"
            } else {
                "introuvable"
            }
        ),
        format!("Clé       : {}", report.key),
        "Valeur    : masquée".into(),
    ]
}

pub fn secret_delete_lines(report: &SecretDeleteReport) -> Vec<String> {
    vec![
        "Secret".into(),
        "Statut    : supprimé si présent".into(),
        format!("Clé       : {}", report.key),
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
        theme.warning("! manquant")
    };
    let mut lines = vec![format!(
        "{:<10} {} via {}",
        status, check.agent_name, check.command
    )];
    if !check.available {
        lines.push(format!(
            "           {}",
            theme.command(&format!(
                "Installer `{}` ou vérifier le PATH",
                check.command
            ))
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
            theme.error("! À corriger")
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

    lines.push(theme.bold(&theme.cyan("Requête DB")));
    lines.push(format!(
        "Résultat  : {}",
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
            "Résultat tronqué après {} ligne(s). Relancer avec --max-rows pour élargir.",
            result.rows.len()
        )));
    }
    lines.join("\n")
}

fn render_sql_guard(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    let mut lines = vec![theme.bold(&theme.cyan("Garde SQL"))];
    lines.push(format!(
        "Statut    : {}",
        db_guard_status_label(result, theme)
    ));
    if result.is_allowed {
        lines.push(format!("Décision  : {}", theme.success("✓")));
        lines.push("Message   : Requête autorisée en lecture seule.".into());
        lines.push(format!(
            "Détail    : {}",
            theme.dim("Aucune exécution n'a été lancée par cette commande.")
        ));
    } else {
        lines.push(format!("Décision  : {}", theme.error("!")));
        lines.push("Message   : Requête bloquée avant exécution.".into());
        lines.push(format!(
            "Raison    : {}",
            result.reason.as_deref().unwrap_or("raison inconnue")
        ));
        lines.push(format!(
            "À faire   : {}",
            theme.warning("Utiliser uniquement SELECT/WITH ou les commandes d'introspection.")
        ));
    }
    lines.join("\n")
}

fn db_row_count_label(result: &QueryResult) -> String {
    let suffix = if result.rows.len() > 1 { "s" } else { "" };
    if result.truncated {
        format!(
            "{} ligne{suffix} affichée{suffix}, résultat tronqué",
            result.rows.len()
        )
    } else {
        format!("{} ligne{suffix}", result.rows.len())
    }
}

fn db_guard_status_label(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    if result.is_allowed {
        theme.success("autorisé")
    } else {
        theme.error("bloqué")
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
    fn upgrade_event_line_renders_one_step_per_action() {
        let line = upgrade_event_line(&dw_upgrade::UpgradeEvent {
            step: dw_upgrade::UpgradeStep::DownloadAsset,
            message: "Téléchargement de dw-linux-x64.tar.gz".into(),
        });

        assert!(line.contains("Upgrade [download"));
        assert!(line.contains("Téléchargement de dw-linux-x64.tar.gz"));
        assert!(!line.contains("download/checksum"));
    }

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

        assert_eq!(lines[0], "PR actives · ha");
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

        assert!(lines.contains(&"ADO assignés".into()));
        assert!(lines.contains(&"Éléments  : 1".into()));
        assert!(lines.contains(&"Item      : #42 [Bug / En developpement] Corriger".into()));
        assert!(lines.contains(&"Démarrer  : dw task start 42 --project ha".into()));
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

        assert!(lines.contains(&"Work items assignés: 1 groupe(s), 2 item(s)".into()));
        assert!(lines.contains(&"Parent    : #42 [User Story / Actif] Parent".into()));
        assert!(lines.contains(&"Démarrer  : dw task start 42,43 --project ha".into()));
        assert!(lines.contains(&"  Enfant  : #43 [Task / Actif] Enfant".into()));
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

        assert_eq!(lines[0], "Mise à jour ADO");
        assert!(lines.contains(&"Projet    : ha".into()));
        assert!(lines.contains(&"État      : Actif".into()));
        assert!(lines.contains(&"Work items: #42, #43".into()));
        assert!(lines.contains(&"2 work items passés en `Actif`.".into()));
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
        assert!(lines.contains(&"Type      : type inconnu".into()));
        assert!(lines.contains(&"État      : état inconnu".into()));
        assert!(lines.contains(&"Titre     : (sans titre)".into()));
        assert!(lines.contains(&"Contexte  : dw ado context 7 --project ha".into()));
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
        assert!(output.contains("État      : Actif"));
        assert!(output.contains("Titre     : Corriger"));
        assert!(output.contains("Assigné   : Sacha"));
        assert!(
            output
                .contains("Métadonnées: area=Produit\\Backlog | iteration=Sprint 1 | tags=urgent")
        );
        assert!(output.contains("Description courte"));
        assert!(output.contains("Critères d'acceptation"));
        assert!(output.contains("Critère A"));
        assert!(output.contains("Pièces jointes (1)"));
        assert!(output.contains("Dossier   : attachments/ado/42"));
        assert!(output.contains("capture.png"));
        assert!(output.contains("- Parent #1"));
        assert!(output.contains("- Bob: OK"));
        assert!(output.contains("Contexte IA: dw ado ai-context 42 --project ha"));
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
            vec![
                "Aucun work item détecté dans les messages de commit de la plage git.".to_string()
            ]
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
            project: dw_core::ProjectKey::from("ha"),
            work_item_ids: vec![dw_core::WorkItemId::from("42")],
            has_blocking_issues: true,
            issues: vec![TaskPreflightIssue {
                code: "missing_attachment".into(),
                severity: "blocking".into(),
                work_item_id: dw_core::WorkItemId::from("42"),
                message: "Piece jointe manquante".into(),
                details: Some("screenshot absent".into()),
                related_ids: vec![],
            }],
        };

        let lines = task_preflight_lines(&report);

        assert_eq!(lines[0], "Préflight task");
        assert!(lines.contains(&"Statut    : ✕ À corriger".into()));
        assert!(lines.contains(&"Blocages  : 1".into()));
        assert!(
            lines.contains(&"✕ [blocage] #42 missing_attachment - Piece jointe manquante".into())
        );
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
        assert!(lines.contains(&"Statut    : ✕ À corriger".into()));
        assert!(lines.contains(&"Handoffs  : 1/1 valides".into()));
        assert!(lines.contains(&"✓ front [done]".into()));
        assert!(
            lines.contains(&"  Synthèse: done=2 decisions=1 risks=0 blockers=0 follow_up=1".into())
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

        assert_eq!(lines[0], "Nettoyage workspaces");
        assert_eq!(lines[1], "Mode      : prévisualisation");
        assert!(lines.contains(&"Candidats : 1".into()));
        assert!(lines.contains(&"À faire   : dw task prune --execute".into()));
        assert!(lines.contains(&"Workspace : /tmp/dw/projects/ha/workspaces/feat-1-done".into()));
        assert!(lines.contains(&"Éléments  : ha / #1 Done [Valide]".into()));
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

        assert_eq!(lines[0], "Commit des repositories");
        assert!(lines.contains(&"Repository: front".into()));
        assert!(lines.contains(&"Statut    : Changements détectés:".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"À faire   : dw task commit --execute".into()));
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
                project: dw_core::ProjectKey::from("ha"),
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

        assert_eq!(lines[0], "Finalisation workspace");
        assert!(lines.contains(&"Statut    : OK".into()));
        assert!(lines.contains(&"- [done] front - OK".into()));
        assert!(lines.contains(&"Commit à créer".into()));
        assert!(lines.contains(&"Message   : feat(42): demo".into()));
        assert!(lines.contains(&"Handoff front".into()));
        assert!(lines.contains(&"Fait      : UI ajustée".into()));
        assert!(lines.contains(&"- front -> develop".into()));
        assert!(lines.contains(&"À faire   : dw task finish --execute".into()));
        assert!(lines.contains(&"Non-TTY   : ajouter --yes pour confirmer sans prompt".into()));
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

        assert_eq!(lines[0], "Ajout repository (prévisualisation)");
        assert!(lines.contains(&"Ancrage   : /tmp/project/repositories/front-anchor".into()));
        assert!(lines.contains(&"À faire   : dw task add-repo front --execute".into()));
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

        assert_eq!(dry_run[0], "Suppression workspace (prévisualisation)");
        assert_eq!(dry_run[2], "Actions   : 1");
        assert_eq!(dry_run[3], "Actions prévues");
        assert_eq!(execute[0], "Suppression workspace exécutée");
        assert_eq!(execute[2], "Actions   : 1");
        assert_eq!(execute[3], "Actions appliquées");
        assert!(dry_run.contains(&"- [front] remove-worktree: /tmp/ws/front".into()));
        assert!(dry_run.contains(&"À faire   : dw task teardown --execute".into()));
        assert!(dry_run.contains(&"Non-TTY   : ajouter --yes pour confirmer sans prompt".into()));
        assert!(!execute.contains(&"À faire   : dw task teardown --execute".into()));
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

        assert_eq!(lines[0], "Synchronisation task");
        assert_eq!(lines[1], "Workspace : /tmp/ws");
        assert_eq!(lines[2], "Items     : 1");
        assert_eq!(lines[4], "Work items ADO");
        assert_eq!(lines[5], "#42 [type inconnu / état inconnu] (sans titre)");
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

        assert_eq!(lines[0], "Renommage workspace");
        assert!(lines.contains(&"Mode      : prévisualisation".into()));
        assert!(lines.contains(&"Slug      : old -> new".into()));
        assert!(lines.contains(&"Branche   : feat/1-old -> feat/1-new".into()));
        assert!(lines.contains(&"À faire   : dw task rename <slug> --execute".into()));
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

        assert_eq!(lines[0], "Sous-tâche ADO");
        assert_eq!(lines[1], "Statut    : enregistrée dans le workspace");
        assert_eq!(lines[2], "Workspace : /tmp/ws");
        assert_eq!(lines[3], "Repository: front");
        assert_eq!(lines[4], "Item      : #42");
        assert_eq!(lines[5], "Titre     : [FRONT] Corriger");
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
        assert_eq!(lines[1], "Mode      : prévisualisation");
        assert_eq!(lines[2], "Action    : ajout");
        assert!(lines.contains(&"Branche   : feat/1-old -> feat/1-2-new".into()));
        assert!(lines.contains(&"Éléments  : #1, #2".into()));
        assert!(lines.contains(&"À faire   : dw task add-work-item --execute".into()));
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
        assert!(lines.contains(&"Relancer avec --execute pour créer le workspace.".into()));
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
