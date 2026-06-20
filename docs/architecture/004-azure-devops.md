# Azure DevOps

Azure DevOps is the workflow source of truth.

The target design uses REST APIs with MSAL/browser authentication. The CLI must not require Azure CLI.

Default REST API version: `7.1`, configurable in `workflow.json`.

The Azure DevOps resource identifier for Microsoft Entra tokens is:

```text
499b84ac-1321-427f-aa17-267ca6975798
```

The default delegated scope is:

```text
499b84ac-1321-427f-aa17-267ca6975798/.default
```

## Required Capabilities

- login/status/logout
- read work item
- assign parent and concrete child work items
- update work item states
- create child tasks when required by skills
- create PRs
- link PRs to work items when automatic linking is insufficient
- add traceability comments for AI-created work items

## REST Endpoints

Work item read:

```text
GET {organizationUrl}/{project}/_apis/wit/workitems/{id}?api-version=7.1
```

PR creation:

```text
POST {organizationUrl}/{project}/_apis/git/repositories/{repositoryIdOrName}/pullrequests?api-version=7.1
```

The first implementation keeps these calls behind `AzureDevOpsClient` so auth, retries and payload conventions can evolve without leaking into commands.

## Auth Configuration

`workflow.json` should provide:

```json
{
  "auth": {
    "tenantId": "organizations",
    "clientId": "<public-client-application-id>",
    "scopes": [
      "499b84ac-1321-427f-aa17-267ca6975798/.default"
    ]
  }
}
```

For automation or emergency fallback, `DW_ADO_TOKEN` can provide an already acquired bearer token.

## Source of Truth for Rules

Rules for states, task naming, Git naming and PRs live under:

```text
docs/references/agents/skills/ado-workitem/
```

The CLI may encode stable mechanics, but business policy should remain configurable or sourced from those reference files.
