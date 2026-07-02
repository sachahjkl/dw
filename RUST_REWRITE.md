# Rust Rewrite Plan For `dw`

## Goal

Rewrite `dw` fully as a native Rust CLI/TUI to remove the .NET runtime dependency and make Rust the long-term implementation platform.

This is a full migration target, not a partial sidecar.

The rewrite must preserve the workflow semantics and deterministic contracts of the current tool, while intentionally improving the terminal UX in the areas where Rust is better:

- richer terminal rendering
- colors and layout
- interactive selection/filtering flows
- better task/workspace list navigation
- better handoff/preflight presentation
- future TUI-oriented workflows

This is not a content redesign. The meaning and workflow behavior should stay aligned with the current tool.
What may change deliberately is the presentation layer:

- layout
- colors
- interactive navigation
- selection widgets
- filtering UX
- richer terminal rendering

The first objective is behavior parity on the existing workflow:

- Azure DevOps work item reads and lifecycle helpers
- task workspace creation and discovery
- deterministic structured outputs
- preflight and handoff contracts
- Git/worktree orchestration
- read-only SQL Server introspection/query commands
- agent/workspace templating
- existing installation and upgrade experience

## Recommendation

Target a full replacement of the .NET implementation, but execute it in organized phases with parity gates.

Do not keep a permanent split architecture.

During migration, use an incremental parity-first rewrite in Rust, using the structured contracts already added to `dw` as stable migration seams:

- `dw ado ai-context`
- `dw task preflight`
- `handoff-*.md`
- `dw task handoff-validate`

These are the best boundaries because they are deterministic and testable.

## Agent Operating Mode

This plan is intended to be executed by an autonomous coding agent over many iterations.

The agent should behave as follows:

1. Work phase by phase; do not start a later phase before the current one has passed its gate.
2. Prefer shipping narrow, validated increments over broad speculative rewrites.
3. Keep the Rust rewrite branch always buildable.
4. Maintain explicit parity notes when a command is incomplete.
5. Never silently change workflow semantics.
6. Treat structured outputs and config compatibility as hard contracts.
7. Treat terminal UX as improvable, but never at the expense of command meaning.

## Hard Safety Rules

The agent must not:

1. Introduce a second config system.
2. Rename or normalize business labels in outputs.
3. Change `workflow.json` / `projects.json` semantics casually.
4. Break the structured contracts already established in the current tool.
5. Merge feature redesign with the rewrite unless explicitly decided.
6. Delete the .NET implementation until Rust parity gates are met.
7. Replace deterministic command output with purely interactive-only flows.
8. Make the TUI mandatory for automation paths.

## Required Execution Pattern Per Phase

For every phase, the agent must produce:

1. scope of commands/modules being migrated
2. current parity status
3. implementation changes
4. validation evidence
5. known gaps
6. explicit go/no-go for the next phase

If any item is missing, the phase is not complete.

## Stop Conditions

The agent must stop and report immediately if any of the following happens:

1. ADO auth is not reliable enough on target Windows environments.
2. Azure DevOps API coverage is insufficient for required commands.
3. SQL Server readonly behavior cannot meet the required scope with acceptable reliability.
4. The rewrite starts drifting from contract parity without an explicit decision.
5. Interactive UX changes make automation or scripting materially worse.
6. Cross-platform/native distribution gains do not justify the rewrite cost anymore.

## Non-Negotiable Compatibility Targets

These must remain compatible unless there is an explicit migration decision:

1. `workflow.json`
2. `projects.json`
3. workspace layout
4. `task.json`
5. `plan.md`
6. `handoff-*.md` summary block keys
7. `dw.ado.ai-context.v1`
8. `dw.task.preflight.v1`
9. `dw.task.handoff-validation.v1`
10. overall command naming and intent

## Rewrite Principles

