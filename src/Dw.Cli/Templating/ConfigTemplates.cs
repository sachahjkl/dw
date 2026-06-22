namespace Dw.Cli.Templating;

internal static partial class Templates
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
      "agent": {
        "default": "opencode"
      },
      "repositories": {
        "front": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-front",
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "gesco-front",
          "anchorName": "hommage-agence-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_git/gesco-back",
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
          "defaultBranch": "develop",
          "pullRequestTargetBranch": "develop",
          "azureDevOpsRepository": "FRONT HOMMAGE EXPLOITATION",
          "anchorName": "hommage-exploitation-front.git",
          "folder": "front"
        },
        "back": {
          "url": "https://digital-factory-ogf@dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_git/HOMMAGE%20EXPLOITATION",
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
""";

    public static string DefaultWorkflowJson => $$"""
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
  "agent": {
    "default": "opencode"
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
    "tenantId": "{{AzureDevOpsDefaults.TenantId}}",
    "clientId": "{{AzureDevOpsDefaults.PublicClientId}}",
    "scopes": [
      "{{AzureDevOpsDefaults.Scopes[0]}}"
    ]
  },
  "updates": {
    "owner": "",
    "repository": "",
    "includePrerelease": false,
    "assetName": "{{UpdateDefaults.ManifestAssetName}}"
  }
}
""";

    public static string OgfWorkflowJson => $$"""
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
  "agent": {
    "default": "opencode"
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
    "tenantId": "{{AzureDevOpsDefaults.TenantId}}",
    "clientId": "{{AzureDevOpsDefaults.PublicClientId}}",
    "scopes": [
      "{{AzureDevOpsDefaults.Scopes[0]}}"
    ]
  },
  "updates": {
    "owner": "{{UpdateDefaults.Owner}}",
    "repository": "{{UpdateDefaults.Repository}}",
    "includePrerelease": false,
    "assetName": "{{UpdateDefaults.ManifestAssetName}}"
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
  "globals": {},
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
""";

}
