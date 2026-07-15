use clap::{Arg, ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::Shell;
use dw_completion::CompletionOutput;

#[derive(Debug, Parser)]
#[command(name = "dw")]
#[command(bin_name = "dw")]
#[command(version = crate::version::PACKAGE_VERSION)]
#[command(propagate_version = true)]
#[command(about = "Dev Workflow")]
#[command(help_template = "{about} {version}\n\n{usage-heading} {usage}\n\n{all-args}")]
pub(crate) struct Cli {
    #[arg(
        short = 'v',
        long = "verbose",
        action = ArgAction::Count,
        global = true,
        help = "Increase diagnostic output (-v info, -vv debug)."
    )]
    pub(crate) verbose: u8,

    #[command(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub(crate) fn parse_localized() -> Self {
        let command = Self::localized_command();
        let matches = command.get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
    }

    pub(crate) fn localized_command() -> clap::Command {
        let display_version: &'static str =
            Box::leak(crate::version::informational_version().into_boxed_str());
        localize_command(Self::command().version(display_version), 0)
    }
}

fn localize_command(command: clap::Command, depth: usize) -> clap::Command {
    let help_template = if command.get_name() == "dw" {
        "{about} {version}\n\nUsage: {usage}\n\n{all-args}"
    } else {
        "{about-with-newline}\nUsage: {usage}\n\n{all-args}"
    };

    let command = command
        .help_template(help_template)
        .subcommand_help_heading("Commands")
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help("Show help."),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .action(ArgAction::Version)
                .help("Show version."),
        );
    let command = if depth < 3 {
        command.mut_subcommands(|subcommand| localize_command(subcommand, depth + 1))
    } else {
        command
    };

    sort_help_options(sort_help_subcommands(command))
}

fn sort_help_options(command: clap::Command) -> clap::Command {
    let mut ordered_args = command
        .get_arguments()
        .filter(|arg| !arg.is_positional())
        .map(|arg| {
            (
                arg.get_id().to_string(),
                arg.get_long()
                    .map(str::to_string)
                    .or_else(|| arg.get_short().map(|short| short.to_string()))
                    .unwrap_or_else(|| arg.get_id().to_string())
                    .to_lowercase(),
            )
        })
        .collect::<Vec<_>>();
    ordered_args.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));

    ordered_args
        .into_iter()
        .enumerate()
        .fold(command, |command, (index, (arg_id, _))| {
            command.mut_arg(arg_id, |arg| arg.display_order(index + 1))
        })
}

