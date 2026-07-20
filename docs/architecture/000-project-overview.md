# Project Overview

`dw` is a developer workflow CLI.

It exists because AI coding tools are strong at reasoning and editing, but weak at remembering enterprise workflow constraints across external work systems, multi-repository Git, worktree layout, pull-request rules, guarded data access, and agent context.

The CLI is the deterministic rail. The AI remains the reasoning layer.

## Primary Goals

1. Bootstrap a developer machine with a predictable DevWorkflow root.
2. Create local task workspaces from provider-neutral external work items.
3. Group related repository worktrees under one subject folder.
4. Expose concise context to OpenCode/Codex through `dw agent context` and `dw work context ai`.
5. Provide safe introspection across configured data providers.
6. Produce commits, pushes, pull requests, and provider updates through explicit workspace finish flows.
7. Keep business conventions in skills and config, not scattered prompts.

## MVP Scope

The first implementation should prioritize:

1. `version`
2. `doctor`
3. `init`
4. `agent context`
5. local workspace creation
6. Git worktree operations
7. provider authentication and work-item reads
8. pull-request creation
9. guarded data-source inspection
10. update through GitHub Releases

## Core Principle

If an operation is deterministic and repeatable, it belongs in `dw`.

If an operation requires judgment, tradeoff analysis or code understanding, it belongs to the AI agent, guided by skills and `dw agent context`.
