package config

import (
	"embed"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	defaultProfileName   = "default"
	workspaceCodexConfig = `# Project-local Codex configuration.
# Primary instructions are loaded from AGENTS.md in this workspace.
`
	defaultProjectsJSON = `{
  "$schema": "../schemas/projects.schema.json",
  "schema": 1,
  "projects": {
    "default": {
      "displayName": "Default project",
      "workProvider": "azure-devops",
      "providers": {},
      "repositories": {
        "front": {
          "url": "",
          "defaultBranch": "main",
          "pullRequestTargetBranch": "main",
          "providerRepository": "",
          "anchorName": "front.git",
          "folder": "front"
        },
        "back": {
          "url": "",
          "defaultBranch": "master",
          "pullRequestTargetBranch": "master",
          "providerRepository": "",
          "anchorName": "back.git",
          "folder": "back"
        }
      }
    }
  }
}
`
	defaultWorkflowJSON = `{
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
  "providers": {
    "azure-devops": {
      "organization": "https://dev.azure.com/acme",
      "apiVersion": "7.1",
      "auth": {
        "tenantId": "organizations",
        "clientId": "04b07795-8ddb-461a-bbee-02f9e1bf7b46",
        "scopes": [
          "499b84ac-1321-427f-aa17-267ca6975798/.default"
        ]
      }
    }
  },
  "taskStart": {
    "updateWorkItemState": true,
    "createChildTasks": false,
    "userStoryState": "Active",
    "anomalyState": "Active",
    "bugState": "Active",
    "taskState": "Active"
  },
  "taskFinish": {
    "runVerification": true,
    "updateWorkItemState": true,
    "bugState": "Resolved",
    "taskState": "Resolved",
    "verificationCommands": {}
  },
  "updates": {
    "owner": "sachahjkl",
    "repository": "dw",
    "includePrerelease": false,
    "assetName": "release.json"
  }
}
`
	defaultDatabasesJSON = `{
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
`
	agentsMarkdown = `# DevWorkflow workspace

This root is managed by DevWorkflow.

1. Use the configured work provider as the source of truth for tracked work.
2. Use current DevWorkflow provider, work, data, agent, and secret actions for external work and workspace operations.
3. Follow the local ` + "`AGENTS.md`" + ` in a task workspace.
4. Preserve project terminology and repository conventions.
5. Ask for clarification when requirements or supporting material are ambiguous.

Primary actions: provider show, work item show, work context ai, workspace current, workspace sync, workspace preflight, work item child create, workspace handoff validate, workspace commit, and workspace finish.
`
	opencodeJSONC = `{
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
`
)

//go:embed resources/*.schema.json
var schemaResources embed.FS

type initProfile struct {
	name          string
	projectsJSON  string
	workflowJSON  string
	databasesJSON string
	agentsMD      string
	opencodeJSONC string
}

func resolveProfile(name string) (initProfile, error) {
	normalized := strings.ToLower(strings.TrimSpace(name))
	if normalized == "" {
		normalized = defaultProfileName
	}
	if normalized != defaultProfileName {
		return initProfile{}, localizedError(l10n.M("config.unknown_profile", l10n.A("profile", name)))
	}
	return initProfile{
		name: defaultProfileName, projectsJSON: defaultProjectsJSON,
		workflowJSON: defaultWorkflowJSON, databasesJSON: defaultDatabasesJSON,
		agentsMD: agentsMarkdown, opencodeJSONC: opencodeJSONC,
	}, nil
}

func detectProfile(string) initProfile {
	profile, _ := resolveProfile(defaultProfileName)
	return profile
}

func repositorySSHURLForHTTP(httpURL string) (string, bool) {
	const githubPrefix = "https://github.com/"
	if strings.HasPrefix(httpURL, githubPrefix) {
		return "git@github.com:" + strings.TrimPrefix(httpURL, githubPrefix), true
	}
	return "", false
}

type messageError struct{ message l10n.Message }

func localizedError(message l10n.Message) error { return messageError{message: message} }
func (problem messageError) Error() string      { return l10n.Render(problem.message) }