fn sort_help_subcommands(command: clap::Command) -> clap::Command {
    let mut ordered_subcommands = command
        .get_subcommands()
        .map(|subcommand| {
            let name = subcommand.get_name().to_string();
            (name.clone(), name.to_lowercase(), name.len())
        })
        .collect::<Vec<_>>();
    ordered_subcommands.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });

    ordered_subcommands
        .into_iter()
        .enumerate()
        .fold(command, |command, (index, (name, _, _))| {
            command.mut_subcommand(name, |subcommand| subcommand.display_order(index + 1))
        })
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "Show the CLI version.")]
    Version,
    #[command(about = "Explain the getting-started flow.", alias = "get-started")]
    Guide,
    #[command(about = "Diagnose machine prerequisites and local configuration.")]
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    #[command(about = "Initialize a local DevWorkflow root.")]
    Init {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long = "dry-run")]
        dry_run: bool,
        #[arg(long = "no-save")]
        no_save: bool,
    },
    #[command(about = "Regenerate schemas and agent contexts.")]
    Refresh {
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "default")]
        profile: String,
    },
    #[command(about = "Open the DevWorkflow TUI dashboard.")]
    Tui {
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
    },
    #[command(about = "Show AI workflow context, open an agent, or manage agent configuration.")]
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    #[command(about = "Manage Azure DevOps authentication.")]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    #[command(about = "Install or inspect shell completions.")]
    Completion {
        #[command(subcommand)]
        command: CompletionCommand,
    },
    #[command(about = "Validate and edit configuration.")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(about = "Azure DevOps commands.")]
    Ado {
        #[command(subcommand)]
        command: AdoCommand,
    },
    #[command(about = "Explore and guard database access.")]
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    #[command(about = "Store local secrets.")]
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    #[command(about = "Upgrade the dw binary.")]
    Upgrade {
        #[arg(long, conflicts_with = "rid")]
        check: bool,
        #[arg(long, conflicts_with = "check")]
        rid: Option<String>,
    },
    #[command(about = "Manage the work cycle: workspace, worktrees, commits, PRs, and cleanup.")]
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommand {
    #[command(about = "Connect Azure DevOps.")]
    Login {
        #[arg(long, help = "DevWorkflow root to use for auth configuration.")]
        root: Option<String>,
    },
    #[command(about = "Show Azure DevOps connection status.")]
    Status {
        #[arg(long, help = "DevWorkflow root to use for auth configuration.")]
        root: Option<String>,
    },
    #[command(about = "Remove the local Azure DevOps session.")]
    Logout {
        #[arg(long, help = "DevWorkflow root to use for auth configuration.")]
        root: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdoCommand {
    #[command(about = "List Azure DevOps work items assigned to the current user.")]
    Assigned {
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Configured project to query; opens an interactive picker when omitted."
        )]
        project: Option<String>,
        #[arg(
            long,
            default_value_t = 20,
            help = "Maximum number of work items to load."
        )]
        top: i32,
        #[arg(long, help = "Also include work items in a final state.")]
        all: bool,
        #[arg(long = "group-by-parent", help = "Group work items by ADO parent.")]
        group_by_parent: bool,
        #[arg(long, help = "Emit the deterministic JSON response.")]
        json: bool,
    },
    #[command(about = "List active Azure DevOps pull requests from configured repositories.")]
    Prs {
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to query.")]
        project: String,
        #[arg(
            long,
            help = "Local or Azure DevOps repository to query; repeat with commas."
        )]
        repo: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON response.")]
        json: bool,
    },
    #[command(about = "Build a changelog from PRs, a git range, or work items.")]
    Changelog {
        #[arg(help = "Work item IDs, PRs, or git range depending on the selected mode.")]
        ids: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(
            long = "from-pr",
            conflicts_with = "from_git",
            help = "Interpret IDs as Azure DevOps pull requests."
        )]
        from_pr: bool,
        #[arg(
            long = "from-git",
            conflicts_with = "from_pr",
            help = "Extract work items from git commits."
        )]
        from_git: bool,
        #[arg(long, help = "Local repository used for --from-git mode.")]
        repo: Option<String>,
        #[arg(long = "group-by-parent", help = "Group the changelog by ADO parent.")]
        group_by_parent: bool,
        #[arg(long, value_parser = ["raw", "markdown", "html"], help = "Output format.")]
        format: Option<String>,
        #[arg(
            long,
            requires = "format",
            help = "Render the markdown/html changelog as a table."
        )]
        table: bool,
        #[arg(
            long = "ids-only",
            help = "Show only resolved IDs, separated by spaces."
        )]
        ids_only: bool,
        #[arg(
            long = "git-to",
            requires = "from_git",
            help = "Ending revision for the git range."
        )]
        git_to: Option<String>,
    },
    #[command(about = "Show a readable summary of Azure DevOps work items.")]
    WorkItem {
        #[arg(help = "Azure DevOps work item ID, or comma-separated list.")]
        id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON response.")]
        json: bool,
    },
    #[command(about = "Change the state of one or more Azure DevOps work items.")]
    SetState {
        #[arg(help = "Azure DevOps work item ID, or comma-separated list.")]
        id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(long, help = "Exact new ADO state to apply.")]
        state: String,
        #[arg(long, help = "ADO history message; default: dw ado set-state.")]
        history: Option<String>,
        #[arg(long, help = "Confirm the destructive state change.")]
        yes: bool,
        #[arg(long, help = "Emit the deterministic JSON response; requires --yes.")]
        json: bool,
    },
    #[command(
        about = "Show detailed context for one or more work items in a human-readable format."
    )]
    Context {
        #[arg(help = "Azure DevOps work item ID, or comma-separated list.")]
        id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(long, help = "Limit context to essential fields.")]
        summary: bool,
        #[arg(
            long,
            default_value_t = 200,
            help = "Maximum number of comments to show; 0 for none."
        )]
        comments: i32,
        #[arg(long, help = "Emit the deterministic JSON response.")]
        json: bool,
    },
    #[command(about = "Emit structured, deterministic AI context for one or more work items.")]
    AiContext {
        #[arg(help = "Azure DevOps work item ID, or comma-separated list.")]
        id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Explicit Azure DevOps organization.")]
        organization: Option<String>,
        #[arg(long, help = "Configured project or explicit Azure DevOps project.")]
        project: Option<String>,
        #[arg(long, help = "Limit the contract to essential fields.")]
        summary: bool,
        #[arg(
            long,
            default_value_t = 200,
            help = "Maximum number of comments to include."
        )]
        comments: i32,
        #[arg(
            long = "include-comments",
            help = "Include comments in the AI context."
        )]
        include_comments: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    #[command(about = "List task workspaces detected under the root.")]
    Status {
        #[arg(long, help = "DevWorkflow root to scan.")]
        root: Option<String>,
    },
    #[command(about = "List task workspaces with project/work item filters.")]
    List {
        #[arg(long, help = "DevWorkflow root to scan.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to filter by.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item to filter by.")]
        work_item: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON list.")]
        json: bool,
    },
    #[command(about = "Show the current task workspace from the current directory.")]
    Current {
        #[arg(long, help = "Emit the current workspace as deterministic JSON.")]
        json: bool,
    },
    #[command(about = "Move ADO work items to their configured in-progress state.")]
    Doing {
        #[arg(help = "Azure DevOps work item ID, or comma-separated list.")]
        id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(long, help = "Confirm the work item state changes.")]
        yes: bool,
        #[arg(long, help = "Emit the deterministic report JSON; requires --yes.")]
        json: bool,
    },
    #[command(about = "Open or resume a task workspace with the configured agent.")]
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path to open directly.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "pr",
            conflicts_with_all = ["workspace", "work_item", "continue"],
            help = "Azure DevOps pull request used to resolve the existing workspace."
        )]
        pull_request: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository to open in the workspace.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent to launch: opencode, cursor, claude, codex, codex-cli, or copilot."
        )]
        agent: Option<String>,
        #[arg(long, help = "Emit resolution JSON instead of launching the agent.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Prepare or create a task workspace from ADO work items.")]
    Start {
        #[arg(help = "Parent or child ADO work item ID to start.")]
        work_item_id: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: Option<String>,
        #[arg(long = "task", help = "Child task ID to add to the workspace.")]
        task: Option<String>,
        #[arg(
            long = "type",
            help = "Branch/workspace type: feature, bugfix, hotfix, or chore."
        )]
        type_name: Option<String>,
        #[arg(
            long = "only",
            help = "Repository to include; repeat through interactive selection when omitted."
        )]
        only: Option<String>,
        #[arg(long, help = "Explicit slug for the branch and workspace name.")]
        slug: Option<String>,
        #[arg(
            long = "skip-ado",
            help = "Do not query Azure DevOps; use the provided local values."
        )]
        skip_ado: bool,
        #[arg(
            long = "with-active-children",
            conflicts_with = "skip_ado",
            help = "Automatically include non-final ADO children of the selected subject."
        )]
        with_active_children: bool,
        #[arg(
            long = "create-child-tasks",
            conflicts_with = "skip_ado",
            help = "Create one ADO child task per included repository before creating the workspace."
        )]
        create_child_tasks: bool,
        #[arg(long, help = "Emit the plan or result as deterministic JSON.")]
        json: bool,
        #[arg(
            long,
            help = "Actually create the workspace; without this flag, show the plan."
        )]
        execute: bool,
    },
    #[command(about = "Prepare or create a workspace from work items linked to a pull request.")]
    StartPr {
        #[arg(help = "Azure DevOps pull request ID.")]
        pull_request_id: String,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to use.")]
        project: String,
        #[arg(long, help = "Local or Azure DevOps repository for the PR.")]
        repo: Option<String>,
        #[arg(
            long = "type",
            help = "Branch/workspace type: feature, bugfix, hotfix, or chore."
        )]
        type_name: Option<String>,
        #[arg(long, help = "Explicit slug for the branch and workspace name.")]
        slug: Option<String>,
        #[arg(long, help = "Emit the plan or result as deterministic JSON.")]
        json: bool,
        #[arg(
            long,
            help = "Actually create the workspace; without this flag, show the plan."
        )]
        execute: bool,
    },
    #[command(about = "Validate blockers and warnings before implementation.")]
    Preflight {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path to audit.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(
            long = "ai-context-file",
            help = "Additional AI context file to verify; repeatable option."
        )]
        ai_context_file: Vec<String>,
        #[arg(long, help = "Emit the deterministic preflight report JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Synchronize task.json with Azure DevOps work items.")]
    Sync {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path to synchronize.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Emit the deterministic result JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Rename a task workspace and its branch using a new slug.")]
    Rename {
        #[arg(help = "New slug for the workspace and branch.")]
        slug: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path to rename.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
        #[arg(
            long,
            help = "Actually apply the rename; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Update workspace repositories from their target branch.")]
    RepoLatest {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Workspace path to synchronize."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(
            long = "only",
            help = "Limit synchronization to one workspace repository."
        )]
        only: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
    },
    #[command(about = "Prepare or create an intermediate commit for workspace repositories.")]
    Commit {
        #[arg(long, conflicts_with = "continue", help = "Workspace path to commit.")]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Actually create commits; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(
            long,
            help = "Explicit commit message; otherwise generated from the task manifest."
        )]
        message: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON report.")]
        json: bool,
    },
    #[command(about = "Add work items to the current task workspace.")]
    AddWorkItem {
        #[arg(help = "Work item IDs to add, separated by commas.")]
        work_item_ids: Option<String>,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path to modify.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(
            long = "skip-ado",
            help = "Do not query Azure DevOps; use the provided local values."
        )]
        skip_ado: bool,
        #[arg(
            long = "type",
            help = "Local type to use when ADO is skipped or incomplete."
        )]
        type_name: Option<String>,
        #[arg(long, help = "Local title to use when ADO is skipped or incomplete.")]
        title: Option<String>,
        #[arg(long, help = "Local state to use when ADO is skipped or incomplete.")]
        state: Option<String>,
        #[arg(
            long,
            help = "Actually apply the change; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Remove work items from the current task workspace.")]
    RemoveWorkItem {
        #[arg(help = "Work item IDs to remove, separated by commas.")]
        work_item_ids: Option<String>,
        #[arg(long, help = "Workspace path to modify.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project used to resolve the workspace.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Actually apply the change; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Add a repository to the task workspace.")]
    AddRepo {
        #[arg(help = "Configured repository to add to the workspace.")]
        repo: Option<String>,
        #[arg(long, help = "Workspace path to modify.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Create the worktree and modify task.json; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
    },
    #[command(about = "Create an ADO child task and add it to the repository handoff.")]
    CreateChildTask {
        #[arg(long, help = "Workspace repository that will carry the task handoff.")]
        repo: String,
        #[arg(long, help = "Title of the ADO child task to create.")]
        title: String,
        #[arg(long, help = "Workspace path to modify.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project used to resolve the workspace.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Emit the deterministic result JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Verify, commit, push, and open a PR to finish the workspace.")]
    Finish {
        #[arg(long, conflicts_with = "continue", help = "Workspace path to finish.")]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Run commits, pushes, PRs, and ADO updates; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirm destructive finish with --execute.")]
        yes: bool,
        #[arg(
            long,
            help = "Explicit commit message; otherwise generated from the task manifest."
        )]
        message: Option<String>,
        #[arg(
            long = "create-pr",
            help = "Create or verify Azure DevOps pull requests after push."
        )]
        create_pr: bool,
        #[arg(
            long,
            requires = "create_pr",
            help = "Create PRs as ready instead of draft."
        )]
        ready: bool,
        #[arg(
            long = "skip-verify",
            help = "Skip configured verification commands before PR."
        )]
        skip_verify: bool,
        #[arg(
            long = "skip-ado",
            help = "Do not call Azure DevOps; incompatible with --create-pr."
        )]
        skip_ado: bool,
        #[arg(
            long = "force-with-lease",
            help = "Allow rewritten workspace branches to replace remote branches only if their remote-tracking refs have not changed."
        )]
        force_with_lease: bool,
        #[arg(long, help = "Emit the deterministic JSON report.")]
        json: bool,
    },
    #[command(about = "Validate handoff files before sub-agents or finishing.")]
    HandoffValidate {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Workspace path whose handoffs must be valid.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve the workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Emit the deterministic JSON report.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Remove worktrees and clean up a task workspace.")]
    Teardown {
        #[arg(long, help = "Workspace path to remove.")]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(long, help = "Configured project used to resolve the workspace.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Actually remove worktrees and the workspace; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirm destructive removal with --execute.")]
        yes: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Clean up workspaces whose work items are finished.")]
    Prune {
        #[arg(long, help = "DevWorkflow root to scan.")]
        root: Option<String>,
        #[arg(long, help = "Configured project to filter by.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item to filter by.")]
        work_item: Option<String>,
        #[arg(
            long,
            help = "Actually remove eligible workspaces; without this flag, show the plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirm destructive removal with --execute.")]
        yes: bool,
        #[arg(
            long = "no-sync",
            help = "Do not synchronize ADO states before determining eligibility."
        )]
        no_sync: bool,
        #[arg(long, help = "Emit the plan/result as deterministic JSON.")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    #[command(about = "Show the root, color mode, and configuration paths.")]
    Show {
        #[arg(long, help = "DevWorkflow root to inspect.")]
        root: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON report.")]
        json: bool,
    },
    #[command(about = "Verify local configuration files and schemas.")]
    Doctor {
        #[arg(long, help = "DevWorkflow root to verify.")]
        root: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON report.")]
        json: bool,
    },
    #[command(about = "Save the user DevWorkflow root.")]
    SetRoot {
        #[arg(help = "DevWorkflow root path to save.")]
        path: String,
    },
    #[command(about = "Configure color mode: auto, always, or never.")]
    SetColor {
        #[arg(help = "Color mode to save: auto, always, or never.")]
        mode: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AgentCommand {
    #[command(about = "Show the DevWorkflow context injected into AI agents.")]
    Context,
    #[command(about = "Open or resume an agent on a task workspace.")]
    Open {
        #[arg(
            long,
            conflicts_with_all = ["project", "work_item", "continue"],
            help = "Workspace path to open directly."
        )]
        workspace: Option<String>,
        #[arg(long, help = "DevWorkflow root to use.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Configured project used to resolve a workspace."
        )]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item used to resolve the workspace.")]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Resume the most recent matching task workspace."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository to open in the workspace, when applicable.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent to launch: opencode, cursor, claude, codex, codex-cli, or copilot."
        )]
        agent: Option<String>,
        #[arg(help = "Positional work item alias used to resolve the workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Show the effective agent configuration.")]
    Config {
        #[arg(long, help = "DevWorkflow root to read.")]
        root: Option<String>,
    },
    #[command(about = "Show the effective agent configuration.")]
    Show {
        #[arg(long, help = "DevWorkflow root to read.")]
        root: Option<String>,
    },
    #[command(about = "Set the default agent for the DevWorkflow root.")]
    SetDefault {
        #[arg(
            help = "Agent to use by default: opencode, cursor, claude, codex, codex-cli, or copilot."
        )]
        agent: String,
        #[arg(long, help = "DevWorkflow root to modify.")]
        root: Option<String>,
    },
    #[command(about = "Diagnose installed agent availability.")]
    Doctor {
        #[arg(long, help = "Limit diagnostics to one agent.")]
        agent: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SecretCommand {
    #[command(about = "Save a secret in the system keyring.")]
    Set {
        #[arg(help = "Logical secret key, for example a credentialReference.")]
        key: String,
        #[arg(long, conflicts_with = "from_env", help = "Secret value to save.")]
        value: Option<String>,
        #[arg(
            long = "from-env",
            conflicts_with = "value",
            help = "Environment variable name containing the secret."
        )]
        from_env: Option<String>,
    },
    #[command(about = "Check whether a secret exists without showing its value.")]
    Get {
        #[arg(help = "Logical secret key to check.")]
        key: String,
    },
    #[command(about = "Delete a secret from the system keyring.")]
    Delete {
        #[arg(help = "Logical secret key to delete.")]
        key: String,
        #[arg(long, help = "Confirm secret deletion in non-interactive mode.")]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DbCommand {
    #[command(about = "Verify that a SQL query respects read-only mode.")]
    Guard {
        #[arg(long, help = "SQL query to analyze without executing.")]
        sql: String,
    },
    #[command(about = "List accessible tables and views on a configured database.")]
    Schema {
        #[arg(long, help = "Configured project containing the database connection.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Connection name declared in databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Database environment alias declared in databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON result.")]
        json: bool,
    },
    #[command(about = "Describe the columns of a SQL table.")]
    Describe {
        #[arg(help = "Table to describe, in table or schema.table format.")]
        table: Option<String>,
        #[arg(long, help = "Configured project containing the database connection.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Connection name declared in databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Database environment alias declared in databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Emit the deterministic JSON result.")]
        json: bool,
    },
    #[command(about = "Execute a read-only SQL query with guards and a row limit.")]
    Query {
        #[arg(long, help = "Read-only SQL query to execute.")]
        sql: Option<String>,
        #[arg(long, help = "Configured project containing the database connection.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Connection name declared in databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Database environment alias declared in databases.json."
        )]
        env: Option<String>,
        #[arg(long = "max-rows", help = "Maximum number of rows to show.")]
        #[arg(value_parser = parse_positive_usize)]
        max_rows: Option<usize>,
        #[arg(long, help = "Emit the deterministic JSON result.")]
        json: bool,
        #[arg(
            value_name = "SQL",
            trailing_var_arg = true,
            help = "Read-only SQL query to execute."
        )]
        sql_parts: Vec<String>,
    },
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "max-rows must be a positive integer.".to_string())?;
    if parsed == 0 {
        return Err("max-rows must be greater than 0.".into());
    }
    Ok(parsed)
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

