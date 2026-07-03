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
        theme.command("Autocomplétion shell"),
        "Installer l'intégration adaptée à votre shell:".into(),
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
        Shell::Fish => println!(
            "complete -c dw -f -a '(commandline -opc | string collect | read -lz tokens; dw completion complete --format fish -- $tokens)'"
        ),
        Shell::PowerShell => println!(
            "Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock {{ param($wordToComplete, $commandAst, $cursorPosition) dw completion complete --format json -- @($commandAst.CommandElements | Select-Object -Skip 1 | ForEach-Object {{ $_.Extent.Text }}) | ConvertFrom-Json | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_.label, $_.label, 'ParameterValue', $_.description) }} }}"
        ),
        Shell::Elvish => {
            println!("Utiliser `dw completion generate elvish` pour générer ce shell.")
        }
        _ => println!("Utiliser `dw completion generate {shell}` pour générer ce shell."),
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
    while index < words.len() && path.len() < 2 {
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
        path.push(word);
        index += 1;
    }
    path
}

fn complete_subcommands(words: &[String]) -> Vec<CompletionItem> {
    let path = command_path(words);
    subcommands_for_path(&command_path(words))
        .unwrap_or_default()
        .iter()
        .copied()
        .map(|label| CompletionItem {
            label: label.into(),
            description: subcommand_description(&path, label),
        })
        .collect()
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
        "--root" => "Root DevWorkflow à utiliser".into(),
        "--project" => "Projet configuré".into(),
        "--work-item" => "Work item pour résoudre le workspace".into(),
        "--workspace" => "Workspace task existant".into(),
        "--repo" | "--only" => "Repository configuré".into(),
        "--database" => "Connexion base déclarée dans databases.json".into(),
        "--env" => "Alias d'environnement base".into(),
        "--json" => "Sortie JSON déterministe".into(),
        "--execute" => "Appliquer réellement l'action".into(),
        "--yes" => "Confirmer sans prompt interactif".into(),
        "--format" => "Format de sortie".into(),
        "--agent" => "Agent IA à lancer".into(),
        "--max-rows" => "Nombre maximum de lignes".into(),
        "--sql" => "Requête SQL read-only".into(),
        "--message" => "Message explicite".into(),
        "--title" => "Titre explicite".into(),
        "--state" => "État local".into(),
        "--type" => "Type de branche/workspace".into(),
        "--profile" => "Profil de templates".into(),
        "--check" => "Vérifier sans mettre à jour".into(),
        "--rid" => "Runtime identifier de l'artefact".into(),
        "--continue" => "Reprendre le workspace récent".into(),
        "--help" => "Afficher l'aide".into(),
        _ => String::new(),
    }
}

fn value_description(option: &str, value: &str) -> String {
    match option {
        "--project" => "Projet configuré".into(),
        "--repo" | "--only" => "Repository".into(),
        "--database" => "Connexion base".into(),
        "--env" => "Environnement base".into(),
        "--workspace" => "Workspace task".into(),
        "--work-item" => "Work item local".into(),
        "--agent" => "Agent IA".into(),
        "--format" => "Format de sortie".into(),
        "--max-rows" => "Limite de lignes".into(),
        "--type" => "Type de branche/workspace".into(),
        "--profile" => "Profil".into(),
        _ if !value.trim().is_empty() => "Valeur".into(),
        _ => String::new(),
    }
}

fn root_command_description(label: &str) -> String {
    match label {
        "version" => "Afficher la version".into(),
        "guide" => "Parcours de démarrage".into(),
        "doctor" => "Diagnostic machine et configuration".into(),
        "init" => "Initialiser un root DevWorkflow".into(),
        "refresh" => "Régénérer schémas et contextes agents".into(),
        "agent" => "Ouvrir/configurer les agents IA".into(),
        "auth" => "Connexion Azure DevOps".into(),
        "completion" => "Installer/interroger l'autocomplétion".into(),
        "config" => "Lire/modifier la configuration".into(),
        "ado" => "Commandes Azure DevOps".into(),
        "db" => "Accès base read-only".into(),
        "secret" => "Secrets locaux".into(),
        "upgrade" => "Mettre à jour le binaire".into(),
        "task" => "Cycle workspace/worktrees/PR".into(),
        _ => String::new(),
    }
}