1. Preserve current user-facing behavior unless there is an explicit decision to change it.
2. Preserve all project/user-facing text in French.
3. Preserve business labels exactly; never normalize them in outputs.
4. Keep `workflow.json` / `projects.json` as the only config system.
5. Reuse existing contract shapes before inventing new ones.
6. Reach parity command-by-command, not module-by-module in the abstract.
7. Validate on Windows first, because current `dw` usage is strongly Windows-oriented.
8. Treat terminal UX as a first-class rewrite opportunity.
9. Preserve content and intent, but feel free to redesign output layout and interaction model where Rust can do better.
10. Preserve the current delivery model as much as possible: Nix, release artifacts, and `dw upgrade` must remain part of the target story.

## Explicit UX Opportunity

The rewrite should deliberately benefit from Rust terminal/TUI strengths.

This means the Rust version is allowed, and encouraged, to improve:

- colors
- emphasis
- column layout
- visual grouping
- filtering interfaces
- item selection interfaces
- interactive confirmations
- richer contextual display in task lists / workspace lists / preflight reports / handoff validation

Likely ecosystems to evaluate:

- `crossterm`
- `ratatui`
- `inquire`
- `skim`-style filtering approaches if needed

Important boundary:

- preserve command meaning
- preserve deterministic structured outputs
- preserve workflow semantics
- preserve French project-facing content
- do not preserve ugly/plain layout just for parity if a better Rust UX is available

## Why Rust Is Plausible

Rust is now credible for this tool because the main external concerns appear to have candidate libraries:

- MSAL: `msal`
- Azure DevOps REST: `azure-devops-rust-api`
- SQL Server: `tiberius`

And Rust is a good fit for:

- native single-binary distribution
- low startup cost
- filesystem/process heavy CLIs
- deterministic parsers/contracts
- future TUI potential

## Main Risks

### 1. Azure DevOps Auth Reality

Library existence is not enough.

Validate real support for:

- silent token resolution
- interactive login
- PAT fallback if needed
- Windows behavior
- token refresh behavior

This is the highest-risk external integration.

### 2. Azure DevOps API Coverage

Validate real coverage for the endpoints already used by `dw`, especially:

- work item expanded reads
- comments
- work item batch reads
- pull request creation
- PR/work item linking
- relation traversal

If the crate is incomplete, direct REST calls may still be needed.

### 3. SQL Server Parity

`tiberius` makes Rust viable, but validate parity for the actual `dw db` scope:

- schema listing
- describe table
- readonly queries
- connection options
- row limiting
- timeouts

### 4. Windows UX Parity

Validate:

- process launching
- git command behavior
- path handling
- credential storage strategy
- shell completion generation

### 5. Rewrite Cost

The expensive part is not command syntax.

The expensive part is preserving:

- exact workflow semantics
- deterministic outputs
- error behavior
- help/completion UX
- integration behavior on real machines

### 6. Delivery / Upgrade Continuity

The rewrite is not only about code parity. It must preserve the current delivery story:

- Nix flake support
- release artifact publishing
- `release.json` continuity
- `dw upgrade` continuity from existing installed .NET versions

Important migration expectation:

- an existing .NET `dw` installation should ideally be able to upgrade into the new Rust `dw` through the normal upgrade path

This requires explicit compatibility planning for:

- artifact naming
- release manifest shape
- binary replacement/install layout
- upgrade command behavior across implementation boundaries

Important note:

- the current .NET implementation already has a self-upgrade path
- this is a useful reference implementation
- but the Rust rewrite must still validate its own replacement/install behavior explicitly

### 7. Interactive vs Non-Interactive Behavior

Because the Rust rewrite should improve TUI/terminal UX, it must also explicitly preserve scripting behavior.

Validate:

- TTY detection
- non-interactive stdout behavior
- CI-friendly output behavior
- structured output behavior when rich UI exists

Rule:

- rich UI in interactive terminals
- deterministic/scriptable behavior outside interactive terminals

## Good Migration Boundaries

### Lowest Risk

- config loading
- JSON serialization
- workspace manifest parsing
- handoff parsing/validation
- template generation
- workspace discovery
- filesystem layout logic
- non-interactive rendering primitives

### Medium Risk

