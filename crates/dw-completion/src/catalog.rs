use dw_contracts::completion::CompletionContext;

pub(super) fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--root"
            | "--profile"
            | "--rid"
            | "--format"
            | "--project"
            | "--task"
            | "--type"
            | "--only"
            | "--slug"
            | "--repo"
            | "--agent"
            | "--workspace"
            | "--work-item"
            | "--message"
            | "--title"
            | "--state"
            | "--ai-context-file"
            | "--sql"
            | "--database"
            | "--env"
            | "--max-rows"
            | "--value"
            | "--from-env"
            | "--git-to"
            | "--top"
            | "--comments"
            | "--organization"
            | "--history"
    )
}

pub(super) fn option_allowed(path: &[&str], option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--check" => &["--rid"][..],
        "--rid" => &["--check"][..],
        "--workspace" => &["--project", "--work-item", "--continue"][..],
        "--project" | "--work-item" | "--continue" => &["--workspace"][..],
        "--skip-ado" => &["--with-active-children", "--create-child-tasks"][..],
        "--with-active-children" | "--create-child-tasks" => &["--skip-ado"][..],
        "--database" => &["--env"][..],
        "--env" => &["--database"][..],
        "--value" => &["--from-env"][..],
        "--from-env" => &["--value"][..],
        "--from-pr" => &["--from-git"][..],
        "--from-git" => &["--from-pr"][..],
        _ => &[][..],
    };
    if conflicts.iter().any(|conflict| selected.contains(conflict)) {
        return false;
    }

    match (path, option) {
        (_, "--ready") => selected.contains(&"--create-pr"),
        (["ado", "changelog"], "--git-to") => selected.contains(&"--from-git"),
        (["ado", "changelog"], "--table") => selected.contains(&"--format"),
        _ => true,
    }
}

pub(super) fn options_for_path(path: &[&str]) -> Vec<&'static str> {
    match path {
        ["init"] => vec!["--profile", "--root", "--dry-run", "--no-save"],
        ["refresh"] => vec!["--root", "--profile"],
        ["tui"] => vec!["--root"],
        ["doctor"] => vec!["--fix"],
        ["upgrade"] => vec!["--check", "--rid"],
        ["completion", "complete"] => vec!["--format"],
        ["completion", _] => Vec::new(),
        ["auth", _] => vec!["--root"],
        ["work", subcommand] => work_options(subcommand),
        ["work", "pr", "start"] => work_options("start-pr"),
        ["work", "item", "doing"] => work_options("doing"),
        ["work", "item", "add"] => work_options("add-work-item"),
        ["work", "item", "remove"] => work_options("remove-work-item"),
        ["work", "repo", "add"] => work_options("add-repo"),
        ["work", "repo", "latest"] => work_options("repo-latest"),
        ["work", "handoff", "validate"] => work_options("handoff-validate"),
        ["work", "task", "child", "create"] => work_options("create-child-task"),
        ["ado", "item" | "state" | "context"] => Vec::new(),
        ["ado", subcommand] => ado_options(subcommand),
        ["ado", "item", "show"] => ado_options("work-item"),
        ["ado", "state", "set"] => ado_options("set-state"),
        ["ado", "context", "show"] => ado_options("context"),
        ["ado", "context", "ai"] => ado_options("ai-context"),
        ["db", subcommand] => db_options(subcommand),
        ["agent", subcommand] => agent_options(subcommand),
        ["agent", "default", "set"] => vec!["--root"],
        ["config", subcommand] => config_options(subcommand),
        ["config", "root", "set"] | ["config", "color", "set"] => Vec::new(),
        ["secret", subcommand] => secret_options(subcommand),
        _ => vec!["--help"],
    }
}

