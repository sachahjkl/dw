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
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains primitive ID field `{}` in action event contracts",
            core.display(),
            forbidden
        );
    }
    for required in ["WorkItemId", "PullRequestId", "AdoRepositoryName"] {
        assert!(
            text.contains(required),
            "{} should expose domain ID type `{}`",
            core.display(),
            required
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
