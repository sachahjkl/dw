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
        ["task", subcommand] => task_options(subcommand),
        ["ado", subcommand] => ado_options(subcommand),
        ["db", subcommand] => db_options(subcommand),
        ["agent", subcommand] => agent_options(subcommand),
        ["config", subcommand] => config_options(subcommand),
        ["secret", subcommand] => secret_options(subcommand),
        _ => vec!["--help"],
    }
}

pub(super) fn subcommands_for_path(path: &[&str]) -> Option<&'static [&'static str]> {
    match path {
        [] | [""] => Some(root_command_labels()),
        ["auth"] => Some(&["login", "status", "logout"]),
        ["completion"] => Some(&["show", "generate", "install"]),
        ["task"] => Some(&[
            "start",
            "start-pr",
            "status",
            "list",
            "current",
            "sync",
            "preflight",
            "handoff-validate",
            "prune",
            "rename",
            "open",
            "teardown",
            "add-repo",
            "create-child-task",
            "repo-latest",
            "add-work-item",
            "remove-work-item",
            "commit",
            "finish",
        ]),
        ["ado"] => Some(&[
            "assigned",
            "prs",
            "changelog",
            "work-item",
            "set-state",
            "context",
            "ai-context",
        ]),
        ["db"] => Some(&["schema", "describe", "query", "guard"]),
        ["agent"] => Some(&["context", "open", "config", "show", "set-default", "doctor"]),
        ["config"] => Some(&["show", "set-root", "set-color", "doctor"]),
        ["secret"] => Some(&["set", "get", "delete"]),
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
        (["init"] | ["refresh"], "--profile") => vec!["business".into(), "default".into()],
        (["task", _] | ["agent", _], "--project") | (["ado", _], "--project") => {
            dw_config::completion::project_values(context.root)
        }
        (["task", _] | ["agent", _], "--repo" | "--only") => {
            dw_workspace::completion::repository_values(
                context.root,
                context.project,
                context.workspace,
            )
        }
        (["task", _] | ["agent", _], "--workspace") => dw_workspace::completion::workspace_values(
            context.root,
            context.project,
            context.work_item,
        ),
        (["task", _] | ["agent", _], "--work-item") => {
            dw_workspace::completion::work_item_values(context.root, context.project)
        }
        (["task", _], "--type") => ["feature", "bugfix", "hotfix", "chore"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        (["task", _] | ["agent", _], "--agent") => agent_values(),
        (["ado", _], "--repo") => {
            dw_workspace::completion::repository_values(context.root, context.project, None)
        }
        (["ado", "changelog"], "--format") => ["raw", "markdown", "html"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        (["ado", "set-state"], "--state") => ado_state_values(context.root),
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
        "task",
    ]
}

fn task_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
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
        "config" | "show" | "set-default" => vec!["--root"],
        "doctor" => vec!["--agent"],
        _ => Vec::new(),
    }
}

fn config_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "show" | "doctor" => vec!["--root", "--json"],
        "set-root" | "set-color" => Vec::new(),
        _ => Vec::new(),
    }
}

fn secret_options(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "set" => vec!["--value", "--from-env"],
        "get" | "delete" => Vec::new(),
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
        .map(|value| (*value).to_string())
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
