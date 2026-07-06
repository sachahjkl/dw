use crate::{ALL_AGENT_KINDS, AgentKind, AgentOpenRequest, build_open_launch};
use anyhow::Result;
use dw_core::Agent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAgentArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub positional_work_item: Option<String>,
    pub r#continue: bool,
    pub repo: Option<String>,
    pub agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAction {
    Open(OpenAgentArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDoctorReport {
    pub checks: Vec<AgentDoctorCheck>,
}

impl AgentDoctorReport {
    pub fn available_count(&self) -> usize {
        self.checks.iter().filter(|check| check.available).count()
    }

    pub fn total_count(&self) -> usize {
        self.checks.len()
    }

    pub fn passed(&self) -> bool {
        self.available_count() == self.total_count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDoctorCheck {
    pub agent_name: String,
    pub command: String,
    pub available: bool,
}

pub fn agent_doctor(requested: Option<Agent>) -> Result<AgentDoctorReport> {
    let agents = if let Some(agent) = requested {
        vec![agent_kind(agent)]
    } else {
        ALL_AGENT_KINDS.to_vec()
    };
    let checks = agents
        .into_iter()
        .map(|agent| {
            let launch = launch_probe(agent);
            let available = command_available(&launch.file_name, &["--help"]);
            AgentDoctorCheck {
                agent_name: agent.name().into(),
                command: launch.file_name,
                available,
            }
        })
        .collect::<Vec<_>>();

    Ok(AgentDoctorReport { checks })
}

fn agent_kind(agent: Agent) -> AgentKind {
    match agent {
        Agent::Opencode => AgentKind::Opencode,
        Agent::Cursor => AgentKind::Cursor,
        Agent::Claude => AgentKind::Claude,
        Agent::Codex | Agent::CodexCli => AgentKind::Codex,
        Agent::Copilot => AgentKind::Copilot,
    }
}

fn launch_probe(agent: AgentKind) -> crate::AgentLaunch {
    build_open_launch(
        Some(agent.name()),
        &AgentOpenRequest {
            root: ".".into(),
            workspace: ".".into(),
            r#continue: false,
        },
    )
    .expect("known agent should build launch")
}

fn command_available(file_name: &str, arguments: &[&str]) -> bool {
    dw_process::command_available(file_name, arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_args_preserve_resolution_inputs() {
        let args = OpenAgentArgs {
            workspace: Some("/tmp/work".into()),
            root: None,
            project: None,
            work_item: None,
            positional_work_item: None,
            r#continue: false,
            repo: Some("front".into()),
            agent: Some("codex".into()),
        };

        assert_eq!(args.workspace.as_deref(), Some("/tmp/work"));
        assert_eq!(args.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn agent_report_counts_availability() {
        let checks = vec![
            AgentDoctorCheck {
                agent_name: "codex".into(),
                command: "codex".into(),
                available: true,
            },
            AgentDoctorCheck {
                agent_name: "missing".into(),
                command: "missing".into(),
                available: false,
            },
        ];
        let report = AgentDoctorReport { checks };

        assert_eq!(report.available_count(), 1);
        assert_eq!(report.total_count(), 2);
        assert!(!report.passed());
    }
}
