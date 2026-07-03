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
