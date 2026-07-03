use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::{Cli, CompletionOutput};
use dw_config::{load_databases_config, load_projects_config, resolve_project, resolve_root};
use dw_workspace::{read_manifest_path, task_list};

pub fn generate_completion(shell: Shell) {
    let mut command = Cli::command();
    generate(shell, &mut command, "dw", &mut std::io::stdout());
}

pub fn print_completion_show() {
    println!("Installer l'autocompletion:");
    println!("  dw completion install bash >> ~/.bashrc");
    println!("  dw completion install zsh >> ~/.zshrc");
    println!("  dw completion install fish > ~/.config/fish/completions/dw.fish");
    println!("  dw completion install powershell >> $PROFILE");
}

pub fn print_completion_install(shell: Shell) {
    match shell {
        Shell::Bash => println!(
            "_dw_complete() {{ COMPREPLY=( $(COMP_LINE=\"$COMP_LINE\" dw completion complete --format bash) ); }}\ncomplete -F _dw_complete dw"
        ),
        Shell::Zsh => println!(
            "#compdef dw\n_dw_complete() {{ local -a values; values=($(dw completion complete --format bash -- $words[2,-1])); compadd -- $values }}\ncompdef _dw_complete dw"
        ),
        Shell::Fish => println!(
            "complete -c dw -f -a '(commandline -opc | string collect | read -lz tokens; dw completion complete --format bash -- $tokens)'"
        ),
        Shell::PowerShell => println!(
            "Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock {{ param($wordToComplete, $commandAst, $cursorPosition) dw completion complete --format json -- @($commandAst.CommandElements | Select-Object -Skip 1 | ForEach-Object {{ $_.Extent.Text }}) | ConvertFrom-Json | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_.label, $_.label, 'ParameterValue', $_.description) }} }}"
        ),
        Shell::Elvish => generate_completion(shell),
        _ => generate_completion(shell),
    }
}

