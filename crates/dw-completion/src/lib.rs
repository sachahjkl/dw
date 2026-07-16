use anyhow::Result;
use clap::ValueEnum;
use clap_complete::{Shell, generate};

use dw_config::resolve_root;
use dw_contracts::completion::CompletionContext;
use dw_ui::TerminalTheme;

mod catalog;

use catalog::{
    option_allowed, option_requires_value, options_for_path, root_command_labels,
    subcommands_for_path, values_for_path as catalog_values_for_path,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CompletionOutput {
    Bash,
    Fish,
    Json,
    Zsh,
}

pub fn generate_completion(shell: Shell, command: &mut clap::Command) {
    generate(shell, command, "dw", &mut std::io::stdout());
}

pub fn print_completion_show() {
    println!("{}", render_completion_show(&TerminalTheme::stdout_auto()));
}

fn render_completion_show(theme: &TerminalTheme) -> String {
    [
        theme.command("Shell completion"),
        "Install the integration for your shell:".into(),
        String::new(),
        completion_install_line("bash", "dw completion install bash >> ~/.bashrc", theme),
        completion_install_line("zsh", "dw completion install zsh >> ~/.zshrc", theme),
        completion_install_line(
            "fish",
            "dw completion install fish > ~/.config/fish/completions/dw.fish",
            theme,
        ),
        completion_install_line(
            "powershell",
            "dw completion install powershell >> $PROFILE",
            theme,
        ),
    ]
    .join("\n")
}

fn completion_install_line(shell: &str, command: &str, theme: &TerminalTheme) -> String {
    format!("  {:<10} {}", shell, theme.command(command))
}

pub fn print_completion_install(shell: Shell) {
    match shell {
        Shell::Bash => println!(
            "_dw_complete() {{ COMPREPLY=( $(COMP_LINE=\"$COMP_LINE\" dw completion complete --format bash) ); }}\ncomplete -F _dw_complete dw"
        ),
        Shell::Zsh => {
            let script = r#"#compdef dw
_dw_complete() {
  local -a rows labels descriptions
  rows=("${(@f)$(dw completion complete --format zsh -- $words[2,-1])}")
  local row label description
  for row in $rows; do
    label=${row%%$'\t'*}
    if [[ "$row" == *$'\t'* ]]; then
      description=${row#*$'\t'}
    else
      description=""
    fi
    labels+=("$label")
    descriptions+=("$description")
  done
  compadd -d descriptions -a labels
}
compdef _dw_complete dw"#;
            println!("{script}");
        }
        Shell::Fish => println!("{}", fish_install_script()),
        Shell::PowerShell => println!(
            "Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock {{ param($wordToComplete, $commandAst, $cursorPosition) dw completion complete --format json -- @($commandAst.CommandElements | Select-Object -Skip 1 | ForEach-Object {{ $_.Extent.Text }}) | ConvertFrom-Json | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_.label, $_.label, 'ParameterValue', $_.description) }} }}"
        ),
        Shell::Elvish => {
            println!("Use `dw completion generate elvish` to generate completion for this shell.")
        }
        _ => {
            println!("Use `dw completion generate {shell}` to generate completion for this shell.")
        }
    }
}

fn fish_install_script() -> &'static str {
    "complete -c dw -f -a '(dw completion complete --format fish -- (commandline -opc)[2..-1])'"
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
        CompletionOutput::Fish | CompletionOutput::Zsh => {
            for item in suggestions {
                println!("{}", rich_shell_row(&item));
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

fn rich_shell_row(item: &CompletionItem) -> String {
    if item.description.trim().is_empty() {
        item.label.clone()
    } else {
        format!("{}\t{}", item.label, item.description)
    }
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

    if let Some(option) = option_waiting_for_value(words) {
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

    if let Some(items) = complete_positional_value(words, current) {
        return items;
    }

    complete_subcommands(words)
}

fn option_waiting_for_value(words: &[String]) -> Option<&str> {
    let current = words.last()?.as_str();
    if current.starts_with("--") {
        return option_requires_value(current).then_some(current);
    }

    let previous = words
        .get(words.len().saturating_sub(2))
        .map(String::as_str)?;
    option_requires_value(previous).then_some(previous)
}

fn complete_option_value(option: &str, words: &[String], current: &str) -> Vec<CompletionItem> {
    let root = option_value(words, "--root").unwrap_or_else(|| resolve_root(None));
    let path = command_path(words);
    let values = values_for_path(&path, option, words, &root);
    values
        .into_iter()
        .filter(|value| value.starts_with(current))
        .map(|label| CompletionItem {
            description: value_description(option, &label),
            label,
        })
        .collect()
}

fn values_for_path(path: &[&str], option: &str, words: &[String], root: &str) -> Vec<String> {
    let project = option_value(words, "--project");
    let workspace = option_value(words, "--workspace");
    let work_item = option_value(words, "--work-item");
    catalog_values_for_path(
        path,
        option,
        CompletionContext {
            root,
            project: project.as_deref(),
            workspace: workspace.as_deref(),
            work_item: work_item.as_deref(),
        },
    )
}

fn complete_positional_value(words: &[String], current: &str) -> Option<Vec<CompletionItem>> {
    let path = command_path(words);
    let root = option_value(words, "--root").unwrap_or_else(|| resolve_root(None));
    let values = positional_values_for_path(&path, &root)?;
    let positionals = positional_words_after_path(words);
    let current_is_positional = !current.starts_with('-') && path.len() >= 2;
    let completed_positionals = if current_is_positional {
        positionals.len().saturating_sub(1)
    } else {
        positionals.len()
    };
    if completed_positionals > 0 {
        return Some(Vec::new());
    }

    Some(
        values
            .into_iter()
            .filter(|value| value.starts_with(current))
            .map(|label| CompletionItem {
                description: positional_value_description(&path, &label),
                label,
            })
            .collect(),
    )
}

fn positional_values_for_path(path: &[&str], root: &str) -> Option<Vec<String>> {
    match path {
        ["completion", "generate"] | ["completion", "install"] => Some(completion_shell_values()),
        ["config", "color", "set"] => Some(
            dw_config::COLOR_MODE_CHOICES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        ),
        ["agent", "default", "set"] => Some(
            dw_config::AGENT_DEFAULT_CHOICES
                .iter()
                .map(ToString::to_string)
                .collect(),
        ),
        ["secret", "set"] | ["secret", "get"] | ["secret", "delete"] => {
            Some(dw_config::completion::secret_key_values(root))
        }
        _ => None,
    }
}

fn completion_shell_values() -> Vec<String> {
    ["bash", "fish", "zsh", "powershell", "elvish"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn positional_words_after_path(words: &[String]) -> Vec<&str> {
    let mut remaining_path_words = command_path(words).len();
    let mut index = 0usize;
    let mut positionals = Vec::new();
    while index < words.len() {
        let word = words[index].as_str();
        if word == "--" {
            index += 1;
            continue;
        }
        if word.starts_with("--") {
            if option_requires_value(word)
                && words
                    .get(index + 1)
                    .is_some_and(|value| !value.starts_with('-'))
            {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if remaining_path_words > 0 {
            remaining_path_words -= 1;
        } else {
            positionals.push(word);
        }
        index += 1;
    }
    positionals
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
        .filter(|word| word.starts_with("--") && word.as_str() != "--")
        .map(String::as_str)
        .collect()
}

fn complete_options(words: &[String]) -> Vec<CompletionItem> {
    let path = command_path(words);
    let selected = selected_options(words);
    options_for_path(&path)
        .into_iter()
        .filter(|option| !selected.contains(option))
        .filter(|option| option_allowed(&path, option, &selected))
        .map(|label| CompletionItem {
            label: label.into(),
            description: option_description(label),
        })
        .collect()
}

fn command_path(words: &[String]) -> Vec<&str> {
    let mut path = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index].as_str();
        if word == "--" {
            index += 1;
            continue;
        }
        if word.starts_with("--") {
            if option_requires_value(word)
                && words
                    .get(index + 1)
                    .is_some_and(|value| !value.starts_with('-'))
            {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if word.is_empty() {
            break;
        }
        let Some(subcommands) = subcommands_for_path(&path) else {
            break;
        };
        if subcommands.contains(&word) || (path == ["completion"] && word == "complete") {
            path.push(word);
        } else if index == words.len() - 1 {
            path.push(word);
            break;
        } else {
            break;
        }
        index += 1;
    }
    path
}

fn complete_subcommands(words: &[String]) -> Vec<CompletionItem> {
    let current = words.last().map(String::as_str).unwrap_or_default();
    let path = command_path(words);
    let (lookup_path, prefix) =
        subcommand_lookup_path_and_prefix(&path, current).unwrap_or((&path[..], ""));
    subcommands_for_path(lookup_path)
        .unwrap_or_default()
        .iter()
        .copied()
        .filter(|label| label.starts_with(prefix))
        .map(|label| CompletionItem {
            label: label.into(),
            description: subcommand_description(lookup_path, label),
        })
        .collect()
}

fn subcommand_lookup_path_and_prefix<'a>(
    path: &'a [&'a str],
    current: &'a str,
) -> Option<(&'a [&'a str], &'a str)> {
    if current.is_empty() || current.starts_with('-') {
        return None;
    }
    if subcommands_for_path(path).is_some() {
        return None;
    }
    if path.last().is_some_and(|last| *last == current) {
        return Some((&path[..path.len().saturating_sub(1)], current));
    }
    None
}

fn root_commands() -> Vec<CompletionItem> {
    root_command_labels()
        .iter()
        .copied()
        .map(|label| CompletionItem {
            label: label.into(),
            description: root_command_description(label),
        })
        .collect()
}

fn option_description(option: &str) -> String {
    match option {
        "--root" => "DevWorkflow root to use".into(),
        "--project" => "Configured project".into(),
        "--work-item" => "Work item used to resolve the workspace".into(),
        "--workspace" => "Existing task workspace".into(),
        "--repo" | "--only" => "Configured repository".into(),
        "--database" => "Database connection declared in databases.json".into(),
        "--env" => "Database environment alias".into(),
        "--json" => "Deterministic JSON output".into(),
        "--execute" => "Apply the action for real".into(),
        "--yes" => "Confirm without interactive prompt".into(),
        "--dry-run" => "Preview without writing".into(),
        "--no-save" => "Do not save the default root".into(),
        "--save" => "Save eligible collected connections securely".into(),
        "--fix" => "Apply automatic fixes".into(),
        "--format" => "Output format".into(),
        "--agent" => "AI agent to launch".into(),
        "--max-rows" => "Maximum number of rows".into(),
        "--sql" => "Read-only SQL query".into(),
        "--message" => "Explicit message".into(),
        "--title" => "Explicit title".into(),
        "--task" => "Child task to include".into(),
        "--slug" => "Explicit slug".into(),
        "--value" => "Secret value".into(),
        "--from-env" => "Environment variable containing the secret".into(),
        "--state" => "ADO state or local state depending on the command".into(),
        "--history" => "ADO history message".into(),
        "--type" => "Branch/workspace type".into(),
        "--profile" => "Template profile".into(),
        "--check" => "Check without updating".into(),
        "--rid" => "Artifact runtime identifier".into(),
        "--continue" => "Resume the recent workspace".into(),
        "--skip-ado" => "Do not call Azure DevOps".into(),
        "--with-active-children" => "Include active ADO children".into(),
        "--create-child-tasks" => "Create ADO tasks per repository".into(),
        "--create-pr" => "Create or verify ADO PRs".into(),
        "--ready" => "Mark the PR as ready".into(),
        "--skip-verify" => "Skip local validations".into(),
        "--force-with-lease" => "Safely replace rewritten remote branches".into(),
        "--no-sync" => "Skip ADO synchronization".into(),
        "--ai-context-file" => "AI context file".into(),
        "--from-pr" => "Read from pull requests".into(),
        "--from-git" => "Read from git history".into(),
        "--group-by-parent" => "Group by ADO parent".into(),
        "--table" => "Table rendering".into(),
        "--ids-only" => "Show IDs only".into(),
        "--git-to" => "Ending git revision".into(),
        "--top" => "Maximum number of items".into(),
        "--all" => "Include final states".into(),
        "--summary" => "Include the ADO summary".into(),
        "--comments" => "Number of comments to include".into(),
        "--include-comments" => "Include comments".into(),
        "--organization" => "Azure DevOps organization".into(),
        "--help" => "Show help".into(),
        _ => String::new(),
    }
}

fn value_description(option: &str, value: &str) -> String {
    match option {
        "--project" => "Configured project".into(),
        "--repo" | "--only" => "Repository".into(),
        "--database" => "Database connection".into(),
        "--env" => "Database environment".into(),
        "--workspace" => "Task workspace".into(),
        "--work-item" => "Local work item".into(),
        "--agent" => "AI agent".into(),
        "--format" => "Output format".into(),
        "--from-env" => "Environment variable".into(),
        "--state" => "ADO/workflow state".into(),
        "--max-rows" => "Row limit".into(),
        "--type" => "Branch/workspace type".into(),
        "--profile" => "Profile".into(),
        _ if !value.trim().is_empty() => "Value".into(),
        _ => String::new(),
    }
}

fn positional_value_description(path: &[&str], value: &str) -> String {
    match path {
        ["config", "color", "set"] => match value {
            "auto" => "Color based on terminal".into(),
            "always" => "Force color".into(),
            "never" => "Disable color".into(),
            _ => "Color mode".into(),
        },
        ["agent", "default", "set"] => "Default AI agent".into(),
        ["completion", "generate"] => "Target shell for static completion".into(),
        ["completion", "install"] => "Target shell for dynamic snippet".into(),
        ["secret", "set"] | ["secret", "get"] | ["secret", "delete"] => {
            "Secret key declared in databases.json".into()
        }
        _ if !value.trim().is_empty() => "Value".into(),
        _ => String::new(),
    }
}

fn root_command_description(label: &str) -> String {
    match label {
        "version" => "Show version".into(),
        "guide" => "Getting started guide".into(),
        "doctor" => "Machine and configuration diagnostics".into(),
        "init" => "Initialize a DevWorkflow root".into(),
        "refresh" => "Regenerate schemas and agent contexts".into(),
        "tui" => "Interactive dashboard".into(),
        "agent" => "Open/configure AI agents".into(),
        "auth" => "Azure DevOps login".into(),
        "completion" => "Install/query shell completion".into(),
        "config" => "Read/modify configuration".into(),
        "ado" => "Azure DevOps commands".into(),
        "db" => "Read-only database access".into(),
        "secret" => "Local secrets".into(),
        "upgrade" => "Update the binary".into(),
        "work" => "Workspace, repository, PR, and work-item lifecycle".into(),
        _ => String::new(),
    }
}

fn subcommand_description(path: &[&str], label: &str) -> String {
    match (path, label) {
        (["work"], "start") => "Create/prepare a workspace".into(),
        (["work"], "status") => "Workspace summary view".into(),
        (["work"], "list") => "List filterable workspaces".into(),
        (["work"], "current") => "Show the current workspace".into(),
        (["work"], "sync") => "Synchronize task.json with ADO".into(),
        (["work"], "preflight") => "Check blockers before development".into(),
        (["work"], "rename") => "Rename workspace and branch".into(),
        (["work"], "open") => "Open the workspace with an agent".into(),
        (["work"], "commit") => "Prepare or create commits".into(),
        (["work"], "finish") => "Commit, push, PR, and ADO state".into(),
        (["work"], "prune") => "Clean up finished workspaces".into(),
        (["work"], "teardown") => "Delete a workspace".into(),
        (["work"], "pr") => "Pull-request-based workspaces".into(),
        (["work", "pr"], "start") => "Create/prepare a workspace from a PR".into(),
        (["work"], "item") => "Workspace work items".into(),
        (["work", "item"], "doing") => {
            "Move work items to their configured in-progress state".into()
        }
        (["work", "item"], "add") => "Add work items to the workspace".into(),
        (["work", "item"], "remove") => "Remove work items from the workspace".into(),
        (["work"], "repo") => "Workspace repositories".into(),
        (["work", "repo"], "add") => "Add a repository to the workspace".into(),
        (["work", "repo"], "latest") => "Update repositories".into(),
        (["work"], "handoff") => "Workspace handoffs".into(),
        (["work", "handoff"], "validate") => "Validate workspace handoff".into(),
        (["work"], "task") => "ADO tasks linked to work".into(),
        (["work", "task"], "child") => "Child tasks".into(),
        (["work", "task", "child"], "create") => "Create an ADO child task".into(),
        (["auth"], "login") => "Interactive Azure DevOps login".into(),
        (["auth"], "status") => "Azure DevOps connection state".into(),
        (["auth"], "logout") => "Delete the local ADO session".into(),
        (["ado"], "assigned") => "Assigned work items".into(),
        (["ado"], "prs") => "Active pull requests".into(),
        (["ado"], "changelog") => "Changelog from PRs/git/work items".into(),
        (["ado"], "item") => "Work item operations".into(),
        (["ado", "item"], "show") => "Work item details".into(),
        (["ado"], "state") => "Work item state operations".into(),
        (["ado", "state"], "set") => "Change ADO state".into(),
        (["ado"], "context") => "Work item context".into(),
        (["ado", "context"], "show") => "Detailed human-readable context".into(),
        (["ado", "context"], "ai") => "Structured context for AI".into(),
        (["db"], "list") => "List configured databases safely".into(),
        (["db"], "collect") => "Discover workspace database connections".into(),
        (["db"], "schema") => "List tables and views".into(),
        (["db"], "describe") => "Describe a table".into(),
        (["db"], "query") => "Run a read-only query".into(),
        (["db"], "guard") => "Check a query without running it".into(),
        (["agent"], "context") => "Show agent context".into(),
        (["agent"], "open") => "Open an agent on a workspace".into(),
        (["agent"], "config") => "Show agent configuration".into(),
        (["agent"], "show") => "Agent configuration alias".into(),
        (["agent"], "default") => "Default agent operations".into(),
        (["agent", "default"], "set") => "Set the default agent".into(),
        (["agent"], "doctor") => "Diagnose installed agents".into(),
        (["completion"], "show") => "Show installation commands".into(),
        (["completion"], "generate") => "Generate static completion".into(),
        (["completion"], "install") => "Show the dynamic shell snippet".into(),
        (["config"], "show") => "Show effective configuration".into(),
        (["config"], "root") => "Root configuration".into(),
        (["config", "root"], "set") => "Set the DevWorkflow root".into(),
        (["config"], "color") => "Color configuration".into(),
        (["config", "color"], "set") => "Set color mode".into(),
        (["config"], "doctor") => "Validate config files".into(),
        (["secret"], "list") => "List configured secret keys safely".into(),
        (["secret"], "set") => "Save a local secret".into(),
        (["secret"], "get") => "Check whether a secret exists".into(),
        (["secret"], "delete") => "Delete a local secret".into(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finish_ready_requires_create_pr() {
        let without_create_pr = labels(complete_words(&words(&["work", "finish", "--"])));
        assert!(!without_create_pr.contains(&"--ready".into()));

        let with_create_pr = labels(complete_words(&words(&[
            "work",
            "finish",
            "--create-pr",
            "--",
        ])));
        assert!(with_create_pr.contains(&"--ready".into()));
    }

    #[test]
    fn workspace_hides_project_work_item_and_continue() {
        let values = labels(complete_words(&words(&[
            "work",
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
    fn agent_open_completion_matches_agent_open_options() {
        let values = labels(complete_words(&words(&["agent", "open", "--"])));

        assert!(values.contains(&"--workspace".into()));
        assert!(values.contains(&"--agent".into()));
        assert!(!values.contains(&"--json".into()));
    }

    #[test]
    fn completions_include_descriptions_for_powershell() {
        let root_commands = complete_words(&words(&[]));
        let task = root_commands
            .iter()
            .find(|item| item.label == "work")
            .expect("work command");
        assert_eq!(
            task.description,
            "Workspace, repository, PR, and work-item lifecycle"
        );

        let task_subcommands = complete_words(&words(&["work"]));
        let pr = task_subcommands
            .iter()
            .find(|item| item.label == "pr")
            .expect("pr command");
        assert_eq!(pr.description, "Pull-request-based workspaces");
        let pr_subcommands = complete_words(&words(&["work", "pr"]));
        let start_pr = pr_subcommands
            .iter()
            .find(|item| item.label == "start")
            .expect("pr start command");
        assert_eq!(start_pr.description, "Create/prepare a workspace from a PR");

        let options = complete_words(&words(&["work", "open", "--"]));
        let workspace = options
            .iter()
            .find(|item| item.label == "--workspace")
            .expect("workspace option");
        assert_eq!(workspace.description, "Existing task workspace");
    }

    #[test]
    fn visible_subcommands_have_descriptions() {
        for root in [
            "auth",
            "completion",
            "work",
            "ado",
            "db",
            "agent",
            "config",
            "secret",
        ] {
            let items = complete_words(&words(&[root]));
            assert!(!items.is_empty(), "{root} should expose subcommands");
            let missing = items
                .iter()
                .filter(|item| item.description.trim().is_empty())
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>();
            assert!(
                missing.is_empty(),
                "{root} missing descriptions: {missing:?}"
            );
        }
    }

    #[test]
    fn visible_options_have_descriptions() {
        let cases = [
            &["init"][..],
            &["doctor"][..],
            &["upgrade"][..],
            &["work", "start"][..],
            &["work", "finish"][..],
            &["work", "prune"][..],
            &["work", "preflight"][..],
            &["ado", "assigned"][..],
            &["ado", "changelog"][..],
            &["ado", "context", "show"][..],
            &["ado", "context", "ai"][..],
            &["db", "query"][..],
            &["secret", "set"][..],
        ];

        for case in cases {
            let mut words = case
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>();
            words.push("--".into());
            let items = complete_words(&words);
            assert!(!items.is_empty(), "{case:?} should expose options");
            let missing = items
                .iter()
                .filter(|item| item.description.trim().is_empty())
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>();
            assert!(
                missing.is_empty(),
                "{case:?} missing descriptions: {missing:?}"
            );
        }
    }

    #[test]
    fn ado_set_state_completion_has_rich_descriptions() {
        let subcommands = complete_words(&words(&["ado", "state"]));
        let set_state = subcommands
            .iter()
            .find(|item| item.label == "set")
            .expect("state set command");
        assert_eq!(set_state.description, "Change ADO state");

        let options = complete_words(&words(&["ado", "state", "set", "--"]));
        let state = options
            .iter()
            .find(|item| item.label == "--state")
            .expect("state option");
        let history = options
            .iter()
            .find(|item| item.label == "--history")
            .expect("history option");

        assert_eq!(
            state.description,
            "ADO state or local state depending on the command"
        );
        assert_eq!(history.description, "ADO history message");
    }

    #[test]
    fn db_guard_completion_only_offers_sql() {
        let values = labels(complete_words(&words(&["db", "guard", "--"])));

        assert_eq!(values, vec!["--sql"]);
    }

    #[test]
    fn config_set_commands_do_not_offer_show_options() {
        let set_root = labels(complete_words(&words(&["config", "root", "set", "--"])));
        let set_color = labels(complete_words(&words(&["config", "color", "set", "--"])));

        assert!(set_root.is_empty());
        assert!(set_color.is_empty());
    }

    #[test]
    fn config_set_color_offers_positional_modes() {
        let values = complete_words(&words(&["config", "color", "set", ""]));
        let value_labels = labels(values.clone());

        assert_eq!(value_labels, vec!["auto", "always", "never"]);
        assert_eq!(values[0].description, "Color based on terminal");

        let filtered = labels(complete_words(&words(&["config", "color", "set", "a"])));
        assert_eq!(filtered, vec!["auto", "always"]);

        let after_value = labels(complete_words(&words(&[
            "config", "color", "set", "always", "",
        ])));
        assert!(after_value.is_empty());
    }

    #[test]
    fn agent_set_default_offers_positional_agents() {
        let values = complete_words(&words(&["agent", "default", "set", ""]));
        let value_labels = labels(values.clone());

        assert_eq!(
            value_labels,
            dw_config::AGENT_DEFAULT_CHOICES
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert!(
            values
                .iter()
                .all(|item| item.description == "Default AI agent")
        );

        let filtered = labels(complete_words(&words(&[
            "agent", "default", "set", "codex",
        ])));
        assert_eq!(filtered, vec!["codex", "codex-cli"]);
    }

    #[test]
    fn completion_commands_offer_shell_positionals() {
        let install = complete_words(&words(&["completion", "install", ""]));
        assert_eq!(
            labels(install.clone()),
            vec!["bash", "fish", "zsh", "powershell", "elvish"]
        );
        assert_eq!(
            install
                .iter()
                .find(|item| item.label == "fish")
                .expect("fish shell")
                .description,
            "Target shell for dynamic snippet"
        );

        let generate = labels(complete_words(&words(&["completion", "generate", "p"])));
        assert_eq!(generate, vec!["powershell"]);
    }

    #[test]
    fn secret_commands_offer_configured_credential_keys() {
        let root = temp_root("completion-secret-keys");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/databases.json"),
            r#"{
  "globals": {
    "shared": { "provider": "sqlserver", "credentialKey": "db/shared" }
  },
  "projects": {
    "acme": {
      "databases": {
        "dev": { "provider": "sqlserver", "credentialKey": "db/acme-dev" }
      }
    }
  }
}"#,
        )
        .expect("databases config");

        let values = complete_words(&words(&[
            "secret",
            "get",
            "--root",
            root.to_str().expect("root"),
            "db/",
        ]));

        assert_eq!(labels(values.clone()), vec!["db/acme-dev", "db/shared"]);
        assert!(
            values
                .iter()
                .all(|item| item.description == "Secret key declared in databases.json")
        );
    }

    #[test]
    fn subcommand_completion_filters_current_prefix() {
        let root_values = labels(complete_words(&words(&["au"])));
        assert_eq!(root_values, vec!["auth"]);

        let auth_values = labels(complete_words(&words(&["auth", "l"])));
        assert_eq!(auth_values, vec!["login", "logout"]);

        let task_values = labels(complete_words(&words(&["work", "st"])));
        assert_eq!(task_values, vec!["start", "status"]);
    }

    #[test]
    fn completion_catalog_contains_no_legacy_command_paths() {
        let root = labels(complete_words(&words(&[])));
        assert!(root.contains(&"work".into()));
        assert!(!root.contains(&"task".into()));

        let work = labels(complete_words(&words(&["work"])));
        for legacy in [
            "start-pr",
            "add-work-item",
            "remove-work-item",
            "add-repo",
            "repo-latest",
            "handoff-validate",
            "create-child-task",
        ] {
            assert!(!work.contains(&legacy.to_string()));
        }

        let ado = labels(complete_words(&words(&["ado"])));
        for legacy in ["work-item", "set-state", "ai-context"] {
            assert!(!ado.contains(&legacy.to_string()));
        }

        let agent = labels(complete_words(&words(&["agent"])));
        assert!(!agent.contains(&"set-default".into()));
        let config = labels(complete_words(&words(&["config"])));
        assert!(!config.contains(&"set-root".into()));
        assert!(!config.contains(&"set-color".into()));
    }

    #[test]
    fn task_agent_completion_uses_canonical_agent_choices() {
        let values = labels(complete_words(&words(&["work", "open", "--agent", ""])));

        assert_eq!(
            values,
            dw_config::AGENT_DEFAULT_CHOICES
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert!(values.contains(&"codex-cli".into()));
    }

    #[test]
    fn secret_set_value_and_from_env_conflict() {
        let initial = labels(complete_words(&words(&["secret", "set", "--"])));
        assert!(initial.contains(&"--value".into()));
        assert!(initial.contains(&"--from-env".into()));

        let with_value = labels(complete_words(&words(&[
            "secret", "set", "--value", "secret", "--",
        ])));
        assert!(!with_value.contains(&"--from-env".into()));
    }

    #[test]
    fn task_start_skip_ado_and_active_children_conflict() {
        let initial = labels(complete_words(&words(&["work", "start", "--"])));
        assert!(initial.contains(&"--skip-ado".into()));
        assert!(initial.contains(&"--with-active-children".into()));
        assert!(initial.contains(&"--create-child-tasks".into()));

        let offline = labels(complete_words(&words(&[
            "work",
            "start",
            "--skip-ado",
            "--",
        ])));
        assert!(!offline.contains(&"--with-active-children".into()));
        assert!(!offline.contains(&"--create-child-tasks".into()));

        let with_children = labels(complete_words(&words(&[
            "work",
            "start",
            "--with-active-children",
            "--",
        ])));
        assert!(!with_children.contains(&"--skip-ado".into()));
    }

    #[test]
    fn top_level_command_options_are_specific() {
        let init = labels(complete_words(&words(&["init", "--"])));
        assert!(init.contains(&"--dry-run".into()));
        assert!(init.contains(&"--no-save".into()));

        let upgrade = labels(complete_words(&words(&["upgrade", "--check", "--"])));
        assert!(!upgrade.contains(&"--rid".into()));
    }

    #[test]
    fn command_path_ignores_option_values() {
        let init = labels(complete_words(&words(&["init", "--root", "/tmp/dw", "--"])));
        assert!(init.contains(&"--profile".into()));
        assert!(init.contains(&"--dry-run".into()));
        assert!(!init.contains(&"--root".into()));

        let task = labels(complete_words(&words(&[
            "work", "start", "42", "--root", "/tmp/dw", "--",
        ])));
        assert!(task.contains(&"--project".into()));
        assert!(!task.contains(&"--help".into()));
    }

    #[test]
    fn option_value_completion_only_uses_current_option_context() {
        let option_name = labels(complete_words(&words(&["work", "start", "--type"])));
        assert_eq!(option_name, vec!["feature", "bugfix", "hotfix", "chore"]);

        let option_value = labels(complete_words(&words(&["work", "start", "--type", "b"])));
        assert_eq!(option_value, vec!["bugfix"]);

        let positional_after_option_value = labels(complete_words(&words(&[
            "work", "start", "--root", "/tmp/dw", "42",
        ])));
        assert!(positional_after_option_value.is_empty());
        assert!(!positional_after_option_value.contains(&"work".into()));
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
    fn db_commands_offer_database_env_and_max_rows_where_supported() {
        assert_eq!(labels(complete_words(&words(&["db", "l"]))), vec!["list"]);
        assert_eq!(
            labels(complete_words(&words(&["db", "c"]))),
            vec!["collect"]
        );

        let collect = labels(complete_words(&words(&["db", "collect", "--"])));
        assert!(collect.contains(&"--save".into()));

        let schema = labels(complete_words(&words(&["db", "schema", "--"])));
        assert!(schema.contains(&"--database".into()));
        assert!(schema.contains(&"--env".into()));

        let describe = labels(complete_words(&words(&["db", "describe", "--"])));
        assert!(describe.contains(&"--database".into()));
        assert!(describe.contains(&"--env".into()));

        let query = labels(complete_words(&words(&["db", "query", "--"])));
        assert!(query.contains(&"--max-rows".into()));
    }

    #[test]
    fn validation_commands_offer_workspace_resolution_options() {
        let preflight = labels(complete_words(&words(&["work", "preflight", "--"])));
        assert!(preflight.contains(&"--workspace".into()));
        assert!(preflight.contains(&"--project".into()));
        assert!(preflight.contains(&"--work-item".into()));
        assert!(preflight.contains(&"--continue".into()));
        assert!(preflight.contains(&"--ai-context-file".into()));

        let handoff = labels(complete_words(&words(&[
            "work", "handoff", "validate", "--",
        ])));
        assert!(handoff.contains(&"--workspace".into()));
        assert!(handoff.contains(&"--project".into()));
        assert!(handoff.contains(&"--work-item".into()));
        assert!(handoff.contains(&"--continue".into()));
    }

    #[test]
    fn arbitrary_depth_commands_complete_subcommands_and_options() {
        let child = complete_words(&words(&["work", "task", "child", ""]));
        assert_eq!(labels(child.clone()), vec!["create"]);
        assert_eq!(child[0].description, "Create an ADO child task");

        let create = labels(complete_words(&words(&[
            "work", "task", "child", "create", "--",
        ])));
        assert!(create.contains(&"--repo".into()));
        assert!(create.contains(&"--title".into()));
        assert!(create.contains(&"--workspace".into()));
    }

    #[test]
    fn newly_exposed_flags_are_completed() {
        let finish = labels(complete_words(&words(&["work", "finish", "--"])));
        assert!(finish.contains(&"--force-with-lease".into()));

        let delete = labels(complete_words(&words(&["secret", "delete", "--"])));
        assert_eq!(delete, vec!["--yes"]);
    }

    #[test]
    fn max_rows_completion_offers_common_limits() {
        let values = labels(complete_words(&words(&["db", "query", "--max-rows", ""])));

        assert_eq!(values, vec!["50", "100", "500", "1000"]);
    }

    #[test]
    fn completion_complete_format_offers_completion_formats() {
        let visible = labels(complete_words(&words(&["completion"])));
        assert!(!visible.contains(&"complete".into()));

        let values = labels(complete_words(&words(&[
            "completion",
            "complete",
            "--format",
            "",
        ])));

        assert_eq!(values, vec!["bash", "fish", "json", "zsh"]);
    }

    #[test]
    fn rich_shell_rows_include_tab_separated_descriptions() {
        let items = complete_words(&words(&["work", "open", "--"]));
        let workspace = items
            .iter()
            .find(|item| item.label == "--workspace")
            .expect("workspace option");

        assert_eq!(
            rich_shell_row(workspace),
            "--workspace\tExisting task workspace"
        );
    }

    #[test]
    fn completion_show_renders_install_commands() {
        let report = render_completion_show(&TerminalTheme::plain());

        assert!(report.contains("Shell completion"));
        assert!(report.contains("Install the integration for your shell"));
        assert!(report.contains("bash       dw completion install bash >> ~/.bashrc"));
        assert!(report.contains("zsh        dw completion install zsh >> ~/.zshrc"));
        assert!(report.contains(
            "fish       dw completion install fish > ~/.config/fish/completions/dw.fish"
        ));
        assert!(report.contains("powershell dw completion install powershell >> $PROFILE"));
    }

    #[test]
    fn fish_install_uses_context_tokens_and_shell_filtering() {
        let script = fish_install_script();

        assert!(script.contains("(commandline -opc)[2..-1]"));
        assert!(!script.contains("read -lz tokens"));
    }

    #[test]
    fn project_values_come_from_live_config() {
        let root = temp_root("completion-projects");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"acme":{"displayName":"Acme","repositories":{}},"dw":{"displayName":"DW","repositories":{}}}}"#,
        )
        .expect("projects config");

        let values = labels(complete_words(&words(&[
            "work",
            "start",
            "--root",
            root.to_str().expect("root"),
            "--project",
        ])));
        assert_eq!(values, vec!["acme".to_string(), "dw".to_string()]);
    }

    #[test]
    fn repository_values_come_from_config_in_config_order() {
        let root = temp_root("completion-repositories");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"acme":{"displayName":"Acme","repositories":{"front":{},"back":{},"db":{}}}}}"#,
        )
        .expect("projects config");

        let task_values = labels(complete_words(&words(&[
            "work",
            "start",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--only",
            "",
        ])));
        let changelog_values = labels(complete_words(&words(&[
            "ado",
            "changelog",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--repo",
            "",
        ])));

        assert_eq!(task_values, vec!["front", "back", "db"]);
        assert_eq!(changelog_values, vec!["front", "back", "db"]);
    }

    #[test]
    fn database_values_come_from_live_config() {
        let root = temp_root("completion-databases");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"acme":{"displayName":"Acme","repositories":{}}}}"#,
        )
        .expect("projects config");
        fs::write(
            root.join("config/databases.json"),
            r#"{"globals":{"shared":{}},"projects":{"acme":{"databases":{"acme-dev":{},"acme-test":{}}}}}"#,
        )
        .expect("databases config");

        let values = labels(complete_words(&words(&[
            "db",
            "schema",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--database",
            "",
        ])));

        assert_eq!(values, vec!["acme-dev", "acme-test", "shared"]);
    }

    #[test]
    fn workspace_and_work_item_values_come_from_manifests() {
        let root = temp_root("completion-workspaces");
        let workspace = root
            .join("projects")
            .join("acme")
            .join("workspaces")
            .join("feature-42-demo");
        fs::create_dir_all(&workspace).expect("workspace dir");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"42","taskId":null,"project":"acme","type":"feature","slug":"demo","branchName":"feature/42-demo","createdAt":"2026-07-03T10:00:00Z","repositories":["front","back"],"status":"created","workItems":[{"id":"42","type":"User Story","title":"Demo","state":"Active"},{"id":"43","type":"Task","title":"Child","state":"Active"}]}"#,
        )
        .expect("manifest");

        let workspace_values = labels(complete_words(&words(&[
            "work",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--workspace",
            "",
        ])));
        let work_item_values = labels(complete_words(&words(&[
            "work",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--work-item",
            "",
        ])));
        let repository_values = labels(complete_words(&words(&[
            "work",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--workspace",
            workspace.to_str().expect("workspace"),
            "--repo",
            "",
        ])));

        assert_eq!(workspace_values, vec![workspace.display().to_string()]);
        assert_eq!(work_item_values, vec!["42", "43"]);
        assert_eq!(repository_values, vec!["front", "back"]);
    }

    #[test]
    fn dynamic_values_include_descriptions() {
        let root = temp_root("completion-value-descriptions");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"acme":{"displayName":"Acme","repositories":{"front":{}}}}}"#,
        )
        .expect("projects config");

        let items = complete_words(&words(&[
            "ado",
            "changelog",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "acme",
            "--repo",
            "",
        ]));
        let front = items
            .iter()
            .find(|item| item.label == "front")
            .expect("front repo");

        assert_eq!(front.description, "Repository");
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