- git/worktree orchestration
- CLI help/completion/validation
- preflight logic
- PR text generation
- interactive terminal flows and TUI states

### Highest Risk

- Azure DevOps auth
- Azure DevOps write flows
- SQL Server integration
- Windows secret storage strategy

## Suggested Rewrite Order

### Phase 0 - Feasibility Spike

Goal: de-risk the externals before committing to the rewrite.

Deliverables:

1. Rust spike for Azure DevOps auth
2. Rust spike for Azure DevOps expanded work item read
3. Rust spike for SQL Server readonly query with `tiberius`
4. Tiny CLI proving Windows packaging/distribution expectations
5. Delivery compatibility note for `dw upgrade` and release artifacts
6. Non-interactive vs interactive terminal behavior note

Exit criteria:

- can authenticate to ADO in the target environment
- can fetch the same work item/context data needed by `dw`
- can execute readonly SQL queries against SQL Server
- there is a credible path to keep `dw upgrade` working across the .NET -> Rust transition
- there is a credible plan for rich TUI behavior without breaking automation

If Phase 0 fails, stop the rewrite.

Phase 0 safety checks:

1. Run the auth spike on real Windows environments, not only mocked tests.
2. Verify at least one real ADO project and one real SQL Server connection.
3. Capture explicit failure notes, not only success notes.
4. Do not proceed to Phase 1 on optimism alone.

### Phase 1 - Contract-First Rust Core

Goal: rebuild deterministic local/core behavior without the risky integrations first.

Implement in Rust:

- config models for `workflow.json`, `projects.json`, `databases.json`
- workspace manifest model
- handoff contract parser/validator
- preflight result model
- ai-context result model
- template rendering
- workspace discovery/path logic
- shared output/rendering layer for text + rich terminal UI

Commands to target first:

- `dw version`
- `dw config show`
- `dw task status`
- `dw task list`
- `dw task current`
- `dw task handoff-validate`

Exit criteria:

- local deterministic commands pass golden-output tests

Phase 1 safety checks:

1. The Rust models must match existing config and manifest shapes.
2. The agent must add tests for every contract parser introduced.
3. No interactive UX dependency should be required for deterministic commands.

### Phase 1.5 - Shared Terminal UX Layer

Goal: define the rendering foundation before command UX diverges ad hoc.

Implement:

- color/theme abstraction
- structured table/list rendering
- interactive chooser patterns
- filterable selection pattern
- consistent success/warning/blocking/error presentation
- fallback non-interactive mode behavior

Commands that should benefit early:

- workspace selection
- task list/status/current
- preflight display
- handoff validation display

Exit criteria:

- one consistent rendering layer used by multiple commands
- interactive UX is clearly better than the current plain text approach

Phase 1.5 safety checks:

1. Every rich interactive flow must have a non-interactive fallback.
2. Structured outputs must remain untouched by UI improvements.
3. Colors and layout must degrade safely on plain terminals.
4. TTY and non-TTY behavior must be tested explicitly.

### Phase 2 - Git/Workspace Flows

Goal: migrate the local task workspace engine.

Implement:

- `task start`
- `task open`
- `task add-work-item`
- `task remove-work-item`
- `task rename`
- `task teardown`
- `task repo-latest`
- interactive workspace selection/filtering where appropriate

Notes:

- Prefer shelling out to `git`, same as current behavior philosophy.
- Preserve branch naming and workspace naming exactly.

Exit criteria:

- workspace structure is identical enough for existing agents and humans

Phase 2 safety checks:

1. Branch naming must match current logic exactly unless explicitly changed.
2. Workspace naming/layout must remain compatible with current expectations.
3. The agent must test Windows path handling explicitly.
4. Interactive selectors must not block scripted usage paths.

### Phase 3 - Azure DevOps Read Flows

Goal: migrate deterministic ADO reads before ADO writes.

Implement:

- `ado work-item`
- `ado ai-context`
- `ado context` if still kept
- `ado assigned`
- `ado changelog`
- relation traversal used by preflight/start
- richer interactive browsing for assigned items and grouped work items if useful