fn subcommand_description(path: &[&str], label: &str) -> String {
    match (path, label) {
        (["task"], "start") => "Créer/préparer un workspace task".into(),
        (["task"], "finish") => "Commit, push, PR et état ADO".into(),
        (["task"], "prune") => "Nettoyer les workspaces terminés".into(),
        (["task"], "teardown") => "Supprimer un workspace".into(),
        (["ado"], "assigned") => "Work items assignés".into(),
        (["ado"], "changelog") => "Changelog depuis PR/git/work items".into(),
        (["db"], "schema") => "Lister tables et vues".into(),
        (["db"], "describe") => "Décrire une table".into(),
        (["db"], "query") => "Exécuter une requête read-only".into(),
        (["agent"], "open") => "Ouvrir un agent sur un workspace".into(),
        (["completion"], "show") => "Afficher les commandes d'installation".into(),
        (["completion"], "generate") => "Générer une completion statique".into(),
        (["completion"], "install") => "Afficher le snippet shell dynamique".into(),
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
            .find(|item| item.label == "task")
            .expect("task command");
        assert_eq!(task.description, "Cycle workspace/worktrees/PR");

        let options = complete_words(&words(&["task", "open", "--"]));
        let workspace = options
            .iter()
            .find(|item| item.label == "--workspace")
            .expect("workspace option");
        assert_eq!(workspace.description, "Workspace task existant");
    }

    #[test]
    fn db_guard_completion_only_offers_sql() {
        let values = labels(complete_words(&words(&["db", "guard", "--"])));

        assert_eq!(values, vec!["--sql"]);
    }

    #[test]
    fn config_set_commands_do_not_offer_show_options() {
        let set_root = labels(complete_words(&words(&["config", "set-root", "--"])));
        let set_color = labels(complete_words(&words(&["config", "set-color", "--"])));

        assert!(set_root.is_empty());
        assert!(set_color.is_empty());
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
        let initial = labels(complete_words(&words(&["task", "start", "--"])));
        assert!(initial.contains(&"--skip-ado".into()));
        assert!(initial.contains(&"--with-active-children".into()));
        assert!(initial.contains(&"--create-child-tasks".into()));

        let offline = labels(complete_words(&words(&[
            "task",
            "start",
            "--skip-ado",
            "--",
        ])));
        assert!(!offline.contains(&"--with-active-children".into()));
        assert!(!offline.contains(&"--create-child-tasks".into()));

        let with_children = labels(complete_words(&words(&[
            "task",
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
            "task", "start", "42", "--root", "/tmp/dw", "--",
        ])));
        assert!(task.contains(&"--project".into()));
        assert!(!task.contains(&"--help".into()));
    }

    #[test]
    fn option_value_completion_only_uses_current_option_context() {
        let option_name = labels(complete_words(&words(&["task", "start", "--type"])));
        assert_eq!(option_name, vec!["feature", "bugfix", "hotfix", "chore"]);

        let option_value = labels(complete_words(&words(&["task", "start", "--type", "b"])));
        assert_eq!(option_value, vec!["bugfix"]);

        let positional_after_option_value = labels(complete_words(&words(&[
            "task", "start", "--root", "/tmp/dw", "42",
        ])));
        assert!(positional_after_option_value.is_empty());
        assert!(!positional_after_option_value.contains(&"task".into()));
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
        let preflight = labels(complete_words(&words(&["task", "preflight", "--"])));
        assert!(preflight.contains(&"--workspace".into()));
        assert!(preflight.contains(&"--project".into()));
        assert!(preflight.contains(&"--work-item".into()));
        assert!(preflight.contains(&"--continue".into()));
        assert!(preflight.contains(&"--ai-context-file".into()));

        let handoff = labels(complete_words(&words(&["task", "handoff-validate", "--"])));
        assert!(handoff.contains(&"--workspace".into()));
        assert!(handoff.contains(&"--project".into()));
        assert!(handoff.contains(&"--work-item".into()));
        assert!(handoff.contains(&"--continue".into()));
    }

    #[test]
    fn max_rows_completion_offers_common_limits() {
        let values = labels(complete_words(&words(&["db", "query", "--max-rows", ""])));

        assert_eq!(values, vec!["50", "100", "500", "1000"]);
    }

    #[test]
    fn completion_complete_format_offers_completion_formats() {
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
        let items = complete_words(&words(&["task", "open", "--"]));
        let workspace = items
            .iter()
            .find(|item| item.label == "--workspace")
            .expect("workspace option");

        assert_eq!(
            rich_shell_row(workspace),
            "--workspace\tWorkspace task existant"
        );
    }

    #[test]
    fn completion_show_renders_install_commands() {
        let report = render_completion_show(&TerminalTheme::plain());

        assert!(report.contains("Autocomplétion shell"));
        assert!(report.contains("Installer l'intégration adaptée à votre shell"));
        assert!(report.contains("bash       dw completion install bash >> ~/.bashrc"));
        assert!(report.contains("zsh        dw completion install zsh >> ~/.zshrc"));
        assert!(report.contains(
            "fish       dw completion install fish > ~/.config/fish/completions/dw.fish"
        ));
        assert!(report.contains("powershell dw completion install powershell >> $PROFILE"));
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
        assert_eq!(values, vec!["ha".to_string(), "dw".to_string()]);
    }

    #[test]
    fn repository_values_come_from_config_in_config_order() {
        let root = temp_root("completion-repositories");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{},"back":{},"db":{}}}}}"#,
        )
        .expect("projects config");

        let task_values = labels(complete_words(&words(&[
            "task",
            "start",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
            "--only",
            "",
        ])));
        let changelog_values = labels(complete_words(&words(&[
            "ado",
            "changelog",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
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
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{}}}}"#,
        )
        .expect("projects config");
        fs::write(
            root.join("config/databases.json"),
            r#"{"globals":{"shared":{}},"projects":{"ha":{"databases":{"ha-dev":{},"ha-recette":{}}}}}"#,
        )
        .expect("databases config");

        let values = labels(complete_words(&words(&[
            "db",
            "schema",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
            "--database",
            "",
        ])));

        assert_eq!(values, vec!["ha-dev", "ha-recette", "shared"]);
    }

    #[test]
    fn workspace_and_work_item_values_come_from_manifests() {
        let root = temp_root("completion-workspaces");
        let workspace = root.join("projects/ha/workspaces/feature-42-demo");
        fs::create_dir_all(&workspace).expect("workspace dir");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"42","taskId":null,"project":"ha","type":"feature","slug":"demo","branchName":"feature/42-demo","createdAt":"2026-07-03T10:00:00Z","repositories":["front","back"],"status":"created","workItems":[{"id":"42","type":"User Story","title":"Demo","state":"Active"},{"id":"43","type":"Task","title":"Child","state":"Active"}]}"#,
        )
        .expect("manifest");

        let workspace_values = labels(complete_words(&words(&[
            "task",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
            "--workspace",
            "",
        ])));
        let work_item_values = labels(complete_words(&words(&[
            "task",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
            "--work-item",
            "",
        ])));
        let repository_values = labels(complete_words(&words(&[
            "task",
            "open",
            "--root",
            root.to_str().expect("root"),
            "--workspace",
            workspace.to_str().expect("workspace"),
            "--repo",
            "",
        ])));

        assert_eq!(workspace_values, vec![workspace.display().to_string()]);
        assert_eq!(work_item_values, vec!["#42 Demo", "#43 Child"]);
        assert_eq!(repository_values, vec!["front", "back"]);
    }

    #[test]
    fn dynamic_values_include_descriptions() {
        let root = temp_root("completion-value-descriptions");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/projects.json"),
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{}}}}}"#,
        )
        .expect("projects config");

        let items = complete_words(&words(&[
            "ado",
            "changelog",
            "--root",
            root.to_str().expect("root"),
            "--project",
            "ha",
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
