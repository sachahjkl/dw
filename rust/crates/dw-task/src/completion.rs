use dw_config::{load_projects_config, resolve_project};
use dw_workspace::{read_manifest_path, task_list};

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
        "preflight" => vec!["--workspace", "--ai-context-file", "--json"],
        "handoff-validate" => vec!["--workspace", "--json"],
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

pub fn agent_open_options() -> Vec<&'static str> {
    workspace_resolution_options(&["--repo", "--agent"])
}

pub fn repository_values(
    root: &str,
    project: Option<&str>,
    workspace: Option<&str>,
) -> Vec<String> {
    let projects = load_projects_config(root);
    let mut values = project
        .and_then(|project| resolve_project(&projects, project))
        .map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| {
            projects
                .projects
                .keys()
                .filter_map(|project| resolve_project(&projects, project))
                .flat_map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
                .collect()
        });
    if let Some(workspace) = workspace
        && let Ok(manifest) = read_manifest_path(&format!("{workspace}/task.json"))
    {
        values = manifest.repositories;
    }
    values.sort();
    values.dedup();
    values
}

pub fn workspace_values(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<String> {
    task_list(root, project, work_item)
        .into_iter()
        .map(|item| item.path)
        .collect()
}

pub fn work_item_values(root: &str, project: Option<&str>) -> Vec<String> {
    let mut values = task_list(root, project, None)
        .into_iter()
        .flat_map(|item| {
            item.display_work_items
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
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
