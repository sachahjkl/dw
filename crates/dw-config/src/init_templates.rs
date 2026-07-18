const DEFAULT_TENANT_ID: &str = "organizations";
const DEFAULT_PUBLIC_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
const DEFAULT_ADO_SCOPE: &str = "499b84ac-1321-427f-aa17-267ca6975798/.default";
const UPDATE_OWNER: &str = "sachahjkl";
const UPDATE_REPOSITORY: &str = "dw";
const UPDATE_ASSET_NAME: &str = "release.json";

#[derive(Debug, Clone)]
pub(crate) struct InitProfile {
    pub(crate) name: &'static str,
    pub(crate) projects_json: String,
    pub(crate) workflow_json: String,
    pub(crate) databases_json: String,
    pub(crate) agents_md: &'static str,
    pub(crate) opencode_jsonc: &'static str,
}

pub(crate) const WORKSPACE_CODEX_CONFIG: &str = r#"# Project-local Codex configuration.
# Primary instructions are loaded from AGENTS.md in this workspace.
"#;

pub(crate) fn resolve_profile(name: &str) -> std::io::Result<InitProfile> {
    match normalized_profile_name(name).as_str() {
        "default" => Ok(InitProfile {
            name: "default",
            projects_json: DEFAULT_PROJECTS_JSON.into(),
            workflow_json: default_workflow_json(),
            databases_json: DEFAULT_DATABASES_JSON.into(),
            agents_md: AGENTS_MD,
            opencode_jsonc: OPENCODE_JSONC,
        }),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unknown init profile: {name}. Available profiles: default."),
        )),
    }
}

pub(crate) fn repository_ssh_url_for_http(http_url: &str) -> Option<String> {
    if let Some(path) = http_url.strip_prefix("https://github.com/") {
        return Some(format!("git@github.com:{path}"));
    }
    None
}

pub(crate) fn detect_profile(_root: &str) -> InitProfile {
    resolve_profile("default").expect("default profile is valid")
}

fn normalized_profile_name(name: &str) -> String {
    if name.trim().is_empty() {
        "default".into()
    } else {
        name.trim().to_ascii_lowercase()
    }
}

fn default_workflow_json() -> String {
    format!(
        r#"{{
  "$schema": "../schemas/workflow.schema.json",
  "schema": 1,
  "branchPrefixes": {{
    "userStory": "feat",
    "anomaly": "fix",
    "bug": "bug",
    "activity": "chore"
  }},
  "worktreeFolders": {{
    "front": "front",
    "back": "back"
  }},
  "agent": {{
    "default": "opencode"
  }},
  "azureDevOps": {{
    "organizationUrl": "https://dev.azure.com/acme",
    "apiVersion": "7.1"
  }},
  "taskStart": {{
    "updateWorkItemState": true,
    "createChildTasks": false,
    "userStoryState": "Active",
    "anomalyState": "Active",
    "bugState": "Active",
    "taskState": "Active"
  }},
  "taskFinish": {{
    "runVerification": true,
    "updateWorkItemState": true,
    "bugState": "Resolved",
    "taskState": "Resolved",
    "verificationCommands": {{}}
  }},
  "auth": {{
    "tenantId": "{DEFAULT_TENANT_ID}",
    "clientId": "{DEFAULT_PUBLIC_CLIENT_ID}",
    "scopes": [
      "{DEFAULT_ADO_SCOPE}"
    ]
  }},
  "updates": {{
    "owner": "{UPDATE_OWNER}",
    "repository": "{UPDATE_REPOSITORY}",
    "includePrerelease": false,
    "assetName": "{UPDATE_ASSET_NAME}"
  }}
}}
"#
    )
}

const DEFAULT_PROJECTS_JSON: &str = r#"{
  "$schema": "../schemas/projects.schema.json",
  "schema": 1,
  "projects": {
    "default": {
      "displayName": "Default project",
      "repositories": {
        "front": {
          "url": "",
          "defaultBranch": "main",
          "pullRequestTargetBranch": "main",
          "azureDevOpsRepository": "",
          "anchorName": "front.git",
          "folder": "front"
        },
        "back": {
          "url": "",
          "defaultBranch": "master",
          "pullRequestTargetBranch": "master",
          "azureDevOpsRepository": "",
          "anchorName": "back.git",
          "folder": "back"
        }
      }
    }
  }
}
"#;

const DEFAULT_DATABASES_JSON: &str = r#"{
  "$schema": "../schemas/databases.schema.json",
  "schema": 1,
  "defaults": {
    "readonly": true,
    "maxRows": 500,
    "timeoutSeconds": 600
  },
  "globals": {},
  "projects": {}
}
"#;

const AGENTS_MD: &str = r#"# DevWorkflow workspace

This root is managed by DevWorkflow.

1. Use Azure DevOps work items as the source of truth for tracked work.
2. Use current DevWorkflow auth, ADO, work, DB, agent, and secret actions for Azure DevOps and worktree operations.
3. Follow the local `AGENTS.md` in a task workspace.
4. Preserve project terminology and repository conventions.
5. Ask for clarification when requirements or supporting material are ambiguous.

Primary actions: ADO item show, ADO context ai, work current, work sync, work preflight, work task child create, work handoff validate, work commit, and work finish.
"#;

const OPENCODE_JSONC: &str = r#"{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "lsp": true,
  "permission": {
    "bash": "allow",
    "edit": "allow"
  }
}
"#;