Exit criteria:

- Rust output matches current command contracts on representative samples

Phase 3 safety checks:

1. Compare Rust and .NET outputs side by side for representative real items.
2. Preserve all structured output field names.
3. Preserve attachment/relation/predecessor semantics exactly.

### Phase 4 - Preflight + Child Task + Handoff Workflow

Goal: migrate the current structured workflow engine fully.

Implement:

- `task preflight`
- `task create-child-task`
- handoff file generation
- handoff validation
- rich terminal presentation for blockers/warnings and handoff status

Exit criteria:

- Rust can drive the same workspace planning workflow end-to-end before implementation

Phase 4 safety checks:

1. Handoff contract parsing must reject malformed summaries deterministically.
2. `task preflight` severity semantics must remain stable.
3. `task handoff-validate` must remain scriptable and machine-readable.

### Phase 5 - Finish / PR / ADO Write Flows

Goal: migrate the riskiest workflow finish behavior last.

Implement:

- `task commit`
- `task finish`
- PR creation
- PR/work item linking
- work item state updates
- richer finish summaries in terminal

Exit criteria:

- successful real PR creation in Azure DevOps from Rust CLI

Phase 5 safety checks:

1. Validate on a safe real project before broad rollout.
2. Confirm PR text contains the expected plan/handoff/verification structure.
3. Confirm work item state transitions match current workflow expectations.
4. Never use production-destructive shortcuts to simulate success.

### Phase 6 - DB + Auth + Secrets Hardening

Goal: close the remaining platform-specific gaps.

Implement/decide:

- final auth model
- SQL command parity
- secret storage approach

Potential decision point:

- if Windows secret storage is awkward, decide whether to:
  - keep native secure storage
  - use environment/secret file strategy
  - provide pluggable secret backends

Phase 6 safety checks:

1. Secret handling must be reviewed separately from core CLI logic.
2. SQL commands must remain readonly by default.
3. Security regressions are blockers, not polish items.

### Phase 7 - Delivery, Nix, Upgrade Migration

Goal: fully replace the .NET shipping path without breaking the user upgrade story.

Implement:

- update the Nix flake to build/package the Rust implementation
- preserve release artifact publication in CI
- preserve or intentionally evolve `release.json`
- preserve install layout expectations for release-binary installs
- make the existing `.NET dw upgrade` path capable of installing the Rust binary when the first Rust release is published
- validate self-replacement/update behavior for the Rust binary itself
- preserve environment-variable based install/upgrade behavior where it already exists

Exit criteria:

- `nix build` / `nix run` work against the Rust implementation
- release artifacts are published successfully from CI
- an older installed .NET `dw` can upgrade to the Rust release through the normal upgrade flow

Phase 7 safety checks:

1. Do not break Nix users while optimizing for Windows binary delivery.
2. Do not change `release.json` casually; version the manifest shape if needed.
3. Test upgrade from a real previously-installed .NET build, not just a fresh Rust install.
4. Treat upgrade continuity as a release blocker.
5. Validate self-update behavior on Windows, including replacement timing/locking behavior.
6. Preserve the delivery contract already consumed by `dw upgrade` unless there is an explicit migration decision.

## Proposed Rust Architecture

Suggested crates/modules:

- `dw-cli`: command parsing and help
- `dw-config`: config models and loading
- `dw-workspace`: manifest, discovery, handoffs, preflight, templating
- `dw-ado`: ADO auth + REST client + mapping
- `dw-git`: git/worktree/process wrappers
- `dw-db`: SQL Server readonly layer
- `dw-contracts`: shared contract constants and structured output models
- `dw-ui`: terminal rendering, colors, selection, filtering, TUI helpers

If you prefer a single crate initially, still keep these as internal modules.

## Crates To Evaluate

Not prescriptions, just likely candidates:

