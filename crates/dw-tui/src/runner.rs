use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dw_app::{DwActionRequest, DwActionResult};
use dw_core::{DwActionEvent, ExternalLaunchPlan};
use std::io;

use crate::history::ActionRunErrorMessage;
use crate::model::{TuiAction, TuiActionRequest};

#[derive(Debug, Clone)]
pub struct ActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
    pub launch: ExternalLaunchPlan,
}

#[derive(Debug, Clone)]
pub struct CapturedActionRunResult {
    pub display_label: String,
    pub status_label: String,
    pub success: bool,
    pub events: Vec<DwActionEvent>,
    pub result: DwActionResult,
}

#[derive(Debug, Clone)]
pub struct CapturedActionRunError {
    pub display_label: String,
    pub events: Vec<DwActionEvent>,
    pub message: ActionRunErrorMessage,
}

impl CapturedActionRunError {
    fn from_error(display_label: String, events: Vec<DwActionEvent>, error: anyhow::Error) -> Self {
        Self {
            display_label,
            events,
            message: ActionRunErrorMessage::new(format!("{error:#}")),
        }
    }

    pub fn interrupted(display_label: String, message: impl Into<String>) -> Self {
        Self {
            display_label,
            events: Vec::new(),
            message: ActionRunErrorMessage::new(message),
        }
    }
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

pub async fn run_attached(action: &TuiAction) -> Result<ActionRunResult> {
    let launch = external_launch_plan(action).await?;
    restore_terminal().ok();
    let status = run_external_launch_plan(&launch);
    install_terminal().ok();
    status?;

    Ok(ActionRunResult {
        display_label: action.display_label(),
        status_label: "ok".into(),
        success: true,
        launch,
    })
}

pub async fn run_captured_streaming<F>(
    action: &TuiAction,
    mut on_event: F,
) -> std::result::Result<CapturedActionRunResult, CapturedActionRunError>
where
    F: FnMut(DwActionEvent),
{
    let mut events = Vec::new();
    let display_label = action.display_label();
    let action_run = dw_app::spawn_action(action.request.clone());
    let mut event_stream = action_run.events;
    let result = action_run.result;

    while let Some(event) = event_stream.recv().await {
        on_event(event.clone());
        events.push(event);
    }
    let result = result
        .await
        .map_err(|error| {
            CapturedActionRunError::from_error(display_label.clone(), events.clone(), error.into())
        })?
        .map_err(|error| {
            CapturedActionRunError::from_error(display_label.clone(), events.clone(), error)
        })?;

    Ok(CapturedActionRunResult {
        display_label,
        status_label: "ok".into(),
        success: true,
        events,
        result,
    })
}

async fn external_launch_plan(action: &TuiAction) -> Result<ExternalLaunchPlan> {
    match &action.request {
        TuiActionRequest::TaskOpen(args) => {
            let action_run = dw_app::spawn_action(DwActionRequest::TaskOpen(args.clone()));
            let mut events = action_run.events;
            while events.recv().await.is_some() {}
            match action_run.result.await?? {
                DwActionResult::Task(result) => match *result {
                    dw_app::TaskActionResult::Open(plan) => Ok(plan),
                    result => anyhow::bail!("Unexpected agent open result: {result:?}"),
                },
                result => anyhow::bail!("Unexpected external launch result: {result:?}"),
            }
        }
        _ => anyhow::bail!(
            "External action is not mapped to ExternalLaunchPlan: {}",
            action.display_label()
        ),
    }
}

fn run_external_launch_plan(launch: &ExternalLaunchPlan) -> Result<()> {
    let status = dw_process::status(
        launch.program_as_str(),
        launch.argument_strings(),
        launch.working_directory.as_ref().map(|path| path.as_str()),
        launch.environment_strings(),
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

        let error = result.expect_err("missing root should fail");
        assert!(output.iter().any(|event| matches!(
            event,
            DwActionEvent::Task(dw_core::TaskActionEvent::ResolvingPullRequestWorkItems {
                pull_request_id
            }) if *pull_request_id == dw_core::PullRequestId::from("42")
        )));
        assert!(error.events.iter().any(|event| matches!(
            event,
            DwActionEvent::Task(dw_core::TaskActionEvent::ResolvingPullRequestWorkItems {
                pull_request_id
            }) if *pull_request_id == dw_core::PullRequestId::from("42")
        )));
    }
}
