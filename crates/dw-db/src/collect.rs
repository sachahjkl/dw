use anyhow::{Context, Result};
use dw_config::{load_projects_config, resolve_project, resolve_root};
use dw_core::{
    DatabaseEnvironmentName, DatabaseKey, DevWorkflowRoot, ExecutionMode, ProjectKey, SecretKey,
    SecretValue, WorkspaceRepositoryName,
};
use dw_secret::{KeyringSecretStore, SecretStore};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_APPSETTINGS_BYTES: u64 = 5 * 1024 * 1024;
const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".idea",
    ".vs",
    ".vscode",
    "bin",
    "dist",
    "node_modules",
    "obj",
    "out",
    "target",
];

#[derive(Debug, Clone)]
pub struct ListArgs {
    pub root: Option<DevWorkflowRoot>,
}

#[derive(Debug, Clone)]
pub struct CollectArgs {
    pub root: Option<DevWorkflowRoot>,
    pub mode: ExecutionMode,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DatabaseListReport {
    pub root: DevWorkflowRoot,
    pub entries: Vec<DatabaseListEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DatabaseListEntry {
    pub project: Option<ProjectKey>,
    pub database: DatabaseKey,
    pub provider: String,
    pub source: DatabaseConnectionSource,
    pub readonly: bool,
    #[serde(rename = "maxRows")]
    pub max_rows: usize,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DatabaseConnectionSource {
    Credential { key: SecretKey },
    Environment { variable: DatabaseEnvironmentName },
    Inline { value_masked: bool },
    Missing,
    Multiple,
}

impl std::fmt::Display for DatabaseConnectionSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credential { key } => write!(formatter, "credential:{key}"),
            Self::Environment { variable } => write!(formatter, "environment:{variable}"),
            Self::Inline { .. } => formatter.write_str("inline:<hidden>"),
            Self::Missing => formatter.write_str("missing"),
            Self::Multiple => formatter.write_str("multiple"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DatabaseCollectReport {
    pub root: DevWorkflowRoot,
    #[serde(rename = "saveRequested")]
    pub save_requested: bool,
    #[serde(rename = "scannedWorkspaces")]
    pub scanned_workspaces: usize,
    #[serde(rename = "scannedFiles")]
    pub scanned_files: usize,
    #[serde(rename = "savedCount")]
    pub saved_count: usize,
    pub findings: Vec<DatabaseCollectFinding>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DatabaseCollectFinding {
    pub project: ProjectKey,
    pub repository: WorkspaceRepositoryName,
    pub application: String,
    pub environment: DatabaseEnvironmentName,
    pub name: String,
    pub database: DatabaseKey,
    #[serde(rename = "credentialKey")]
    pub credential_key: SecretKey,
    pub status: DatabaseCollectStatus,
    pub detail: Option<String>,
    #[serde(rename = "valueMasked")]
    pub value_masked: bool,
    #[serde(rename = "sourcePaths")]
    pub source_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseCollectStatus {
    Eligible,
    Saved,
    AlreadyConfigured,
    Skipped,
    Conflict,
}

impl std::fmt::Display for DatabaseCollectStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Eligible => "eligible",
            Self::Saved => "saved",
            Self::AlreadyConfigured => "already configured",
            Self::Skipped => "skipped",
            Self::Conflict => "conflict",
        })
    }
}

struct DatabaseCandidate {
    finding: DatabaseCollectFinding,
    value: SecretValue,
}

pub fn list_databases(args: ListArgs) -> Result<DatabaseListReport> {
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let config = load_databases_config_checked(&root)?;
    let (entries, warnings) = database_inventory(&config);
    Ok(DatabaseListReport {
        root,
        entries,
        warnings,
    })
}

pub fn collect_databases(args: CollectArgs) -> Result<DatabaseCollectReport> {
    collect_databases_with_store(args, &KeyringSecretStore)
}

fn collect_databases_with_store(
    args: CollectArgs,
    store: &impl SecretStore,
) -> Result<DatabaseCollectReport> {
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let projects = load_projects_config(root.as_str());
    let workspaces = dw_workspace::find_workspaces(root.as_str());
    let mut candidates = Vec::<DatabaseCandidate>::new();
    let mut identities = BTreeMap::<String, usize>::new();
    let mut warnings = Vec::new();
    let mut scanned_files = 0;

    for workspace in &workspaces {
        let project = resolve_project(&projects, workspace.manifest.project.as_str());
        for repository in &workspace.manifest.repositories {
            let repository_root = match dw_workspace::resolve_open_target(
                &workspace.path,
                &workspace.manifest,
                project.as_ref(),
                Some(repository.as_str()),
            ) {
                Ok(path) => PathBuf::from(path),
                Err(error) => {
                    warnings.push(format!(
                        "Could not resolve {}/{} in '{}': {error}",
                        workspace.manifest.project, repository, workspace.path
                    ));
                    continue;
                }
            };
            scan_repository(
                &repository_root,
                &workspace.manifest.project,
                repository,
                &mut candidates,
                &mut identities,
                &mut scanned_files,
                &mut warnings,
            );
        }
    }

    if args.mode.executes() {
        save_candidates(&root, &mut candidates, store)?;
    }
    let findings = candidates
        .into_iter()
        .map(|candidate| candidate.finding)
        .collect::<Vec<_>>();
    let saved_count = findings
        .iter()
        .filter(|finding| finding.status == DatabaseCollectStatus::Saved)
        .count();
    Ok(DatabaseCollectReport {
        root,
        save_requested: args.mode.executes(),
        scanned_workspaces: workspaces.len(),
        scanned_files,
        saved_count,
        findings,
        warnings,
    })
}

fn load_databases_config_checked(root: &DevWorkflowRoot) -> Result<dw_config::DatabasesConfig> {
    let path = databases_path(root);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("reading database configuration '{}'", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parsing database configuration '{}'", path.display()))
}

fn database_inventory(
    config: &dw_config::DatabasesConfig,
) -> (Vec<DatabaseListEntry>, Vec<String>) {
    let readonly = config
        .defaults
        .as_ref()
        .and_then(|value| value.get("readonly"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_rows = config
        .defaults
        .as_ref()
        .and_then(|value| value.get("maxRows"))
        .and_then(Value::as_u64)
        .and_then(|value| value.try_into().ok())
        .unwrap_or(500);
    let timeout_seconds = config
        .defaults
        .as_ref()
        .and_then(|value| value.get("timeoutSeconds"))
        .and_then(Value::as_u64)
        .unwrap_or(600);
    let mut entries = Vec::new();
    let mut warnings = Vec::new();

    for (database, value) in &config.globals {
        match inventory_entry(None, database, value, readonly, max_rows, timeout_seconds) {
            Some(entry) => entries.push(entry),
            None => warnings.push(format!("Invalid global database entry: {database}")),
        }
    }
    for (project, value) in &config.projects {
        let Some(databases) = value.get("databases").and_then(Value::as_object) else {
            warnings.push(format!("Invalid project database section: {project}"));
            continue;
        };
        for (database, value) in databases {
            match inventory_entry(
                Some(ProjectKey::from(project.as_str())),
                database,
                value,
                readonly,
                max_rows,
                timeout_seconds,
            ) {
                Some(entry) => entries.push(entry),
                None => warnings.push(format!("Invalid database entry: {project}/{database}")),
            }
        }
    }
    (entries, warnings)
}

fn inventory_entry(
    project: Option<ProjectKey>,
    database: &str,
    value: &Value,
    default_readonly: bool,
    default_max_rows: usize,
    default_timeout_seconds: u64,
) -> Option<DatabaseListEntry> {
    let object = value.as_object()?;
    let provider = object.get("provider")?.as_str()?.to_string();
    let inline = non_empty_string(object.get("connectionString"));
    let environment = non_empty_string(object.get("connectionStringEnvironmentVariable"));
    let credential = non_empty_string(object.get("credentialKey"));
    let source_count = usize::from(inline.is_some())
        + usize::from(environment.is_some())
        + usize::from(credential.is_some());
    let source = match source_count {
        0 => DatabaseConnectionSource::Missing,
        2.. => DatabaseConnectionSource::Multiple,
        _ if inline.is_some() => DatabaseConnectionSource::Inline { value_masked: true },
        _ if environment.is_some() => DatabaseConnectionSource::Environment {
            variable: DatabaseEnvironmentName::from(environment.unwrap_or_default()),
        },
        _ => DatabaseConnectionSource::Credential {
            key: SecretKey::from(credential.unwrap_or_default()),
        },
    };
    let readonly = object
        .get("readonly")
        .and_then(Value::as_bool)
        .unwrap_or(default_readonly);
    let mut warnings = Vec::new();
    if !provider.eq_ignore_ascii_case("sqlserver") {
        warnings.push("unsupported provider".into());
    }
    if source_count == 0 {
        warnings.push("missing connection source".into());
    } else if source_count > 1 {
        warnings.push("multiple connection sources".into());
    }
    if !readonly {
        warnings.push("readonly is false".into());
    }
    Some(DatabaseListEntry {
        project,
        database: DatabaseKey::from(database),
        provider,
        source,
        readonly,
        max_rows: object
            .get("maxRows")
            .and_then(Value::as_u64)
            .and_then(|value| value.try_into().ok())
            .unwrap_or(default_max_rows),
        timeout_seconds: object
            .get("timeoutSeconds")
            .and_then(Value::as_u64)
            .unwrap_or(default_timeout_seconds),
        warnings,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_repository(
    repository_root: &Path,
    project: &ProjectKey,
    repository: &WorkspaceRepositoryName,
    candidates: &mut Vec<DatabaseCandidate>,
    identities: &mut BTreeMap<String, usize>,
    scanned_files: &mut usize,
    warnings: &mut Vec<String>,
) {
    let mut files = Vec::new();
    collect_appsettings_files(repository_root, repository_root, &mut files, warnings);
    for file in files {
        *scanned_files += 1;
        scan_appsettings_file(
            repository_root,
            &file,
            project,
            repository,
            candidates,
            identities,
            warnings,
        );
    }
}

fn collect_appsettings_files(
    repository_root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            if directory == repository_root {
                warnings.push(format!(
                    "Could not scan repository '{}': {error}",
                    repository_root.display()
                ));
            }
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                collect_appsettings_files(repository_root, &path, files, warnings);
            }
        } else if file_type.is_file() && is_appsettings_file(&path) {
            files.push(path);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_appsettings_file(
    repository_root: &Path,
    file: &Path,
    project: &ProjectKey,
    repository: &WorkspaceRepositoryName,
    candidates: &mut Vec<DatabaseCandidate>,
    identities: &mut BTreeMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    if fs::metadata(file)
        .map(|metadata| metadata.len() > MAX_APPSETTINGS_BYTES)
        .unwrap_or(false)
    {
        warnings.push(format!(
            "Skipped oversized appsettings file: {}",
            file.display()
        ));
        return;
    }
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("Could not read '{}': {error}", file.display()));
            return;
        }
    };
    let value: Value = match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("Could not parse '{}': {error}", file.display()));
            return;
        }
    };
    let Some(root) = value.as_object() else {
        return;
    };
    let Some(connection_strings) = root
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("ConnectionStrings"))
        .and_then(|(_, value)| value.as_object())
    else {
        return;
    };
    let application = application_name(repository_root, file);
    let environment = appsettings_environment(file);
    for (name, value) in connection_strings {
        let Some(connection_string) = value.as_str().filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let eligible = is_sql_server_connection_string(connection_string);
        let database = DatabaseKey::from(generated_database_key(
            repository.as_str(),
            &application,
            environment.as_str(),
            name,
        ));
        let credential_key = SecretKey::from(generated_credential_key(
            project.as_str(),
            repository.as_str(),
            &application,
            environment.as_str(),
            name,
        ));
        let identity = format!(
            "{}|{}|{}|{}|{}",
            project, repository, application, environment, name
        )
        .to_ascii_lowercase();
        let source_path = file.display().to_string();
        if let Some(index) = identities.get(&identity).copied() {
            let existing = &mut candidates[index];
            if existing.value.as_str() == connection_string {
                if !existing.finding.source_paths.contains(&source_path) {
                    existing.finding.source_paths.push(source_path);
                }
            } else {
                existing.finding.status = DatabaseCollectStatus::Conflict;
                existing.finding.detail = Some(
                    "Different values were found for the same logical connection; nothing will be saved."
                        .into(),
                );
                if !existing.finding.source_paths.contains(&source_path) {
                    existing.finding.source_paths.push(source_path);
                }
            }
            continue;
        }
        identities.insert(identity.clone(), candidates.len());
        candidates.push(DatabaseCandidate {
            finding: DatabaseCollectFinding {
                project: project.clone(),
                repository: repository.clone(),
                application: application.clone(),
                environment: environment.clone(),
                name: name.clone(),
                database,
                credential_key,
                status: if eligible {
                    DatabaseCollectStatus::Eligible
                } else {
                    DatabaseCollectStatus::Skipped
                },
                detail: (!eligible)
                    .then(|| "Not recognized as a concrete SQL Server connection string.".into()),
                value_masked: true,
                source_paths: vec![source_path],
            },
            value: SecretValue::from(connection_string),
        });
    }
}

fn save_candidates(
    root: &DevWorkflowRoot,
    candidates: &mut [DatabaseCandidate],
    store: &impl SecretStore,
) -> Result<()> {
    let path = databases_path(root);
    let original = fs::read_to_string(&path)
        .with_context(|| format!("reading database configuration '{}'", path.display()))?;
    let mut config: Value = serde_json::from_str(&original)
        .with_context(|| format!("parsing database configuration '{}'", path.display()))?;
    let mut newly_stored = Vec::<SecretKey>::new();
    let mut config_changed = false;

    for candidate in candidates
        .iter_mut()
        .filter(|candidate| candidate.finding.status == DatabaseCollectStatus::Eligible)
    {
        let existing = configured_database(
            &config,
            &candidate.finding.project,
            &candidate.finding.database,
        );
        let config_already_matches = existing.is_some_and(|value| {
            value
                .get("provider")
                .and_then(Value::as_str)
                .is_some_and(|provider| provider.eq_ignore_ascii_case("sqlserver"))
                && value
                    .get("credentialKey")
                    .and_then(Value::as_str)
                    .is_some_and(|key| key == candidate.finding.credential_key.as_str())
        });
        if existing.is_some() && !config_already_matches {
            candidate.finding.status = DatabaseCollectStatus::Conflict;
            candidate.finding.detail = Some(
                "A different databases.json entry already uses this generated database key.".into(),
            );
            continue;
        }

        match store.get(&candidate.finding.credential_key)? {
            Some(value) if value != candidate.value => {
                candidate.finding.status = DatabaseCollectStatus::Conflict;
                candidate.finding.detail = Some(
                    "The generated credential key already contains a different secret.".into(),
                );
                continue;
            }
            Some(_) => {}
            None => {
                if let Err(error) = store.set(&candidate.finding.credential_key, &candidate.value) {
                    rollback_secrets(store, &newly_stored);
                    return Err(error.into());
                }
                newly_stored.push(candidate.finding.credential_key.clone());
            }
        }

        if !config_already_matches {
            if let Err(error) = insert_database_reference(
                &mut config,
                &candidate.finding.project,
                &candidate.finding.database,
                &candidate.finding.credential_key,
            ) {
                rollback_secrets(store, &newly_stored);
                return Err(error);
            }
            config_changed = true;
        }
        candidate.finding.status = if config_already_matches
            && !newly_stored.contains(&candidate.finding.credential_key)
        {
            DatabaseCollectStatus::AlreadyConfigured
        } else {
            DatabaseCollectStatus::Saved
        };
        candidate.finding.detail = None;
    }

    if config_changed && let Err(error) = persist_database_config(&path, &original, &config) {
        rollback_secrets(store, &newly_stored);
        return Err(error);
    }
    Ok(())
}

fn configured_database<'a>(
    config: &'a Value,
    project: &ProjectKey,
    database: &DatabaseKey,
) -> Option<&'a Value> {
    config
        .get("projects")?
        .get(project.as_str())?
        .get("databases")?
        .get(database.as_str())
}