#[cfg(test)]
mod tests {
    use super::{Cli, Command, TaskCommand};
    use clap::Parser;

    #[test]
    fn localized_help_uses_english_builtin_labels() {
        let mut command = Cli::localized_command();
        let mut output = Vec::new();
        command.write_long_help(&mut output).expect("help output");
        let help = String::from_utf8(output).expect("utf8 help");

        assert!(help.contains("Show help."));
        assert!(help.contains("Show version."));
        assert!(!help.contains("Print help"));
        assert!(!help.contains("Print version"));
        assert!(!help.contains("Print this message"));
    }

    #[test]
    fn localized_subcommand_help_uses_english_builtin_labels() {
        let error = Cli::localized_command()
            .try_get_matches_from(["dw", "ado", "ai-context", "--help"])
            .expect_err("help exits through clap");
        let help = error.to_string();

        assert!(help.contains("Show help."));
        assert!(help.contains("Show version."));
        assert!(!help.contains("Print help"));
        assert!(!help.contains("Print version"));
    }

    #[test]
    fn verbose_flag_counts_globally() {
        let cli = Cli::parse_from(["dw", "-vv", "version"]);

        assert_eq!(cli.verbose, 2);
    }

    #[test]
    fn task_doing_accepts_ids_without_hash_prefix() {
        let cli = Cli::parse_from(["dw", "task", "doing", "116", "--project", "acme", "--yes"]);

        let Command::Task {
            command: TaskCommand::Doing {
                id, project, yes, ..
            },
        } = cli.command
        else {
            panic!("expected task doing command");
        };
        assert_eq!(id, "116");
        assert_eq!(project.as_deref(), Some("acme"));
        assert!(yes);
    }

