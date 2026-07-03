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
    &["context", "open", "config", "show", "set-default", "doctor"]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "config" | "show" => vec!["--root"],
        "set-default" => vec!["--root"],
        "doctor" => vec!["--agent"],
        _ => Vec::new(),
    }
}

pub fn option_requires_value(option: &str) -> bool {
    matches!(option, "--root" | "--agent")
}

pub fn option_allowed(_option: &str, _selected: &[&str]) -> bool {
    true
}

pub fn values_for(option: &str) -> Option<Vec<String>> {
    match option {
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

fn values_for_catalog(option: &str, _context: CompletionContext<'_>) -> Option<Vec<String>> {
    values_for(option)
}
