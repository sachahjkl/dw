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
        ["secret", subcommand] => dw_secret::completion::options_for(subcommand),
        ["config", subcommand] => dw_config::completion::options_for(subcommand),
        ["task", subcommand] => dw_task::completion::options_for(subcommand),
        ["agent", "open"] => dw_task::completion::agent_open_options(),
        ["agent", subcommand] => dw_agent::completion::options_for(subcommand),
        ["ado", subcommand] => dw_ado_commands::completion::options_for(subcommand),
        ["db", subcommand] => dw_db::completion::options_for(subcommand),
        _ => vec!["--help"],
    }
}

pub(super) fn subcommands_for_path(path: &[&str]) -> Option<&'static [&'static str]> {
    match path {
        [] | [""] => Some(root_command_labels()),
        ["task"] => Some(dw_task::completion::subcommands()),
        ["ado"] => Some(dw_ado_commands::completion::subcommands()),
        ["db"] => Some(dw_db::completion::subcommands()),
        ["agent"] => Some(dw_agent::completion::subcommands()),
        ["auth"] => Some(&["login", "status", "logout"]),
        ["config"] => Some(dw_config::completion::subcommands()),
        ["completion"] => Some(&["show", "generate", "install"]),
        ["secret"] => Some(dw_secret::completion::subcommands()),
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
