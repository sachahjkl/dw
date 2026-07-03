pub(super) fn option_requires_value(option: &str) -> bool {
    root_option_requires_value(option)
        || dw_task::completion::option_requires_value(option)
        || dw_ado_commands::completion::option_requires_value(option)
        || dw_db::completion::option_requires_value(option)
        || dw_config::completion::option_requires_value(option)
        || dw_secret::completion::option_requires_value(option)
        || dw_agent::completion::option_requires_value(option)
}

pub(super) fn option_allowed(path: &[&str], option: &str, selected: &[&str]) -> bool {
    match path {
        ["task", _] | ["agent", "open"] => dw_task::completion::option_allowed(option, selected),
        ["ado", _] => dw_ado_commands::completion::option_allowed(option, selected),
        ["db", _] => dw_db::completion::option_allowed(option, selected),
        ["config", _] => dw_config::completion::option_allowed(option, selected),
        ["secret", _] => dw_secret::completion::option_allowed(option, selected),
        ["agent", _] => dw_agent::completion::option_allowed(option, selected),
        _ => root_option_allowed(option, selected),
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

fn root_option_requires_value(option: &str) -> bool {
    matches!(option, "--root" | "--profile" | "--rid" | "--format")
}

fn root_option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--check" => &["--rid"][..],
        "--rid" => &["--check"][..],
        _ => &[][..],
    };
    !conflicts.iter().any(|conflict| selected.contains(conflict))
}
