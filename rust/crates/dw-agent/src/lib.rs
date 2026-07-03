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
            content: "# Project-local Codex config placeholder.\n# Primary execution instructions are loaded from AGENTS.md in this workspace.\n".into(),
        },
        AgentWorkspaceConfigFile {
            relative_path: ".github/copilot-instructions.md".into(),
            content: workspace_agents_md(&request.work_items, &request.project),
        },
    ]
}

pub fn agent_context(root: &str) -> String {
    format!(
        r#"# DevWorkflow agent context

You are working inside a DevWorkflow-managed environment.

Use `dw` for workflow operations:

- `dw doctor` checks local prerequisites.
- `dw auth login` connects Azure DevOps when the silent token is unavailable.
- `dw ado assigned --project <name>` lists assigned work items.
- `dw ado work-item <workItemId> --project <name>` reads a work item summary.
- `dw ado ai-context <workItemId> --project <name>` reads the deterministic structured work item context for AI consumption.
- `dw task current` prints the active task workspace and branch.
- `dw task sync --continue` refreshes `task.json` from ADO when the local context may be stale.
- `dw task preflight --continue` checks deterministic blockers/warnings before implementation or child-task decomposition.
- `dw task handoff-validate --continue` validates handoff contracts before `task finish` or sub-agent execution.
- `dw task open --workspace <path>` opens a new agent session for a workspace.
- `dw task open --continue` resumes an existing agent session on the latest workspace.
- `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"` creates ADO child tasks after the plan is written.
- `dw task commit --continue --execute` creates an intermediate commit without push or PR.
- `dw task finish --continue --execute --create-pr` is the expected commit/push/PR flow when finishing.
- `dw db schema`, `dw db describe` and `dw db query` are the SQL entrypoints and are read-only by default.

Current configured root:

```text
{root}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Use `dw` for every ADO, Git naming, PR and worktree operation.
3. Do not use Azure DevOps MCP tools.
4. Read the work item with `dw ado work-item` before coding, then run `dw ado ai-context` before acting on ADO context.
5. Before working, make sure the initial project setup required by the environment is in place: `pnpm install`, `pnpm approve-builds --all`, `npm install` fallback, or `dotnet restore` as appropriate.
6. Update `plan.md` before implementing.
7. Write all user-facing and project-facing text in French.
8. Do not normalize business labels or domain wording from ADO, screenshots, mockups or project text.
9. Treat screenshots, mockups and attachments as factual source material.
10. Branches, commits and PR titles are created by `dw`; do not create them manually.
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
        "# DevWorkflow Workspace\n\nThis workspace is managed by `dw`.\n\nContext:\n\n- Project: `{project}`\n- Work items:\n{items}\n\nRules:\n\n1. Run `dw task current` to identify the current task workspace.\n2. Read each work item with `dw ado work-item <id> --project {project}` before coding.\n3. Read `dw ado ai-context <id> --project {project}` before acting on ADO context.\n4. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.\n5. Before working, make sure the initial project setup required by the environment is in place.\n6. Fill `plan.md` before implementing.\n7. Run `dw task preflight --continue` before implementation, child-task creation, or other irreversible work.\n8. Run `dw task handoff-validate --continue` before launching sub-agents and before `dw task finish`.\n9. If the primary work item is a `User Story` or an `Anomalie`, once `plan.md` is complete and before implementation starts, create at least one ADO child task, then as many as needed from the plan, with `dw task create-child-task --continue --repo <front|back|db|foo> --title \"<action explicite>\"`.\n10. Write all user-facing and project-facing text in French: plans, comments, commit/PR text, task titles, progress summaries and final explanations.\n11. Structure the plan explicitly by domain when possible: front, back, db or other repos. Use sub-agents for independent tracks whenever possible.\n12. Use `dw task sync --continue` before lifecycle decisions if the local ADO context may be stale.\n13. Use `dw task commit` for intermediate commits.\n14. Use `dw task finish` for final push/PR workflows.\n15. Use `dw task teardown` or `dw task prune` for cleanup.\n"
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

        assert!(agents.content.contains("# DevWorkflow Workspace"));
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
                .contains("Use sub-agents for independent tracks whenever possible")
        );
    }
}