pub fn print_completion_complete(format: CompletionOutput, words: Vec<String>) -> Result<()> {
    let words = if words.is_empty() {
        completion_words_from_env()
    } else {
        words
    };
    let suggestions = complete_words(&words);
    match format {
        CompletionOutput::Bash => {
            for item in suggestions {
                println!("{}", item.label);
            }
        }
        CompletionOutput::Json => {
            let values = suggestions
                .into_iter()
                .map(|item| serde_json::json!({ "label": item.label, "description": item.description }))
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string(&values)?);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CompletionItem {
    label: String,
    description: String,
}

fn completion_words_from_env() -> Vec<String> {
    let line = std::env::var("COMP_LINE").unwrap_or_default();
    line.split_whitespace()
        .skip_while(|word| *word != "dw")
        .skip(1)
        .map(str::to_string)
        .collect()
}

fn complete_words(words: &[String]) -> Vec<CompletionItem> {
    let current = words.last().map(String::as_str).unwrap_or_default();
    let option_waiting_for_value = words
        .iter()
        .rev()
        .find(|word| word.starts_with("--"))
        .map(String::as_str);

    if let Some(option) = option_waiting_for_value
        && option_requires_value(option)
        && (!current.starts_with("--") || current == option)
    {
        let prefix = if current == option { "" } else { current };
        return complete_option_value(option, words, prefix);
    }

    if current.starts_with('-') {
        return complete_options(words)
            .into_iter()
            .filter(|item| item.label.starts_with(current))
            .collect();
    }

    if words.is_empty() || command_path(words).is_empty() {
        return root_commands();
    }

    complete_subcommands(words)
}

fn option_requires_value(option: &str) -> bool {
    matches!(
        option,
        "--project"
            | "--repo"
            | "--workspace"
            | "--work-item"
            | "--only"
            | "--database"
            | "--env"
            | "--format"
            | "--type"
            | "--agent"
            | "--root"
            | "--rid"
            | "--slug"
            | "--task"
            | "--title"
            | "--message"
    )
}

fn complete_option_value(option: &str, words: &[String], current: &str) -> Vec<CompletionItem> {
    let root = option_value(words, "--root").unwrap_or_else(|| resolve_root(None));
    let values = match option {
        "--project" => project_values(&root),
        "--repo" | "--only" => {
            repository_values(&root, option_value(words, "--project").as_deref(), words)
        }
        "--workspace" => workspace_values(
            &root,
            option_value(words, "--project").as_deref(),
            option_value(words, "--work-item").as_deref(),
        ),
        "--work-item" => work_item_values(&root, option_value(words, "--project").as_deref()),
        "--database" => database_values(&root, option_value(words, "--project").as_deref()),
        "--env" => env_values(&root, option_value(words, "--project").as_deref()),
        "--format" => vec!["raw".into(), "markdown".into(), "html".into()],
        "--type" => vec![
            "feature".into(),
            "bugfix".into(),
            "hotfix".into(),
            "chore".into(),
        ],
        "--agent" => vec![
            "opencode".into(),
            "cursor".into(),
            "claude".into(),
            "codex".into(),
            "copilot".into(),
        ],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter(|value| value.starts_with(current))
        .map(|label| CompletionItem {
            label,
            description: String::new(),
        })
        .collect()
}

fn option_value(words: &[String], option: &str) -> Option<String> {
    words.windows(2).find_map(|pair| {
        if pair[0] == option && !pair[1].starts_with('-') {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}

fn selected_options(words: &[String]) -> Vec<&str> {
    words
        .iter()
        .filter(|word| word.starts_with("--"))
        .map(String::as_str)
        .collect()
}

fn complete_options(words: &[String]) -> Vec<CompletionItem> {
    let path = command_path(words);
    let selected = selected_options(words);
    options_for_path(&path)
        .into_iter()
        .filter(|option| !selected.contains(option))
        .filter(|option| option_allowed(option, &selected))
        .map(|label| CompletionItem {
            label: label.into(),
            description: String::new(),
        })
        .collect()
}

fn option_allowed(option: &str, selected: &[&str]) -> bool {
    let conflicts = match option {
        "--workspace" => &["--project", "--work-item", "--continue"][..],
        "--project" | "--work-item" | "--continue" => &["--workspace"][..],
        "--database" => &["--env"][..],
        "--env" => &["--database"][..],
        "--from-pr" => &["--from-git"][..],
        "--from-git" => &["--from-pr"][..],
        _ => &[][..],
    };
    if conflicts.iter().any(|conflict| selected.contains(conflict)) {
        return false;
    }
    match option {
        "--ready" => selected.contains(&"--create-pr"),
        "--git-to" => selected.contains(&"--from-git"),
        "--table" => selected.contains(&"--format"),
        _ => true,
    }
}

fn command_path(words: &[String]) -> Vec<&str> {
    words
        .iter()
        .filter(|word| !word.starts_with('-'))
        .take(2)
        .map(String::as_str)
        .collect()
}

fn options_for_path(path: &[&str]) -> Vec<&'static str> {
    match path {
        ["task", "start"] => vec![
            "--root",
            "--project",
            "--task",
            "--type",
            "--only",
            "--slug",
            "--skip-ado",
            "--json",
            "--execute",
        ],
        ["task", "open"] | ["agent", "open"] => vec![
            "--workspace",
            "--root",
            "--project",
            "--work-item",
            "--continue",
            "--repo",
            "--agent",
            "--json",
        ],
        ["task", "sync"]
        | ["task", "rename"]
        | ["task", "teardown"]
        | ["task", "create-child-task"] => vec![
            "--workspace",
            "--root",
            "--project",
            "--work-item",
            "--continue",
            "--repo",
            "--title",
            "--json",
            "--execute",
            "--yes",
        ],
        ["task", "repo-latest"] => vec!["--workspace", "--continue", "--only", "--root", "--json"],
        ["task", "finish"] => vec![
            "--workspace",
            "--continue",
            "--root",
            "--execute",
            "--message",
            "--create-pr",
            "--ready",
            "--skip-verify",
            "--skip-ado",
            "--json",
        ],
        ["ado", "changelog"] => vec![
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
        ["ado", _] => vec![
            "--root",
            "--project",
            "--summary",
            "--comments",
            "--include-comments",
            "--json",
        ],
        ["db", "query"] => vec!["--sql", "--project", "--database", "--env", "--json"],
        ["db", _] => vec!["--project", "--database", "--env", "--json"],
        ["config", _] => vec!["--root", "--json"],
        ["task", _] => vec![
            "--root",
            "--project",
            "--work-item",
            "--workspace",
            "--continue",
            "--execute",
            "--json",
        ],
        _ => vec!["--help"],
    }
}

fn complete_subcommands(words: &[String]) -> Vec<CompletionItem> {
    match command_path(words).as_slice() {
        ["task"] => [
            "start",
            "status",
            "list",
            "current",
            "sync",
            "preflight",
            "handoff-validate",
            "prune",
            "rename",
            "open",
            "teardown",
            "add-repo",
            "create-child-task",
            "repo-latest",
            "add-work-item",
            "remove-work-item",
            "commit",
            "finish",
        ]
        .into_iter()
        .map(simple_completion)
        .collect(),
        ["ado"] => [
            "assigned",
            "changelog",
            "work-item",
            "context",
            "ai-context",
        ]
        .into_iter()
        .map(simple_completion)
        .collect(),
        ["db"] => ["schema", "describe", "query", "guard"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        ["agent"] => ["context", "open", "config", "show", "set-default", "doctor"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        ["auth"] => ["login", "status", "logout"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        ["config"] => ["show", "set-root", "set-color", "doctor"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        ["completion"] => ["show", "generate", "install"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        ["secret"] => ["set", "get", "delete"]
            .into_iter()
            .map(simple_completion)
            .collect(),
        _ => root_commands(),
    }
}

fn root_commands() -> Vec<CompletionItem> {
    [
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
    .into_iter()
    .map(simple_completion)
    .collect()
}

fn simple_completion(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        description: String::new(),
    }
}

fn project_values(root: &str) -> Vec<String> {
    let mut values = load_projects_config(root)
        .projects
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn repository_values(root: &str, project: Option<&str>, words: &[String]) -> Vec<String> {
    let projects = load_projects_config(root);
    let mut values = project
        .and_then(|project| resolve_project(&projects, project))
        .map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| {
            projects
                .projects
                .keys()
                .filter_map(|project| resolve_project(&projects, project))
                .flat_map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
                .collect()
        });
    if let Some(workspace) = option_value(words, "--workspace")
        && let Ok(manifest) = read_manifest_path(&format!("{workspace}/task.json"))
    {
        values = manifest.repositories;
    }
    values.sort();
    values.dedup();
    values
}

fn workspace_values(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<String> {
    task_list(root, project, work_item)
        .into_iter()
        .map(|item| item.path)
        .collect()
}

fn work_item_values(root: &str, project: Option<&str>) -> Vec<String> {
    let mut values = task_list(root, project, None)
        .into_iter()
        .flat_map(|item| {
            item.display_work_items
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn database_values(root: &str, project: Option<&str>) -> Vec<String> {
    let config = load_databases_config(root);
    let mut values = config.globals.keys().cloned().collect::<Vec<_>>();
    if let Some(project) = project.and_then(|project| config.projects.get(project))
        && let Some(map) = project.as_object()
    {
        values.extend(map.keys().cloned());
    }
    values.sort();
    values.dedup();
    values
}

fn env_values(root: &str, project: Option<&str>) -> Vec<String> {
    database_values(root, project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finish_ready_requires_create_pr() {
        let without_create_pr = labels(complete_words(&words(&["task", "finish", "--"])));
        assert!(!without_create_pr.contains(&"--ready".into()));

        let with_create_pr = labels(complete_words(&words(&[
            "task",
            "finish",
            "--create-pr",
            "--",
        ])));
        assert!(with_create_pr.contains(&"--ready".into()));
    }

    #[test]
    fn workspace_hides_project_work_item_and_continue() {
        let values = labels(complete_words(&words(&[
            "task",
            "open",
            "--workspace",
            "/tmp/ws",
            "--",
        ])));
        assert!(!values.contains(&"--project".into()));
        assert!(!values.contains(&"--work-item".into()));
        assert!(!values.contains(&"--continue".into()));
    }

    #[test]
    fn changelog_table_requires_format() {
        let without_format = labels(complete_words(&words(&["ado", "changelog", "--"])));
        assert!(!without_format.contains(&"--table".into()));

        let with_format = labels(complete_words(&words(&[
            "ado",
            "changelog",
            "--format",
            "markdown",
            "--",
        ])));
        assert!(with_format.contains(&"--table".into()));
    }

    #[test]
    fn project_values_come_from_live_config() {
        let root = temp_root("completion-projects");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{}},"dw":{"displayName":"DW","repositories":{}}}}"#,
        )
        .expect("projects config");

        let values = labels(complete_words(&words(&[
            "task",
            "start",
            "--root",
            root.to_str().expect("root"),
            "--project",
        ])));
        assert_eq!(values, vec!["dw".to_string(), "ha".to_string()]);
    }

    fn words(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    fn labels(items: Vec<CompletionItem>) -> Vec<String> {
        items.into_iter().map(|item| item.label).collect()
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("dw-{name}-{suffix}"))
    }
}
