pub fn subcommands() -> &'static [&'static str] {
    &["show", "set-root", "set-color", "doctor"]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "show" | "doctor" => vec!["--root", "--json"],
        "set-root" | "set-color" => Vec::new(),
        _ => Vec::new(),
    }
}
