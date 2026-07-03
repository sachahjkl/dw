use dw_contracts::completion::{CompletionCatalog, CompletionContext};
use dw_workspace::completion::{
    repository_values, work_item_values, workspace_resolution_options, workspace_values,
};

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
        "open" => workspace_resolution_options(&["--repo", "--agent"]),
        "config" | "show" => vec!["--root"],
        "set-default" => vec!["--root"],
        "doctor" => vec!["--agent"],
        _ => Vec::new(),
    }
}

pub fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--root" | "--project" | "--work-item" | "--workspace" | "--repo" | "--agent"
    )
}

pub fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--workspace" => &["--project", "--work-item", "--continue"][..],
        "--project" | "--work-item" | "--continue" => &["--workspace"][..],
        _ => &[][..],
    };
    !conflicts.iter().any(|conflict| selected.contains(conflict))
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
        "--repo" => Some(repository_values(root, project, workspace)),
        "--workspace" => Some(workspace_values(root, project, work_item)),
        "--work-item" => Some(work_item_values(root, project)),
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

fn values_for_catalog(option: &str, context: CompletionContext<'_>) -> Option<Vec<String>> {
    values_for(
        option,
        context.root,
        context.project,
        context.workspace,
        context.work_item,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_catalog_exposes_workspace_resolution_options() {
        let options = options_for("open");

        assert!(options.contains(&"--workspace"));
        assert!(options.contains(&"--project"));
        assert!(options.contains(&"--work-item"));
        assert!(options.contains(&"--continue"));
        assert!(options.contains(&"--repo"));
        assert!(options.contains(&"--agent"));
    }

    #[test]
    fn open_catalog_filters_conflicting_workspace_options() {
        assert!(!option_allowed("--project", &["--workspace"]));
        assert!(!option_allowed("--work-item", &["--workspace"]));
        assert!(!option_allowed("--continue", &["--workspace"]));
        assert!(!option_allowed("--workspace", &["--project"]));
        assert!(option_allowed("--repo", &["--workspace"]));
    }

    #[test]
    fn open_catalog_suggests_agent_values() {
        let values = values_for("--agent", "/tmp/dw", None, None, None).expect("agent values");

        assert_eq!(
            values,
            vec!["opencode", "cursor", "claude", "codex", "copilot"]
        );
    }
}
