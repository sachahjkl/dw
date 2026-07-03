pub mod command;
pub mod completion;

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
    #[error("Agent inconnu: {0}. Agents disponibles: opencode, cursor, claude, codex-cli, copilot")]
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

Utilise `dw` pour les opérations du workflow:

- `dw doctor` vérifie les prérequis locaux.
- `dw auth login` connecte Azure DevOps quand la connexion silencieuse est indisponible.
- `dw ado assigned --project <name>` liste les work items assignés.
- `dw ado work-item <workItemId> --project <name>` lit le résumé d'un work item.
- `dw ado ai-context <workItemId> --project <name>` lit le contexte work item structuré et déterministe pour usage IA.
- `dw task current` affiche le workspace task actif et la branche.
- `dw task sync --continue` rafraîchit `task.json` depuis ADO quand le contexte local peut être obsolète.
- `dw task preflight --continue` vérifie les blocages et alertes déterministes avant implémentation ou découpage en child tasks.
- `dw task handoff-validate --continue` valide les contrats handoff avant `task finish` ou exécution de sub-agents.
- `dw task open --workspace <path>` ouvre une nouvelle session agent pour un workspace.
- `dw task open --continue` reprend une session agent existante sur le workspace le plus récent.
- `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"` crée des child tasks ADO après rédaction du plan.
- `dw task commit --continue --execute` crée un commit intermédiaire sans push ni PR.
- `dw task finish --continue --execute --create-pr` est le flow commit/push/PR attendu en fin de travail.
- `dw db schema`, `dw db describe` et `dw db query` sont les points d'entrée SQL et restent read-only par défaut.

Root configuré courant:

```text
{root}
```

Règles importantes:

1. Les work items Azure DevOps sont la source de vérité.
2. Utiliser `dw` pour toute opération ADO, nommage Git, PR et worktree.
3. Ne pas utiliser les outils MCP Azure DevOps.
4. Lire le work item avec `dw ado work-item` avant de coder, puis lancer `dw ado ai-context` avant d'agir sur le contexte ADO.
5. Avant de travailler, vérifier que le setup initial requis par l'environnement est fait: `pnpm install`, `pnpm approve-builds --all`, `npm install` en fallback, ou `dotnet restore` selon le projet.
6. Mettre à jour `plan.md` avant d'implémenter.
7. Écrire tout texte utilisateur/projet en français.
8. Ne pas normaliser les labels métier ni le vocabulaire de domaine issus d'ADO, des screenshots, mockups ou textes projet.
9. Traiter les screenshots, mockups et attachments comme sources factuelles.
10. Les branches, commits et titres de PR sont créés par `dw`; ne pas les créer manuellement.
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
        "# Workspace DevWorkflow\n\nCe workspace est géré par `dw`.\n\nContexte:\n\n- Project: `{project}`\n- Work items:\n{items}\n\nRègles:\n\n1. Lancer `dw task current` pour identifier le workspace task courant.\n2. Lire chaque work item avec `dw ado work-item <id> --project {project}` avant de coder.\n3. Lire `dw ado ai-context <id> --project {project}` avant d'agir sur le contexte ADO.\n4. Utiliser `dw db schema`, `dw db describe` et `dw db query` quand le contexte base de données peut clarifier le changement.\n5. Avant de travailler, vérifier que le setup initial requis par l'environnement est en place.\n6. Remplir `plan.md` avant d'implémenter.\n7. Lancer `dw task preflight --continue` avant implémentation, création de child tasks ou autre action irréversible.\n8. Lancer `dw task handoff-validate --continue` avant de lancer des sub-agents et avant `dw task finish`.\n9. Si le work item principal est une `User Story` ou une `Anomalie`, une fois `plan.md` complet et avant le début de l'implémentation, créer au moins une child task ADO, puis autant que nécessaire depuis le plan, avec `dw task create-child-task --continue --repo <front|back|db|foo> --title \"<action explicite>\"`.\n10. Écrire tout texte utilisateur/projet en français: plans, commentaires, messages de commit/PR, titres des tasks, synthèses d'avancement et explications finales.\n11. Structurer le plan explicitement par domaine quand c'est possible: front, back, db ou autres repositories. Utiliser des sub-agents pour les chantiers indépendants quand c'est possible.\n12. Utiliser `dw task sync --continue` avant les décisions de cycle de vie si le contexte ADO local peut être obsolète.\n13. Utiliser `dw task commit` pour les commits intermédiaires.\n14. Utiliser `dw task finish` pour les flows finaux push/PR.\n15. Utiliser `dw task teardown` ou `dw task prune` pour le nettoyage.\n"
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
        assert!(agents.content.contains("dw task create-child-task"));
        assert!(agents.content.contains("dw task preflight --continue"));
        assert!(
            agents
                .content
                .contains("dw task handoff-validate --continue")
        );
        assert!(
            agents
                .content
                .contains("Utiliser des sub-agents pour les chantiers indépendants")
        );
    }
}
