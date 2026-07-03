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
