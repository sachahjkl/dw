use dw_contracts::completion::{CompletionCatalog, CompletionContext};

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
        "assigned",
        "changelog",
        "work-item",
        "context",
        "ai-context",
    ]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
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
        "work-item" => vec!["--root", "--project", "--json"],
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

pub fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--root"
            | "--project"
            | "--repo"
            | "--format"
            | "--git-to"
            | "--top"
            | "--comments"
            | "--organization"
    )
}

pub fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--from-pr" => &["--from-git"][..],
        "--from-git" => &["--from-pr"][..],
        _ => &[][..],
    };
    if conflicts.iter().any(|conflict| selected.contains(conflict)) {
        return false;
    }
    match option {
        "--git-to" => selected.contains(&"--from-git"),
        "--table" => selected.contains(&"--format"),
        _ => true,
    }
}

pub fn values_for(option: &str, root: &str) -> Option<Vec<String>> {
    match option {
        "--project" => Some(dw_config::completion::project_values(root)),
        "--format" => Some(vec!["raw".into(), "markdown".into(), "html".into()]),
        _ => None,
    }
}

fn values_for_catalog(option: &str, context: CompletionContext<'_>) -> Option<Vec<String>> {
    values_for(option, context.root)
}
