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
