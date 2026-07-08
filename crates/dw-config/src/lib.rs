pub mod base_dirs;
pub mod command;
pub mod completion;
mod doctor;
mod init;
mod init_templates;
mod json;
mod projects;
mod settings;
mod store;
mod types;

pub use base_dirs::PlatformBaseDirs;
pub use doctor::{config_doctor, config_show, root_status};
pub use init::{InitReport, InitRequest, RefreshReport, RefreshRequest, init_root, refresh_root};
pub use projects::{load_projects_config, project_choices, repository_config, resolve_project};
pub use settings::{
    COLOR_MODE_CHOICES, default_root, load_user_settings, normalize_color_mode, parse_color_mode,
    resolve_root, save_user_settings, set_color_mode, set_user_root, user_config_directory,
    user_settings_path,
};
pub use store::{
    AGENT_DEFAULT_CHOICES, default_agent, load_databases_config, load_workflow_config,
    normalize_default_agent, set_default_agent,
};
pub use types::{
    AgentOptions, ConfigDoctorCheck, ConfigDoctorReport, ConfigShow, DatabasesConfig,
    ProjectChoice, ProjectConfig, ProjectsConfig, RepositoryConfig, RootStatus, UserSettings,
    WorkflowConfig,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_root_uses_dev_dw_suffix() {
        let value = default_root();
        assert!(Path::new(&value).ends_with(Path::new("dev").join("dw")));
    }

    #[test]
    fn config_show_reports_expected_paths() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("demo-root");
        let report = config_show(Some(root.to_str().expect("utf8 path")));
        assert_eq!(Path::new(&report.root), root.as_path());
        assert_eq!(
            Path::new(&report.workflow_path),
            root.join("config").join("workflow.json")
        );
        assert_eq!(
            Path::new(&report.projects_path),
            root.join("config").join("projects.json")
        );
        assert_eq!(
            Path::new(&report.databases_path),
            root.join("config").join("databases.json")
        );
    }

    #[test]
    fn root_status_reports_missing_init_files() {
        let temp = tempdir().expect("tempdir");
        let status = root_status(Some(temp.path().to_str().expect("utf8 path")));

        assert!(!status.initialized);
        assert_eq!(status.missing_paths.len(), 3);
        assert!(
            status
                .missing_paths
                .iter()
                .any(|path| Path::new(path).ends_with(Path::new("config").join("projects.json")))
        );
    }

    #[test]
    fn root_status_is_initialized_after_init() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir");

        init_root(InitRequest {
            root: Some(temp.path().display().to_string()),
            profile: "business".into(),
            no_save: true,
            dry_run: false,
        })
        .expect("init should create root");
        let status = root_status(Some(temp.path().to_str().expect("utf8 path")));

        assert!(status.initialized);
        assert!(status.missing_paths.is_empty());
    }

    #[test]
    fn resolve_project_merges_included_projects() {
        let config: ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "base": {
      "displayName": "BASE",
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" }
      }
    },
    "ha": {
      "displayName": "HA",
      "includedProjects": ["base"],
      "repositories": {
        "back": { "url": "", "defaultBranch": "main" }
      },
      "agent": { "default": "claude" }
    }
  }
}"#,
        )
        .expect("projects config should parse");

        let project = resolve_project(&config, "ha").expect("project should resolve");
        assert!(project.repositories.contains_key("front"));
        assert!(project.repositories.contains_key("back"));
        assert_eq!(project.agent.expect("agent should exist").default, "claude");
    }

    #[test]
    fn repository_config_reads_folder_override() {
        let project: ProjectConfig = serde_json::from_str(
            r#"{
  "displayName": "HA",
  "repositories": {
    "front": { "url": "", "defaultBranch": "develop", "folder": "custom-front" }
  }
}"#,
        )
        .expect("project should parse");

        let repository = repository_config(&project, "front").expect("repository should resolve");
        assert_eq!(repository.folder.as_deref(), Some("custom-front"));
    }

    #[test]
    fn project_choices_keep_config_order_and_include_display_name() {
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "zz": { "displayName": "Projet Z", "repositories": {} },
    "ha": { "displayName": "HOMMAGE AGENCE", "repositories": {} }
  }
}"#,
        )
        .expect("projects config should parse");

        let choices = project_choices(&projects);

        assert_eq!(
            choices,
            vec![
                ProjectChoice {
                    key: "zz".into(),
                    label: "zz - Projet Z".into()
                },
                ProjectChoice {
                    key: "ha".into(),
                    label: "ha - HOMMAGE AGENCE".into()
                }
            ]
        );
    }

    #[test]
    fn config_doctor_passes_when_required_files_are_valid() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        std::fs::create_dir_all(root.join("config/opencode")).expect("config dir");
        std::fs::create_dir_all(root.join("schemas")).expect("schemas dir");
        std::fs::write(
            root.join("config/projects.json"),
            r#"{"schema":1,"projects":{}}"#,
        )
        .expect("projects");
        std::fs::write(
            root.join("config/workflow.json"),
            r#"{"schema":1,"branchPrefixes":{},"azureDevOps":{},"auth":{},"updates":{}}"#,
        )
        .expect("workflow");
        std::fs::write(
            root.join("config/databases.json"),
            r#"{"schema":1,"defaults":{},"globals":{},"projects":{}}"#,
        )
        .expect("databases");
        std::fs::write(
            root.join("config/opencode/opencode.jsonc"),
            "{\n  // comments are allowed\n  \"instructions\": []\n}",
        )
        .expect("opencode");
        std::fs::write(root.join("schemas/projects.schema.json"), "{}").expect("schema");
        std::fs::write(root.join("schemas/workflow.schema.json"), "{}").expect("schema");
        std::fs::write(root.join("schemas/databases.schema.json"), "{}").expect("schema");

        let report = config_doctor(Some(root.to_str().expect("utf8 path")));

        assert!(report.passed);
        assert_eq!(report.checks.len(), 7);
    }

    #[test]
    fn set_color_persists_normalized_mode() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir should be created");
        let env = isolate_user_config_env(temp.path());

        let result =
            set_color_mode(dw_core::ConfigColorMode::Always).expect("color should be saved");
        let settings = load_user_settings();

        restore_env_vars(env);
        assert_eq!(result, dw_core::ConfigColorMode::Always);
        assert_eq!(settings.color, Some(dw_core::ConfigColorMode::Always));
    }

    #[test]
    fn set_color_rejects_unknown_mode() {
        let error = parse_color_mode(Some("rainbow")).expect_err("color should fail");
        assert!(error.contains("Unknown color mode"));
    }

    #[test]
    fn set_default_agent_accepts_all_documented_choices() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        std::fs::create_dir_all(root.join("config")).expect("config dir");
        std::fs::write(root.join("config/workflow.json"), "{}").expect("workflow");
        let root = dw_core::DevWorkflowRoot::from(root.to_str().expect("utf8 path"));

        for agent in dw_core::Agent::ALL {
            let result = set_default_agent(&root, agent).expect("agent should save");
            assert_eq!(result, agent);
        }
    }

    #[test]
    fn default_agent_reads_parsed_agent() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        std::fs::create_dir_all(root.join("config")).expect("config dir");
        std::fs::write(root.join("config/workflow.json"), "{}").expect("workflow");
        let root = dw_core::DevWorkflowRoot::from(root.to_str().expect("utf8 path"));

        let result = set_default_agent(&root, dw_core::Agent::CodexCli).expect("agent should save");

        assert_eq!(result, dw_core::Agent::CodexCli);
        assert_eq!(default_agent(&root), dw_core::Agent::CodexCli);
    }

    #[test]
    fn agent_parse_rejects_unknown_agent() {
        let error = "unknown"
            .parse::<dw_core::Agent>()
            .expect_err("unknown agent should fail")
            .to_string();

        assert!(error.contains("unknown"));
    }

    #[test]
    fn set_root_persists_absolute_path() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir should be created");
        let env = isolate_user_config_env(temp.path());
        let root = temp.path().join("dw-root");

        let result = set_user_root(root.to_str().expect("utf8 path")).expect("root should save");
        let settings = load_user_settings();

        restore_env_vars(env);
        assert_eq!(
            result,
            dw_core::DevWorkflowRoot::from(root.display().to_string())
        );
        assert_eq!(
            settings.root.as_deref(),
            Some(root.to_str().expect("utf8 path"))
        );
    }

    #[test]
    fn set_root_normalizes_relative_segments_like_dotnet() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir should be created");
        let env = isolate_user_config_env(temp.path());

        let result = set_user_root("./relative-root/../dw-root").expect("root should save");

        restore_env_vars(env);
        assert_eq!(
            result,
            dw_core::DevWorkflowRoot::from(
                std::env::current_dir()
                    .unwrap()
                    .join("dw-root")
                    .display()
                    .to_string()
            )
        );
    }

    fn isolate_user_config_env(root: &std::path::Path) -> Vec<(&'static str, Option<String>)> {
        let previous = [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
            "LOCALAPPDATA",
            "APPDATA",
        ]
        .into_iter()
        .map(|key| (key, std::env::var(key).ok()))
        .collect::<Vec<_>>();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", root.join("xdg-config"));
            std::env::set_var("XDG_DATA_HOME", root.join("xdg-data"));
            std::env::set_var("XDG_CACHE_HOME", root.join("xdg-cache"));
            std::env::set_var("XDG_STATE_HOME", root.join("xdg-state"));
            std::env::set_var("LOCALAPPDATA", root.join("local-app-data"));
            std::env::set_var("APPDATA", root.join("app-data"));
        }
        previous
    }

    fn restore_env_vars(previous: Vec<(&'static str, Option<String>)>) {
        for (key, value) in previous {
            restore_env(key, value);
        }
    }

    fn restore_env(key: &str, previous: Option<String>) {
        unsafe {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}
