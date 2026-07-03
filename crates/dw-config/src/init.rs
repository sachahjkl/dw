use crate::init_templates::{WORKSPACE_CODEX_CONFIG, detect_profile, resolve_profile};
use crate::settings::normalize_path_lossy;
use crate::{UserSettings, default_root, save_user_settings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRequest {
    pub root: Option<String>,
    pub profile: String,
    pub no_save: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitReport {
    pub root: String,
    pub profile: String,
    pub dry_run: bool,
    pub no_save: bool,
    pub planned_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshRequest {
    pub root: String,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefreshReport {
    pub root: String,
    pub profile: String,
}

pub fn init_root(request: InitRequest) -> std::io::Result<InitReport> {
    let root = normalize_root(request.root.as_deref());
    let profile = resolve_profile(&request.profile)?;
    let planned_paths = planned_paths(&root);
    let report = InitReport {
        root: root.clone(),
        profile: profile.name.into(),
        dry_run: request.dry_run,
        no_save: request.no_save,
        planned_paths,
    };

    if request.dry_run {
        return Ok(report);
    }

    create_directories(&root)?;
    write_schemas_if_missing(&root)?;
    write_if_missing(
        path(&root, &["config", "projects.json"]),
        &profile.projects_json,
    )?;
    write_if_missing(
        path(&root, &["config", "workflow.json"]),
        &profile.workflow_json,
    )?;
    write_if_missing(
        path(&root, &["config", "databases.json"]),
        &profile.databases_json,
    )?;
    write_if_missing(
        path(&root, &["config", "opencode", "AGENTS.md"]),
        profile.agents_md,
    )?;
    write_if_missing(
        path(&root, &["config", "opencode", "opencode.jsonc"]),
        profile.opencode_jsonc,
    )?;
    write_if_missing(
        path(&root, &["config", "claude", "CLAUDE.md"]),
        profile.agents_md,
    )?;
    write_if_missing(
        path(&root, &["config", "cursor", "devworkflow.mdc"]),
        profile.agents_md,
    )?;
    write_if_missing(
        path(&root, &["config", "codex", "AGENTS.md"]),
        profile.agents_md,
    )?;
    write_if_missing(
        path(&root, &["config", "codex", "config.toml"]),
        WORKSPACE_CODEX_CONFIG,
    )?;
    write_if_missing(
        path(&root, &["config", "copilot", "copilot-instructions.md"]),
        profile.agents_md,
    )?;

    if !request.no_save {
        save_user_settings(&UserSettings {
            root: Some(root.clone()),
            color: None,
        })?;
    }

    Ok(report)
}

pub fn refresh_root(request: RefreshRequest) -> std::io::Result<RefreshReport> {
    let root = normalize_root(Some(&request.root));
    if !Path::new(&root).is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Root DevWorkflow introuvable: {root}"),
        ));
    }
    let profile = match request
        .profile
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(name) => resolve_profile(name)?,
        None => detect_profile(&root),
    };

    create_directories(&root)?;
    write_schemas(&root, true)?;
    write_root_agent_files(&root, &profile, true)?;

    Ok(RefreshReport {
        root,
        profile: profile.name.into(),
    })
}

fn create_directories(root: &str) -> std::io::Result<()> {
    for parts in [
        &[][..],
        &["config"],
        &["config", "opencode"],
        &["config", "claude"],
        &["config", "cursor"],
        &["config", "codex"],
        &["config", "copilot"],
        &["projects"],
        &["cache"],
    ] {
        fs::create_dir_all(path(root, parts))?;
    }
    Ok(())
}

fn write_schemas_if_missing(root: &str) -> std::io::Result<()> {
    write_schemas(root, false)
}

fn write_schemas(root: &str, overwrite: bool) -> std::io::Result<()> {
    fs::create_dir_all(path(root, &["schemas"]))?;
    for (file_name, content) in [
        (
            "projects.schema.json",
            include_str!("../../../schemas/projects.schema.json"),
        ),
        (
            "workflow.schema.json",
            include_str!("../../../schemas/workflow.schema.json"),
        ),
        (
            "databases.schema.json",
            include_str!("../../../schemas/databases.schema.json"),
        ),
        (
            "release.schema.json",
            include_str!("../../../schemas/release.schema.json"),
        ),
    ] {
        write_file(path(root, &["schemas", file_name]), content, overwrite)?;
    }
    Ok(())
}

fn write_root_agent_files(
    root: &str,
    profile: &crate::init_templates::InitProfile,
    overwrite: bool,
) -> std::io::Result<()> {
    write_file(
        path(root, &["config", "opencode", "AGENTS.md"]),
        profile.agents_md,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "opencode", "opencode.jsonc"]),
        profile.opencode_jsonc,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "claude", "CLAUDE.md"]),
        profile.agents_md,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "cursor", "devworkflow.mdc"]),
        profile.agents_md,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "codex", "AGENTS.md"]),
        profile.agents_md,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "codex", "config.toml"]),
        WORKSPACE_CODEX_CONFIG,
        overwrite,
    )?;
    write_file(
        path(root, &["config", "copilot", "copilot-instructions.md"]),
        profile.agents_md,
        overwrite,
    )?;
    Ok(())
}