pub(super) fn subcommands_for_path(path: &[&str]) -> Option<&'static [&'static str]> {
    match path {
        [] | [""] => Some(root_command_labels()),
        ["auth"] => Some(&["login", "status", "logout"]),
        ["completion"] => Some(&["show", "generate", "install"]),
        ["work"] => Some(&[
            "commit",
            "current",
            "finish",
            "handoff",
            "item",
            "list",
            "open",
            "pr",
            "preflight",
            "prune",
            "rename",
            "repo",
            "start",
            "status",
            "sync",
            "task",
            "teardown",
        ]),
        ["work", "pr"] => Some(&["start"]),
        ["work", "item"] => Some(&["add", "doing", "remove"]),
        ["work", "repo"] => Some(&["add", "latest"]),
        ["work", "handoff"] => Some(&["validate"]),
        ["work", "task"] => Some(&["child"]),
        ["work", "task", "child"] => Some(&["create"]),
        ["ado"] => Some(&["assigned", "changelog", "context", "item", "prs", "state"]),
        ["ado", "item"] => Some(&["show"]),
        ["ado", "state"] => Some(&["set"]),
        ["ado", "context"] => Some(&["ai", "show"]),
        ["db"] => Some(&["collect", "describe", "guard", "list", "query", "schema"]),
        ["agent"] => Some(&["config", "context", "default", "doctor", "open", "show"]),
        ["agent", "default"] => Some(&["set"]),
        ["config"] => Some(&["color", "doctor", "root", "show"]),
        ["config", "root"] | ["config", "color"] => Some(&["set"]),
        ["secret"] => Some(&["delete", "get", "list", "set"]),
        _ => None,
    }
}

pub(super) fn values_for_path(
    path: &[&str],
    option: &str,
    context: CompletionContext<'_>,
) -> Vec<String> {
    match (path, option) {
        (["completion", "complete"], "--format") => {
            vec!["bash".into(), "fish".into(), "json".into(), "zsh".into()]
        }
        (["init"] | ["refresh"], "--profile") => vec!["default".into()],
        (path, "--project")
            if is_work_path(path) || path.first() == Some(&"agent") || is_ado_path(path) =>
        {
            dw_config::completion::project_values(context.root)
        }
        (path, "--repo" | "--only") if is_work_path(path) || path.first() == Some(&"agent") => {
            dw_workspace::completion::repository_values(
                context.root,
                context.project,
                context.workspace,
            )
        }
        (path, "--workspace") if is_work_path(path) || path.first() == Some(&"agent") => {
            dw_workspace::completion::workspace_values(
                context.root,
                context.project,
                context.work_item,
            )
        }
        (path, "--work-item") if is_work_path(path) || path.first() == Some(&"agent") => {
            dw_workspace::completion::work_item_values(context.root, context.project)
        }
        (path, "--type") if is_work_path(path) => ["feature", "bugfix", "hotfix", "chore"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        (path, "--agent") if is_work_path(path) || path.first() == Some(&"agent") => agent_values(),
        (path, "--repo") if is_ado_path(path) => {
            dw_workspace::completion::repository_values(context.root, context.project, None)
        }
        (["ado", "changelog"], "--format") => ["raw", "markdown", "html"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        (["ado", "state", "set"], "--state") => ado_state_values(context.root),
        (["db", _], "--project") => dw_config::completion::project_values(context.root),
        (["db", _], "--database") => {
            dw_config::completion::database_values(context.root, context.project)
        }
        (["db", _], "--env") => dw_config::completion::env_values(context.root, context.project),
        (["db", _], "--max-rows") => ["50", "100", "500", "1000"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        (["secret", "set"], "--from-env") => environment_variable_values(),
        (["secret", "set"], "--value") => Vec::new(),
        (["config", _], "--root") => Vec::new(),
        _ => Vec::new(),
    }
}

pub(super) fn root_command_labels() -> &'static [&'static str] {
    &[
        "version",
        "guide",
        "doctor",
        "init",
        "refresh",
        "tui",
        "agent",
        "auth",
        "completion",
        "config",
        "ado",
        "db",
        "secret",
        "upgrade",
        "work",
    ]
}

fn work_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "doing" => vec!["--root", "--project", "--yes", "--json"],
        "start" => vec![
            "--root",
            "--project",
            "--task",
            "--type",
            "--only",
            "--slug",
            "--skip-ado",
            "--with-active-children",
            "--create-child-tasks",
            "--json",
            "--execute",
        ],
        "start-pr" => vec![
            "--root",
            "--project",
            "--repo",
            "--type",
            "--slug",
            "--json",
            "--execute",
        ],
        "open" => workspace_resolution_options(&["--repo", "--agent", "--json"]),
        "sync" => workspace_resolution_options(&["--json"]),
        "rename" => workspace_resolution_options(&["--json", "--execute"]),
        "teardown" => workspace_resolution_options(&["--execute", "--yes", "--json"]),
        "create-child-task" => workspace_resolution_options(&["--repo", "--title", "--json"]),
        "repo-latest" => vec!["--workspace", "--continue", "--only", "--root", "--json"],
        "commit" => vec![
            "--workspace",
            "--continue",
            "--root",
            "--execute",
            "--message",
            "--json",
        ],
        "add-work-item" => workspace_resolution_options(&[
            "--skip-ado",
            "--type",
            "--title",
            "--state",
            "--execute",
            "--json",
        ]),
        "remove-work-item" => workspace_resolution_options(&["--execute", "--json"]),
        "add-repo" => vec!["--workspace", "--root", "--execute", "--json"],
        "finish" => vec![
            "--workspace",
            "--continue",
            "--root",
            "--execute",
            "--yes",
            "--message",
            "--create-pr",
            "--ready",
            "--skip-verify",
            "--skip-ado",
            "--force-with-lease",
            "--json",
        ],
        "status" => vec!["--root"],
        "list" => vec!["--root", "--project", "--work-item", "--json"],
        "current" => vec!["--json"],
        "preflight" => workspace_resolution_options(&["--ai-context-file", "--json"]),
        "handoff-validate" => workspace_resolution_options(&["--json"]),
        "prune" => vec![
            "--root",
            "--project",
            "--work-item",
            "--execute",
            "--yes",
            "--no-sync",
            "--json",
        ],
        _ => Vec::new(),
    }
}

