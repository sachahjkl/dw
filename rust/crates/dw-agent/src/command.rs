use crate::{ALL_AGENT_KINDS, AgentKind, AgentOpenRequest, agent_context, build_open_launch};
use anyhow::Result;
use clap::Subcommand;
use dw_config::{default_agent, resolve_root, set_default_agent};
use dw_ui::TerminalTheme;
use std::process::Command;

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    #[command(about = "Affiche le contexte DevWorkflow injecté aux agents IA.")]
    Context,
    #[command(about = "Ouvre ou reprend un agent sur un workspace task.")]
    Open {
        #[arg(
            long,
            conflicts_with_all = ["project", "work_item", "continue"],
            help = "Chemin du workspace à ouvrir directement."
        )]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré à utiliser pour résoudre un workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item servant à résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository à ouvrir dans le workspace, si applicable.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent à lancer: opencode, cursor, claude, codex ou copilot."
        )]
        agent: Option<String>,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Affiche la configuration agent effective.")]
    Config {
        #[arg(long, help = "Root DevWorkflow à lire.")]
        root: Option<String>,
    },
    #[command(about = "Affiche la configuration agent effective.")]
    Show {
        #[arg(long, help = "Root DevWorkflow à lire.")]
        root: Option<String>,
    },
    #[command(about = "Définit l'agent par défaut du root DevWorkflow.")]
    SetDefault {
        #[arg(help = "Agent à utiliser par défaut: opencode, cursor, claude, codex ou copilot.")]
        agent: String,
        #[arg(long, help = "Root DevWorkflow à modifier.")]
        root: Option<String>,
    },
    #[command(about = "Diagnostique la disponibilité des agents installés.")]
    Doctor {
        #[arg(long, help = "Limiter le diagnostic à un agent.")]
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
            println!(
                "{}",
                render_agent_config(&root, &default_agent(&root), &TerminalTheme::stdout_auto())
            );
            Ok(AgentAction::Handled)
        }
        AgentCommand::SetDefault { root, agent } => {
            let root = resolve_root(root.as_deref());
            let agent = set_default_agent(&root, &agent)?;
            println!(
                "{}",
                render_agent_config_updated(&root, &agent, &TerminalTheme::stdout_auto())
            );
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
    let available_count = checks.iter().filter(|check| check.available).count();
    let total_count = checks.len();
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
    for check in checks {
        let status = if check.available {
            theme.success("✓ OK")
        } else {
            theme.warning("! manquant")
        };
        lines.push(format!(
            "{:<10} {} via {}",
            status, check.agent_name, check.command
        ));
        if !check.available {
            lines.push(format!(
                "           {}",
                theme.command(&format!(
                    "Installer `{}` ou vérifier le PATH",
                    check.command
                ))
            ));
        }
    }
    lines.join("\n")
}

fn render_agent_config(root: &str, agent: &str, theme: &TerminalTheme) -> String {
    [
        theme.command("Config agent"),
        format!("Agent par défaut: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
    .join("\n")
}

fn render_agent_config_updated(root: &str, agent: &str, theme: &TerminalTheme) -> String {
    [
        theme.success("✓ Config agent mise à jour"),
        format!("Agent par défaut: {}", theme.bold(agent)),
        format!("Root DevWorkflow: {}", theme.path(root)),
    ]
    .join("\n")
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

        assert!(report.contains("Diagnostic agents"));
        assert!(report.contains("! 1/2 agents disponibles"));
        assert!(report.contains("codex"));
        assert!(report.contains("✓ OK"));
        assert!(report.contains("! manquant"));
        assert!(report.contains("Installer `missing` ou vérifier le PATH"));
    }

    #[test]
    fn agent_config_lines_include_root_and_default_agent() {
        let report = render_agent_config("/tmp/dw", "codex", &TerminalTheme::plain());

        assert!(report.contains("Config agent"));
        assert!(report.contains("Agent par défaut: codex"));
        assert!(report.contains("Root DevWorkflow: /tmp/dw"));
    }

    #[test]
    fn agent_config_updated_lines_include_status() {
        let report = render_agent_config_updated("/tmp/dw", "opencode", &TerminalTheme::plain());

        assert!(report.contains("✓ Config agent mise à jour"));
        assert!(report.contains("Agent par défaut: opencode"));
        assert!(report.contains("Root DevWorkflow: /tmp/dw"));
    }
}