    #[test]
    fn secret_delete_help_requires_explicit_confirmation_option() {
        let error = Cli::localized_command()
            .try_get_matches_from(["dw", "secret", "delete", "--help"])
            .expect_err("help exits through clap");
        let help = error.to_string();

        assert!(help.contains("--yes"));
        assert!(help.contains("Confirm secret deletion"));
    }

    #[test]
    fn task_finish_help_exposes_force_with_lease() {
        let error = Cli::localized_command()
            .try_get_matches_from(["dw", "task", "finish", "--help"])
            .expect_err("help exits through clap");
        let help = error.to_string();

        assert!(help.contains("--force-with-lease"));
        assert!(help.contains("remote-tracking refs have not changed"));
    }

    #[test]
    fn localized_options_are_sorted_alphabetically_in_help() {
        let error = Cli::localized_command()
            .try_get_matches_from(["dw", "task", "open", "--help"])
            .expect_err("help exits through clap");
        let help = error.to_string();
        let options = help
            .lines()
            .filter_map(|line| line.find("--").map(|index| &line[index + 2..]))
            .filter_map(|line| line.split_whitespace().next())
            .map(|option| option.trim_end_matches(',').trim_end_matches('.'))
            .collect::<Vec<_>>();

        assert_eq!(
            options,
            vec![
                "agent",
                "continue",
                "verbose",
                "help",
                "json",
                "pr",
                "project",
                "repo",
                "root",
                "version",
                "work-item",
                "workspace",
            ]
        );
    }

    #[test]
    fn localized_subcommands_are_sorted_alphabetically_in_help() {
        let mut command = Cli::localized_command();
        let mut output = Vec::new();
        command.write_long_help(&mut output).expect("help output");
        let help = String::from_utf8(output).expect("utf8 help");
        let commands = help
            .lines()
            .skip_while(|line| line.trim() != "Commands:")
            .skip(1)
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| line.split_whitespace().next())
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![
                "ado",
                "agent",
                "auth",
                "completion",
                "config",
                "db",
                "doctor",
                "guide",
                "init",
                "refresh",
                "secret",
                "task",
                "tui",
                "upgrade",
                "version",
            ]
        );
    }
}