fn ado_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "changelog" => vec![
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
        "assigned" => vec![
            "--root",
            "--project",
            "--top",
            "--all",
            "--group-by-parent",
            "--json",
        ],
        "prs" => vec!["--root", "--project", "--repo", "--json"],
        "work-item" => vec!["--root", "--project", "--json"],
        "set-state" => vec![
            "--root",
            "--project",
            "--state",
            "--history",
            "--yes",
            "--json",
        ],
        "context" => vec!["--root", "--project", "--summary", "--comments", "--json"],
        "ai-context" => vec![
            "--root",
            "--organization",
            "--project",
            "--summary",
            "--comments",
            "--include-comments",
        ],
        _ => Vec::new(),
    }
}

fn db_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "list" => vec!["--root", "--json"],
        "collect" => vec!["--root", "--save", "--json"],
        "guard" => vec!["--sql"],
        "query" => vec![
            "--sql",
            "--project",
            "--database",
            "--env",
            "--max-rows",
            "--json",
        ],
        "schema" | "describe" => vec!["--project", "--database", "--env", "--json"],
        _ => Vec::new(),
    }
}

fn agent_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "open" => workspace_resolution_options(&["--repo", "--agent"]),
        "config" | "show" => vec!["--root"],
        "doctor" => vec!["--agent"],
        _ => Vec::new(),
    }
}

fn config_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "show" | "doctor" => vec!["--root", "--json"],
        _ => Vec::new(),
    }
}

fn is_work_path(path: &[&str]) -> bool {
    path.first() == Some(&"work")
}

fn is_ado_path(path: &[&str]) -> bool {
    path.first() == Some(&"ado")
}

fn secret_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "set" => vec!["--value", "--from-env"],
        "list" => vec!["--root", "--json"],
        "delete" => vec!["--yes"],
        "get" => Vec::new(),
        _ => Vec::new(),
    }
}

fn workspace_resolution_options(extra: &[&'static str]) -> Vec<&'static str> {
    let mut options = vec![
        "--workspace",
        "--root",
        "--project",
        "--work-item",
        "--continue",
    ];
    options.extend_from_slice(extra);
    options
}

fn agent_values() -> Vec<String> {
    dw_config::AGENT_DEFAULT_CHOICES
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn ado_state_values(root: &str) -> Vec<String> {
    let workflow = dw_config::load_workflow_config(root);
    let start = dw_workspace::task_start_options(&workflow);
    let finish = dw_workspace::task_finish_options(&workflow);
    let mut values = vec![
        start.user_story_state.to_string(),
        start.anomaly_state.to_string(),
        start.bug_state.to_string(),
        start.task_state.to_string(),
        finish.bug_state.to_string(),
        finish.task_state.to_string(),
        "Nouveau".into(),
        "Actif".into(),
        "En cours".into(),
        "En réalisation".into(),
        "Résolu".into(),
        "Clos".into(),
        "Fermé".into(),
    ];
    values.sort();
    values.dedup();
    values
}

fn environment_variable_values() -> Vec<String> {
    let mut values = std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
