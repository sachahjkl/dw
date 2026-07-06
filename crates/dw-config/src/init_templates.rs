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

pub(crate) const WORKSPACE_CODEX_CONFIG: &str = r#"# Configuration Codex locale au projet.
# Les instructions d'exécution principales sont chargées depuis AGENTS.md dans ce workspace.
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
        "business" => Ok(InitProfile {
            name: "business",
            projects_json: BUSINESS_PROJECTS_JSON.into(),
            workflow_json: business_workflow_json(),
            databases_json: BUSINESS_DATABASES_JSON.into(),
            agents_md: BUSINESS_AGENTS_MD,
            opencode_jsonc: BUSINESS_OPENCODE_JSONC,
        }),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unknown init profile: {name}. Available profiles: business, default."),
        )),
    }
}

pub(crate) fn detect_profile(root: &str) -> InitProfile {
    let projects = std::fs::read_to_string(std::path::Path::new(root).join("config/projects.json"))
        .unwrap_or_default();
    if projects.contains("\"ha\"")
        || projects.contains("digital-factory-ogf")
        || projects.contains("HOMMAGE")
    {
        resolve_profile("business").expect("business profile is valid")
    } else {
        resolve_profile("default").expect("default profile is valid")
    }
}

fn normalized_profile_name(name: &str) -> String {
    if name.trim().is_empty() {
        "business".into()
    } else {
        name.trim().to_ascii_lowercase()
    }
}

fn default_workflow_json() -> String {
    workflow_json("", "", "", &[])
}

fn business_workflow_json() -> String {
    workflow_json(
        "https://dev.azure.com/digital-factory-ogf",
        UPDATE_OWNER,
        UPDATE_REPOSITORY,
        &[("front", "pnpm test"), ("back", "dotnet test")],
    )
}

fn workflow_json(
    organization_url: &str,
    update_owner: &str,
    update_repository: &str,
    verification_commands: &[(&str, &str)],
) -> String {
    let verification = if verification_commands.is_empty() {
        "{}".into()
    } else {
        let entries = verification_commands
            .iter()
            .map(|(repo, command)| format!("      \"{repo}\": [\n        \"{command}\"\n      ]"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{{\n{entries}\n    }}")
    };

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
    "organizationUrl": "{organization_url}",
    "apiVersion": "7.1"
  }},
  "taskStart": {{
    "updateWorkItemState": true,
    "createChildTasks": false,
    "userStoryState": "En réalisation",
    "anomalyState": "En réalisation",
    "bugState": "En développement",
    "taskState": "En développement"
  }},
  "taskFinish": {{
    "runVerification": true,
    "updateWorkItemState": true,
    "bugState": "PR en attente",
    "taskState": "PR en attente",
    "verificationCommands": {verification}
  }},
  "auth": {{
    "tenantId": "{DEFAULT_TENANT_ID}",
    "clientId": "{DEFAULT_PUBLIC_CLIENT_ID}",
    "scopes": [
      "{DEFAULT_ADO_SCOPE}"
    ]
  }},
  "updates": {{
    "owner": "{update_owner}",
    "repository": "{update_repository}",
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

const BUSINESS_PROJECTS_JSON: &str = r#"{
  "$schema": "../schemas/projects.schema.json",
  "schema": 1,
  "projects": {
    "ha": {
      "displayName": "Hommage Agence",
      "azureDevOps": {
        "organizationUrl": "https://dev.azure.com/digital-factory-ogf",
        "project": "HOMMAGE AGENCE",
        "apiVersion": "7.1"
      },
      "agent": {
        "default": "opencode"
      },
      "repositories": {
        "front": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-front",
          "gitCredentialSecret": "git/azure-devops",
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "gesco-front",
          "anchorName": "hommage-agence-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-back",
          "gitCredentialSecret": "git/azure-devops",
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "gesco-back",
          "anchorName": "hommage-agence-back.git",
          "folder": "back"
        }
      }
    },
    "he": {
      "displayName": "Hommage Exploitation",
      "azureDevOps": {
        "organizationUrl": "https://dev.azure.com/digital-factory-ogf",
        "project": "HOMMAGE EXPLOITATION",
        "apiVersion": "7.1"
      },
      "agent": {
        "default": "opencode"
      },
      "repositories": {
        "front": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_git/FRONT%20HOMMAGE%20EXPLOITATION",
          "gitCredentialSecret": "git/azure-devops",
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "FRONT HOMMAGE EXPLOITATION",
          "anchorName": "hommage-exploitation-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_git/HOMMAGE%20EXPLOITATION",
          "gitCredentialSecret": "git/azure-devops",
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "HOMMAGE EXPLOITATION",
          "anchorName": "hommage-exploitation-back.git",
          "folder": "back"
        }
      }
    },
    "cross-ha-he": {
      "displayName": "Cross HA HE",
      "repositories": {},
      "includedProjects": ["ha", "he"]
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

const BUSINESS_DATABASES_JSON: &str = r#"{
  "$schema": "../schemas/databases.schema.json",
  "schema": 1,
  "defaults": {
    "readonly": true,
    "maxRows": 500,
    "timeoutSeconds": 600
  },
  "globals": {},
  "projects": {
    "ha": {
      "databases": {}
    },
    "he": {
      "databases": {}
    },
    "cross-ha-he": {
      "databases": {}
    }
  }
}
"#;

const AGENTS_MD: &str = r#"# Règles globales DevWorkflow

Ce root est géré par DevWorkflow.

Règles globales:

1. Utiliser les work items Azure DevOps comme source de vérité.
2. Utiliser uniquement les actions DevWorkflow ADO, auth et task pour les opérations Azure DevOps/worktree; ne pas utiliser les outils MCP Azure DevOps.
3. Une fois dans un workspace task, suivre le `AGENTS.md` local comme contrat d'exécution principal.
4. Écrire tout texte utilisateur/projet en français, sauf convention contraire d'un repository.
5. Ne pas normaliser les labels métier ni le vocabulaire de domaine issus d'ADO, des screenshots, mockups, attachments ou textes projet. Préserver les termes exacts sauf demande explicite de renommage.
6. Traiter les screenshots, mockups et attachments comme sources factuelles. Si un point est ambigu, demander à l'utilisateur au lieu de deviner.
"#;

const BUSINESS_AGENTS_MD: &str = r#"# Règles globales DevWorkflow BUSINESS

Ce root est géré par DevWorkflow.

Règles globales:

1. Utiliser les work items Azure DevOps comme source de vérité.
2. Utiliser uniquement les actions DevWorkflow ADO, auth et task pour les opérations Azure DevOps/worktree; ne pas utiliser les outils MCP Azure DevOps.
3. Une fois dans un workspace task, suivre le `AGENTS.md` local comme contrat d'exécution principal.
4. Écrire tout texte utilisateur/projet en français, sauf convention contraire d'un repository.
5. Ne pas normaliser les labels métier ni le vocabulaire de domaine issus d'ADO, des screenshots, mockups, attachments ou textes projet. Préserver les termes exacts sauf demande explicite de renommage.
6. Traiter les screenshots, mockups et attachments comme sources factuelles. Si un point est ambigu, demander à l'utilisateur au lieu de deviner.
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

const BUSINESS_OPENCODE_JSONC: &str = OPENCODE_JSONC;
