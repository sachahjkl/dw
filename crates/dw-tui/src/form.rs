use crate::model::{ActionRisk, TuiAction, TuiActionRequest, TuiSnapshot};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTemplate {
    TaskStart,
    TaskStartPr,
    TaskFinish,
    TaskTeardown,
    TaskPrune,
    TaskAddWorkItem,
    TaskRemoveWorkItem,
    TaskAddRepo,
    TaskRename,
    AdoAssigned,
    AdoSetState,
    DbSchema,
    DbDescribe,
    DbQuery,
    AgentOpen,
    Secret,
    ConfigSetRoot,
}

impl FormTemplate {
    pub const ALL: [FormTemplate; 17] = [
        FormTemplate::TaskStart,
        FormTemplate::TaskStartPr,
        FormTemplate::TaskFinish,
        FormTemplate::TaskTeardown,
        FormTemplate::TaskPrune,
        FormTemplate::TaskAddWorkItem,
        FormTemplate::TaskRemoveWorkItem,
        FormTemplate::TaskAddRepo,
        FormTemplate::TaskRename,
        FormTemplate::AdoAssigned,
        FormTemplate::AdoSetState,
        FormTemplate::DbSchema,
        FormTemplate::DbDescribe,
        FormTemplate::DbQuery,
        FormTemplate::AgentOpen,
        FormTemplate::Secret,
        FormTemplate::ConfigSetRoot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FormTemplate::TaskStart => "Create workspace",
            FormTemplate::TaskStartPr => "Create workspace from PR",
            FormTemplate::TaskFinish => "Finish workspace",
            FormTemplate::TaskTeardown => "Remove workspace",
            FormTemplate::TaskPrune => "Clean workspaces",
            FormTemplate::TaskAddWorkItem => "Add work item",
            FormTemplate::TaskRemoveWorkItem => "Remove work item",
            FormTemplate::TaskAddRepo => "Add repository",
            FormTemplate::TaskRename => "Rename workspace",
            FormTemplate::AdoAssigned => "My work items",
            FormTemplate::AdoSetState => "Move work item state",
            FormTemplate::DbSchema => "Explore database structure",
            FormTemplate::DbDescribe => "Describe DB table",
            FormTemplate::DbQuery => "Guided DB query",
            FormTemplate::AgentOpen => "Open agent",
            FormTemplate::Secret => "Secret",
            FormTemplate::ConfigSetRoot => "Change root",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            FormTemplate::TaskStart => "Create or preview a task workspace",
            FormTemplate::TaskStartPr => "Create or preview a workspace from a PR",
            FormTemplate::TaskFinish => "Preview or execute workspace finish",
            FormTemplate::TaskTeardown => "Preview or remove a workspace",
            FormTemplate::TaskPrune => "Clean finished workspaces",
            FormTemplate::TaskAddWorkItem => "Add work items to the workspace",
            FormTemplate::TaskRemoveWorkItem => "Remove work items from the workspace",
            FormTemplate::TaskAddRepo => "Add a repository to the workspace",
            FormTemplate::TaskRename => "Rename workspace and branch",
            FormTemplate::AdoAssigned => "List assigned work items with filters",
            FormTemplate::AdoSetState => "Move selected ADO work items to a destination state",
            FormTemplate::DbSchema => "List tables and views from a database",
            FormTemplate::DbDescribe => "Describe table columns",
            FormTemplate::DbQuery => "Run a read-only SQL query",
            FormTemplate::AgentOpen => "Open a workspace with an AI agent",
            FormTemplate::Secret => "Check, remove or populate a secret",
            FormTemplate::ConfigSetRoot => "Change the user DevWorkflow root",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub label: String,
    pub help: String,
    pub value: String,
    pub kind: FieldKind,
}

impl FormField {
    fn text(label: &str, help: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            help: help.into(),
            value: value.into(),
            kind: FieldKind::Text,
        }
    }

    fn toggle_field(label: &str, help: &str, value: bool) -> Self {
        Self {
            label: label.into(),
            help: help.into(),
            value: if value { "true" } else { "false" }.into(),
            kind: FieldKind::Toggle,
        }
    }

    pub fn enabled(&self) -> bool {
        self.value == "true"
    }

