# Azure DevOps

Azure DevOps is the current implementation of the work-provider contracts. Projects select it by the provider name `azure-devops`; work and workspace orchestration remain provider-neutral.

The provider uses Azure DevOps REST APIs with Microsoft Entra browser or device-code OAuth. The CLI does not require Azure CLI.

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
- extract work-item references from provider-specific commit conventions

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

## CLI Selection

```text
dw provider show azure-devops
dw provider capabilities azure-devops
dw provider auth login azure-devops
dw work item show <work-item> --provider azure-devops
```

The product name is a provider value, never a command namespace. Project `workProvider` configuration supplies the default when `--provider` is absent.

## Provider Configuration

`workflow.json` stores provider-specific extension data beneath the generic provider registry. A project selects the entry through `workProvider`; repository mappings use `providerRepository` when the remote provider name differs from the local key.

```json
{
  "providers": {
    "azure-devops": {
      "organization": "https://dev.azure.com/example",
      "project": "Example",
      "apiVersion": "7.1",
      "auth": {
        "tenantId": "organizations",
        "clientId": "<public-client-application-id>",
        "scopes": [
          "499b84ac-1321-427f-aa17-267ca6975798/.default"
        ]
      }
    }
  }
}
```

`projects.json` keeps provider selection and repository mapping in generic fields:

```json
{
  "projects": {
    "example": {
      "displayName": "Example",
      "workProvider": "azure-devops",
      "providers": {
        "azure-devops": {
          "project": "Example"
        }
      },
      "repositories": {
        "api": {
          "providerRepository": "Api"
        }
      }
    }
  }
}
```

For automation or emergency fallback, `DW_ADO_TOKEN` can provide an already acquired bearer token to this provider.

## Source of Truth for Rules

Rules for states, task naming, Git naming and PRs live under:

```text
docs/references/agents/skills/ado-workitem/
```

The CLI may encode stable mechanics, but business policy should remain configurable or sourced from those reference files.
