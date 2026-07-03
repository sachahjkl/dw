use dw_contracts::completion::{CompletionCatalog, CompletionContext};

#[derive(Clone, Copy)]
struct DomainCatalog {
    root: &'static str,
    catalog: CompletionCatalog,
}

fn domain_catalogs() -> [DomainCatalog; 6] {
    [
        DomainCatalog {
            root: "task",
            catalog: dw_task::completion::catalog(),
        },
        DomainCatalog {
            root: "ado",
            catalog: dw_ado_commands::completion::catalog(),
        },
        DomainCatalog {
            root: "db",
            catalog: dw_db::completion::catalog(),
        },
        DomainCatalog {
            root: "agent",
            catalog: dw_agent::completion::catalog(),
        },
        DomainCatalog {
            root: "config",
            catalog: dw_config::completion::catalog(),
        },
        DomainCatalog {
            root: "secret",
            catalog: dw_secret::completion::catalog(),
        },
    ]
}

pub(super) fn option_requires_value(option: &str) -> bool {
    root_option_requires_value(option)
        || domain_catalogs()
            .iter()
            .any(|domain| (domain.catalog.option_requires_value)(option))
}

pub(super) fn option_allowed(path: &[&str], option: &str, selected: &[&str]) -> bool {
    command_catalog(path)
        .map(|catalog| (catalog.option_allowed)(option, selected))
        .unwrap_or_else(|| root_option_allowed(option, selected))
}

pub(super) fn options_for_path(path: &[&str]) -> Vec<&'static str> {
    match path {
        ["init"] => vec!["--profile", "--root", "--dry-run", "--no-save"],
        ["refresh"] => vec!["--root", "--profile"],
        ["doctor"] => vec!["--fix"],
        ["upgrade"] => vec!["--check", "--rid"],
        ["completion", "complete"] => vec!["--format"],
        ["completion", _] => Vec::new(),
        ["auth", _] => vec!["--root"],
        ["agent", "open"] => dw_task::completion::agent_open_options(),
        [_, subcommand] => command_catalog(path)
            .map(|catalog| (catalog.options_for)(subcommand))
            .unwrap_or_default(),
        _ => vec!["--help"],
    }
}

pub(super) fn subcommands_for_path(path: &[&str]) -> Option<&'static [&'static str]> {
    match path {
        [] | [""] => Some(root_command_labels()),
        ["auth"] => Some(&["login", "status", "logout"]),
        ["completion"] => Some(&["show", "generate", "install"]),
        [root] => root_catalog(root).map(|catalog| (catalog.subcommands)()),
        _ => None,
    }
}

pub(super) fn values_for_path(
    path: &[&str],
    option: &str,
    context: CompletionContext<'_>,
) -> Vec<String> {
    match path {
        ["agent", "open"] => {
            (dw_task::completion::catalog().values_for)(option, context).unwrap_or_default()
        }
        ["completion", "complete"] if option == "--format" => vec!["bash".into(), "json".into()],
        ["init"] | ["refresh"] if option == "--profile" => {
            vec!["business".into(), "default".into()]
        }
        _ => command_catalog(path)
            .and_then(|catalog| (catalog.values_for)(option, context))
            .unwrap_or_default(),
    }
}

pub(super) fn root_command_labels() -> &'static [&'static str] {
    &[
        "version",
        "guide",
        "doctor",
        "init",
        "refresh",
        "agent",
        "auth",
        "completion",
        "config",
        "ado",
        "db",
        "secret",
        "upgrade",
        "task",
    ]
}

fn root_option_requires_value(option: &str) -> bool {
    matches!(option, "--root" | "--profile" | "--rid" | "--format")
}

fn root_option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--check" => &["--rid"][..],
        "--rid" => &["--check"][..],
        _ => &[][..],
    };
    !conflicts.iter().any(|conflict| selected.contains(conflict))
}

fn command_catalog(path: &[&str]) -> Option<CompletionCatalog> {
    match path {
        ["agent", "open"] => Some(dw_task::completion::catalog()),
        [root, _] => root_catalog(root),
        _ => None,
    }
}

fn root_catalog(root: &str) -> Option<CompletionCatalog> {
    domain_catalogs()
        .into_iter()
        .find(|domain| domain.root == root)
        .map(|domain| domain.catalog)
}
