use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use crate::history::{ActionRunId, ActionRunLabel};
use crate::model::{
    self, ActionEffect, AdoAssignedProject, TuiAction, TuiPullRequest, TuiSnapshot,
};
use crate::runner::{self, CapturedActionRunError, CapturedActionRunResult};
use dw_core::DwActionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundKind {
    Snapshot,
    Assigned,
    PullRequests,
    Action,
}

#[derive(Debug)]
pub enum BackgroundResult {
    Snapshot {
        generation: u64,
        snapshot: Box<TuiSnapshot>,
    },
    Assigned {
        generation: u64,
        items: Vec<AdoAssignedProject>,
    },
    PullRequests {
        generation: u64,
        items: Vec<TuiPullRequest>,
    },
    ActionEvent {
        generation: u64,
        run_id: ActionRunId,
        event: DwActionEvent,
    },
    Action {
        generation: u64,
        run_id: ActionRunId,
        label: ActionRunLabel,
        refresh_after_success: bool,
        open_after_success: bool,
        effect: Option<ActionEffect>,
        result: Box<Result<CapturedActionRunResult, CapturedActionRunError>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionStart {
    Started {
        run_id: ActionRunId,
        label: ActionRunLabel,
    },
    Queued {
        label: ActionRunLabel,
        position: usize,
    },
}

pub struct BackgroundJobs {
    generation: u64,
    sender: Sender<BackgroundResult>,
    receiver: Receiver<BackgroundResult>,
    next_action_run_id: u64,
    snapshot: Option<RunningJob>,
    assigned: Option<RunningJob>,
    pull_requests: Option<RunningJob>,
    action: Option<RunningJob>,
    action_label: Option<ActionRunLabel>,
    pending_actions: VecDeque<TuiAction>,
}

#[derive(Debug, Clone)]
struct RunningJob {
    generation: u64,
    started_at: Instant,
}

impl RunningJob {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            started_at: Instant::now(),
        }
    }

    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

impl BackgroundJobs {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            generation: 0,
            sender,
            receiver,
            next_action_run_id: 0,
            snapshot: None,
            assigned: None,
            pull_requests: None,
            action: None,
            action_label: None,
            pending_actions: VecDeque::new(),
        }
    }

    pub fn is_loading(&self, kind: BackgroundKind) -> bool {
        self.running(kind).is_some()
    }

    pub fn elapsed_label(&self, kind: BackgroundKind) -> Option<String> {
        self.running(kind).map(|job| format_elapsed(job.elapsed()))
    }

    pub fn action_label(&self) -> Option<&ActionRunLabel> {
        self.action_label.as_ref()
    }

    pub fn pending_action_count(&self) -> usize {
        self.pending_actions.len()
    }

    pub fn pending_action_labels(&self) -> Vec<ActionRunLabel> {
        self.pending_actions
            .iter()
            .map(|action| ActionRunLabel::new(action.display_label()))
            .collect()
    }

    pub fn start_snapshot(&mut self, root: Option<String>) -> bool {
        if self.is_loading(BackgroundKind::Snapshot) {
            return false;
        }
        self.cancel_data_loads();
        let generation = self.start_job(BackgroundKind::Snapshot);
        self.spawn_blocking(move |sender| {
            let snapshot = TuiSnapshot::load(root.as_deref());
            let _ = sender.send(BackgroundResult::Snapshot {
                generation,
                snapshot: Box::new(snapshot),
            });
        });
        true
    }

    pub fn start_assigned(&mut self, snapshot: &mut TuiSnapshot) -> bool {
        if self.is_loading(BackgroundKind::Assigned) {
            return false;
        }
        snapshot.assigned_loaded = false;
        let generation = self.start_job(BackgroundKind::Assigned);
        let root = snapshot.root.clone();
        let projects = snapshot.projects.clone();
        let workflow = snapshot.workflow.clone();
        self.spawn_async(move |sender| async move {
            let items = model::load_assigned_data(root, projects, workflow).await;
            let _ = sender.send(BackgroundResult::Assigned { generation, items });
        });
        true
    }

    pub fn start_pull_requests(&mut self, snapshot: &mut TuiSnapshot) -> bool {
        if self.is_loading(BackgroundKind::PullRequests) {
            return false;
        }
        snapshot.pull_requests_loaded = false;
        let generation = self.start_job(BackgroundKind::PullRequests);
        let root = snapshot.root.clone();
        let projects = snapshot.projects.clone();
        let workflow = snapshot.workflow.clone();
        let workspaces = snapshot.workspaces.clone();
        self.spawn_async(move |sender| async move {
            let items = model::load_pull_request_data(root, projects, workflow, workspaces).await;
            let _ = sender.send(BackgroundResult::PullRequests { generation, items });
        });
        true
    }

    pub fn start_action(&mut self, action: TuiAction) -> ActionStart {
        let label = ActionRunLabel::new(action.display_label());
        if self.is_loading(BackgroundKind::Action) {
            self.pending_actions.push_back(action);
            return ActionStart::Queued {
                label,
                position: self.pending_actions.len(),
            };
        }
        let run_id = self.spawn_action(action, label.clone());
        ActionStart::Started { run_id, label }
    }

    pub fn poll(&mut self) -> Vec<BackgroundResult> {
        let mut results = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(result) => results.push(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.snapshot = None;
                    self.assigned = None;
                    self.pull_requests = None;
                    self.action = None;
                    self.action_label = None;
                    self.pending_actions.clear();
                    break;
                }
            }
        }
        results
    }

    pub fn accept_assigned(&mut self, generation: u64) -> bool {
        self.accept_job(BackgroundKind::Assigned, generation)
    }

    pub fn accept_snapshot(&mut self, generation: u64) -> bool {
        self.accept_job(BackgroundKind::Snapshot, generation)
    }

    pub fn accept_pull_requests(&mut self, generation: u64) -> bool {
        self.accept_job(BackgroundKind::PullRequests, generation)
    }

    pub fn accept_action(&mut self, generation: u64) -> bool {
        if self.accept_job(BackgroundKind::Action, generation) {
            self.action_label = None;
            true
        } else {
            false
        }
    }

    pub fn accepts_action_output(&self, generation: u64) -> bool {
        self.running(BackgroundKind::Action)
            .is_some_and(|job| job.generation == generation)
    }

    pub fn start_next_action(&mut self) -> Option<(ActionRunId, ActionRunLabel)> {
        if self.is_loading(BackgroundKind::Action) {
            return None;
        }
        let action = self.pending_actions.pop_front()?;
        let label = ActionRunLabel::new(action.display_label());
        let run_id = self.spawn_action(action, label.clone());
        Some((run_id, label))
    }

    pub fn restart_assigned(&mut self, snapshot: &mut TuiSnapshot) {
        self.assigned = None;
        let _ = self.start_assigned(snapshot);
    }

    pub fn restart_pull_requests(&mut self, snapshot: &mut TuiSnapshot) {
        self.pull_requests = None;
        let _ = self.start_pull_requests(snapshot);
    }

    fn cancel_data_loads(&mut self) {
        self.assigned = None;
        self.pull_requests = None;
    }

    fn start_job(&mut self, kind: BackgroundKind) -> u64 {
        self.generation += 1;
        let generation = self.generation;
        *self.running_mut(kind) = Some(RunningJob::new(generation));
        generation
    }

    fn spawn_action(&mut self, action: TuiAction, label: ActionRunLabel) -> ActionRunId {
        let generation = self.start_job(BackgroundKind::Action);
        let run_id = self.next_action_run_id();
        let refresh_after_success = action.should_refresh_after_success();
        let open_after_success = action.opens_result_after_success();
        let effect = action.successful_effect();
        self.action_label = Some(label.clone());
        self.spawn_async(move |sender| async move {
            let output_sender = sender.clone();
            let action_label = label.to_string();
            let result = match tokio::spawn(async move {
                runner::run_captured_streaming(&action, move |event| {
                    let _ = output_sender.send(BackgroundResult::ActionEvent {
                        generation,
                        run_id,
                        event,
                    });
                })
                .await
            })
            .await
            {
                Ok(result) => result,
                Err(error) if error.is_panic() => Err(CapturedActionRunError::interrupted(
                    action_label.clone(),
                    "TUI action interrupted by an internal panic. The action was stopped cleanly.",
                )),
                Err(error) => Err(CapturedActionRunError::interrupted(
                    action_label,
                    format!("TUI action interrupted: {error}"),
                )),
            };
            let _ = sender.send(BackgroundResult::Action {
                generation,
                run_id,
                label,
                refresh_after_success,
                open_after_success,
                effect,
                result: Box::new(result),
            });
        });
        run_id
    }

    fn next_action_run_id(&mut self) -> ActionRunId {
        self.next_action_run_id += 1;
        ActionRunId::new(self.next_action_run_id)
    }

    fn accept_job(&mut self, kind: BackgroundKind, generation: u64) -> bool {
        if self
            .running(kind)
            .is_some_and(|job| job.generation == generation)
        {
            *self.running_mut(kind) = None;
            true
        } else {
            false
        }
    }

    fn running(&self, kind: BackgroundKind) -> Option<&RunningJob> {
        match kind {
            BackgroundKind::Snapshot => self.snapshot.as_ref(),
            BackgroundKind::Assigned => self.assigned.as_ref(),
            BackgroundKind::PullRequests => self.pull_requests.as_ref(),
            BackgroundKind::Action => self.action.as_ref(),
        }
    }

    fn running_mut(&mut self, kind: BackgroundKind) -> &mut Option<RunningJob> {
        match kind {
            BackgroundKind::Snapshot => &mut self.snapshot,
            BackgroundKind::Assigned => &mut self.assigned,
            BackgroundKind::PullRequests => &mut self.pull_requests,
            BackgroundKind::Action => &mut self.action,
        }
    }

    fn spawn_blocking<F>(&self, work: F)
    where
        F: FnOnce(Sender<BackgroundResult>) + Send + 'static,
    {
        let sender = self.sender.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(move || work(sender));
        } else {
            work(sender);
        }
    }

    fn spawn_async<F, Fut>(&self, work: F)
    where
        F: FnOnce(Sender<BackgroundResult>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let sender = self.sender.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(work(sender));
        } else {
            let _ = self.sender.send(BackgroundResult::ActionEvent {
                generation: self.generation,
                run_id: ActionRunId::new(0),
                event: DwActionEvent::Started {
                    action_id: "runtime-unavailable".into(),
                },
            });
        }
    }
}

fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 1 {
        "<1s".into()
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_jobs_track_single_action_slot() {
        let mut jobs = BackgroundJobs::new();
        let action = crate::model::TuiAction {
            label: "Version".into(),
            request: crate::model::TuiActionRequest::Version,
            description: "Version".into(),
            kind: crate::model::ActionRisk::Safe,
        };

        assert!(matches!(
            jobs.start_action(action.clone()),
            ActionStart::Started { .. }
        ));
        assert!(matches!(
            jobs.start_action(action),
            ActionStart::Queued { position: 1, .. }
        ));
        assert!(jobs.is_loading(BackgroundKind::Action));
        assert!(jobs.action_label().is_some());
        assert!(jobs.elapsed_label(BackgroundKind::Action).is_some());
        assert_eq!(jobs.pending_action_count(), 1);
        assert_eq!(
            jobs.pending_action_labels(),
            [ActionRunLabel::new("Version")]
        );
    }

    #[test]
    fn accepting_wrong_generation_keeps_job_running() {
        let mut jobs = BackgroundJobs::new();
        let mut snapshot = TuiSnapshot::load(Some("/tmp/missing-dw-root"));

        assert!(jobs.start_assigned(&mut snapshot));
        assert!(!jobs.accept_assigned(999));
        assert!(jobs.is_loading(BackgroundKind::Assigned));
    }

    #[test]
    fn queued_action_starts_after_previous_action_is_accepted() {
        let mut jobs = BackgroundJobs::new();
        let first = crate::model::TuiAction {
            label: "First".into(),
            request: crate::model::TuiActionRequest::Version,
            description: "Version".into(),
            kind: crate::model::ActionRisk::Safe,
        };
        let second = crate::model::TuiAction {
            label: "Second".into(),
            request: crate::model::TuiActionRequest::Doctor,
            description: "Doctor".into(),
            kind: crate::model::ActionRisk::Safe,
        };

        assert!(matches!(
            jobs.start_action(first),
            ActionStart::Started { .. }
        ));
        assert!(matches!(
            jobs.start_action(second),
            ActionStart::Queued { position: 1, .. }
        ));
        assert_eq!(jobs.pending_action_count(), 1);
        assert_eq!(
            jobs.pending_action_labels(),
            [ActionRunLabel::new("Second")]
        );
        assert!(jobs.accept_action(1));

        let (_, label) = jobs.start_next_action().expect("queued action");

        assert_eq!(label, ActionRunLabel::new("Second"));
        assert_eq!(jobs.pending_action_count(), 0);
        assert_eq!(jobs.action_label(), Some(&ActionRunLabel::new("Second")));
        assert!(jobs.is_loading(BackgroundKind::Action));
    }
}