    pub fn toggle(&mut self) {
        if self.kind == FieldKind::Toggle {
            self.value = if self.enabled() { "false" } else { "true" }.into();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormMode {
    Selecting,
    Editing,
}

#[derive(Debug, Clone)]
pub struct FormState {
    pub mode: FormMode,
    pub template_index: usize,
    pub template: FormTemplate,
    pub fields: Vec<FormField>,
    pub selected_field: usize,
}

impl FormState {
    pub fn selecting() -> Self {
        Self {
            mode: FormMode::Selecting,
            template_index: 0,
            template: FormTemplate::TaskStart,
            fields: Vec::new(),
            selected_field: 0,
        }
    }

    pub fn begin_editing(&mut self, snapshot: &TuiSnapshot) {
        self.template = FormTemplate::ALL[self.template_index];
        self.fields = default_fields(self.template, snapshot);
        self.selected_field = 0;
        self.mode = FormMode::Editing;
    }

    pub fn move_template_up(&mut self) {
        self.template_index = self.template_index.saturating_sub(1);
    }

    pub fn move_template_down(&mut self) {
        self.template_index = (self.template_index + 1).min(FormTemplate::ALL.len() - 1);
    }

    pub fn move_field_up(&mut self) {
        self.selected_field = self.selected_field.saturating_sub(1);
    }

    pub fn move_field_down(&mut self) {
        if !self.fields.is_empty() {
            self.selected_field = (self.selected_field + 1).min(self.fields.len() - 1);
        }
    }

    pub fn push_char(&mut self, value: char) {
        if let Some(field) = self.fields.get_mut(self.selected_field)
            && field.kind == FieldKind::Text
        {
            field.value.push(value);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(field) = self.fields.get_mut(self.selected_field)
            && field.kind == FieldKind::Text
        {
            field.value.pop();
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(field) = self.fields.get_mut(self.selected_field) {
            field.toggle();
        }
    }

    pub fn apply_suggestion(&mut self, snapshot: &TuiSnapshot) -> Option<String> {
        let next = self.selected_suggestion(snapshot)?;
        let field = self.fields.get_mut(self.selected_field)?;
        field.value = next.clone();
        Some(next)
    }

    pub fn selected_suggestion(&self, snapshot: &TuiSnapshot) -> Option<String> {
        let field = self.fields.get(self.selected_field)?;
        if field.kind != FieldKind::Text {
            return None;
        }
        let suggestions = field_suggestions(field.label.as_str(), snapshot);
        next_suggestion(field.value.as_str(), &suggestions)
    }

    pub fn build_action(&self, root: &str) -> Option<TuiAction> {
        let value = |label: &str| field_value(&self.fields, label);
        let enabled = |label: &str| field_enabled(&self.fields, label);
        let request = match self.template {
            FormTemplate::TaskStart => TuiActionRequest::TaskStart(dw_task::start::StartArgs {
                work_item_ids: value("Work item")
                    .as_deref()
                    .map(dw_core::WorkItemId::parse_many)
                    .unwrap_or_default(),
                root: Some(root.into()),
                project: value("Project").map(dw_core::ProjectKey::from),
                task: None,
                type_name: value("Type"),
                repositories: parse_workspace_repository_names(value("Repository").as_deref()),
                slug: value("Slug"),
                skip_ado: enabled("Skip ADO"),
                with_active_children: false,
                create_child_tasks: false,
                mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
            }),
            FormTemplate::TaskStartPr => {
                TuiActionRequest::TaskStartPr(dw_task::start::StartPrArgs {
                    pull_request_id: dw_core::PullRequestId::from(value("Pull request")?),
                    root: Some(root.into()),
                    project: dw_core::ProjectKey::from(value("Project")?),
                    repo: value("Repository"),
                    type_name: value("Type"),
                    slug: value("Slug"),
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                })
            }
            FormTemplate::TaskFinish => TuiActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                workspace: value("Workspace"),
                r#continue: enabled("Continue"),
                root: Some(root.into()),
                mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                yes: enabled("Execute"),
                message: value("Message"),
                create_pr: enabled("Create PR"),
                ready: enabled("Ready"),
                skip_verify: enabled("Skip verify"),
                skip_ado: enabled("Skip ADO"),
            }),
            FormTemplate::TaskTeardown => {
                TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    project: value("Project"),
                    work_item: value("Work item"),
                    r#continue: enabled("Continue"),
                    positional_work_item: None,
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                    yes: enabled("Execute"),
                })
            }
            FormTemplate::TaskPrune => TuiActionRequest::TaskPrune(dw_task::prune::PruneArgs {
                root: Some(root.into()),
                project: value("Project"),
                work_item: value("Work item"),
                mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                yes: enabled("Execute"),
                no_sync: enabled("No sync"),
            }),
            FormTemplate::TaskAddWorkItem => {
                TuiActionRequest::TaskAddWorkItem(dw_task::work_item::AddWorkItemArgs {
                    work_item_ids: value("Work items")
                        .as_deref()
                        .map(dw_core::WorkItemId::parse_many)
                        .unwrap_or_default(),
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    project: value("Project"),
                    work_item: value("Workspace work item"),
                    r#continue: enabled("Continue"),
                    positional_work_item: None,
                    skip_ado: enabled("Skip ADO"),
                    type_name: value("Type"),
                    title: value("Title"),
                    state: value("State"),
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                })
            }
            FormTemplate::TaskRemoveWorkItem => {
                TuiActionRequest::TaskRemoveWorkItem(dw_task::work_item::RemoveWorkItemArgs {
                    work_item_ids: value("Work items")
                        .as_deref()
                        .map(dw_core::WorkItemId::parse_many)
                        .unwrap_or_default(),
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    project: value("Project"),
                    work_item: value("Workspace work item"),
                    r#continue: enabled("Continue"),
                    positional_work_item: None,
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                })
            }
            FormTemplate::TaskAddRepo => {
                TuiActionRequest::TaskAddRepo(dw_task::repo::AddRepoArgs {
                    repo: value("Repository")?,
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                })
            }
            FormTemplate::TaskRename => {
                TuiActionRequest::TaskRename(dw_task::lifecycle::RenameArgs {
                    slug: value("Slug")?,
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    project: value("Project"),
                    work_item: value("Work item"),
                    r#continue: enabled("Continue"),
                    mode: dw_core::ExecutionMode::from_execute(enabled("Execute")),
                    positional_work_item: None,
                })
            }
            FormTemplate::AdoAssigned => {
                TuiActionRequest::AdoAssigned(dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(root.into()),
                    project: value("Project"),
                    top: value("Top")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(20),
                    all: enabled("Include final states"),
                    group_by_parent: enabled("Group by parent"),
                })
            }
            FormTemplate::AdoSetState => {
                TuiActionRequest::AdoSetState(dw_ado_commands::commands::set_state::SetStateArgs {
                    ids: dw_core::WorkItemId::parse_many(&value("Work item IDs")?),
                    root: Some(root.into()),
                    project: value("Project"),
                    state: value("Destination state")?,
                    history: value("ADO note"),
                    yes: true,
                })
            }
            FormTemplate::DbSchema => TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                project: value("Project"),
                database: value("Database"),
                env: None,
            }),
            FormTemplate::DbDescribe => {
                TuiActionRequest::DbDescribe(dw_db::commands::DescribeArgs {
                    table: value("Table"),
                    project: value("Project"),
                    database: value("Database"),
                    env: None,
                })
            }
            FormTemplate::DbQuery => TuiActionRequest::DbQuery(dw_db::commands::QueryArgs {
                sql: value("SQL")?,
                project: value("Project"),
                database: value("Database"),
                env: None,
                max_rows: value("Max rows").and_then(|value| value.parse().ok()),
            }),
            FormTemplate::AgentOpen => {
                TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                    workspace: value("Workspace"),
                    root: Some(root.into()),
                    project: value("Project"),
                    work_item: value("Work item"),
                    positional_work_item: None,
                    pull_request: None,
                    r#continue: enabled("Continue"),
                    repo: value("Repository"),
                    agent: value("Agent"),
                })
            }
            FormTemplate::Secret => {
                let key = value("Key")?;
                if enabled("Delete") {
                    TuiActionRequest::SecretDelete { key }
                } else if enabled("Set from env") {
                    TuiActionRequest::SecretSetFromEnv {
                        key,
                        env: value("From env")?,
                    }
                } else {
                    TuiActionRequest::SecretGet { key }
                }
            }
            FormTemplate::ConfigSetRoot => TuiActionRequest::ConfigSetRoot {
                path: value("Root")?,
            },
        };

        Some(TuiAction {
            label: format!("Composer · {}", self.template.label()),
            request,
            description: self.template.description().into(),
            kind: action_kind(self.template, enabled("Execute"), enabled("Delete")),
        })
    }
}