fn insert_database_reference(
    config: &mut Value,
    project: &ProjectKey,
    database: &DatabaseKey,
    credential_key: &SecretKey,
) -> Result<()> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("databases.json must contain a JSON object"))?;
    let projects = object_entry(root, "projects")?;
    let project_node = projects
        .entry(project.to_string())
        .or_insert_with(|| json!({ "databases": {} }));
    let project_object = project_node
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("databases.json project '{}' must be an object", project))?;
    let databases = object_entry(project_object, "databases")?;
    databases.insert(
        database.to_string(),
        json!({
            "provider": "sqlserver",
            "credentialKey": credential_key.as_str(),
            "readonly": true
        }),
    );
    Ok(())
}

fn object_entry<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>> {
    object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("databases.json '{key}' must be an object"))
}

fn persist_database_config(path: &Path, original: &str, value: &Value) -> Result<()> {
    let current = fs::read_to_string(path)
        .with_context(|| format!("re-reading database configuration '{}'", path.display()))?;
    if current != original {
        return Err(anyhow::anyhow!(
            "databases.json changed while connections were being collected; no configuration was written."
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("database configuration has no parent directory"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    temporary
        .persist(path)
        .map_err(|error| anyhow::Error::new(error.error))?;
    Ok(())
}

fn rollback_secrets(store: &impl SecretStore, keys: &[SecretKey]) {
    for key in keys {
        let _ = store.delete(key);
    }
}

fn databases_path(root: &DevWorkflowRoot) -> PathBuf {
    Path::new(root.as_str())
        .join("config")
        .join("databases.json")
}

fn is_appsettings_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    name == "appsettings.json" || (name.starts_with("appsettings.") && name.ends_with(".json"))
}

fn appsettings_environment(path: &Path) -> DatabaseEnvironmentName {
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("appsettings");
    let environment = name
        .strip_prefix("appsettings.")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    DatabaseEnvironmentName::from(environment)
}

fn application_name(repository_root: &Path, file: &Path) -> String {
    let parent = file.parent().unwrap_or(repository_root);
    let relative = parent.strip_prefix(repository_root).unwrap_or(parent);
    let joined = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("-");
    if joined.is_empty() {
        "root".into()
    } else {
        sanitize_segment(&joined)
    }
}

fn is_sql_server_connection_string(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.contains("${")
        || normalized.contains("#{")
        || normalized.contains("{{")
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
    {
        return false;
    }
    let has_server = [
        "server=",
        "data source=",
        "address=",
        "addr=",
        "network address=",
    ]
    .iter()
    .any(|marker| normalized.contains(marker));
    let has_database = ["database=", "initial catalog="]
        .iter()
        .any(|marker| normalized.contains(marker));
    has_server && has_database && normalized.contains(';')
}

fn generated_database_key(
    repository: &str,
    application: &str,
    environment: &str,
    name: &str,
) -> String {
    ["collected", repository, application, environment, name]
        .into_iter()
        .map(sanitize_segment)
        .collect::<Vec<_>>()
        .join("-")
}

fn generated_credential_key(
    project: &str,
    repository: &str,
    application: &str,
    environment: &str,
    name: &str,
) -> String {
    [
        "db",
        "collected",
        "v1",
        project,
        repository,
        application,
        environment,
        name,
    ]
    .into_iter()
    .map(sanitize_segment)
    .collect::<Vec<_>>()
    .join(".")
}

fn sanitize_segment(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            previous_dash = false;
        } else if !previous_dash && !output.is_empty() {
            output.push('-');
            previous_dash = true;
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "default".into()
    } else {
        output.chars().take(48).collect()
    }
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_secret::MemorySecretStore;

    const SECRET: &str =
        "Server=sql.example.test;Database=Application;User Id=reader;Password=very-secret";

    #[test]
    fn inventory_masks_inline_connection_strings() {
        let config: dw_config::DatabasesConfig = serde_json::from_value(json!({
            "defaults": { "readonly": true, "maxRows": 100, "timeoutSeconds": 30 },
            "globals": {
                "dev": { "provider": "sqlserver", "connectionString": SECRET }
            },
            "projects": {}
        }))
        .expect("database config");

        let (entries, warnings) = database_inventory(&config);
        let serialized = serde_json::to_string(&entries).expect("serialized inventory");

        assert!(warnings.is_empty());
        assert_eq!(
            entries[0].source,
            DatabaseConnectionSource::Inline { value_masked: true }
        );
        assert!(!serialized.contains(SECRET));
        assert!(!format!("{entries:?}").contains(SECRET));
    }

    #[test]
    fn scans_appsettings_and_reports_only_masked_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("appsettings.Development.json");
        fs::write(
            &file,
            serde_json::to_string(&json!({
                "ConnectionStrings": { "Application": SECRET }
            }))
            .expect("appsettings json"),
        )
        .expect("appsettings");
        let mut candidates = Vec::new();
        let mut identities = BTreeMap::new();
        let mut scanned_files = 0;
        let mut warnings = Vec::new();

        scan_repository(
            temp.path(),
            &ProjectKey::from("acme"),
            &WorkspaceRepositoryName::from("api"),
            &mut candidates,
            &mut identities,
            &mut scanned_files,
            &mut warnings,
        );

        assert_eq!(scanned_files, 1);
        assert!(warnings.is_empty());
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].finding.status,
            DatabaseCollectStatus::Eligible
        );
        assert_eq!(candidates[0].finding.environment.as_str(), "Development");
        let serialized = serde_json::to_string(&candidates[0].finding).expect("serialized finding");
        assert!(!serialized.contains("very-secret"));
        assert!(serialized.contains("valueMasked"));
    }

    #[test]
    fn conflicting_workspace_values_are_not_eligible_to_save() {
        let first = tempfile::tempdir().expect("first tempdir");
        let second = tempfile::tempdir().expect("second tempdir");
        write_appsettings(first.path(), SECRET);
        write_appsettings(
            second.path(),
            "Server=other.example.test;Database=Application;User Id=reader;Password=other",
        );
        let mut candidates = Vec::new();
        let mut identities = BTreeMap::new();
        let mut scanned_files = 0;
        let mut warnings = Vec::new();

        for root in [first.path(), second.path()] {
            scan_repository(
                root,
                &ProjectKey::from("acme"),
                &WorkspaceRepositoryName::from("api"),
                &mut candidates,
                &mut identities,
                &mut scanned_files,
                &mut warnings,
            );
        }

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].finding.status,
            DatabaseCollectStatus::Conflict
        );
        assert_eq!(candidates[0].finding.source_paths.len(), 2);
    }

    #[test]
    fn save_writes_only_a_credential_reference_and_is_idempotent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_directory = temp.path().join("config");
        fs::create_dir(&config_directory).expect("config directory");
        let config_path = config_directory.join("databases.json");
        fs::write(
            &config_path,
            r#"{
  "$schema": "../schemas/databases.schema.json",
  "schema": 1,
  "defaults": { "readonly": true, "maxRows": 500, "timeoutSeconds": 600 },
  "globals": {},
  "projects": { "acme": { "databases": {} } }
}"#,
        )
        .expect("database config");
        let root = DevWorkflowRoot::from(temp.path().display().to_string());
        let store = MemorySecretStore::new();
        let mut candidates = vec![candidate()];

        save_candidates(&root, &mut candidates, &store).expect("save candidate");

        let saved_config = fs::read_to_string(&config_path).expect("saved config");
        assert!(!saved_config.contains(SECRET));
        assert!(saved_config.contains("$schema"));
        assert!(saved_config.contains("db.collected.v1.acme.api.root.development.application"));
        assert_eq!(candidates[0].finding.status, DatabaseCollectStatus::Saved);
        assert_eq!(
            store
                .get(&candidates[0].finding.credential_key)
                .expect("stored secret")
                .as_ref()
                .map(SecretValue::as_str),
            Some(SECRET)
        );

        let mut repeated = vec![candidate()];
        save_candidates(&root, &mut repeated, &store).expect("repeat save");
        assert_eq!(
            repeated[0].finding.status,
            DatabaseCollectStatus::AlreadyConfigured
        );
    }

    fn write_appsettings(root: &Path, value: &str) {
        fs::write(
            root.join("appsettings.Development.json"),
            serde_json::to_string(&json!({
                "ConnectionStrings": { "Application": value }
            }))
            .expect("appsettings json"),
        )
        .expect("appsettings");
    }

    fn candidate() -> DatabaseCandidate {
        DatabaseCandidate {
            finding: DatabaseCollectFinding {
                project: ProjectKey::from("acme"),
                repository: WorkspaceRepositoryName::from("api"),
                application: "root".into(),
                environment: DatabaseEnvironmentName::from("Development"),
                name: "Application".into(),
                database: DatabaseKey::from("collected-api-root-development-application"),
                credential_key: SecretKey::from(
                    "db.collected.v1.acme.api.root.development.application",
                ),
                status: DatabaseCollectStatus::Eligible,
                detail: None,
                value_masked: true,
                source_paths: vec!["appsettings.Development.json".into()],
            },
            value: SecretValue::from(SECRET),
        }
    }
}
