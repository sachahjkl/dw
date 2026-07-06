pub mod command;

use dw_core::{
    Agent, AgentExecutableName, DevWorkflowRoot, ProjectKey, WorkItemId, WorkItemTitle,
    WorkItemTypeName, WorkspacePath,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_AGENT: Agent = Agent::Opencode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLaunch {
    #[serde(rename = "fileName")]
    pub file_name: AgentExecutableName,
    pub arguments: Vec<String>,
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
            program: launch.file_name.to_string(),
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

impl AgentAdapter for Opencode {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: AgentExecutableName::from("opencode"),
            arguments: if request.r#continue {
                vec!["-c".into(), request.workspace.to_string()]
            } else {
                vec![request.workspace.to_string()]
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
                    "--workspace".into(),
                    request.workspace.to_string(),
                    "--continue".into(),
                ]
            } else {
                vec!["--workspace".into(), request.workspace.to_string()]
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
                .then(|| "--continue".into())
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
                    "resume".into(),
                    "--last".into(),
                    "--cd".into(),
                    request.workspace.to_string(),
                ]
            } else {
                vec!["--cd".into(), request.workspace.to_string()]
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
                .then(|| "--continue".into())
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

pub fn agent_context(root: &str) -> String {
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
{root}
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
"#
    )
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
            vec!["resume", "--last", "--cd", "/workspace"]
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
            vec!["--workspace", "/workspace", "--continue"]
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
