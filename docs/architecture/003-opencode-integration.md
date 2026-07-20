# OpenCode Integration

OpenCode is detected and configured, not installed automatically.

`dw init` creates an OpenCode config folder under the DevWorkflow root:

```text
config/opencode/
  AGENTS.md
  opencode.jsonc
```

The repository also contains reference OpenCode assets in:

```text
docs/references/opencode/
```

## Agent Context

`dw agent context` is the stable bridge between the CLI and AI agents.

It should stay short, deterministic and easy to paste into any agent. It should tell the agent:

- where the root is
- which commands are available
- which operations must go through skills
- what not to do directly

## Responsibility Split

OpenCode/Codex:

- reads code
- plans
- edits
- reviews
- explains

`dw`:

- creates local workspaces
- manages deterministic Git operations
- reads/writes local config
- performs guarded data-source access
- runs provider-neutral external work operations
- checks environment health
