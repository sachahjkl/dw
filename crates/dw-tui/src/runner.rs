use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dw_core::{ActionEvent, ExternalLaunchPlan};
use dw_tui_adapter::render;
use dw_ui::TerminalTheme;
use std::io;

use crate::model::{TuiAction, TuiActionRequest};
use crate::ui_text::guide_detail_lines;

#[derive(Debug, Clone)]
pub struct ActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
    pub output: String,
}

pub fn install_terminal() -> Result<()> {
    enable_raw_mode().context("enable raw terminal mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).context("open TUI screen")?;
    Ok(())
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode().ok();
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)
        .context("restore terminal")?;
    Ok(())
}

pub fn run_attached(action: &TuiAction) -> Result<ActionRunResult> {
    let launch = external_launch_plan(action)?;
    restore_terminal().ok();
    let status = run_external_launch_plan(&launch);
    install_terminal().ok();
    status?;

    Ok(ActionRunResult {
        display_label: action.display_label(),
        status_label: "ok".into(),
        success: true,
    })
}

pub async fn run_captured_streaming<F>(
    action: &TuiAction,
    mut on_output: F,
) -> Result<CapturedActionRunResult>
where
    F: FnMut(String),
{
    let text = dispatch_internal_action(action, |event| on_output(event.message)).await?;

    Ok(CapturedActionRunResult {
        display_label: action.display_label(),
        status_label: "ok".into(),
        success: true,
        output: text,
    })
}

