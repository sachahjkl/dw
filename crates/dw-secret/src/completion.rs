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
    &["set", "get", "delete"]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "set" => vec!["--value", "--from-env"],
        "get" | "delete" => Vec::new(),
        _ => Vec::new(),
    }
}

pub fn option_requires_value(option: &str) -> bool {
    matches!(option, "--value" | "--from-env")
}

pub fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--value" => &["--from-env"][..],
        "--from-env" => &["--value"][..],
        _ => &[][..],
    };
    !conflicts.iter().any(|conflict| selected.contains(conflict))
}

fn values_for_catalog(_option: &str, _context: CompletionContext<'_>) -> Option<Vec<String>> {
    None
}
