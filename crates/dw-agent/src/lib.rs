pub mod command;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const DEFAULT_AGENT: &str = "opencode";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Opencode,
    Cursor,
    Claude,
    Codex,
    Copilot,
}

pub const ALL_AGENT_KINDS: [AgentKind; 5] = [
    AgentKind::Opencode,
    AgentKind::Cursor,
    AgentKind::Claude,
    AgentKind::Codex,
    AgentKind::Copilot,
];

impl AgentKind {
    pub fn name(self) -> &'static str {
        match self {
            AgentKind::Opencode => "opencode",
            AgentKind::Cursor => "cursor",
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Copilot => "copilot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLaunch {
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    #[serde(rename = "workingDirectory")]
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOpenRequest {
    pub root: String,
    pub workspace: String,
    pub r#continue: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkspaceConfigRequest {
    pub workspace: String,
    pub work_items: Vec<WorkspaceWorkItemRef>,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWorkItemRef {
    pub id: String,
    pub kind: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkspaceConfigFile {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error(
        "Agent inconnu: {0}. Agents disponibles: opencode, cursor, claude, codex, codex-cli, copilot"
    )]
    UnknownAgent(String),
}

pub trait AgentAdapter {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch;
}

pub fn parse_agent_kind(agent: Option<&str>) -> Result<AgentKind, AgentError> {
    let name = agent
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_AGENT);
    match name.to_ascii_lowercase().as_str() {
        "opencode" => Ok(AgentKind::Opencode),
        "cursor" | "cursor-agent" | "agent" => Ok(AgentKind::Cursor),
        "claude" => Ok(AgentKind::Claude),
        "codex-cli" | "codex" => Ok(AgentKind::Codex),
        "copilot" => Ok(AgentKind::Copilot),
        other => Err(AgentError::UnknownAgent(other.into())),
    }
}

pub fn build_open_launch(
    agent: Option<&str>,
    request: &AgentOpenRequest,
) -> Result<AgentLaunch, AgentError> {
    Ok(parse_agent_kind(agent)?.launch(request))
}

pub fn build_open_launch_plan(
    agent: Option<&str>,
    request: &AgentOpenRequest,
) -> Result<dw_core::ExternalLaunchPlan, AgentError> {
    Ok(build_open_launch(agent, request)?.into())
}

impl From<AgentLaunch> for dw_core::ExternalLaunchPlan {
    fn from(launch: AgentLaunch) -> Self {
        Self {
            program: launch.file_name,
            arguments: launch.arguments,
            environment: launch.environment,
            working_directory: Some(launch.working_directory),
        }
    }
}

impl AgentAdapter for AgentKind {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        match self {
            AgentKind::Opencode => Opencode.launch(request),
            AgentKind::Cursor => CursorAgent.launch(request),
            AgentKind::Claude => Claude.launch(request),
            AgentKind::Codex => Codex.launch(request),
            AgentKind::Copilot => Copilot.launch(request),
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
            file_name: "opencode".into(),
            arguments: if request.r#continue {
                vec!["-c".into(), request.workspace.clone()]
            } else {
                vec![request.workspace.clone()]
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
            file_name: "agent".into(),
            arguments: if request.r#continue {
                vec![
                    "--workspace".into(),
                    request.workspace.clone(),
                    "--continue".into(),
                ]
            } else {
                vec!["--workspace".into(), request.workspace.clone()]
            },
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for Claude {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: "claude".into(),
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
            file_name: "codex".into(),
            arguments: if request.r#continue {
                vec![
                    "resume".into(),
                    "--last".into(),
                    "--cd".into(),
                    request.workspace.clone(),
                ]
            } else {
                vec!["--cd".into(), request.workspace.clone()]
            },
            environment: BTreeMap::new(),
            working_directory: request.workspace.clone(),
        }
    }
}

impl AgentAdapter for Copilot {
    fn launch(&self, request: &AgentOpenRequest) -> AgentLaunch {
        AgentLaunch {
            file_name: "copilot".into(),
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

fn workspace_agents_md(work_items: &[WorkspaceWorkItemRef], project: &str) -> String {
    let items = work_items
        .iter()
        .map(|item| {
            let suffix = match (&item.kind, &item.title) {
                (None, None) => String::new(),
                (kind, title) => format!(
                    " [{}] {}",
                    kind.clone().unwrap_or_else(|| "?".into()),
                    title.clone().unwrap_or_default()
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
            Some("codex"),
            &AgentOpenRequest {
                root: "/root".into(),
                workspace: "/workspace".into(),
                r#continue: true,
            },
        )
        .expect("launch should build");

        assert_eq!(
            launch.arguments,
            vec!["resume", "--last", "--cd", "/workspace"]
        );
    }

    #[test]
    fn cursor_uses_workspace_flag() {
        let launch = build_open_launch(
            Some("cursor"),
            &AgentOpenRequest {
                root: "/root".into(),
                workspace: "/workspace".into(),
                r#continue: true,
            },
        )
        .expect("launch should build");

        assert_eq!(launch.file_name, "agent");
        assert_eq!(
            launch.arguments,
            vec!["--workspace", "/workspace", "--continue"]
        );
    }

    #[test]
    fn codex_cli_alias_and_unknown_error_are_documented() {
        assert_eq!(
            parse_agent_kind(Some("codex-cli")).unwrap(),
            AgentKind::Codex
        );

        let error = parse_agent_kind(Some("unknown"))
            .expect_err("unknown agent should fail")
            .to_string();

        assert!(error.contains("codex"));
        assert!(error.contains("codex-cli"));
    }

    #[test]
    fn workspace_config_files_include_expected_paths() {
        let files = workspace_config_files(&AgentWorkspaceConfigRequest {
            workspace: "/workspace".into(),
            work_items: vec![WorkspaceWorkItemRef {
                id: "55222".into(),
                kind: None,
                title: None,
            }],
            project: "ha".into(),
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
            workspace: "/workspace".into(),
            work_items: vec![WorkspaceWorkItemRef {
                id: "11010".into(),
                kind: Some("User Story".into()),
                title: Some("Titre HA".into()),
            }],
            project: "ha".into(),
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
