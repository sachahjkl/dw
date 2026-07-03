use dw_contracts::completion::{CompletionCatalog, CompletionContext};
use dw_workspace::completion::{
    repository_values, work_item_values, workspace_resolution_options, workspace_values,
};

pub fn catalog() -> CompletionCatalog {
    CompletionCatalog {
        subcommands,
        options_for,
        option_requires_value,
        option_allowed,
        values_for: values_for_catalog,
    }
}

pub fn subcommands() -> &'static [&'static str] {
    &[
        "start",
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
    ]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
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

pub fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--root"
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
    )
}

pub fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--workspace" => &["--project", "--work-item", "--continue"][..],
        "--project" | "--work-item" | "--continue" => &["--workspace"][..],
        "--skip-ado" => &["--with-active-children", "--create-child-tasks"][..],
        "--with-active-children" => &["--skip-ado"][..],
        "--create-child-tasks" => &["--skip-ado"][..],
        _ => &[][..],
    };
    if conflicts.iter().any(|conflict| selected.contains(conflict)) {
        return false;
    }
    match option {
        "--ready" => selected.contains(&"--create-pr"),
        _ => true,
    }
}

pub fn values_for(
    option: &str,
    root: &str,
    project: Option<&str>,
    workspace: Option<&str>,
    work_item: Option<&str>,
) -> Option<Vec<String>> {
    match option {
        "--project" => Some(dw_config::completion::project_values(root)),
        "--repo" | "--only" => Some(repository_values(root, project, workspace)),
        "--workspace" => Some(workspace_values(root, project, work_item)),
        "--work-item" => Some(work_item_values(root, project)),
        "--type" => Some(vec![
            "feature".into(),
            "bugfix".into(),
            "hotfix".into(),
            "chore".into(),
        ]),
        "--agent" => Some(vec![
            "opencode".into(),
            "cursor".into(),
            "claude".into(),
            "codex".into(),
            "copilot".into(),
        ]),
        _ => None,
    }
}

fn values_for_catalog(option: &str, context: CompletionContext<'_>) -> Option<Vec<String>> {
    values_for(
        option,
        context.root,
        context.project,
        context.workspace,
        context.work_item,
    )
}

pub fn agent_open_options() -> Vec<&'static str> {
    workspace_resolution_options(&["--repo", "--agent"])
}
