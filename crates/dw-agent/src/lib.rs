pub mod command;

use dw_core::{
    Agent, AgentExecutableName, DevWorkflowRoot, ExternalLaunchArgument, ExternalProgramName,
    ProjectKey, WorkItemId, WorkItemTitle, WorkItemTypeName, WorkspacePath,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_AGENT: Agent = Agent::Opencode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLaunch {
    #[serde(rename = "fileName")]
    pub file_name: AgentExecutableName,
    pub arguments: Vec<ExternalLaunchArgument>,
    pub environment: BTreeMap<String, String>,
    #[serde(rename = "workingDirectory")]
    pub working_directory: WorkspacePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOpenRequest {
    pub root: DevWorkflowRoot,
    pub workspace: WorkspacePath,
    pub r#continue: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkspaceConfigRequest {
    pub workspace: WorkspacePath,
    pub work_items: Vec<WorkspaceWorkItemRef>,
    pub project: ProjectKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWorkItemRef {
    pub id: WorkItemId,
    pub kind: Option<WorkItemTypeName>,
    pub title: Option<WorkItemTitle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkspaceConfigFile {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContextReport {
    pub root: DevWorkflowRoot,
}

pub trait AgentAdapter {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch;
}

pub fn build_open_launch(agent: Option<Agent>, request: &AgentOpenRequest) -> AgentLaunch {
    agent.unwrap_or(DEFAULT_AGENT).launch(request)
}

pub fn build_open_launch_plan(
    agent: Option<Agent>,
    request: &AgentOpenRequest,
) -> dw_core::ExternalLaunchPlan {
    build_open_launch(agent, request).into()
}

impl From<AgentLaunch> for dw_core::ExternalLaunchPlan {
    fn from(launch: AgentLaunch) -> Self {
        Self {
            program: ExternalProgramName::from(launch.file_name.to_string()),
            arguments: launch.arguments,
            environment: launch.environment,
            working_directory: Some(launch.working_directory.to_string()),
        }
    }
}

impl AgentAdapter for Agent {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        match self {
            Agent::Opencode => Opencode.launch(request),
            Agent::Cursor => CursorAgent.launch(request),
            Agent::Claude => Claude.launch(request),
            Agent::Codex | Agent::CodexCli => Codex.launch(request),
            Agent::Copilot => Copilot.launch(request),
        }
    }
}

struct Opencode;
struct CursorAgent;
struct Claude;
struct Codex;
struct Copilot;

fn arg(value: impl Into<String>) -> ExternalLaunchArgument {
    ExternalLaunchArgument::from(value.into())
}

impl AgentAdapter for Opencode {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("opencode"),
            arguments: if request.r#continue {
                vec![arg("-c"), arg(request.workspace.to_string())]
            } else {
                vec![arg(request.workspace.to_string())]
            },
            environment: BTreeMap::from([(
                "OPENCODE_CONFIG".into(),
                format!("{}/config/opencode/opencode.jsonc", request.root),
            )]),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for CursorAgent {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("agent"),
            arguments: if request.r#continue {
                vec![
                    arg("--workspace"),
                    arg(request.workspace.to_string()),
                    arg("--continue"),
                ]
            } else {
                vec![arg("--workspace"), arg(request.workspace.to_string())]
            },
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for Claude {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("claude"),
            arguments: request
                .r#continue
                .then(|| arg("--continue"))
                .into_iter()
                .collect(),
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for Codex {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("codex"),
            arguments: if request.r#continue {
                vec![
                    arg("resume"),
                    arg("--last"),
                    arg("--cd"),
                    arg(request.workspace.to_string()),
                ]
            } else {
                vec![arg("--cd"), arg(request.workspace.to_string())]
            },
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for Copilot {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("copilot"),
            arguments: request
                .r#continue
                .then(|| arg("--continue"))
                .into_iter()
                .collect(),
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

pub fn workspace_config_files(
    request: &AgentWorkspaceConfigRequest,
) -> Vec<AgentWorkspaceConfigFile> {
    vec![
        AgentWorkspaceConfigFile {
            relative_path: "AGENTS.md".into(),
            content: workspace_agents_md(&request.work_items, &request.project),
        },
        AgentWorkspaceConfigFile {
            relative_path: "CLAUDE.md".into(),
            content: workspace_agents_md(&request.work_items, &request.project),
        },
        AgentWorkspaceConfigFile {
            relative_path: ".claude/CLAUDE.md".into(),
            content: workspace_agents_md(&request.work_items, &request.project),
        },
        AgentWorkspaceConfigFile {
            relative_path: ".cursor/rules/devworkflow.mdc".into(),
            content: format!(
                "---\nalwaysApply: true\n---\n\n{}",
                workspace_agents_md(&request.work_items, &request.project)
            ),
        },
        AgentWorkspaceConfigFile {
            relative_path: ".codex/config.toml".into(),
            content: "# Configuration Codex locale au projet.\n# Les instructions d'exécution principales sont chargées depuis AGENTS.md dans ce workspace.\n".into(),
        },
        AgentWorkspaceConfigFile {
            relative_path: ".github/copilot-instructions.md".into(),
            content: workspace_agents_md(&request.work_items, &request.project),
        },
    ]
}

pub fn agent_context(root: &DevWorkflowRoot) -> AgentContextReport {
    AgentContextReport { root: root.clone() }
}

fn workspace_agents_md(work_items: &[WorkspaceWorkItemRef], project: &ProjectKey) -> String {
    let items = work_items
        .iter()
        .map(|item| {
            let suffix = match (&item.kind, &item.title) {
                (None, None) => String::new(),
                (kind, title) => format!(
                    " [{}] {}",
                    kind.as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "?".into()),
                    title.as_ref().map(ToString::to_string).unwrap_or_default()
                )
                .trim_end()
                .to_string(),
            };
            format!("  - `#{}`{}", item.id, suffix)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "# Workspace DevWorkflow\n\nCe workspace est géré par DevWorkflow.\n\nContexte:\n\n- Project: `{project}`\n- Work items:\n{items}\n\nRègles:\n\n1. Identifier le workspace task courant avant d'agir.\n2. Lire chaque work item ADO avant de coder.\n3. Lire le contexte IA ADO avant d'agir sur le contexte ADO.\n4. Utiliser les actions DB schema, describe et query quand le contexte base de données peut clarifier le changement.\n5. Avant de travailler, vérifier que le setup initial requis par l'environnement est en place.\n6. Remplir `plan.md` avant d'implémenter.\n7. Lancer le préflight task avant implémentation, création de child tasks ou autre action irréversible.\n8. Valider les contrats handoff avant de lancer des sub-agents et avant la finalisation task.\n9. Si le work item principal est une `User Story` ou une `Anomalie`, une fois `plan.md` complet et avant le début de l'implémentation, créer au moins une child task ADO, puis autant que nécessaire depuis le plan.\n10. Écrire tout texte utilisateur/projet en français: plans, commentaires, messages de commit/PR, titres des tasks, synthèses d'avancement et explications finales.\n11. Structurer le plan explicitement par domaine quand c'est possible: front, back, db ou autres repositories. Utiliser des sub-agents pour les chantiers indépendants quand c'est possible.\n12. Synchroniser la task avant les décisions de cycle de vie si le contexte ADO local peut être obsolète.\n13. Utiliser l'action commit task pour les commits intermédiaires.\n14. Utiliser l'action finalisation task pour les flows finaux push/PR.\n15. Utiliser les actions teardown ou prune pour le nettoyage.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_continue_uses_resume_last_with_cd() {
        let launch = build_open_launch(
            Some(Agent::Codex),
            &AgentOpenRequest {
                root: DevWorkflowRoot::from("/root"),
                workspace: WorkspacePath::from("/workspace"),
                r#continue: true,
            },
        );

        assert_eq!(
            launch.arguments,
            vec![arg("resume"), arg("--last"), arg("--cd"), arg("/workspace")]
        );
    }

    #[test]
    fn cursor_uses_workspace_flag() {
        let launch = build_open_launch(
            Some(Agent::Cursor),
            &AgentOpenRequest {
                root: DevWorkflowRoot::from("/root"),
                workspace: WorkspacePath::from("/workspace"),
                r#continue: true,
            },
        );

        assert_eq!(launch.file_name, AgentExecutableName::from("agent"));
        assert_eq!(
            launch.arguments,
            vec![arg("--workspace"), arg("/workspace"), arg("--continue")]
        );
    }

    #[test]
    fn codex_cli_agent_uses_codex_executable() {
        let launch = build_open_launch(
            Some(Agent::CodexCli),
            &AgentOpenRequest {
                root: DevWorkflowRoot::from("/root"),
                workspace: WorkspacePath::from("/workspace"),
                r#continue: false,
            },
        );

        assert_eq!(launch.file_name, AgentExecutableName::from("codex"));
    }

    #[test]
    fn workspace_config_files_include_expected_paths() {
        let files = workspace_config_files(&AgentWorkspaceConfigRequest {
            workspace: WorkspacePath::from("/workspace"),
            work_items: vec![WorkspaceWorkItemRef {
                id: WorkItemId::from("55222"),
                kind: None,
                title: None,
            }],
            project: ProjectKey::from("ha"),
        });

        let paths = files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"AGENTS.md"));
        assert!(paths.contains(&"CLAUDE.md"));
        assert!(paths.contains(&".claude/CLAUDE.md"));
        assert!(paths.contains(&".cursor/rules/devworkflow.mdc"));
        assert!(paths.contains(&".codex/config.toml"));
        assert!(paths.contains(&".github/copilot-instructions.md"));
    }

    #[test]
    fn workspace_agents_content_contains_workspace_rules() {
        let files = workspace_config_files(&AgentWorkspaceConfigRequest {
            workspace: WorkspacePath::from("/workspace"),
            work_items: vec![WorkspaceWorkItemRef {
                id: WorkItemId::from("11010"),
                kind: Some(WorkItemTypeName::from("User Story")),
                title: Some(WorkItemTitle::from("Titre HA")),
            }],
            project: ProjectKey::from("ha"),
        });

        let agents = files
            .iter()
            .find(|file| file.relative_path == "AGENTS.md")
            .expect("AGENTS.md should exist");

        assert!(agents.content.contains("# Workspace DevWorkflow"));
        assert!(agents.content.contains("#11010"));
        assert!(agents.content.contains("child task ADO"));
        assert!(agents.content.contains("préflight task"));
        assert!(agents.content.contains("Valider les contrats handoff"));
        assert!(
            agents
                .content
                .contains("Utiliser des sub-agents pour les chantiers indépendants")
        );
    }
}