- CLI: `clap`
- JSON: `serde`, `serde_json`
- HTTP: `reqwest`
- async runtime: `tokio`
- terminal: `crossterm`
- TUI: `ratatui`
- prompts/selection: `inquire`
- templating: plain string templates first, maybe `askama` later if needed
- SQL Server: `tiberius`
- auth: `msal`
- filesystem walk: `walkdir`
- error handling: `anyhow` / `thiserror`
- snapshots/golden tests: `insta`

## Compatibility Strategy

### Keep Command Surface Stable First

Prefer keeping the existing commands and flags:

- easier rollout
- easier comparison
- lower migration cost for agents/users

But do not freeze the terminal presentation.

Allowed to improve:

- colors
- spacing
- grouping
- alignment
- selection interactions
- filtering UX
- interactive lists

### Preserve Structured Contracts Exactly Where Possible

Especially preserve:

- `dw.ado.ai-context.v1`
- `dw.task.preflight.v1`
- `dw.task.handoff-validation.v1`
- handoff summary block keys

### Tolerate Internal Refactors, Not Contract Drift

The Rust version can be internally cleaner, but externally should remain predictable.

Predictable means:

- command purpose remains stable
- structured outputs remain stable
- workflow meaning remains stable

Predictable does not mean:

- identical plain-text formatting forever
- identical low-UX terminal rendering

## Test Strategy

### 1. Golden Output Tests

For deterministic commands, capture expected outputs and compare exactly.

Good candidates:

- `ado ai-context`
- `task preflight --json`
- `task handoff-validate --json`
- generated `handoff-*.md`

For human-readable terminal output, prefer semantic/snapshot tests over brittle whitespace parity where the Rust rewrite is intentionally improving layout.

### 2. Workspace Fixture Tests

Create fixture roots/workspaces and assert:

- generated files
- branch naming
- manifest contents
- handoff parsing behavior

### 3. Integration Tests Against Real Services

Need at least a small real-environment suite for:

- ADO auth
- work item reads
- PR creation
- SQL readonly queries

### 4. Side-by-Side Parity Harness

Strongly recommended.

For selected commands:

- run current .NET `dw`
- run Rust `dw`
- compare normalized outputs

This is the best way to de-risk the rewrite.

## Mandatory Gates Before Declaring Rust Ready

The Rust rewrite is not ready to replace .NET `dw` until all of the following are true:

1. critical commands have parity coverage
2. structured outputs are stable
3. workspace generation is compatible
4. preflight/handoff workflow is validated end-to-end
5. PR creation works on real ADO
6. readonly SQL commands work on real SQL Server
7. interactive UX has safe non-interactive fallbacks
8. Windows usage is validated on real machines
9. Nix packaging works
10. release-binary upgrade continuity from .NET to Rust is validated
11. non-interactive usage remains reliable
12. self-update behavior is validated on the Rust binary

If even one of these is false, the .NET version remains the source of truth.

## Agent Deliverables Per PR

Every rewrite PR should include:

1. short scope summary
2. exact commands/features migrated
3. tests added
4. parity status versus current `.NET dw`
5. known limitations
6. next recommended phase step
7. impact on delivery/upgrade path if any
8. impact on interactive vs non-interactive behavior if any

If the PR cannot state these clearly, it is too broad.

## Suggested Working Rhythm For The Agent

The agent should prefer this loop:

1. choose one narrow migration slice
2. implement
3. add tests
4. compare with current `.NET dw`
5. document remaining gaps
6. only then move forward

Avoid:

1. rewriting many commands at once
2. large speculative refactors before parity exists
3. mixing UI redesign with contract changes in the same step
4. making an interactive UX improvement without defining the non-interactive fallback

## Additional Migration Checklist

The agent should explicitly track these items during the rewrite:

1. preserve supported environment variables
2. preserve exit code semantics for automation-critical commands
3. preserve dynamic shell completion behavior
4. define the secret/token cache migration story
5. validate Unicode/French output behavior on Windows terminals
6. maintain a coherent human-output design system for rich terminal views
7. capture golden fixtures from real representative ADO/workspace cases
8. define the final release cutover scenario from .NET to Rust

## Specific Areas To Validate Explicitly

### Self-Upgrade