fn default_fields(template: FormTemplate, snapshot: &TuiSnapshot) -> Vec<FormField> {
    let first_project = snapshot
        .projects
        .projects
        .keys()
        .next()
        .cloned()
        .unwrap_or_default();
    let first_database = snapshot
        .databases
        .globals
        .keys()
        .next()
        .cloned()
        .or_else(|| {
            snapshot.databases.projects.values().find_map(|value| {
                value
                    .get("databases")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|items| items.keys().next().cloned())
            })
        })
        .unwrap_or_default();
    let first_database_entry = snapshot.database_entries.first();
    let first_database_key = first_database_entry
        .map(|database| database.key.clone())
        .unwrap_or_else(|| first_database.clone());
    let first_database_project = first_database_entry
        .and_then(|database| database.project.clone())
        .unwrap_or_else(|| first_project.clone());
    let first_workspace = snapshot
        .workspaces
        .first()
        .map(|workspace| workspace.path.clone())
        .unwrap_or_default();
    let first_repository = snapshot
        .workspaces
        .first()
        .and_then(|workspace| workspace.repositories.first().cloned())
        .unwrap_or_default();
    let first_assigned_work_item = snapshot
        .assigned_work_item_prompt_specs()
        .into_iter()
        .flat_map(|spec| spec.choices.into_iter())
        .map(|choice| choice.value.to_string())
        .next()
        .unwrap_or_default();
    let first_assigned_work_item_for_state = snapshot
        .assigned
        .iter()
        .flat_map(|project| project.items.iter())
        .map(|item| item.id.clone())
        .next()
        .unwrap_or_default();
    let first_state = state_suggestions(snapshot)
        .into_iter()
        .next()
        .unwrap_or_default();
    let first_pull_request = snapshot
        .pull_requests
        .iter()
        .find(|item| item.pull_request_id.is_some());
    let first_pull_request_id = first_pull_request
        .and_then(|item| item.pull_request_id)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let first_pull_request_project = first_pull_request
        .map(|item| item.project.clone())
        .unwrap_or_else(|| first_project.clone());
    let first_pull_request_repository = first_pull_request
        .map(|item| item.repository.clone())
        .unwrap_or_else(|| first_repository.clone());

    match template {
        FormTemplate::TaskStart => vec![
            FormField::text(
                "Work item",
                "Main or child ADO ID",
                first_assigned_work_item,
            ),
            FormField::text("Project", "Configured project", first_project.clone()),
            FormField::text("Repository", "Single repository to process", ""),
            FormField::text("Type", "feature, bugfix, hotfix or chore", "feature"),
            FormField::text("Slug", "Optional explicit slug", ""),
            FormField::toggle_field("Skip ADO", "Do not query Azure DevOps", false),
            FormField::toggle_field("Execute", "Actually create the workspace", false),
        ],
        FormTemplate::TaskStartPr => vec![
            FormField::text(
                "Pull request",
                "Azure DevOps PR ID; Ctrl+Space suggests loaded PRs",
                first_pull_request_id,
            ),
            FormField::text(
                "Project",
                "Configured project for the PR",
                first_pull_request_project,
            ),
            FormField::text(
                "Repository",
                "Local or Azure DevOps repository for the PR",
                first_pull_request_repository,
            ),
            FormField::text("Type", "feature, bugfix, hotfix or chore", "feature"),
            FormField::text("Slug", "Optional explicit slug", ""),
            FormField::toggle_field("Execute", "Actually create the workspace", false),
        ],
        FormTemplate::TaskFinish => vec![
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                first_workspace.clone(),
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text("Message", "Optional commit message", ""),
            FormField::toggle_field("Create PR", "Create or check ADO PRs", false),
            FormField::toggle_field("Ready", "Mark PR ready; requires Create PR", false),
            FormField::toggle_field("Skip verify", "Skip configured checks", false),
            FormField::toggle_field("Skip ADO", "Do not call Azure DevOps", false),
            FormField::toggle_field("Execute", "Actually commit/push/PR", false),
        ],
        FormTemplate::TaskTeardown => vec![
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                first_workspace,
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text(
                "Project",
                "Configured project when workspace is empty",
                first_project.clone(),
            ),
            FormField::text("Work item", "Work item when workspace is empty", ""),
            FormField::toggle_field("Execute", "Actually remove", false),
        ],
        FormTemplate::TaskPrune => vec![
            FormField::text(
                "Project",
                "Optional configured project",
                first_project.clone(),
            ),
            FormField::text("Work item", "Limit to a work item", ""),
            FormField::toggle_field("No sync", "Do not sync ADO before pruning", true),
            FormField::toggle_field("Execute", "Actually remove", false),
        ],
        FormTemplate::TaskAddWorkItem => vec![
            FormField::text("Work items", "IDs to add, comma-separated", ""),
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                first_workspace.clone(),
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text(
                "Project",
                "Configured project when workspace is empty",
                first_project.clone(),
            ),
            FormField::text(
                "Workspace work item",
                "Work item used to resolve the workspace",
                "",
            ),
            FormField::text("Type", "Local type when Skip ADO is enabled", ""),
            FormField::text("Title", "Local title when Skip ADO is enabled", ""),
            FormField::text("State", "Local state when Skip ADO is enabled", ""),
            FormField::toggle_field("Skip ADO", "Do not enrich from Azure DevOps", false),
            FormField::toggle_field("Execute", "Actually modify task.json", false),
        ],
        FormTemplate::TaskRemoveWorkItem => vec![
            FormField::text("Work items", "IDs to remove, comma-separated", ""),
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                first_workspace.clone(),
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text(
                "Project",
                "Configured project when workspace is empty",
                first_project.clone(),
            ),
            FormField::text(
                "Workspace work item",
                "Work item used to resolve the workspace",
                "",
            ),
            FormField::toggle_field("Execute", "Actually modify task.json", false),
        ],
        FormTemplate::TaskAddRepo => vec![
            FormField::text("Repository", "Configured repository to add", ""),
            FormField::text(
                "Workspace",
                "Workspace path to modify",
                first_workspace.clone(),
            ),
            FormField::toggle_field("Execute", "Actually create the worktree", false),
        ],
        FormTemplate::TaskRename => vec![
            FormField::text("Slug", "New workspace slug", ""),
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                first_workspace.clone(),
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text(
                "Project",
                "Configured project when workspace is empty",
                first_project.clone(),
            ),
            FormField::text("Work item", "Work item when workspace is empty", ""),
            FormField::toggle_field("Execute", "Actually rename", false),
        ],
        FormTemplate::AdoAssigned => vec![
            FormField::text(
                "Project",
                "Configured project; empty = interactive TUI choice",
                first_project.clone(),
            ),
            FormField::text("Top", "Maximum number of work items", "20"),
            FormField::toggle_field("Include final states", "Include final states", false),
            FormField::toggle_field("Group by parent", "Display by ADO parent", false),
        ],
        FormTemplate::AdoSetState => vec![
            FormField::text(
                "Work item IDs",
                "ADO IDs to update, comma-separated",
                first_assigned_work_item_for_state,
            ),
            FormField::text("Project", "Configured project", first_project.clone()),
            FormField::text("Destination state", "Exact ADO state", first_state),
            FormField::text("ADO note", "History message added on the work item", "tui"),
        ],
        FormTemplate::DbSchema => vec![
            FormField::text(
                "Project",
                "Optional configured project",
                first_database_project.clone(),
            ),
            FormField::text(
                "Database",
                "Database connection",
                first_database_key.clone(),
            ),
        ],
        FormTemplate::DbDescribe => vec![
            FormField::text("Table", "Table in table or schema.table format", ""),
            FormField::text(
                "Project",
                "Optional configured project",
                first_database_project.clone(),
            ),
            FormField::text(
                "Database",
                "Database connection",
                first_database_key.clone(),
            ),
        ],
        FormTemplate::DbQuery => vec![
            FormField::text(
                "Project",
                "Optional configured project",
                first_project.clone(),
            ),
            FormField::text("Database", "Database connection", first_database),
            FormField::text("SQL", "Read-only query", "select 1"),
            FormField::text("Max rows", "Row limit", "100"),
        ],
        FormTemplate::AgentOpen => vec![
            FormField::text(
                "Workspace",
                "Workspace path; empty when Continue is enabled",
                snapshot
                    .workspaces
                    .first()
                    .map(|workspace| workspace.path.clone())
                    .unwrap_or_default(),
            ),
            FormField::toggle_field("Continue", "Reuse the recent workspace", false),
            FormField::text(
                "Project",
                "Configured project when workspace is empty",
                first_project,
            ),
            FormField::text("Work item", "Work item when workspace is empty", ""),
            FormField::text("Repository", "Repository to open", first_repository),
            FormField::text(
                "Agent",
                "opencode, cursor, claude, codex, codex-cli or copilot",
                "",
            ),
        ],
        FormTemplate::Secret => vec![
            FormField::text("Key", "Logical secret key", ""),
            FormField::toggle_field("Set from env", "Store from an environment variable", false),
            FormField::text("From env", "Environment variable for secret set", ""),
            FormField::toggle_field("Delete", "Delete the key instead of get/set", false),
        ],
        FormTemplate::ConfigSetRoot => {
            vec![FormField::text(
                "Root",
                "New DevWorkflow root",
                &snapshot.root,
            )]
        }
    }
}

