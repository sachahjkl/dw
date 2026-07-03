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
