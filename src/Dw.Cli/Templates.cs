using System.Text.Json;

namespace Dw.Cli;

internal static class Templates
{
    public const string DefaultProjectsJson = """
{
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
""";

    public const string OgfProjectsJson = """
{
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
      "repositories": {
        "front": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-front",
          "defaultBranch": "main",
          "pullRequestTargetBranch": "main",
          "azureDevOpsRepository": "gesco-front",
          "anchorName": "hommage-agence-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-back",
          "defaultBranch": "master",
          "pullRequestTargetBranch": "master",
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
      "repositories": {
        "front": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_git/FRONT%20HOMMAGE%20EXPLOITATION",
          "defaultBranch": "main",
          "pullRequestTargetBranch": "main",
          "azureDevOpsRepository": "FRONT HOMMAGE EXPLOITATION",
          "anchorName": "hommage-exploitation-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_git/HOMMAGE%20EXPLOITATION",
          "defaultBranch": "master",
          "pullRequestTargetBranch": "master",
          "azureDevOpsRepository": "HOMMAGE EXPLOITATION",
          "anchorName": "hommage-exploitation-back.git",
          "folder": "back"
        }
      }
    }
  }
}
""";

    public const string DefaultWorkflowJson = """
{
  "$schema": "../schemas/workflow.schema.json",
  "schema": 1,
  "branchPrefixes": {
    "userStory": "feat",
    "anomaly": "fix",
    "bug": "bug",
    "activity": "chore"
  },
  "worktreeFolders": {
    "front": "front",
    "back": "back"
  },
  "azureDevOps": {
    "organizationUrl": "",
    "apiVersion": "7.1"
  },
  "taskStart": {
    "updateWorkItemState": true,
    "createChildTasks": false,
    "userStoryState": "En réalisation",
    "anomalyState": "En réalisation",
    "bugState": "En développement",
    "taskState": "En développement"
  },
  "taskFinish": {
    "runVerification": true,
    "updateWorkItemState": true,
    "bugState": "PR en attente",
    "taskState": "PR en attente",
    "verificationCommands": {}
  },
  "auth": {
    "tenantId": "organizations",
    "clientId": "04b07795-8ddb-461a-bbee-02f9e1bf7b46",
    "scopes": [
      "499b84ac-1321-427f-aa17-267ca6975798/.default"
    ]
  },
  "updates": {
    "owner": "",
    "repository": "",
    "includePrerelease": false,
    "assetName": "release.json"
  }
}
""";

    public const string OgfWorkflowJson = """
{
  "$schema": "../schemas/workflow.schema.json",
  "schema": 1,
  "branchPrefixes": {
    "userStory": "feat",
    "anomaly": "fix",
    "bug": "bug",
    "activity": "chore"
  },
  "worktreeFolders": {
    "front": "front",
    "back": "back"
  },
  "azureDevOps": {
    "organizationUrl": "https://dev.azure.com/digital-factory-ogf",
    "apiVersion": "7.1"
  },
  "taskStart": {
    "updateWorkItemState": true,
    "createChildTasks": false,
    "userStoryState": "En réalisation",
    "anomalyState": "En réalisation",
    "bugState": "En développement",
    "taskState": "En développement"
  },
  "taskFinish": {
    "runVerification": true,
    "updateWorkItemState": true,
    "bugState": "PR en attente",
    "taskState": "PR en attente",
    "verificationCommands": {
      "front": [
        "npm test"
      ],
      "back": [
        "dotnet test"
      ]
    }
  },
  "auth": {
    "tenantId": "organizations",
    "clientId": "04b07795-8ddb-461a-bbee-02f9e1bf7b46",
    "scopes": [
      "499b84ac-1321-427f-aa17-267ca6975798/.default"
    ]
  },
  "updates": {
    "owner": "sachahjkl",
    "repository": "dw",
    "includePrerelease": false,
    "assetName": "release.json"
  }
}
""";

    public const string DefaultDatabasesJson = """
{
  "$schema": "../schemas/databases.schema.json",
  "schema": 1,
  "defaults": {
    "readonly": true,
    "maxRows": 500,
    "timeoutSeconds": 600
  },
  "projects": {}
}
""";

    public const string OgfDatabasesJson = """
{
  "$schema": "../schemas/databases.schema.json",
  "schema": 1,
  "defaults": {
    "readonly": true,
    "maxRows": 500,
    "timeoutSeconds": 600
  },
  "projects": {
    "ha": {
      "databases": {}
    },
    "he": {
      "databases": {}
    }
  }
}
""";

    public const string AgentsMd = """
# DevWorkflow Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Use `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth for task state.
3. Use one subject workspace per work item.
4. Keep front and back as separate Git repositories.
5. For API contract changes, always check both front and back.
6. Do not commit, push or open PRs unless the user explicitly asks for the finish step.
""";

    public const string OpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "permission": {
    "bash": "ask",
    "edit": "ask"
  }
}
""";

    public const string OgfAgentsMd = """
# DevWorkflow OGF Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Run `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth.
3. Use the skills in the repository references for ADO, Git naming, PRs and HA/HE conventions.
4. Keep front and back as separate Git repositories.
5. Group worktrees for the same subject under one subject workspace.
6. For API contract changes, always check both front and back.
7. Write ADO/PR/commit text in French unless a repository convention says otherwise.
8. Do not bypass `dw task finish` for commit/push/PR workflows.
""";

    public const string OgfOpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "permission": {
    "bash": "ask",
    "edit": "ask"
  },
  "mcp": {
    "ado": {
      "type": "local",
      "command": [
        "npx",
        "-y",
        "@azure-devops/mcp@next",
        "digital-factory-ogf"
      ],
      "environment": {
        "LOG_LEVEL": "debug"
      }
    }
  }
}
""";

    public static string AgentContext(string root) => $$"""
# DevWorkflow agent context

You are working inside a DevWorkflow-managed environment.

Use `dw` for workflow operations:

- `dw doctor` checks local prerequisites.
- `dw task status` lists detected task workspaces.
- `dw task start <workItemId> --project <name> --slug <slug>` creates a task workspace.
- `dw db ...` is the only intended SQL entrypoint and is read-only by default.

Current configured root:

```text
{{root}}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Git repositories remain separate per front/back repo.
3. A subject workspace groups related worktrees under one work item.
4. Plans live as `plan.md` in the subject workspace.
5. Branches, commits and PR titles must follow the loaded skills.
6. Never bypass skills when ADO, Git naming, PRs or worktrees are involved.
""";

    public static string PlanMd(string workItemId, string project) => $$"""
# Plan - Work item {{workItemId}}

Project: `{{project}}`

## Functional Summary

TODO

## Affected Repositories

- front: TODO
- back: TODO

## Code Analysis

TODO

## Technical Plan

TODO

## Risks

TODO

## Verification

TODO
""";
}