fn field_suggestions(label: &str, snapshot: &TuiSnapshot) -> Vec<String> {
    match label {
        "Project" => stable_unique(snapshot.projects.projects.keys().cloned()),
        "Workspace" => stable_unique(
            snapshot
                .workspaces
                .iter()
                .map(|workspace| workspace.path.clone()),
        ),
        "Repository" => repository_suggestions(snapshot),
        "Database" => stable_unique(
            snapshot
                .database_entries
                .iter()
                .map(|database| database.key.clone()),
        ),
        "Pull request" => pull_request_suggestions(snapshot),
        "Work item" | "Workspace work item" | "Work items" | "Work item IDs" => {
            work_item_suggestions(snapshot)
        }
        "Key" => secret_key_suggestions(snapshot),
        "From env" => environment_variable_suggestions(),
        "Agent" => dw_config::AGENT_DEFAULT_CHOICES
            .iter()
            .map(|agent| (*agent).to_string())
            .collect(),
        "Type" => ["feature", "bugfix", "hotfix", "chore"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        "State" | "Destination state" => state_suggestions(snapshot),
        _ => Vec::new(),
    }
}

fn state_suggestions(snapshot: &TuiSnapshot) -> Vec<String> {
    let start = dw_workspace::task_start_options(&snapshot.workflow);
    let finish = dw_workspace::task_finish_options(&snapshot.workflow);
    let mut values = vec![
        start.user_story_state,
        start.anomaly_state,
        start.bug_state,
        start.task_state,
        finish.bug_state,
        finish.task_state,
    ];
    values.extend(
        snapshot
            .assigned
            .iter()
            .flat_map(|project| project.items.iter().map(|item| item.state.clone())),
    );
    values.extend([
        "Nouveau".into(),
        "Actif".into(),
        "En réalisation".into(),
        "En développement".into(),
        "PR en attente".into(),
        "Validé".into(),
        "Clôturé".into(),
    ]);
    stable_unique(values.into_iter().filter(|value| !value.trim().is_empty()))
}

fn repository_suggestions(snapshot: &TuiSnapshot) -> Vec<String> {
    let mut values = Vec::new();
    for workspace in &snapshot.workspaces {
        values.extend(workspace.repositories.iter().cloned());
    }
    for project in snapshot.projects.projects.values() {
        if let Ok(project) = serde_json::from_value::<dw_config::ProjectConfig>(project.clone()) {
            values.extend(project.repositories.keys().cloned());
        }
    }
    stable_unique(values)
}

fn work_item_suggestions(snapshot: &TuiSnapshot) -> Vec<String> {
    let mut values = Vec::new();
    for workspace in &snapshot.workspaces {
        if !workspace.work_item_id.trim().is_empty() {
            values.push(workspace.work_item_id.clone());
        }
        if let Some(task_id) = workspace
            .task_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            values.push(task_id.to_string());
        }
    }
    for project in &snapshot.assigned {
        for item in &project.items {
            values.push(item.id.clone());
        }
    }
    stable_unique(values)
}

