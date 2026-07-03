use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use dw_ado_commands::auth::AuthCommand;
use dw_ado_commands::command::AdoCommand;
use dw_agent::command::AgentCommand;
use dw_config::command::ConfigCommand;
use dw_db::command::DbCommand;
use dw_secret::command::SecretCommand;
use dw_task::command::TaskCommand;

#[derive(Debug, Parser)]
#[command(name = "dw")]
#[command(bin_name = "dw")]
#[command(version = crate::version::PACKAGE_VERSION)]
#[command(propagate_version = true)]
#[command(about = "Dev Workflow")]
#[command(help_template = "{about} {version}\n\n{usage-heading} {usage}\n\n{all-args}")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "Affiche la version du CLI.")]
    Version,
    #[command(about = "Explique le parcours de demarrage.", alias = "get-started")]
    Guide,
    #[command(about = "Diagnostique les prerequis machine et la configuration locale.")]
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    #[command(about = "Initialise un root DevWorkflow local.")]
    Init {
        #[arg(long, default_value = "business")]
        profile: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long = "dry-run")]
        dry_run: bool,
        #[arg(long = "no-save")]
        no_save: bool,
    },
    #[command(about = "Regenere schemas et contextes agents.")]
    Refresh {
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "business")]
        profile: String,
    },
    #[command(about = "Affiche le contexte workflow IA, ouvre un agent, ou gere sa configuration.")]
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    #[command(about = "Gere la connexion Azure DevOps.")]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    #[command(about = "Installe ou interroge l'autocompletion shell.")]
    Completion {
        #[command(subcommand)]
        command: CompletionCommand,
    },
    #[command(about = "Valide et modifie la configuration.")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(about = "Commandes Azure DevOps.")]
    Ado {
        #[command(subcommand)]
        command: AdoCommand,
    },
    #[command(about = "Explore et protege les acces base de donnees.")]
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    #[command(about = "Stocke des secrets locaux.")]
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    #[command(about = "Met a jour le binaire dw.")]
    Upgrade {
        #[arg(long, conflicts_with = "rid")]
        check: bool,
        #[arg(long, conflicts_with = "check")]
        rid: Option<String>,
    },
    #[command(about = "Gere le cycle de travail: workspace, worktrees, commits, PR et cleanup.")]
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompletionCommand {
    Show,
    Generate {
        shell: Shell,
    },
    Install {
        shell: Shell,
    },
    #[command(hide = true)]
    Complete {
        #[arg(long, value_enum, default_value_t = CompletionOutput::Bash)]
        format: CompletionOutput,
        words: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CompletionOutput {
    Bash,
    Json,
}
