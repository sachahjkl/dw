use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dw_app::{DwActionRequest, DwActionResult};
use dw_core::{DwActionEvent, ExternalLaunchPlan};
use std::io;

use crate::model::{TuiAction, TuiActionRequest};

#[derive(Debug, Clone)]
pub struct ActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct CapturedActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
    pub events: Vec<DwActionEvent>,
    pub result: DwActionResult,
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
    mut on_event: F,
) -> Result<CapturedActionRunResult>
where
    F: FnMut(DwActionEvent),
{
    let mut events = Vec::new();
    let request = action_request(action)?;
    let result = dw_app::run_action(request, |event| {
        on_event(event.clone());
        events.push(event);
    })
    .await?;

    Ok(CapturedActionRunResult {
        display_label: action.display_label(),
        status_label: "ok".into(),
        success: true,
        events,
        result,
    })
}

fn action_request(action: &TuiAction) -> Result<DwActionRequest> {
    match &action.request {
        TuiActionRequest::Version => Ok(DwActionRequest::Version),
        TuiActionRequest::Doctor => Ok(DwActionRequest::Doctor),
        TuiActionRequest::Guide => Ok(DwActionRequest::Guide),
        TuiActionRequest::Refresh(args) => Ok(DwActionRequest::Refresh(args.clone())),
        TuiActionRequest::ConfigShow { root } => {
            Ok(DwActionRequest::ConfigShow { root: root.clone() })
        }
        TuiActionRequest::ConfigInit(args) => Ok(DwActionRequest::ConfigInit(args.clone())),
        TuiActionRequest::ConfigDoctor { root } => {
            Ok(DwActionRequest::ConfigDoctor { root: root.clone() })
        }
        TuiActionRequest::ConfigSetColor { mode } => {
            Ok(DwActionRequest::ConfigSetColor { mode: *mode })
        }
        TuiActionRequest::ConfigSetRoot { path } => {
            Ok(DwActionRequest::ConfigSetRoot { path: path.clone() })
        }
        TuiActionRequest::AgentConfig { root } => {
            Ok(DwActionRequest::AgentConfig { root: root.clone() })
        }
        TuiActionRequest::AgentSetDefault { root, agent } => Ok(DwActionRequest::AgentSetDefault {
            root: root.clone(),
            agent: *agent,
        }),
        TuiActionRequest::AgentDoctor { agent } => {
            Ok(DwActionRequest::AgentDoctor { agent: *agent })
        }
        TuiActionRequest::AgentOpen(_) => {
            anyhow::bail!("External action executed by run_attached.")
        }
        TuiActionRequest::DbGuard(args) => Ok(DwActionRequest::DbGuard(args.clone())),
        TuiActionRequest::DbSchema(args) => Ok(DwActionRequest::DbSchema(args.clone())),
        TuiActionRequest::DbDescribe(args) => Ok(DwActionRequest::DbDescribe(args.clone())),
        TuiActionRequest::DbQuery(args) => Ok(DwActionRequest::DbQuery(args.clone())),
        TuiActionRequest::AdoAssigned(args) => Ok(DwActionRequest::AdoAssigned(args.clone())),
        TuiActionRequest::AdoPrs(args) => Ok(DwActionRequest::AdoPrs(args.clone())),
        TuiActionRequest::AdoChangelog(args) => Ok(DwActionRequest::AdoChangelog(args.clone())),
        TuiActionRequest::AdoContext(args) => Ok(DwActionRequest::AdoContext(args.clone())),
        TuiActionRequest::AdoAiContext(args) => Ok(DwActionRequest::AdoAiContext(args.clone())),
        TuiActionRequest::AdoWorkItem(args) => Ok(DwActionRequest::AdoWorkItem(args.clone())),
        TuiActionRequest::AdoSetState(args) => Ok(DwActionRequest::AdoSetState(args.clone())),
        TuiActionRequest::TaskStart(args) => Ok(DwActionRequest::TaskStart(args.clone())),
        TuiActionRequest::TaskStartPr(args) => Ok(DwActionRequest::TaskStartPr(args.clone())),
        TuiActionRequest::TaskPreflight(args) => Ok(DwActionRequest::TaskPreflight(args.clone())),
        TuiActionRequest::TaskHandoffValidate(args) => {
            Ok(DwActionRequest::TaskHandoffValidate(args.clone()))
        }
        TuiActionRequest::TaskSync(args) => Ok(DwActionRequest::TaskSync(args.clone())),
        TuiActionRequest::TaskRename(args) => Ok(DwActionRequest::TaskRename(args.clone())),
        TuiActionRequest::TaskRepoLatest(args) => Ok(DwActionRequest::TaskRepoLatest(args.clone())),
        TuiActionRequest::TaskCommit(args) => Ok(DwActionRequest::TaskCommit(args.clone())),
        TuiActionRequest::TaskAddRepo(args) => Ok(DwActionRequest::TaskAddRepo(args.clone())),
        TuiActionRequest::TaskTeardown(args) => Ok(DwActionRequest::TaskTeardown(args.clone())),
        TuiActionRequest::TaskFinish(args) => Ok(DwActionRequest::TaskFinish(args.clone())),
        TuiActionRequest::TaskPrune(args) => Ok(DwActionRequest::TaskPrune(args.clone())),
        TuiActionRequest::TaskCreateChildTask(args) => {
            Ok(DwActionRequest::TaskCreateChildTask(args.clone()))
        }
        TuiActionRequest::TaskAddWorkItem(args) => {
            Ok(DwActionRequest::TaskAddWorkItem(args.clone()))
        }
        TuiActionRequest::TaskRemoveWorkItem(args) => {
            Ok(DwActionRequest::TaskRemoveWorkItem(args.clone()))
        }
        TuiActionRequest::SecretGet { key } => Ok(DwActionRequest::SecretGet { key: key.clone() }),
        TuiActionRequest::SecretSetFromEnv { key, env } => Ok(DwActionRequest::SecretSetFromEnv {
            key: key.clone(),
            env: env.clone(),
        }),
        TuiActionRequest::SecretDelete { key } => {
            Ok(DwActionRequest::SecretDelete { key: key.clone() })
        }
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
                pull_request_id: dw_core::PullRequestId::from("42"),
                root: Some(dw_core::DevWorkflowRoot::from("/tmp/missing-dw-root")),
                project: dw_core::ProjectKey::from("ha"),
                repositories: Vec::new(),
                type_name: None,
                slug: None,
                mode: dw_core::ExecutionMode::Preview,
            }),
            description: "test".into(),
            kind: ActionRisk::Safe,
        };
        let mut output = Vec::new();

        let result = run_captured_streaming(&action, |event| output.push(event)).await;

        assert!(result.is_err());
        assert!(output.iter().any(|event| matches!(
            event,
            DwActionEvent::Task(dw_core::TaskActionEvent::ResolvingPullRequestWorkItems {
                pull_request_id
            }) if pull_request_id.as_str() == "42"
        )));
    }
}
