pub(super) fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--project"
            | "--repo"
            | "--workspace"
            | "--work-item"
            | "--only"
            | "--database"
            | "--env"
            | "--max-rows"
            | "--format"
            | "--type"
            | "--agent"
            | "--root"
            | "--rid"
            | "--slug"
            | "--task"
            | "--title"
            | "--message"
            | "--profile"
            | "--sql"
            | "--value"
            | "--from-env"
            | "--top"
            | "--comments"
            | "--organization"
            | "--state"
            | "--ai-context-file"
    )
}

pub(super) fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--workspace" => &["--project", "--work-item", "--continue"][..],
        "--project" | "--work-item" | "--continue" => &["--workspace"][..],
        "--database" => &["--env"][..],
        "--env" => &["--database"][..],
        "--from-pr" => &["--from-git"][..],
        "--from-git" => &["--from-pr"][..],
        "--value" => &["--from-env"][..],
        "--from-env" => &["--value"][..],
        "--check" => &["--rid"][..],
        "--rid" => &["--check"][..],
        _ => &[][..],
    };
    if conflicts.iter().any(|conflict| selected.contains(conflict)) {
        return false;
    }
    match option {
        "--ready" => selected.contains(&"--create-pr"),
        "--git-to" => selected.contains(&"--from-git"),
        "--table" => selected.contains(&"--format"),
        _ => true,
    }
}

pub(super) fn options_for_path(path: &[&str]) -> Vec<&'static str> {
    match path {
        ["init"] => vec!["--profile", "--root", "--dry-run", "--no-save"],
        ["refresh"] => vec!["--root", "--profile"],
        ["doctor"] => vec!["--fix"],
        ["upgrade"] => vec!["--check", "--rid"],
        ["completion", "complete"] => vec!["--format"],
        ["completion", _] => Vec::new(),
        ["auth", _] => vec!["--root"],
        ["secret", "set"] => vec!["--value", "--from-env"],
        ["secret", _] => Vec::new(),
        ["config", "show"] | ["config", "doctor"] => vec!["--root", "--json"],
        ["config", "set-root"] | ["config", "set-color"] => Vec::new(),
        ["task", subcommand] => dw_task::completion::options_for(subcommand),
        ["agent", "open"] => dw_task::completion::agent_open_options(),
        ["ado", "changelog"] => vec![
            "--root",
            "--project",
            "--from-pr",
            "--from-git",
            "--repo",
            "--group-by-parent",
            "--format",
            "--table",
            "--ids-only",
            "--git-to",
        ],
        ["ado", "assigned"] => vec![
            "--root",
            "--project",
            "--top",
            "--all",
            "--group-by-parent",
            "--json",
        ],
        ["ado", "work-item"] => vec!["--root", "--project", "--json"],
        ["ado", "context"] => vec!["--root", "--project", "--summary", "--comments", "--json"],
        ["ado", "ai-context"] => vec![
            "--root",
            "--organization",
            "--project",
            "--summary",
            "--comments",
            "--include-comments",
        ],
        ["db", "guard"] => vec!["--sql"],
        ["db", "query"] => vec![
            "--sql",
            "--project",
            "--database",
            "--env",
            "--max-rows",
            "--json",
        ],
        ["db", "schema"] | ["db", "describe"] => {
            vec!["--project", "--database", "--env", "--json"]
        }
        ["agent", "config"] | ["agent", "show"] => vec!["--root"],
        ["agent", "set-default"] => vec!["--root"],
        ["agent", "doctor"] => vec!["--agent"],
        _ => vec!["--help"],
    }
}

pub(super) fn subcommands_for_path(path: &[&str]) -> Option<&'static [&'static str]> {
    match path {
        [] | [""] => Some(root_command_labels()),
        ["task"] => Some(dw_task::completion::subcommands()),
        ["ado"] => Some(&[
            "assigned",
            "changelog",
            "work-item",
            "context",
            "ai-context",
        ]),
        ["db"] => Some(&["schema", "describe", "query", "guard"]),
        ["agent"] => Some(&["context", "open", "config", "show", "set-default", "doctor"]),
        ["auth"] => Some(&["login", "status", "logout"]),
        ["config"] => Some(&["show", "set-root", "set-color", "doctor"]),
        ["completion"] => Some(&["show", "generate", "install"]),
        ["secret"] => Some(&["set", "get", "delete"]),
        _ => None,
    }
}

pub(super) fn root_command_labels() -> &'static [&'static str] {
    &[
        "version",
        "guide",
        "doctor",
        "init",
        "refresh",
        "agent",
        "auth",
        "completion",
        "config",
        "ado",
        "db",
        "secret",
        "upgrade",
        "task",
    ]
}