async fn dispatch_internal_action(
    action: &TuiAction,
    mut emit: impl FnMut(ActionEvent),
) -> Result<String> {
    let theme = TerminalTheme::plain();
    match &action.request {
        TuiActionRequest::Version => Ok(format!("Dev Workflow {}", env!("CARGO_PKG_VERSION"))),
        TuiActionRequest::Doctor => {
            let report = dw_doctor::run_doctor(false)?;
            Ok(render::doctor_report_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::Guide => Ok(guide_detail_lines().join("\n")),
        TuiActionRequest::Refresh(args) => {
            let report = dw_config::command::refresh(args.clone())?;
            Ok(render::refresh_report_lines(&report).join("\n"))
        }
        TuiActionRequest::ConfigShow { root } => {
            let report = dw_config::command::show(root.as_deref());
            Ok(render::config_show_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::ConfigInit(args) => {
            let report = dw_config::command::init(args.clone())?;
            Ok(render::init_report_lines(&report).join("\n"))
        }
        TuiActionRequest::ConfigDoctor { root } => {
            let report = dw_config::command::doctor(root.as_deref());
            Ok(render::config_doctor_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::ConfigSetColor { mode } => {
            let report = dw_config::command::set_color(mode)?;
            Ok([
                "Configuration updated".into(),
                format!("Color     : {}", report.value),
            ]
            .join("\n"))
        }
        TuiActionRequest::ConfigSetRoot { path } => {
            let report = dw_config::command::set_root(path)?;
            Ok([
                "Configuration updated".into(),
                format!("Root      : {}", report.value),
            ]
            .join("\n"))
        }
        TuiActionRequest::AgentConfig { root } => {
            let root = dw_config::resolve_root(root.as_deref());
            let agent = dw_config::default_agent(&root);
            Ok(render::agent_config_lines(&root, &agent, &theme).join("\n"))
        }
        TuiActionRequest::AgentSetDefault { root, agent } => {
            let root = dw_config::resolve_root(root.as_deref());
            let agent = dw_config::set_default_agent(&root, agent)?;
            Ok(render::agent_config_updated_lines(&root, &agent, &theme).join("\n"))
        }
        TuiActionRequest::AgentDoctor { agent } => {
            let report = dw_agent::command::agent_doctor(agent.as_deref())?;
            Ok(render::agent_doctor_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::AgentOpen(_) => {
            anyhow::bail!("External action executed by run_attached.")
        }
        TuiActionRequest::DbGuard(args) => {
            Ok(render::db_guard_lines(&dw_db::commands::guard(args.clone()), &theme).join("\n"))
        }
        TuiActionRequest::DbSchema(args) => Ok(render::db_query_table(
            &dw_db::commands::schema(args.clone()).await?,
            &theme,
        )),
        TuiActionRequest::DbDescribe(args) => Ok(dw_db::commands::describe(args.clone())
            .await?
            .map(|result| render::db_query_table(&result, &theme))
            .unwrap_or_default()),
        TuiActionRequest::DbQuery(args) => Ok(render::db_query_table(
            &dw_db::commands::query(args.clone()).await?,
            &theme,
        )),
        TuiActionRequest::AdoAssigned(args) => {
            let report =
                dw_ado_commands::commands::assigned::report_with_events(args.clone(), &mut emit)
                    .await?;
            Ok(render::ado_assigned_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::AdoPrs(args) => Ok(render::ado_prs_lines(
            &dw_ado_commands::commands::prs::report(args.clone()).await?,
        )
        .join("\n")),
        TuiActionRequest::AdoChangelog(args) => {
            let report =
                dw_ado_commands::commands::changelog::report_with_events(args.clone(), &mut emit)
                    .await?;
            Ok(render::ado_changelog_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::AdoContext(args) => {
            let report = dw_ado_commands::commands::context::context_report_with_events(
                args.clone(),
                &mut emit,
            )
            .await?;
            Ok(render::ado_context_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::AdoAiContext(args) => {
            let report = dw_ado_commands::commands::context::ai_context_report_with_events(
                args.clone(),
                &mut emit,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&report.items)?)
        }
        TuiActionRequest::AdoWorkItem(args) => {
            let report =
                dw_ado_commands::commands::work_item::report_with_events(args.clone(), &mut emit)
                    .await?;
            Ok(render::ado_work_item_lines(&report, &theme).join("\n"))
        }
        TuiActionRequest::AdoSetState(args) => {
            let plan = dw_ado_commands::commands::set_state::plan(args.clone())?;
            let execution =
                dw_ado_commands::commands::set_state::execute_with_events(plan, &mut emit).await?;
            Ok(render::ado_set_state_execution_lines(&execution).join("\n"))
        }
        TuiActionRequest::TaskStart(args) => {
            let report = dw_task::start::start_plan(args.clone()).await?;
            if args.mode.executes() {
                Ok(render::task_start_execution_lines(
                    &dw_task::start::execute_start(report, args).await?,
                )
                .join("\n"))
            } else {
                Ok(render::task_start_plan_lines(&report).join("\n"))
            }
        }
        TuiActionRequest::TaskStartPr(args) => {
            emit(ActionEvent::info(dw_task::start::start_pr_fetch_line(
                &args.pull_request_id,
                &[],
            )));
            let report = dw_task::start::start_pr_plan(args.clone()).await?;
            emit(ActionEvent::info(dw_task::start::start_pr_resolved_line(
                &report.work_item_ids,
            )));
            if args.mode.executes() {
                Ok(render::task_start_execution_lines(
                    &dw_task::start::execute_start_pr(report, args).await?,
                )
                .join("\n"))
            } else {
                Ok(render::task_start_pr_plan_lines(&report).join("\n"))
            }
        }
        TuiActionRequest::TaskPreflight(args) => Ok(render::task_preflight_lines(
            &dw_task::validate::preflight_report(args.clone())?,
        )
        .join("\n")),
        TuiActionRequest::TaskHandoffValidate(args) => Ok(render::task_handoff_validation_lines(
            &dw_task::validate::handoff_validation_report(args.clone())?,
        )
        .join("\n")),
        TuiActionRequest::TaskSync(args) => Ok(render::task_sync_lines(
            &dw_task::lifecycle::sync_report(args.clone()).await?,
        )
        .join("\n")),
        TuiActionRequest::TaskRename(args) => {
            let plan = dw_task::lifecycle::rename_plan(args.clone())?;
            if args.mode.executes() {
                Ok(
                    render::task_rename_execution_lines(&dw_task::lifecycle::execute_rename(
                        &plan,
                    )?)
                    .join("\n"),
                )
            } else {
                Ok(render::task_rename_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::TaskRepoLatest(args) => {
            let plan = dw_task::repo::repo_latest_plan(args.clone())?;
            let mut lines = render::task_repo_latest_plan_lines(&plan);
            lines.extend(render::task_repo_latest_execution_lines(
                &dw_task::repo::execute_repo_latest(&plan)?,
            ));
            Ok(lines.join("\n"))
        }
        TuiActionRequest::TaskCommit(args) => {
            let plan = dw_task::repo::commit_plan(args.clone())?;
            if args.mode.executes() {
                let mut lines = render::task_commit_plan_lines(&plan, true);
                lines.extend(render::task_commit_execution_lines(
                    &dw_task::repo::execute_commit(&plan)?,
                ));
                Ok(lines.join("\n"))
            } else {
                Ok(render::task_commit_plan_lines(&plan, false).join("\n"))
            }
        }
        TuiActionRequest::TaskAddRepo(args) => {
            let plan = dw_task::repo::add_repo_plan(args.clone())?;
            if args.mode.executes() {
                let mut lines = render::task_add_repo_plan_lines(&plan);
                lines.extend(render::task_add_repo_execution_lines(
                    &dw_task::repo::execute_add_repo(&plan)?,
                ));
                Ok(lines.join("\n"))
            } else {
                Ok(render::task_add_repo_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::TaskTeardown(args) => {
            let plan = dw_task::repo::teardown_plan(args.clone())?;
            if args.mode.executes() && plan.workspace.is_some() {
                Ok(
                    render::task_teardown_execution_lines(&dw_task::repo::execute_teardown(&plan)?)
                        .join("\n"),
                )
            } else {
                Ok(render::task_teardown_plan_lines(&plan, args.mode.executes()).join("\n"))
            }
        }
        TuiActionRequest::TaskFinish(args) => {
            let plan = dw_task::finish::finish_plan(args.clone())?;
            if args.mode.executes() {
                Ok(render::task_finish_execution_lines(
                    &dw_task::finish::execute_finish(plan, args).await?,
                )
                .join("\n"))
            } else {
                Ok(render::task_finish_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::TaskPrune(args) => {
            let plan = dw_task::prune::plan(args.clone()).await?;
            if args.mode.executes() {
                Ok(render::task_prune_execution_lines(&dw_task::prune::execute(
                    &plan.root,
                    plan.candidates.clone(),
                )?)
                .join("\n"))
            } else {
                Ok(render::task_prune_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::TaskCreateChildTask(args) => Ok(render::task_child_task_lines(
            &dw_task::lifecycle::create_child_task_report(args.clone()).await?,
        )
        .join("\n")),
        TuiActionRequest::TaskAddWorkItem(args) => {
            let plan = dw_task::work_item::add_plan(args.clone()).await?;
            if args.mode.executes() {
                let mut lines = render::task_work_item_plan_lines(&plan);
                if let Some(execution) = dw_task::work_item::execute_update(&plan)? {
                    lines.extend(render::task_work_item_execution_lines(&execution));
                }
                Ok(lines.join("\n"))
            } else {
                Ok(render::task_work_item_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::TaskRemoveWorkItem(args) => {
            let plan = dw_task::work_item::remove_plan(args.clone())?;
            if args.mode.executes() {
                let mut lines = render::task_work_item_plan_lines(&plan);
                if let Some(execution) = dw_task::work_item::execute_update(&plan)? {
                    lines.extend(render::task_work_item_execution_lines(&execution));
                }
                Ok(lines.join("\n"))
            } else {
                Ok(render::task_work_item_plan_lines(&plan).join("\n"))
            }
        }
        TuiActionRequest::SecretGet { key } => {
            Ok(render::secret_get_lines(&dw_secret::command::get_secret(key)?).join("\n"))
        }
        TuiActionRequest::SecretSetFromEnv { key, env } => {
            let secret = dw_secret::secret_from_env(env)?;
            Ok(render::secret_set_lines(&dw_secret::command::set_secret(key, &secret)?).join("\n"))
        }
        TuiActionRequest::SecretDelete { key } => Ok(render::secret_delete_lines(
            &dw_secret::command::delete_secret_key(key)?,
        )
        .join("\n")),
    }
}

fn external_launch_plan(action: &TuiAction) -> Result<ExternalLaunchPlan> {
    match &action.request {
        TuiActionRequest::AgentOpen(args) => dw_task::open::resolve_open_launch(args.clone()),
        _ => anyhow::bail!(
            "External action is not mapped to ExternalLaunchPlan: {}",
            action.display_label()
        ),
    }
}

fn run_external_launch_plan(launch: &ExternalLaunchPlan) -> Result<()> {
    let status = dw_process::status(
        &launch.program,
        &launch.arguments,
        launch.working_directory.as_deref(),
        launch
            .environment
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    )?;
    if !status.success() {
        anyhow::bail!("external process exited with status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActionRisk, TuiAction, TuiActionRequest};

    #[tokio::test]
    async fn task_start_pr_streams_resolution_before_failure() {
        let action = TuiAction {
            label: "Start PR preview".into(),
            request: TuiActionRequest::TaskStartPr(dw_task::start::StartPrArgs {
                pull_request_id: "42".into(),
                root: Some("/tmp/missing-dw-root".into()),
                project: "ha".into(),
                repo: None,
                type_name: None,
                slug: None,
                mode: dw_core::ExecutionMode::Preview,
            }),
            description: "test".into(),
            kind: ActionRisk::Safe,
        };
        let mut output = Vec::new();

        let result = run_captured_streaming(&action, |line| output.push(line)).await;

        assert!(result.is_err());
        assert!(
            output
                .iter()
                .any(|line| line == "Resolving work items linked to PR #42...")
        );
    }
}
