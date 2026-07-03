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
    &["schema", "describe", "query", "guard"]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
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

pub fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--sql" | "--project" | "--database" | "--env" | "--max-rows"
    )
}

pub fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--database" => &["--env"][..],
        "--env" => &["--database"][..],
        _ => &[][..],
    };
    !conflicts.iter().any(|conflict| selected.contains(conflict))
}

pub fn values_for(option: &str, root: &str, project: Option<&str>) -> Option<Vec<String>> {
    match option {
        "--project" => Some(dw_config::completion::project_values(root)),
        "--database" => Some(dw_config::completion::database_values(root, project)),
        "--env" => Some(dw_config::completion::env_values(root, project)),
        "--max-rows" => Some(vec!["50".into(), "100".into(), "500".into(), "1000".into()]),
        _ => None,
    }
}

fn values_for_catalog(option: &str, context: CompletionContext<'_>) -> Option<Vec<String>> {
    values_for(option, context.root, context.project)
}