The agent should inspect the current .NET self-upgrade implementation and treat it as a behavioral reference.

But it must still validate for Rust:

- binary replacement while not clobbering a running executable
- temporary file/bootstrap strategy if needed
- rollback/failure behavior

### Environment Variables

Create and preserve an explicit inventory of environment-variable-driven behavior, including at least:

- `DW_ADO_TOKEN`
- install/upgrade-related variables already supported by scripts

### Exit Codes

Automation-relevant commands must preserve intentional exit code behavior, especially:

- `task preflight`
- `task handoff-validate`
- `task finish`
- CLI validation failures

### Shell Completion

Preserve the current quality bar for:

- PowerShell
- bash
- zsh
- fish
- dynamic project/workspace/work item suggestions

### Secret / Auth Cache Strategy

The agent must decide whether secrets/tokens are:

- migrated
- re-acquired cleanly
- or intentionally reset with an explicit user-facing migration note

### Unicode / French Output

The agent must validate:

- accents
- French labels
- exact business wording
- rendering quality on Windows terminal environments

### Human Output Redesign

Human-readable output may be redesigned, but it must remain coherent.

The agent should define a shared rendering system rather than redesigning each command independently.

### Real Golden Fixtures

Capture representative real cases for parity checks, including:

- work item with parent/children/predecessors
- work item with screenshots/attachments
- workspace with multiple repos
- handoffs in `todo` / `blocked` / `done`

### Final Cutover Strategy

The rewrite should define an explicit release transition such as:

1. release N: .NET source of truth
2. release N+1: .NET `dw upgrade` capable of installing Rust `dw`
3. release N+2: Rust becomes the primary implementation

This sequence must be written down and validated, not left implicit.

## Rollout Options

### Option A - New Binary During Migration

Examples:

- `dw-rs`
- `dwr`

Pros:

- safe parallel validation
- easy A/B testing

Cons:

- temporary duplication

### Option B - Replace `dw` Late

Keep .NET `dw` as source of truth until the Rust version is proven.

Recommendation: prefer this.

But the final target is still one Rust `dw`, not a permanent dual-track toolchain.

## What To Reuse From Current `dw`

Use the current repo as behavior spec for:

- command names/flags
- generated file shapes
- tests and fixtures
- workflow wording
- structured output shapes

Do not treat the .NET code as the architecture to mechanically port 1:1.
Treat it as the contract/reference implementation.

## Recommended First Rust Milestone

The first serious milestone should be:

1. `ado ai-context`
2. `task preflight`
3. handoff generation
4. `task handoff-validate`
5. richer terminal rendering for list/preflight/handoff views

Reason:

- these are deterministic
- they already have explicit contracts
- they unlock agent workflows early
- they provide the best parity harness for later migration
- they are also excellent places to prove the Rust UX upgrade story

## Go / No-Go Recommendation

### Go If

- ADO auth spike succeeds in the real environment
- SQL readonly spike with `tiberius` is good enough
- you are willing to fund a multi-phase rewrite
- you want a long-term native CLI platform

### Not Now If

- the primary need is just shipping workflow features quickly
- current .NET distribution pain is tolerable
- no bandwidth exists for a parity-heavy rewrite

## Concrete Next Step

Before any rewrite starts, assign a short feasibility spike with these outputs:

1. Rust POC: silent/interactive ADO auth on Windows
2. Rust POC: fetch one real expanded work item + comments + relations
3. Rust POC: readonly SQL query against SQL Server with `tiberius`
4. Written recommendation: continue / stop

If those 3 technical spikes succeed, the rewrite becomes a product decision, not a feasibility gamble.

## Immediate Handoff Brief For Another Agent

If another agent picks this up now, its first mission should be:

1. execute Phase 0 only
2. validate real Windows ADO auth
3. validate one real expanded work item read path
4. validate one readonly SQL Server query path with `tiberius`
5. assess whether the current release/upgrade model can bridge `.NET dw` to Rust `dw`
6. produce a written go/no-go recommendation

The next agent should not start broad rewriting before these checks pass.
