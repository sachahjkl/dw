# Project Overview

`dw` is a productized internal developer workflow CLI.

It exists because AI coding tools are strong at reasoning and editing, but weak at remembering enterprise workflow constraints across Azure DevOps, multi-repository Git, worktree layout, PR rules, database access and agent context.

The CLI is the deterministic rail. The AI remains the reasoning layer.

## Primary Goals

1. Bootstrap a developer machine with a predictable DevWorkflow root.
2. Create task workspaces from Azure DevOps work items.
3. Group related front/back worktrees under one subject folder.
4. Expose concise context to OpenCode/Codex through `dw agent context`.
5. Provide safe SQL Server introspection for agents.
6. Produce commits, pushes and PRs through explicit finish flows.
7. Keep business conventions in skills and config, not scattered prompts.

## MVP Scope

The first implementation should prioritize:

1. `version`
2. `doctor`
3. `init`
4. `agent context`
5. task workspace creation
6. Git worktree operations
7. Azure DevOps auth and work item reads
8. PR creation
9. SQL read-only module
10. update through GitHub Releases

## Core Principle

If an operation is deterministic and repeatable, it belongs in `dw`.

If an operation requires judgment, tradeoff analysis or code understanding, it belongs to the AI agent, guided by skills and `dw agent context`.