fn write_if_missing(path: PathBuf, content: &str) -> std::io::Result<()> {
    write_file(path, content, false)
}

fn write_file(path: PathBuf, content: &str, overwrite: bool) -> std::io::Result<()> {
    if overwrite || !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }
    Ok(())
}

fn planned_paths(root: &str) -> Vec<String> {
    [
        vec![],
        vec!["config"],
        vec!["config", "projects.json"],
        vec!["config", "workflow.json"],
        vec!["config", "databases.json"],
        vec!["config", "opencode", "AGENTS.md"],
        vec!["config", "opencode", "opencode.jsonc"],
        vec!["config", "claude", "CLAUDE.md"],
        vec!["config", "cursor", "devworkflow.mdc"],
        vec!["config", "codex", "AGENTS.md"],
        vec!["config", "codex", "config.toml"],
        vec!["config", "copilot", "copilot-instructions.md"],
        vec!["projects"],
        vec!["cache"],
        vec!["schemas"],
        vec!["schemas", "projects.schema.json"],
        vec!["schemas", "workflow.schema.json"],
        vec!["schemas", "databases.schema.json"],
        vec!["schemas", "release.schema.json"],
    ]
    .into_iter()
    .map(|parts| path(root, &parts).display().to_string())
    .collect()
}

fn path(root: &str, parts: &[&str]) -> PathBuf {
    let mut path = PathBuf::from(root);
    for part in parts {
        path.push(part);
    }
    path
}

fn normalize_root(value: Option<&str>) -> String {
    let fallback = default_root();
    let root = value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&fallback);
    normalize_path_lossy(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_writes_config_and_schemas_with_relative_links() {
        let root = tempfile::tempdir().expect("tempdir");
        let report = init_root(InitRequest {
            root: Some(root.path().display().to_string()),
            profile: "default".into(),
            no_save: true,
            dry_run: false,
        })
        .expect("init should succeed");

        assert_eq!(report.profile, "default");
        assert!(root.path().join("config/projects.json").exists());
        assert!(root.path().join("schemas/projects.schema.json").exists());
        let projects =
            fs::read_to_string(root.path().join("config/projects.json")).expect("projects");
        assert!(projects.contains(r#""$schema": "../schemas/projects.schema.json""#));
    }

    #[test]
    fn init_dry_run_writes_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let target = root.path().join("dw-root");
        let report = init_root(InitRequest {
            root: Some(target.display().to_string()),
            profile: "business".into(),
            no_save: true,
            dry_run: true,
        })
        .expect("dry run should succeed");

        assert_eq!(report.profile, "business");
        assert!(!target.exists());
        assert!(
            report
                .planned_paths
                .iter()
                .any(|path| path.ends_with("projects.schema.json"))
        );
    }

    #[test]
    fn init_does_not_overwrite_existing_files() {
        let root = tempfile::tempdir().expect("tempdir");
        let config = root.path().join("config");
        fs::create_dir_all(&config).expect("config dir");
        fs::write(config.join("projects.json"), "custom").expect("custom projects");

        init_root(InitRequest {
            root: Some(root.path().display().to_string()),
            profile: "business".into(),
            no_save: true,
            dry_run: false,
        })
        .expect("init should succeed");

        assert_eq!(
            fs::read_to_string(config.join("projects.json")).expect("projects"),
            "custom"
        );
    }

    #[test]
    fn refresh_regenerates_generated_files_and_preserves_user_files() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("config/opencode")).expect("config dir");
        fs::create_dir_all(root.path().join("schemas")).expect("schemas dir");
        fs::write(root.path().join("config/projects.json"), "custom projects").expect("projects");
        fs::write(root.path().join("config/workflow.json"), "custom workflow").expect("workflow");
        fs::write(
            root.path().join("config/databases.json"),
            "custom databases",
        )
        .expect("databases");
        fs::write(
            root.path().join("config/opencode/AGENTS.md"),
            "stale agents",
        )
        .expect("agents");
        fs::write(
            root.path().join("schemas/projects.schema.json"),
            "stale schema",
        )
        .expect("schema");

        let report = refresh_root(RefreshRequest {
            root: root.path().display().to_string(),
            profile: Some("business".into()),
        })
        .expect("refresh should succeed");

        assert_eq!(report.profile, "business");
        assert_ne!(
            fs::read_to_string(root.path().join("schemas/projects.schema.json")).expect("schema"),
            "stale schema"
        );
        assert!(
            fs::read_to_string(root.path().join("config/opencode/AGENTS.md"))
                .expect("agents")
                .contains("dw ado")
        );
        assert_eq!(
            fs::read_to_string(root.path().join("config/workflow.json")).expect("workflow"),
            "custom workflow"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("config/databases.json")).expect("databases"),
            "custom databases"
        );
    }
}