fn pull_request_suggestions(snapshot: &TuiSnapshot) -> Vec<String> {
    stable_unique(
        snapshot
            .pull_requests
            .iter()
            .filter_map(|item| item.pull_request_id.map(|id| id.to_string())),
    )
}

fn secret_key_suggestions(snapshot: &TuiSnapshot) -> Vec<String> {
    let globals = snapshot
        .databases
        .globals
        .values()
        .filter_map(database_credential_key);
    let projects = snapshot
        .databases
        .projects
        .values()
        .filter_map(|project| {
            project
                .get("databases")
                .and_then(serde_json::Value::as_object)
        })
        .flat_map(|databases| databases.values().filter_map(database_credential_key));

    stable_unique(globals.chain(projects).map(str::to_string))
}

fn database_credential_key(value: &serde_json::Value) -> Option<&str> {
    value
        .get("credentialKey")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn environment_variable_suggestions() -> Vec<String> {
    let mut values = std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn stable_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            unique.push(value);
        }
    }
    unique
}

fn next_suggestion(current: &str, suggestions: &[String]) -> Option<String> {
    if suggestions.is_empty() {
        return None;
    }
    let current = current.trim();
    let next_index = suggestions
        .iter()
        .position(|value| value == current)
        .map(|index| (index + 1) % suggestions.len())
        .unwrap_or_default();
    suggestions.get(next_index).cloned()
}

