use crate::{ALL_AGENT_KINDS, AgentKind, AgentOpenRequest, agent_context, build_open_launch};
use anyhow::Result;
use clap::Subcommand;
use dw_config::{default_agent, resolve_root, set_default_agent};
use dw_ui::TerminalTheme;
use std::process::Command;

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    Context,
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        positional_work_item: Option<String>,
    },
    Config {
        #[arg(long)]
        root: Option<String>,
    },
    Show {
        #[arg(long)]
        root: Option<String>,
    },
    SetDefault {
        agent: String,
        #[arg(long)]
        root: Option<String>,
    },
    Doctor {
        #[arg(long)]
        agent: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Handled,
    Open(OpenAgentArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentDoctorCheck {
    agent_name: String,
    command: String,
    available: bool,
}

pub fn handle_agent(command: AgentCommand) -> Result<AgentAction> {
    match command {
        AgentCommand::Context => {
            let root = resolve_root(None);
            println!("{}", agent_context(&root));
            Ok(AgentAction::Handled)
        }
        AgentCommand::Open {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            repo,
            agent,
            positional_work_item,
        } => Ok(AgentAction::Open(OpenAgentArgs {
            workspace,
            root,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
        })),
        AgentCommand::Config { root } | AgentCommand::Show { root } => {
            let root = resolve_root(root.as_deref());
            print_styled(&format!("Agent par defaut: {}", default_agent(&root)));
            Ok(AgentAction::Handled)
        }
        AgentCommand::SetDefault { root, agent } => {
            let root = resolve_root(root.as_deref());
            let agent = set_default_agent(&root, &agent)?;
            print_styled(&format!("Agent par defaut: {agent}"));
            Ok(AgentAction::Handled)
        }
        AgentCommand::Doctor { agent } => {
            run_agent_doctor(agent.as_deref())?;
            Ok(AgentAction::Handled)
        }
    }
}

fn run_agent_doctor(requested: Option<&str>) -> Result<()> {
    let agents = if let Some(agent) = requested.filter(|value| !value.trim().is_empty()) {
        vec![crate::parse_agent_kind(Some(agent))?]
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

    println!(
        "{}",
        render_agent_report(&checks, &TerminalTheme::stdout_auto())
    );
    Ok(())
}

fn render_agent_report(checks: &[AgentDoctorCheck], theme: &TerminalTheme) -> String {
    let mut lines = vec![
        theme.command("Agents disponibles"),
        String::new(),
        format!("{:<12} {:<12} Status", "Agent", "Command"),
    ];
    for check in checks {
        let status = if check.available {
            theme.success("✓ OK")
        } else {
            theme.warning("! missing")
        };
        lines.push(format!(
            "{:<12} {:<12} {}",
            check.agent_name, check.command, status
        ));
    }
    lines.join("\n")
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
    Command::new(file_name)
        .args(arguments)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_command_returns_open_action() {
        let action = handle_agent(AgentCommand::Open {
            workspace: Some("/tmp/work".into()),
            root: None,
            project: None,
            work_item: None,
            r#continue: false,
            repo: Some("front".into()),
            agent: Some("codex".into()),
            positional_work_item: None,
        })
        .expect("open action should be built");

        assert_eq!(
            action,
            AgentAction::Open(OpenAgentArgs {
                workspace: Some("/tmp/work".into()),
                root: None,
                project: None,
                work_item: None,
                positional_work_item: None,
                r#continue: false,
                repo: Some("front".into()),
                agent: Some("codex".into()),
            })
        );
    }

    #[test]
    fn agent_report_marks_missing_agent_without_probe_side_effects() {
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

        let report = render_agent_report(&checks, &TerminalTheme::plain());

        assert!(report.contains("Agents disponibles"));
        assert!(report.contains("codex"));
        assert!(report.contains("✓ OK"));
        assert!(report.contains("! missing"));
    }
}
