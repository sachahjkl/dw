use std::fs;
use std::path::{Path, PathBuf};

const CLEAN_CORE_CRATES: &[&str] = &[
    "dw-ado",
    "dw-ado-commands",
    "dw-config",
    "dw-agent",
    "dw-doctor",
    "dw-secret",
    "dw-db",
    "dw-task",
    "dw-workspace",
    "dw-upgrade",
];

const CORE_FORBIDDEN: &[&str] = &[
    "clap",
    "inquire",
    "ratatui",
    "TerminalTheme",
    "AnsiRender",
    "println!",
    "eprintln!",
    "tokio::runtime",
    "Runtime::new",
    "block_on",
];

#[test]
fn migrated_core_crates_do_not_depend_on_cli_or_tui_rendering() {
    let repo = repo_root();
    for crate_name in CLEAN_CORE_CRATES {
        let crate_dir = repo.join("crates").join(crate_name);
        for file in rust_and_manifest_files(&crate_dir) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in CORE_FORBIDDEN {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden core token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn migrated_core_crates_do_not_embed_dw_cli_command_hints() {
    let repo = repo_root();
    for crate_name in CLEAN_CORE_CRATES {
        let crate_dir = repo.join("crates").join(crate_name);
        for file in rust_files(&crate_dir.join("src")) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in [
                "dw task ",
                "dw ado ",
                "dw db ",
                "dw auth ",
                "dw config ",
                "`dw`",
                "par `dw`",
                "Utilise `dw`",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden core CLI command hint `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn migrated_core_requests_do_not_carry_cli_json_output_flags() {
    let repo = repo_root();
    for crate_name in CLEAN_CORE_CRATES {
        let crate_dir = repo.join("crates").join(crate_name);
        for file in rust_files(&crate_dir.join("src")) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in ["json: bool", "pub json: bool", "json: _"] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden CLI JSON flag token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn migrated_core_requests_use_execution_mode_not_execute_flags() {
    let repo = repo_root();
    for crate_name in CLEAN_CORE_CRATES {
        let crate_dir = repo.join("crates").join(crate_name);
        for file in rust_files(&crate_dir.join("src")) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in ["execute: bool", "pub execute: bool"] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden CLI execute flag token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn async_domain_actions_isolate_blocking_ado_http_calls() {
    let repo = repo_root();
    let checked_roots = [
        repo.join("crates/dw-ado-commands/src"),
        repo.join("crates/dw-task/src"),
    ];
    let blocking_calls = [
        "get_work_item_snapshot_authenticated(",
        "get_work_item_snapshots_authenticated(",
        "get_related_work_item_ids(",
        "group_work_items_by_parent(",
        "create_child_task_authenticated(",
        "create_pull_request_authenticated(",
        "link_work_item_to_pull_request_authenticated(",
        "try_find_active_pull_request_authenticated(",
        "update_work_item_state_authenticated(",
        "list_active_pull_requests_authenticated(",
        "get_work_item_ids_from_pull_requests(",
        "load_changelog_items(",
    ];

    for root in checked_roots {
        for file in rust_files(&root) {
            if file.ends_with("crates/dw-task/src/start/ado.rs") {
                continue;
            }
            let text = fs::read_to_string(&file).expect("read source file");
            for call in blocking_calls {
                for (line_index, line) in text.lines().enumerate() {
                    if !line.contains(call) {
                        continue;
                    }
                    let window = surrounding_lines(&text, line_index, 6);
                    assert!(
                        window.contains("run_blocking_ado")
                            || window.contains("spawn_blocking")
                            || window.contains("fn ") && !window.contains("async fn "),
                        "{}:{} calls blocking ADO HTTP `{}` without isolation",
                        file.display(),
                        line_index + 1,
                        call
                    );
                }
            }
        }
    }
}

#[test]
fn tui_does_not_relaunch_current_dw_for_internal_actions() {
    let repo = repo_root();
    let tui_dir = repo.join("crates").join("dw-tui").join("src");
    let mut files = rust_files(&tui_dir);
    files.push(repo.join("crates/dw-tui/Cargo.toml"));
    for file in files {
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "dw-cli-adapter",
            "dw_cli_adapter",
            "CommandAction",
            "CommandKind",
            "CommandEffect",
            "CommandStart",
            "CommandRunResult",
            "CapturedCommandRunResult",
            "BackgroundKind::Command",
            "BackgroundResult::Command",
            "RunHistoryEntry { command:",
            "pub command: String",
            "entry.command",
            "display_command",
            "action.command",
            "command_accepts_root",
            "command_refreshes_snapshot_after_success",
            "command_successful_effect",
            "Action interne non portée",
            "current_exe",
            "run_current_dw",
            "LegacyShellAction",
            "CompletionShow",
            "QuickOptionAction::Completion",
            "Confirmation CLI",
            "std::thread",
            "Runtime::new",
            "block_on",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden TUI shell token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

#[test]
fn tui_runner_returns_typed_action_results_not_rendered_strings() {
    let repo = repo_root();
    let runner = repo.join("crates/dw-tui/src/runner.rs");
    let text = fs::read_to_string(&runner).expect("read TUI runner");
    for forbidden in [
        "dispatch_internal_action",
        "Result<String>",
        "pub output: String",
        "output: String",
        ".join(\"\\n\")",
        "render::",
        "TerminalTheme",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains forbidden string-rendered action token `{}`",
            runner.display(),
            forbidden
        );
    }
    assert!(
        text.contains("DwActionResult"),
        "{} should carry typed action results",
        runner.display()
    );
}

#[test]
fn tui_history_and_detail_store_typed_action_records_not_rendered_output() {
    let repo = repo_root();
    let checked_files = [
        repo.join("crates/dw-tui/src/history.rs"),
        repo.join("crates/dw-tui/src/background.rs"),
        repo.join("crates/dw-tui/src/model.rs"),
        repo.join("crates/dw-tui/src/app.rs"),
    ];

    for file in checked_files {
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "output_preview",
            "output_lines",
            "append_running_line",
            "OperationResult",
            "operation_result",
            "ActionOutput",
            "line: String",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden rendered history/detail token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

#[test]
fn action_stream_protocol_does_not_use_generic_progress_strings() {
    let repo = repo_root();
    let checked_roots = [
        repo.join("crates/dw-core/src"),
        repo.join("crates/dw-app/src"),
        repo.join("crates/dw-tui/src"),
        repo.join("crates/dw-tui-adapter/src"),
        repo.join("crates/dw-cli-adapter/src"),
    ];

    for root in checked_roots {
        for file in rust_files(&root) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in [
                "DwActionEvent::Progress",
                "core_event_to_dw",
                "Progress {",
                "phase: String",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden generic action event token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn core_does_not_expose_legacy_string_action_event_contracts() {
    let repo = repo_root();
    let core = repo.join("crates/dw-core/src/lib.rs");
    let text = fs::read_to_string(&core).expect("read core lib");
    for forbidden in [
        "pub enum ActionSeverity",
        "pub struct ActionEvent",
        "impl ActionEvent",
        "pub trait CoreAction",
        "FnMut(ActionEvent)",
        "message: String",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains forbidden legacy action event contract `{}`",
            core.display(),
            forbidden
        );
    }
}

#[test]
fn ado_usecase_streams_structured_domain_events_not_actionevent_text() {
    let repo = repo_root();
    let ado_commands = repo.join("crates/dw-ado-commands/src");
    for file in rust_files(&ado_commands) {
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "use dw_core::ActionEvent",
            "FnMut(ActionEvent)",
            "ActionEvent::info",
            "events: Vec<String>",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden ADO string event token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

#[test]
fn action_events_use_domain_id_types_not_primitive_id_strings() {
    let repo = repo_root();
    let core = repo.join("crates/dw-core/src/lib.rs");
    let text = fs::read_to_string(&core).expect("read core lib");
    for forbidden in [
        "pull_request_id: String",
        "work_item_ids: Vec<String>",
        "ids: Vec<String>",
        "project: String",
        "project: Option<String>",
        "state: String",
        "LoadingWorkItem {\n        id: String",
        "LoadingWorkItemContext {\n        id: String",
        "UpdatingWorkItemState {\n        ids: Vec<WorkItemId>,\n        state: String",
        "UpdatedWorkItemState {\n        id: WorkItemId,\n        state: String",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive ID field `{}` in action event contracts",
            core.display(),
            forbidden
        );
    }
    for required in [
        "WorkItemId",
        "WorkItemState",
        "PullRequestId",
        "AdoRepositoryName",
        "ProjectKey",
    ] {
        assert!(
            text.contains(required),
            "{} should expose domain ID type `{}`",
            core.display(),
            required
        );
    }
}

#[test]
fn migrated_contracts_use_domain_id_types_not_structured_strings() {
    let repo = repo_root();
    let contracts = repo.join("crates/dw-contracts/src/lib.rs");
    let text = fs::read_to_string(&contracts).expect("read contracts lib");
    for forbidden in [
        "pub struct TaskHandoffValidationReport {\n    #[serde(rename = \"schemaVersion\")]\n    pub schema_version: String,\n    pub workspace: String,\n    pub project: String",
        "pub struct TaskHandoffValidationReport {\n    #[serde(rename = \"schemaVersion\")]\n    pub schema_version: String,\n    pub workspace: String",
        "pub struct AdoAiContextWorkItem {\n    pub id: String",
        "pub parent_ids: Vec<String>",
        "pub child_ids: Vec<String>",
        "pub predecessor_ids: Vec<String>",
        "pub successor_ids: Vec<String>",
        "pub work_item_id: Option<String>",
        "pub struct TaskPreflightReport {\n    #[serde(rename = \"schemaVersion\")]\n    pub schema_version: String,\n    pub workspace: String,\n    pub project: String",
        "pub struct TaskPreflightReport {\n    #[serde(rename = \"schemaVersion\")]\n    pub schema_version: String,\n    pub workspace: String",
        "pub work_item_ids: Vec<String>",
        "pub work_item_id: String",
        "pub related_ids: Vec<String>",
        "pub severity: String",
        "pub repository: String",
        "pub status: String",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive structured field `{}` in migrated contracts",
            contracts.display(),
            forbidden
        );
    }
    for required in [
        "ProjectKey",
        "WorkItemId",
        "WorkspacePath",
        "WorkspaceRepositoryName",
        "TaskHandoffValidationStatus",
        "TaskPreflightSeverity",
    ] {
        assert!(
            text.contains(required),
            "{} should expose contract domain type `{}`",
            contracts.display(),
            required
        );
    }
}

#[test]
fn input_dialogue_protocol_uses_typed_prompt_identifiers_and_choice_values() {
    let repo = repo_root();
    let core = repo.join("crates/dw-core/src/lib.rs");
    let text = fs::read_to_string(&core).expect("read core lib");
    for forbidden in [
        "pub struct PromptSpec {\n    pub id: String",
        "pub struct PromptChoice {\n    pub value: String",
        "Confirm {\n        id: String",
        "SelectOne {\n        id: String",
        "SelectMany {\n        id: String",
        "Text {\n        id: String",
        "Secret {\n        id: String",
        "SelectOne { value: String }",
        "SelectMany { values: Vec<String> }",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive dialogue protocol field `{}`",
            core.display(),
            forbidden
        );
    }
    for required in ["PromptId", "PromptChoiceValue"] {
        assert!(
            text.contains(required),
            "{} should expose dialogue domain type `{}`",
            core.display(),
            required
        );
    }
}

#[test]
fn migrated_action_requests_use_domain_id_types_not_structured_strings() {
    let repo = repo_root();
    let checked: &[(&str, &[&str])] = &[
        (
            "crates/dw-task/src/start.rs",
            &[
                "pub work_item_id: Option<String>",
                "pub pull_request_id: String",
                "pub root: Option<String>",
                "pub root: String",
                "pub project: Option<String>",
                "pub project: String",
                "pub only: Option<String>",
                "pub repo: Option<String>",
                "pub task: Option<String>",
                "pub type_name: Option<String>",
                "pub slug: Option<String>",
                "work_item_ids.join(",
                "workspace_repositories.join(\",\")",
                ".split(',')",
            ],
        ),
        (
            "crates/dw-task/src/open.rs",
            &[
                "pub workspace: Option<String>",
                "pub root: Option<String>",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub positional_work_item",
                "pub pull_request: Option<String>",
                "pub repo: Option<String>",
                "pub agent: Option<String>",
                "work_item_ids.join(",
                "resolve_workspace(",
            ],
        ),
        (
            "crates/dw-task/src/work_item.rs",
            &[
                "pub work_item_ids: Option<String>",
                "pub workspace: Option<String>",
                "pub root: Option<String>",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub positional_work_item",
                "parse_work_item_ids as parse_workspace_work_item_ids",
                "work_item_ids.join(",
                "work_item_id_values(&args.work_item_ids).join",
                "let work_item_selection",
                "resolve_workspace(",
                "pub workspace: String",
                "pub project: String",
                "pub new_workspace: String",
                "pub type_name: Option<String>",
                "pub title: Option<String>",
                "pub state: Option<String>",
            ],
        ),
        (
            "crates/dw-task/src/lifecycle.rs",
            &[
                "pub workspace: Option<String>",
                "pub root: Option<String>",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub positional_work_item",
                "pub repo: String",
                "pub slug: String",
                "pub title: String",
                "pub workspace: String",
                "pub repository: String",
                "resolve_workspace(",
            ],
        ),
        (
            "crates/dw-task/src/prune.rs",
            &[
                "pub root: Option<String>",
                "pub root: String",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub work_item_ids: Vec<String>",
                "pub workspace: String",
                "pub deleted: Vec<String>",
            ],
        ),
        (
            "crates/dw-task/src/finish.rs",
            &[
                "pub workspace: Option<String>",
                "pub root: Option<String>",
                "pub root: String",
                "pub workspace: String",
                "pub repository: String",
                "pub id: String",
            ],
        ),
        (
            "crates/dw-task/src/validate.rs",
            &[
                "pub workspace: Option<String>",
                "pub root: Option<String>",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub ai_context_file: Vec<String>",
                "pub ai_context_files: Vec<String>",
                "pub positional_work_item",
                "resolve_workspace(",
            ],
        ),
        (
            "crates/dw-task/src/repo.rs",
            &[
                "pub workspace: Option<String>",
                "pub workspace: String",
                "pub root: Option<String>",
                "pub project: Option<String>",
                "pub work_item: Option<String>",
                "pub positional_work_item",
                "pub repo: String",
                "pub repository: String",
                "pub path: String",
                "pub branch_name: String",
                "pub default_branch: String",
                "pub only: Option<String>",
                "pub choices: Vec<String>",
                "pub committed: Vec<String>",
                "resolve_workspace(",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/assigned.rs",
            &[
                "pub root: Option<String>",
                "pub root: String",
                "pub project: Option<String>",
                "pub project: String",
                "pub fn suggested_start_ids(parent: &WorkItemSnapshot, children: &[WorkItemSnapshot]) -> String",
                "ids.join(\",\")",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/prs.rs",
            &[
                "pub root: Option<String>",
                "pub root: String",
                "pub repo: Option<String>",
                "pub repositories: Vec<String>",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/changelog.rs",
            &[
                "pub ids: String",
                "pub git_to: Option<String>",
                "pub root: Option<String>",
                "pub root: String",
                "pub repo: Option<String>",
                "pub format: Option<String>",
                "pub format: String",
                "parse_changelog_format(format.as_deref())",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/context.rs",
            &[
                "pub id: String",
                "pub root: Option<String>",
                "pub root: String",
                "parse_work_item_ids_as_strings",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/set_state.rs",
            &[
                "pub id: String",
                "pub ids: Vec<String>",
                "pub root: Option<String>",
                "pub root: String",
                "pub state: String",
                "pub history: Option<String>",
                "pub history: String",
                "set_state_confirmation_prompt",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/work_item.rs",
            &[
                "pub id: String",
                "pub root: Option<String>",
                "pub root: String",
                "parse_work_item_ids_as_strings",
                "parse_id_set",
            ],
        ),
        (
            "crates/dw-db/src/commands.rs",
            &[
                "pub sql: String",
                "pub project: Option<String>",
                "pub database: Option<String>",
                "pub env: Option<String>",
                "pub table: Option<String>",
                "project: Option<&str>",
                "database: Option<&str>",
                "env: Option<&str>",
            ],
        ),
    ];

    for (relative, forbidden_tokens) in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read source file");
        for forbidden in *forbidden_tokens {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden structured string action token `{}`",
                path.display(),
                forbidden
            );
        }
    }
}

#[test]
fn db_command_contracts_use_typed_query_connection_and_table_values() {
    let repo = repo_root();
    let commands = repo.join("crates/dw-db/src/commands.rs");
    let text = fs::read_to_string(&commands).expect("read db commands");
    for required in [
        "SqlQuery",
        "DatabaseKey",
        "DatabaseEnvironmentName",
        "DatabaseTableName",
        "ProjectKey",
        "pub sql: SqlQuery",
        "pub project: Option<ProjectKey>",
        "pub database: Option<DatabaseKey>",
        "pub env: Option<DatabaseEnvironmentName>",
        "pub table: Option<DatabaseTableName>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed DB command token `{}`",
            commands.display(),
            required
        );
    }

    let config =
        fs::read_to_string(repo.join("crates/dw-db/src/config.rs")).expect("read db config source");
    for required in [
        "pub project: &'a ProjectKey",
        "pub database: &'a DatabaseKey",
        "project: &ProjectKey",
        "database: &DatabaseKey",
    ] {
        assert!(
            config.contains(required),
            "dw-db config should expose typed selection token `{required}`"
        );
    }
}

#[test]
fn ado_assigned_contract_uses_typed_root_project_and_suggested_ids() {
    let repo = repo_root();
    let path = repo.join("crates/dw-ado-commands/src/commands/assigned.rs");
    let text = fs::read_to_string(&path).expect("read assigned source");
    for required in [
        "DevWorkflowRoot",
        "ProjectKey",
        "WorkItemId",
        "pub root: Option<DevWorkflowRoot>",
        "pub root: DevWorkflowRoot",
        "pub project: Option<ProjectKey>",
        "pub project: ProjectKey",
        "-> Vec<WorkItemId>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed ADO assigned contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn ado_prs_contract_uses_typed_root_and_repositories() {
    let repo = repo_root();
    let path = repo.join("crates/dw-ado-commands/src/commands/prs.rs");
    let text = fs::read_to_string(&path).expect("read prs source");
    for required in [
        "AdoRepositoryName",
        "DevWorkflowRoot",
        "ProjectKey",
        "pub root: Option<DevWorkflowRoot>",
        "pub root: DevWorkflowRoot",
        "pub repo: Option<AdoRepositoryName>",
        "pub repositories: Vec<AdoRepositoryName>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed ADO PR contract token `{}`",
            path.display(),
            required
        );
    }

    let changelog =
        fs::read_to_string(repo.join("crates/dw-ado-commands/src/commands/changelog.rs"))
            .expect("read changelog source");
    for required in [
        "pub repo: Option<AdoRepositoryName>",
        "repository: Option<&AdoRepositoryName>",
        "-> Vec<AdoRepositoryName>",
    ] {
        assert!(
            changelog.contains(required),
            "changelog repository resolver should expose typed token `{required}`"
        );
    }
}

#[test]
fn ado_changelog_contract_uses_typed_root_repository_and_format() {
    let repo = repo_root();
    let path = repo.join("crates/dw-ado-commands/src/commands/changelog.rs");
    let text = fs::read_to_string(&path).expect("read changelog source");
    for required in [
        "AdoRepositoryName",
        "ChangelogOutputFormat",
        "DevWorkflowRoot",
        "ProjectKey",
        "PullRequestId",
        "WorkItemId",
        "pub root: Option<DevWorkflowRoot>",
        "pub root: DevWorkflowRoot",
        "pub repo: Option<AdoRepositoryName>",
        "pub format: ChangelogOutputFormat",
        "pub enum ChangelogOutputFormat",
        "impl FromStr for ChangelogOutputFormat",
        "pub fn as_ado_format(self) -> ChangelogFormat",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed ADO changelog contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn ado_work_item_and_context_contracts_use_typed_roots() {
    let repo = repo_root();
    let checked: &[(&str, &[&str])] = &[
        (
            "crates/dw-ado-commands/src/commands/work_item.rs",
            &[
                "DevWorkflowRoot",
                "ProjectKey",
                "WorkItemId",
                "pub root: Option<DevWorkflowRoot>",
                "pub root: DevWorkflowRoot",
                "pub project: Option<ProjectKey>",
                "pub project: ProjectKey",
                "pub requested_ids: Vec<WorkItemId>",
            ],
        ),
        (
            "crates/dw-ado-commands/src/commands/context.rs",
            &[
                "DevWorkflowRoot",
                "ProjectKey",
                "WorkItemId",
                "pub root: Option<DevWorkflowRoot>",
                "pub root: DevWorkflowRoot",
                "pub project: Option<ProjectKey>",
                "pub project: ProjectKey",
                "pub requested_ids: Vec<WorkItemId>",
            ],
        ),
    ];

    for (relative, required_tokens) in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read ADO source");
        for required in *required_tokens {
            assert!(
                text.contains(required),
                "{} should expose typed ADO work-item/context contract token `{}`",
                path.display(),
                required
            );
        }
    }
}

#[test]
fn ado_set_state_contract_uses_typed_root_state_and_history() {
    let repo = repo_root();
    let path = repo.join("crates/dw-ado-commands/src/commands/set_state.rs");
    let text = fs::read_to_string(&path).expect("read set-state source");
    for required in [
        "DevWorkflowRoot",
        "ProjectKey",
        "WorkItemHistoryComment",
        "WorkItemId",
        "WorkItemState",
        "pub root: Option<DevWorkflowRoot>",
        "pub root: DevWorkflowRoot",
        "pub project: Option<ProjectKey>",
        "pub project: ProjectKey",
        "pub ids: Vec<WorkItemId>",
        "pub state: WorkItemState",
        "pub history: Option<WorkItemHistoryComment>",
        "pub history: WorkItemHistoryComment",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed ADO set-state contract token `{}`",
            path.display(),
            required
        );
    }

    let core = fs::read_to_string(repo.join("crates/dw-core/src/lib.rs")).expect("read core lib");
    for required in ["pub struct WorkItemHistoryComment", "state: WorkItemState"] {
        assert!(
            core.contains(required),
            "dw-core should expose typed set-state event/value token `{required}`"
        );
    }
}

#[test]
fn task_finish_contract_uses_typed_paths_repositories_and_work_items() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/finish.rs");
    let text = fs::read_to_string(&path).expect("read finish source");
    for required in [
        "DevWorkflowRoot",
        "WorkItemId",
        "WorkspacePath",
        "WorkspaceRepositoryName",
        "RepositoryPath",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub root: DevWorkflowRoot",
        "pub workspace: WorkspacePath",
        "pub repository: WorkspaceRepositoryName",
        "pub id: WorkItemId",
        "pub changed_repositories: Vec<WorkspaceRepositoryName>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed finish contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn app_and_tui_core_requests_use_typed_config_agent_and_secret_values() {
    let repo = repo_root();
    let checked: &[(&str, &[&str], &[&str])] = &[
        (
            "crates/dw-app/src/lib.rs",
            &[
                "Agent",
                "ConfigColorMode",
                "ConfigRootPath",
                "DevWorkflowRoot",
                "EnvironmentVariableName",
                "SecretKey",
                "root: Option<DevWorkflowRoot>",
                "mode: ConfigColorMode",
                "path: ConfigRootPath",
                "agent: Agent",
                "agent: Option<Agent>",
                "key: SecretKey",
                "key: SecretKey",
                "env: EnvironmentVariableName",
            ],
            &[
                "ConfigShow { root: Option<String> }",
                "ConfigDoctor { root: Option<String> }",
                "ConfigSetColor { mode: String }",
                "ConfigSetRoot { path: String }",
                "AgentSetDefault { root: Option<String>, agent: String }",
                "AgentDoctor { agent: Option<String> }",
                "SecretGet { key: String }",
                "SecretDelete { key: String }",
                "SecretSetFromEnv { key: String, env: String }",
            ],
        ),
        (
            "crates/dw-tui/src/model.rs",
            &[
                "Agent",
                "ConfigColorMode",
                "ConfigRootPath",
                "DevWorkflowRoot",
                "EnvironmentVariableName",
                "SecretKey",
                "root: Option<DevWorkflowRoot>",
                "mode: ConfigColorMode",
                "path: ConfigRootPath",
                "agent: Agent",
                "agent: Option<Agent>",
                "key: SecretKey",
                "key: SecretKey",
                "env: EnvironmentVariableName",
            ],
            &[
                "ConfigShow { root: Option<String> }",
                "ConfigDoctor { root: Option<String> }",
                "ConfigSetColor { mode: String }",
                "ConfigSetRoot { path: String }",
                "AgentSetDefault { root: Option<String>, agent: String }",
                "AgentDoctor { agent: Option<String> }",
                "SecretGet { key: String }",
                "SecretDelete { key: String }",
                "SecretSetFromEnv { key: String, env: String }",
            ],
        ),
        (
            "crates/dw-config/src/store.rs",
            &[
                "pub fn set_default_agent(root: &DevWorkflowRoot, agent: Agent)",
                "pub fn default_agent(root: &DevWorkflowRoot) -> Agent",
            ],
            &[
                "pub fn set_default_agent(root: &str, agent: &str)",
                "pub fn default_agent(root: &str) -> String",
            ],
        ),
        (
            "crates/dw-config/src/command.rs",
            &[
                "pub fn show(root: Option<&DevWorkflowRoot>)",
                "pub fn doctor(root: Option<&DevWorkflowRoot>)",
                "pub fn set_root(path: &ConfigRootPath)",
                "pub fn set_color(mode: &ConfigColorMode)",
            ],
            &[
                "pub fn show(root: Option<&str>)",
                "pub fn doctor(root: Option<&str>)",
                "pub fn set_root(path: &str)",
                "pub fn set_color(mode: &str)",
            ],
        ),
        (
            "crates/dw-secret/src/command.rs",
            &[
                "pub key: SecretKey",
                "pub fn set_secret(key: &SecretKey",
                "pub fn get_secret(key: &SecretKey)",
                "pub fn delete_secret_key(key: &SecretKey)",
            ],
            &[
                "pub key: String",
                "pub fn set_secret(key: &str",
                "pub fn get_secret(key: &str)",
                "pub fn delete_secret_key(key: &str)",
            ],
        ),
    ];

    for &(relative, required_tokens, forbidden_tokens) in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read source file");
        for required in required_tokens {
            assert!(
                text.contains(required),
                "{} should expose typed core contract token `{}`",
                path.display(),
                required
            );
        }
        for forbidden in forbidden_tokens {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden primitive core contract token `{}`",
                path.display(),
                forbidden
            );
        }
    }
}

#[test]
fn config_color_and_set_reports_are_domain_typed() {
    let repo = repo_root();
    let checked: &[(&str, &[&str], &[&str])] = &[
        (
            "crates/dw-config/src/types.rs",
            &[
                "pub color: Option<dw_core::ConfigColorMode>",
                "pub color: dw_core::ConfigColorMode",
            ],
            &["pub color: Option<String>", "pub color: String"],
        ),
        (
            "crates/dw-config/src/command.rs",
            &[
                "pub struct ConfigRootSetReport",
                "pub path: ConfigRootPath",
                "pub struct ConfigColorSetReport",
                "pub mode: ConfigColorMode",
                "pub fn set_root(path: &ConfigRootPath) -> Result<ConfigRootSetReport>",
                "pub fn set_color(mode: &ConfigColorMode) -> Result<ConfigColorSetReport>",
            ],
            &[
                "pub struct ConfigSetReport",
                "pub field: String",
                "pub value: String",
            ],
        ),
        (
            "crates/dw-tui/src/model.rs",
            &[
                "pub color_mode: ConfigColorMode",
                "ColorMode(ConfigColorMode)",
                "DefaultAgent(Agent)",
                "pub fn default_agent(&self) -> Agent",
            ],
            &[
                "pub color_mode: String",
                "ColorMode(String)",
                "DefaultAgent(String)",
                "pub fn default_agent(&self) -> String",
            ],
        ),
        (
            "crates/dw-tui/src/actions.rs",
            &[
                "Agent(Agent)",
                "Color(ConfigColorMode)",
                "QuickOptionState::Agent(Agent::Codex)",
                "QuickOptionState::Color(ConfigColorMode::Always)",
            ],
            &[
                "Agent(&'static str)",
                "Color(&'static str)",
                "QuickOptionState::Agent(\"codex\")",
                "QuickOptionState::Color(\"always\")",
            ],
        ),
    ];

    for (path, required, forbidden) in checked {
        let path = repo.join(path);
        let text = fs::read_to_string(&path).expect("read source file");
        for token in *required {
            assert!(
                text.contains(token),
                "{} should expose typed config token `{}`",
                path.display(),
                token
            );
        }
        for token in *forbidden {
            assert!(
                !text.contains(token),
                "{} contains forbidden string config contract `{}`",
                path.display(),
                token
            );
        }
    }
}

#[test]
fn task_open_contract_uses_typed_workspace_filters_repositories_and_agent() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/open.rs");
    let text = fs::read_to_string(&path).expect("read open source");
    for required in [
        "AgentName",
        "DevWorkflowRoot",
        "ProjectKey",
        "PullRequestId",
        "WorkItemId",
        "WorkspacePath",
        "WorkspaceRepositoryName",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub pull_request: Option<PullRequestId>",
        "pub repo: Option<WorkspaceRepositoryName>",
        "pub agent: Option<AgentName>",
        "resolve_workspace_by_work_item_ids",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed open contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_lifecycle_contract_uses_typed_workspace_filters_and_repositories() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/lifecycle.rs");
    let text = fs::read_to_string(&path).expect("read lifecycle source");
    for required in [
        "DevWorkflowRoot",
        "ProjectKey",
        "TaskSlug",
        "WorkItemId",
        "WorkItemTitle",
        "WorkspacePath",
        "WorkspaceRepositoryName",
        "pub slug: TaskSlug",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub repo: WorkspaceRepositoryName",
        "pub title: WorkItemTitle",
        "pub requested_title: WorkItemTitle",
        "pub requested_ids: Vec<WorkItemId>",
        "resolve_workspace_by_work_item_ids",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed lifecycle contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_work_item_contract_uses_typed_workspace_filters_and_ids() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/work_item.rs");
    let text = fs::read_to_string(&path).expect("read work item source");
    for required in [
        "DevWorkflowRoot",
        "ProjectKey",
        "WorkItemId",
        "WorkItemState",
        "WorkItemTitle",
        "WorkItemTypeName",
        "WorkspacePath",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub workspace_work_item_ids: Vec<WorkItemId>",
        "pub type_name: Option<WorkItemTypeName>",
        "pub title: Option<WorkItemTitle>",
        "pub state: Option<WorkItemState>",
        "pub requested_ids: Vec<WorkItemId>",
        "pub skipped_existing_ids: Vec<WorkItemId>",
        "pub fn work_item_id_from_choice(label: &str) -> WorkItemId",
        "resolve_workspace_by_work_item_ids",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed work-item contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_repo_contract_uses_typed_paths_repositories_and_branches() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/repo.rs");
    let text = fs::read_to_string(&path).expect("read repo source");
    for required in [
        "DevWorkflowRoot",
        "WorkspacePath",
        "WorkspaceRepositoryName",
        "ProjectKey",
        "WorkItemId",
        "RepositoryPath",
        "BranchName",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub repo: WorkspaceRepositoryName",
        "pub repositories: Vec<WorkspaceRepositoryName>",
        "pub branch_name: BranchName",
        "pub committed: Vec<WorkspaceRepositoryName>",
        "resolve_workspace_by_work_item_ids",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed repo contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn workspace_rename_and_work_item_update_plans_use_typed_paths_slugs_and_branches() {
    let repo = repo_root();
    let path = repo.join("crates/dw-workspace/src/lib.rs");
    let text = fs::read_to_string(&path).expect("read workspace source");
    for forbidden in [
        "pub struct TaskRenamePlan {\n    pub workspace: String",
        "pub struct TaskRenamePlan {\n    pub workspace: WorkspacePath,\n    #[serde(rename = \"newWorkspace\")]\n    pub new_workspace: String",
        "pub old_slug: String",
        "pub new_slug: String",
        "pub old_branch: String",
        "pub new_branch: String",
        "pub struct TaskWorkItemUpdatePlan {\n    pub workspace: String",
        "pub struct TaskWorkItemUpdatePlan {\n    pub workspace: WorkspacePath,\n    #[serde(rename = \"newWorkspace\")]\n    pub new_workspace: String",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive workspace plan token `{}`",
            path.display(),
            forbidden
        );
    }
    for required in [
        "pub workspace: WorkspacePath",
        "pub new_workspace: WorkspacePath",
        "pub old_slug: TaskSlug",
        "pub new_slug: TaskSlug",
        "pub old_branch: BranchName",
        "pub new_branch: BranchName",
        "-> Result<(WorkspaceManifest, WorkspacePath), WorkspaceError>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed workspace plan token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn workspace_list_current_and_resolution_contracts_use_domain_types() {
    let repo = repo_root();
    let path = repo.join("crates/dw-workspace/src/lib.rs");
    let text = fs::read_to_string(&path).expect("read workspace source");
    for forbidden in [
        "pub struct WorkspaceSummary {\n    pub path: String",
        "pub struct TaskListItem {\n    pub path: String",
        "pub struct TaskListItem {\n    pub path: WorkspacePath,\n    pub project: String",
        "pub work_item_id: String,\n    #[serde(rename = \"displayWorkItems\")]",
        "pub task_id: Option<String>,\n    #[serde(rename = \"displayWorkItems\")]",
        "pub all_known_work_item_ids: Vec<String>",
        "pub slug: TaskSlug,\n    #[serde(rename = \"branchName\")]\n    pub branch_name: String",
        "pub repositories: Vec<String>,\n}",
        "pub struct TaskCurrentItem {\n    pub workspace: String",
        "pub child_task_ids: BTreeMap<String, String>",
        ") -> Result<String, WorkspaceError> {\n    let work_item = resolve_work_item_ids",
        ") -> Result<String, WorkspaceError> {\n    if let Some(workspace) = workspace.filter",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive workspace result token `{}`",
            path.display(),
            forbidden
        );
    }
    for required in [
        "pub struct WorkspaceSummary {\n    pub path: WorkspacePath",
        "pub path: WorkspacePath",
        "pub project: ProjectKey",
        "pub work_item_id: WorkItemId",
        "pub task_id: Option<TaskId>",
        "pub all_known_work_item_ids: Vec<WorkItemId>",
        "pub kind: WorkItemTypeName",
        "pub slug: TaskSlug",
        "pub branch_name: BranchName",
        "pub repositories: Vec<WorkspaceRepositoryName>",
        "pub workspace: WorkspacePath",
        "pub primary_work_item_id: WorkItemId",
        "pub child_task_ids: BTreeMap<WorkspaceRepositoryName, WorkItemId>",
        "pub branch: BranchName",
        ") -> Result<WorkspacePath, WorkspaceError> {\n    let work_item = resolve_work_item_ids",
        ") -> Result<WorkspacePath, WorkspaceError> {\n    if let Some(workspace) = workspace.filter",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed workspace result token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_validate_contract_uses_typed_workspace_filters_and_ai_context_paths() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/validate.rs");
    let text = fs::read_to_string(&path).expect("read validate source");
    for required in [
        "AiContextFilePath",
        "DevWorkflowRoot",
        "ProjectKey",
        "WorkItemId",
        "WorkspacePath",
        "pub workspace: Option<WorkspacePath>",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub ai_context_files: Vec<AiContextFilePath>",
        "resolve_workspace_by_work_item_ids",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed validate contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_prune_contract_uses_typed_paths_and_ids() {
    let repo = repo_root();
    let path = repo.join("crates/dw-task/src/prune.rs");
    let text = fs::read_to_string(&path).expect("read prune source");
    for required in [
        "DevWorkflowRoot",
        "ProjectKey",
        "WorkItemId",
        "WorkspacePath",
        "pub root: Option<DevWorkflowRoot>",
        "pub project: Option<ProjectKey>",
        "pub work_item_ids: Vec<WorkItemId>",
        "pub deleted: Vec<WorkspacePath>",
    ] {
        assert!(
            text.contains(required),
            "{} should expose typed prune contract token `{}`",
            path.display(),
            required
        );
    }
}

#[test]
fn task_start_contracts_parse_repository_selection_at_boundaries() {
    let repo = repo_root();
    let checked: &[(&str, &[&str], &[&str])] = &[
        (
            "crates/dw-task/src/start.rs",
            &[
                "pub root: Option<String>",
                "pub root: String",
                "pub project: Option<String>",
                "pub project: String",
                "pub only: Option<String>",
                "pub repo: Option<String>",
                "pub task: Option<String>",
                "pub type_name: Option<String>",
                "pub slug: Option<String>",
                ".split(',')",
                "only: workspace_repositories.join",
                "only: args.repo.clone()",
            ],
            &[
                "DevWorkflowRoot",
                "TaskId",
                "TaskSlug",
                "WorkItemTypeName",
                "WorkItemState",
                "pub root: Option<DevWorkflowRoot>",
                "pub root: DevWorkflowRoot",
                "pub project: Option<ProjectKey>",
                "pub project: ProjectKey",
                "pub task: Option<TaskId>",
                "pub type_name: Option<WorkItemTypeName>",
                "pub slug: Option<TaskSlug>",
                "pub repositories: Vec<WorkspaceRepositoryName>",
                "pub target_state: WorkItemState",
            ],
        ),
        (
            "crates/dw-workspace/src/lib.rs",
            &[
                "pub project: Option<&'a str>",
                "pub only: Option<&'a str>",
                "fn resolve_repositories(project_config: Option<&ProjectConfig>, only: Option<&str>)",
                "pub struct TaskStartPlan {\n    #[serde(rename = \"workItemIds\")]\n    pub work_item_ids: Vec<String>",
                "pub struct TaskStartPlan {\n    #[serde(rename = \"workItemIds\")]\n    pub work_item_ids: Vec<WorkItemId>,\n    #[serde(rename = \"primaryWorkItemId\")]\n    pub primary_work_item_id: String",
                "pub struct TaskStartRepositoryPlan {\n    pub repository: String",
                "pub struct TaskStartRepositoryPlan {\n    pub repository: WorkspaceRepositoryName,\n    #[serde(rename = \"projectRoot\")]\n    pub project_root: String",
            ],
            &[
                "GitAnchorName",
                "ProjectRootPath",
                "RepositoryPath",
                "TaskId",
                "TaskSlug",
                "WorkItemId",
                "WorkItemTypeName",
                "pub project: Option<&'a ProjectKey>",
                "pub repositories: &'a [WorkspaceRepositoryName]",
                "pub work_item_ids: Vec<WorkItemId>",
                "pub primary_work_item_id: WorkItemId",
                "pub project: ProjectKey",
                "pub task_id: Option<TaskId>",
                "pub kind: WorkItemTypeName",
                "pub slug: TaskSlug",
                "pub branch_name: BranchName",
                "pub workspace: WorkspacePath",
                "pub repositories: Vec<WorkspaceRepositoryName>",
                "pub repository_folders: BTreeMap<WorkspaceRepositoryName, RepositoryPath>",
                "pub repository: WorkspaceRepositoryName",
                "pub project_root: ProjectRootPath",
                "pub worktree_path: RepositoryPath",
                "pub default_branch: BranchName",
                "pub anchor_name: GitAnchorName",
            ],
        ),
    ];

    for (relative, forbidden_tokens, required_tokens) in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read source file");
        for forbidden in *forbidden_tokens {
            assert!(
                !text.contains(forbidden),
                "{} contains primitive task start contract token `{}`",
                path.display(),
                forbidden
            );
        }
        for required in *required_tokens {
            assert!(
                text.contains(required),
                "{} should expose typed task start contract token `{}`",
                path.display(),
                required
            );
        }
    }
}

#[test]
fn migrated_ado_project_contracts_use_project_key() {
    let repo = repo_root();
    let checked: &[&str] = &[
        "crates/dw-ado-commands/src/commands/assigned.rs",
        "crates/dw-ado-commands/src/commands/changelog.rs",
        "crates/dw-ado-commands/src/commands/prs.rs",
        "crates/dw-ado-commands/src/commands/set_state.rs",
        "crates/dw-ado-commands/src/commands/work_item.rs",
        "crates/dw-ado-commands/src/commands/context.rs",
    ];

    for relative in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read source file");
        assert!(
            text.contains("ProjectKey"),
            "{} should use ProjectKey for migrated ADO project contracts",
            path.display()
        );
        assert!(
            !text.contains("pub project: String"),
            "{} contains primitive ADO project contract token `pub project: String`",
            path.display()
        );
    }

    for relative in checked {
        let path = repo.join(relative);
        let text = fs::read_to_string(&path).expect("read source file");
        assert!(
            !text.contains("pub project: Option<String>"),
            "{} contains primitive ADO project contract token `pub project: Option<String>`",
            path.display()
        );
    }
}

#[test]
fn tui_and_ui_layers_do_not_embed_cli_command_hints() {
    let repo = repo_root();
    let checked_roots = [
        repo.join("crates/dw-tui/src"),
        repo.join("crates/dw-tui-adapter/src"),
        repo.join("crates/dw-ui/src"),
    ];

    for root in checked_roots {
        for file in rust_files(&root) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in [
                "dw task ",
                "dw ado ",
                "dw db ",
                "dw auth ",
                "dw config ",
                "AnsiRender",
                "Non-TTY",
                "CompletionShow",
                "QuickOptionAction::Completion",
                "Confirmation CLI",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden CLI presentation token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn tui_visible_text_uses_intentions_not_cli_subcommand_names() {
    let repo = repo_root();
    let checked_roots = [repo.join("crates/dw-tui/src")];

    for root in checked_roots {
        for file in rust_files(&root) {
            let text = fs::read_to_string(&file).expect("read source file");
            for forbidden in [
                "Task start",
                "Task prune",
                "ADO assigned",
                "DB schema",
                "start-pr",
                "--execute",
                "--yes",
                "argv",
                "display_command",
                "std::process::Command",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "{} contains forbidden TUI implementation/presentation token `{}`",
                    file.display(),
                    forbidden
                );
            }
        }
    }
}

#[test]
fn tui_internal_actions_are_typed_requests_not_cli_argv() {
    let repo = repo_root();
    let tui_dir = repo.join("crates").join("dw-tui").join("src");
    for file in rust_files(&tui_dir) {
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "pub command: Vec<String>",
            "command: vec!",
            "command: Vec<String>",
            "display_command(&self) -> Vec<String>",
            "display_command",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden TUI argv model token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

#[test]
fn domain_completion_catalogs_live_in_cli_completion_adapter() {
    let repo = repo_root();
    for relative in [
        "crates/dw-task/src/completion.rs",
        "crates/dw-ado-commands/src/completion.rs",
        "crates/dw-agent/src/completion.rs",
        "crates/dw-db/src/completion.rs",
        "crates/dw-secret/src/completion.rs",
        "crates/dw-db/src/command.rs",
    ] {
        assert!(
            !repo.join(relative).exists(),
            "{} should not exist; CLI completion/parsing belongs to dw-completion or dw-cli",
            relative
        );
    }

    let config_completion = fs::read_to_string(repo.join("crates/dw-config/src/completion.rs"))
        .expect("config completion");
    for forbidden in [
        "CompletionCatalog",
        "options_for",
        "option_requires_value",
        "option_allowed",
        "\"--",
    ] {
        assert!(
            !config_completion.contains(forbidden),
            "dw-config completion should expose value sources only, found `{}`",
            forbidden
        );
    }
}

#[test]
fn migrated_task_modules_do_not_depend_on_cli_or_terminal_rendering() {
    let repo = repo_root();
    for relative in [
        "crates/dw-task/src/open.rs",
        "crates/dw-task/src/finish.rs",
        "crates/dw-task/src/lifecycle.rs",
        "crates/dw-task/src/prune.rs",
        "crates/dw-task/src/repo.rs",
        "crates/dw-task/src/start.rs",
        "crates/dw-task/src/start/ado.rs",
        "crates/dw-task/src/validate.rs",
        "crates/dw-task/src/work_item.rs",
    ] {
        let file = repo.join(relative);
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in CORE_FORBIDDEN {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden task core token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

#[test]
fn domain_progress_contracts_are_structured_not_line_helpers() {
    let repo = repo_root();
    let checked_files = [
        repo.join("crates/dw-task/src/finish.rs"),
        repo.join("crates/dw-task/src/lifecycle.rs"),
        repo.join("crates/dw-task/src/prune.rs"),
        repo.join("crates/dw-task/src/start.rs"),
        repo.join("crates/dw-task/src/work_item.rs"),
        repo.join("crates/dw-ado-commands/src/commands/changelog.rs"),
        repo.join("crates/dw-ado-commands/src/commands/work_item.rs"),
        repo.join("crates/dw-upgrade/src/lib.rs"),
    ];

    for file in checked_files {
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "events: Vec<String>",
            "pub events: Vec<String>",
            "finish_verification_start_line",
            "finish_git_start_line",
            "start_pr_fetch_line",
            "start_pr_resolved_line",
            "work_item_fetch_line",
            "sync_fetch_line",
            "teardown_git_progress_line",
            "changelog_git_extract_line",
            "changelog_pr_fetch_line",
            "changelog_items_fetch_line",
            "pub message: String",
            "pub action: String",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden string progress contract `{}`",
                file.display(),
                forbidden
            );
        }
    }

    let repo_text =
        fs::read_to_string(repo.join("crates/dw-task/src/repo.rs")).expect("read repo source file");
    assert!(
        !repo_text.contains("teardown_git_progress_line"),
        "dw-task repo contains forbidden string progress helper `teardown_git_progress_line`"
    );
}

#[test]
fn migrated_ado_command_modules_do_not_render_or_prompt() {
    let repo = repo_root();
    for relative in [
        "crates/dw-ado-commands/src/auth.rs",
        "crates/dw-ado-commands/src/commands/assigned.rs",
        "crates/dw-ado-commands/src/commands/changelog.rs",
        "crates/dw-ado-commands/src/commands/context.rs",
        "crates/dw-ado-commands/src/commands/project.rs",
        "crates/dw-ado-commands/src/commands/prs.rs",
        "crates/dw-ado-commands/src/commands/set_state.rs",
        "crates/dw-ado-commands/src/commands/work_item.rs",
    ] {
        let file = repo.join(relative);
        let text = fs::read_to_string(&file).expect("read source file");
        for forbidden in [
            "inquire",
            "ratatui",
            "TerminalTheme",
            "AnsiRender",
            "println!",
            "eprintln!",
            "dw_ui",
            "print_styled",
            "terminal_theme",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden ADO adapter token `{}`",
                file.display(),
                forbidden
            );
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn rust_and_manifest_files(root: &Path) -> Vec<PathBuf> {
    let mut files = rust_files(&root.join("src"));
    files.push(root.join("Cargo.toml"));
    files
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files
}

fn surrounding_lines(text: &str, line_index: usize, radius: usize) -> String {
    let start = line_index.saturating_sub(radius);
    let end = line_index.saturating_add(radius + 1);
    text.lines()
        .skip(start)
        .take(end - start)
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