fn action_kind(template: FormTemplate, execute: bool, delete: bool) -> ActionRisk {
    match template {
        FormTemplate::TaskStart if execute => ActionRisk::Destructive,
        FormTemplate::TaskStartPr if execute => ActionRisk::Destructive,
        FormTemplate::TaskFinish
        | FormTemplate::TaskTeardown
        | FormTemplate::TaskPrune
        | FormTemplate::TaskAddWorkItem
        | FormTemplate::TaskRemoveWorkItem
        | FormTemplate::TaskAddRepo
        | FormTemplate::TaskRename
            if execute =>
        {
            ActionRisk::Destructive
        }
        FormTemplate::AgentOpen => ActionRisk::OpensExternal,
        FormTemplate::AdoSetState => ActionRisk::Destructive,
        FormTemplate::Secret if delete => ActionRisk::Destructive,
        FormTemplate::ConfigSetRoot => ActionRisk::Destructive,
        _ => ActionRisk::Safe,
    }
}

fn field_value(fields: &[FormField], label: &str) -> Option<String> {
    fields
        .iter()
        .find(|field| field.label == label)
        .map(|field| field.value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_workspace_repository_names(value: Option<&str>) -> Vec<dw_core::WorkspaceRepositoryName> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(dw_core::WorkspaceRepositoryName::from)
        .collect()
}

fn field_enabled(fields: &[FormField], label: &str) -> bool {
    fields
        .iter()
        .find(|field| field.label == label)
        .is_some_and(FormField::enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_config::{DatabasesConfig, ProjectsConfig, WorkflowConfig};

    fn snapshot() -> TuiSnapshot {
        TuiSnapshot {
            root: "/tmp/dw".into(),
            needs_init: false,
            projects: ProjectsConfig::default(),
            workflow: WorkflowConfig::default(),
            databases: DatabasesConfig::default(),
            database_entries: Vec::new(),
            config_doctor: dw_config::ConfigDoctorReport {
                root: "/tmp/dw".into(),
                checks: Vec::new(),
                passed: true,
            },
            workspaces: Vec::new(),
            assigned: Vec::new(),
            assigned_loaded: false,
            pull_requests: Vec::new(),
            pull_requests_loaded: false,
            prune_candidates: 0,
            actions: Vec::new(),
            color_mode: "auto".into(),
        }
    }

    #[test]
    fn task_start_form_builds_dry_run_action() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.begin_editing(&snapshot);
        form.fields[0].value = "42".into();

        let action = form.build_action(&snapshot.root).expect("action");

        match &action.request {
            TuiActionRequest::TaskStart(args) => {
                assert_eq!(args.work_item_ids, vec![dw_core::WorkItemId::from("42")]);
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
                assert!(!args.mode.executes());
            }
            _ => panic!("expected task start request"),
        }
    }

    #[test]
    fn task_start_execute_form_is_confirmed_but_background() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.begin_editing(&snapshot);
        form.fields[0].value = "42".into();
        form.fields
            .iter_mut()
            .find(|field| field.label == "Execute")
            .expect("execute field")
            .toggle();

        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(matches!(
            action.request,
            TuiActionRequest::TaskStart(ref args) if args.mode.executes()
        ));
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn task_start_pr_form_builds_preview_and_execute_actions() {
        let mut snapshot = snapshot();
        snapshot.projects.projects.insert(
            "ha".into(),
            serde_json::json!({"displayName": "HA", "repositories": {}}),
        );
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::TaskStartPr)
            .expect("PR workspace template");
        form.begin_editing(&snapshot);
        form.fields[0].value = "123".into();
        form.fields[1].value = "ha".into();
        form.fields[2].value = "front".into();

        let preview = form.build_action(&snapshot.root).expect("preview");
        match &preview.request {
            TuiActionRequest::TaskStartPr(args) => {
                assert_eq!(args.pull_request_id, dw_core::PullRequestId::from("123"));
                assert_eq!(args.project.as_str(), "ha");
                assert_eq!(args.repo.as_deref(), Some("front"));
                assert_eq!(args.type_name.as_deref(), Some("feature"));
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
                assert!(!args.mode.executes());
            }
            _ => panic!("expected PR workspace request"),
        }
        assert!(matches!(preview.kind, ActionRisk::Safe));

        form.fields
            .iter_mut()
            .find(|field| field.label == "Execute")
            .expect("execute field")
            .toggle();
        let execute = form.build_action(&snapshot.root).expect("execute");
        assert!(matches!(
            execute.request,
            TuiActionRequest::TaskStartPr(ref args) if args.mode.executes()
        ));
        assert!(matches!(execute.kind, ActionRisk::Destructive));
        assert!(!execute.runs_attached_in_tui());
    }

    #[test]
    fn db_query_form_requires_sql() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbQuery)
            .expect("db query template");
        form.begin_editing(&snapshot);
        form.fields[2].value.clear();

        assert!(form.build_action(&snapshot.root).is_none());
    }

    #[test]
    fn db_schema_form_uses_selected_database_defaults() {
        let mut snapshot = snapshot();
        snapshot.database_entries = vec![crate::model::TuiDatabase {
            project: Some("ha".into()),
            key: "ha-dev".into(),
        }];
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbSchema)
            .expect("db schema template");
        form.begin_editing(&snapshot);

        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(
            action.request,
            TuiActionRequest::DbSchema(ref args)
                if args.project.as_deref() == Some("ha")
                    && args.database.as_deref() == Some("ha-dev")
        ));
        assert!(matches!(action.kind, ActionRisk::Safe));
    }

    #[test]
    fn db_describe_form_builds_read_only_request() {
        let mut snapshot = snapshot();
        snapshot.database_entries = vec![crate::model::TuiDatabase {
            project: None,
            key: "shared".into(),
        }];
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbDescribe)
            .expect("db describe template");
        form.begin_editing(&snapshot);
        form.fields
            .iter_mut()
            .find(|field| field.label == "Table")
            .expect("table field")
            .value = "dbo.Customer".into();
        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(
            action.request,
            TuiActionRequest::DbDescribe(ref args)
                if args.table.as_deref() == Some("dbo.Customer")
                    && args.database.as_deref() == Some("shared")
        ));
        assert!(matches!(action.kind, ActionRisk::Safe));
    }

    #[test]
    fn task_finish_execute_form_is_destructive() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::TaskFinish)
            .expect("finish template");
        form.begin_editing(&snapshot);
        form.fields
            .iter_mut()
            .find(|field| field.label == "Continue")
            .expect("continue field")
            .toggle();
        form.fields
            .iter_mut()
            .find(|field| field.label == "Execute")
            .expect("execute field")
            .toggle();

        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(matches!(
            action.request,
            TuiActionRequest::TaskFinish(ref args) if args.mode.executes() && args.yes
        ));
    }

    #[test]
    fn ado_set_state_form_builds_confirmed_background_action() {
        let mut snapshot = snapshot();
        snapshot
            .projects
            .projects
            .insert("ha".into(), serde_json::json!({"displayName": "HA"}));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::AdoSetState)
            .expect("ado set-state template");
        form.begin_editing(&snapshot);
        form.fields[0].value = "42,43".into();
        form.fields[1].value = "ha".into();
        form.fields[2].value = "En réalisation".into();

        let action = form.build_action(&snapshot.root).expect("action");

        match &action.request {
            TuiActionRequest::AdoSetState(args) => {
                assert_eq!(
                    args.ids,
                    vec![
                        dw_core::WorkItemId::from("42"),
                        dw_core::WorkItemId::from("43")
                    ]
                );
                assert_eq!(args.state, "En réalisation");
                assert_eq!(args.project.as_deref(), Some("ha"));
                assert_eq!(args.history.as_deref(), Some("tui"));
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
                assert!(args.yes);
            }
            _ => panic!("expected ado set-state request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(action.bypasses_cli_confirmation());
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn secret_delete_form_is_destructive() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::Secret)
            .expect("secret template");
        form.begin_editing(&snapshot);
        form.fields[0].value = "db/password".into();
        form.fields[3].toggle();

        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(
            action.request,
            TuiActionRequest::SecretDelete { ref key } if key == "db/password"
        ));
        assert!(matches!(action.kind, ActionRisk::Destructive));
    }

    #[test]
    fn secret_set_from_env_form_requires_env_and_builds_safe_action() {
        let snapshot = snapshot();
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::Secret)
            .expect("secret template");
        form.begin_editing(&snapshot);
        form.fields[0].value = "db/password".into();
        form.fields[1].toggle();

        assert!(form.build_action(&snapshot.root).is_none());

        form.fields[2].value = "DW_DB_PASSWORD".into();
        let action = form.build_action(&snapshot.root).expect("action");

        assert!(matches!(
            action.request,
            TuiActionRequest::SecretSetFromEnv { ref key, ref env }
                if key == "db/password" && env == "DW_DB_PASSWORD"
        ));
        assert!(matches!(action.kind, ActionRisk::Safe));
    }

    #[test]
    fn suggestion_cycles_project_values_from_config() {
        let mut snapshot = snapshot();
        snapshot
            .projects
            .projects
            .insert("ops".into(), serde_json::json!({"displayName": "OPS"}));
        snapshot
            .projects
            .projects
            .insert("ha".into(), serde_json::json!({"displayName": "HA"}));
        let mut form = FormState::selecting();
        form.begin_editing(&snapshot);
        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Project")
            .expect("project field");
        form.fields[form.selected_field].value.clear();

        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("ops"));
        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("ha"));
    }

    #[test]
    fn selected_suggestion_previews_next_text_value_only() {
        let mut snapshot = snapshot();
        snapshot
            .projects
            .projects
            .insert("ha".into(), serde_json::json!({"displayName": "HA"}));
        let mut form = FormState::selecting();
        form.begin_editing(&snapshot);
        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Project")
            .expect("project field");
        form.fields[form.selected_field].value.clear();

        assert_eq!(form.selected_suggestion(&snapshot).as_deref(), Some("ha"));

        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.kind == FieldKind::Toggle)
            .expect("toggle field");
        assert!(form.selected_suggestion(&snapshot).is_none());
    }

    #[test]
    fn suggestion_reads_agent_values_from_config_choices() {
        let snapshot = snapshot();
        let suggestions = field_suggestions("Agent", &snapshot);

        assert_eq!(
            suggestions,
            dw_config::AGENT_DEFAULT_CHOICES
                .iter()
                .map(|agent| (*agent).to_string())
                .collect::<Vec<_>>()
        );
        assert!(suggestions.contains(&"codex-cli".into()));
    }

    #[test]
    fn suggestion_reads_secret_keys_from_database_credentials() {
        let mut snapshot = snapshot();
        snapshot.databases.globals.insert(
            "shared".into(),
            serde_json::json!({
                "provider": "sqlserver",
                "credentialKey": "db/shared"
            }),
        );
        snapshot.databases.projects.insert(
            "ha".into(),
            serde_json::json!({
                "databases": {
                    "ha-dev": {
                        "provider": "sqlserver",
                        "credentialKey": "db/ha-dev"
                    },
                    "inline": {
                        "provider": "sqlserver",
                        "connectionString": "Server=."
                    }
                }
            }),
        );
        let suggestions = field_suggestions("Key", &snapshot);

        assert_eq!(suggestions, vec!["db/shared", "db/ha-dev"]);
    }

    #[test]
    fn suggestion_reads_from_env_values_locally() {
        let suggestions = field_suggestions("From env", &snapshot());

        if std::env::var_os("PATH").is_some() {
            assert!(suggestions.iter().any(|value| value == "PATH"));
        }
        assert!(suggestions.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn suggestion_reads_repository_from_workspaces_and_config() {
        let mut snapshot = snapshot();
        snapshot.projects.projects.insert(
            "ha".into(),
            serde_json::json!({
                "displayName": "HA",
                "repositories": {
                    "back": {"url": "git@example/back", "defaultBranch": "develop"}
                }
            }),
        );
        snapshot.workspaces = vec![dw_workspace::TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            display_work_items: "#42 Demo".into(),
            task_id: None,
            all_known_work_item_ids: vec!["42".into()],
            kind: "feature".into(),
            slug: "demo".into(),
            branch_name: "feature/42-demo".into(),
            created_at: "2026-07-04T00:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Demo".into()),
            work_item_state: Some("Active".into()),
            repositories: vec!["front".into()],
        }];
        let mut form = FormState::selecting();
        form.begin_editing(&snapshot);
        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Repository")
            .expect("repository field");

        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("front"));
        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("back"));
    }

    #[test]
    fn suggestion_reads_database_entries() {
        let mut snapshot = snapshot();
        snapshot.database_entries = vec![
            crate::model::TuiDatabase {
                project: None,
                key: "shared".into(),
            },
            crate::model::TuiDatabase {
                project: Some("ha".into()),
                key: "ha-dev".into(),
            },
        ];
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbQuery)
            .expect("db query template");
        form.begin_editing(&snapshot);
        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Database")
            .expect("database field");
        form.fields[form.selected_field].value.clear();

        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("shared"));
        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("ha-dev"));
    }

    #[test]
    fn start_pr_form_prefills_and_suggests_loaded_pull_requests() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![
            crate::model::TuiPullRequest {
                workspace: None,
                project: "ha".into(),
                repository: "front".into(),
                ado_repository: "HA Front".into(),
                branch: "feature/42-demo".into(),
                target_branch: "develop".into(),
                pull_request_id: Some(42),
                title: Some("Demo".into()),
                is_draft: false,
                work_item_ids: vec!["123".into()],
                url: None,
                error: None,
            },
            crate::model::TuiPullRequest {
                workspace: None,
                project: "ops".into(),
                repository: "tools".into(),
                ado_repository: "OPS Tools".into(),
                branch: "feature/77-tools".into(),
                target_branch: "main".into(),
                pull_request_id: Some(77),
                title: Some("Tools".into()),
                is_draft: false,
                work_item_ids: vec!["777".into()],
                url: None,
                error: None,
            },
        ];
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::TaskStartPr)
            .expect("PR workspace template");
        form.begin_editing(&snapshot);

        assert_eq!(
            field_value(&form.fields, "Pull request").as_deref(),
            Some("42")
        );
        assert_eq!(field_value(&form.fields, "Project").as_deref(), Some("ha"));
        assert_eq!(
            field_value(&form.fields, "Repository").as_deref(),
            Some("front")
        );

        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Pull request")
            .expect("pull request field");
        form.fields[form.selected_field].value.clear();
        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("42"));
        assert_eq!(form.apply_suggestion(&snapshot).as_deref(), Some("77"));
    }

    #[test]
    fn suggestion_reads_states_from_workflow_and_assigned_items() {
        let mut snapshot = snapshot();
        snapshot.assigned = vec![crate::model::AdoAssignedProject {
            key: "ha".into(),
            label: "HA".into(),
            items: vec![crate::model::AdoAssignedItem {
                id: "42".into(),
                kind: "Task".into(),
                state: "Custom Review".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::AdoSetState)
            .expect("ado set-state template");
        form.begin_editing(&snapshot);
        form.selected_field = form
            .fields
            .iter()
            .position(|field| field.label == "Destination state")
            .expect("state field");
        form.fields[form.selected_field].value.clear();

        assert_eq!(
            form.apply_suggestion(&snapshot).as_deref(),
            Some("En réalisation")
        );
        assert_eq!(
            form.apply_suggestion(&snapshot).as_deref(),
            Some("En développement")
        );
        assert_eq!(
            state_suggestions(&snapshot).last().map(String::as_str),
            Some("Clôturé")
        );
        assert!(state_suggestions(&snapshot).contains(&"Custom Review".into()));
    }
}
