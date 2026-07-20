# Azure DevOps

Azure DevOps is the workflow source of truth.

The implementation uses Azure DevOps REST APIs with Microsoft Entra browser or device-code OAuth. The CLI does not require Azure CLI.

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

These calls stay behind the statically registered provider in `internal/work/ado`. Command orchestration requests its typed work capabilities, so authentication, retries, and payload conventions do not leak into commands and a future GitHub or Jira provider can implement only the capabilities it supports.

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
