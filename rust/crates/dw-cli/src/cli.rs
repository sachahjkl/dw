use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
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
pub(crate) enum AgentCommand {
    Context,
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        positional_work_item: Option<String>,
    },
    Config {
        #[arg(long)]
        root: Option<String>,
    },
    Show {
        #[arg(long)]
        root: Option<String>,
    },
    SetDefault {
        agent: String,
        #[arg(long)]
        root: Option<String>,
    },
    Doctor {
        #[arg(long)]
        agent: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommand {
    Login {
        #[arg(long)]
        root: Option<String>,
    },
    Status {
        #[arg(long)]
        root: Option<String>,
    },
    Logout {
        #[arg(long)]
        root: Option<String>,
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

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    Show {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    SetRoot {
        path: String,
    },
    SetColor {
        mode: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdoCommand {
    Assigned {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 20)]
        top: i32,
        #[arg(long)]
        all: bool,
        #[arg(long = "group-by-parent")]
        group_by_parent: bool,
        #[arg(long)]
        json: bool,
    },
    Changelog {
        ids: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "from-pr", conflicts_with = "from_git")]
        from_pr: bool,
        #[arg(long = "from-git", conflicts_with = "from_pr")]
        from_git: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long = "group-by-parent")]
        group_by_parent: bool,
        #[arg(long, value_parser = ["raw", "markdown", "html"])]
        format: Option<String>,
        #[arg(long, requires = "format")]
        table: bool,
        #[arg(long = "ids-only")]
        ids_only: bool,
        #[arg(long = "git-to", requires = "from_git")]
        git_to: Option<String>,
    },
    WorkItem {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Context {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        summary: bool,
        #[arg(long, default_value_t = 200)]
        comments: i32,
        #[arg(long)]
        json: bool,
    },
    AiContext {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        summary: bool,
        #[arg(long, default_value_t = 200)]
        comments: i32,
        #[arg(long = "include-comments")]
        include_comments: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DbCommand {
    Guard {
        #[arg(long)]
        sql: String,
    },
    Schema {
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Describe {
        table: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Query {
        #[arg(long)]
        sql: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long = "max-rows")]
        max_rows: Option<usize>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SecretCommand {
    Set {
        key: String,
        #[arg(long, conflicts_with = "from_env")]
        value: Option<String>,
        #[arg(long = "from-env", conflicts_with = "value")]
        from_env: Option<String>,
    },
    Get {
        key: String,
    },
    Delete {
        key: String,
    },
}
