use crate::json::{read_json_value, read_jsonc_value};
use crate::settings::{resolve_root, user_settings_path};
use crate::types::{ConfigDoctorCheck, ConfigDoctorReport, ConfigShow};
use std::path::Path;

pub fn config_show(explicit_root: Option<&str>) -> ConfigShow {
    let root = resolve_root(explicit_root);
    let settings = crate::settings::load_user_settings();
    let workflow_path = Path::new(&root).join("config").join("workflow.json");
    let projects_path = Path::new(&root).join("config").join("projects.json");
    let databases_path = Path::new(&root).join("config").join("databases.json");

    ConfigShow {
        root,
        color: crate::settings::normalize_color_mode(settings.color.as_deref())
            .unwrap_or_else(|_| "auto".into()),
        settings_path: user_settings_path(),
        workflow_path: workflow_path.display().to_string(),
        projects_path: projects_path.display().to_string(),
        databases_path: databases_path.display().to_string(),
        workflow_exists: workflow_path.exists(),
        projects_exists: projects_path.exists(),
        databases_exists: databases_path.exists(),
    }
}

pub fn config_doctor(explicit_root: Option<&str>) -> ConfigDoctorReport {
    let root = resolve_root(explicit_root);
    let checks = vec![
        check_known_config(
            &Path::new(&root).join("config").join("projects.json"),
            &["schema", "projects"],
        ),
        check_known_config(
            &Path::new(&root).join("config").join("workflow.json"),
            &["schema", "branchPrefixes", "azureDevOps", "auth", "updates"],
        ),
        check_known_config(
            &Path::new(&root).join("config").join("databases.json"),
            &["schema", "defaults", "globals", "projects"],
        ),
        check_jsonc(&Path::new(&root).join("config/opencode/opencode.jsonc")),
        check_exists(&Path::new(&root).join("schemas/projects.schema.json")),
        check_exists(&Path::new(&root).join("schemas/workflow.schema.json")),
        check_exists(&Path::new(&root).join("schemas/databases.schema.json")),
    ];
    let passed = checks.iter().all(|check| check.passed);
    ConfigDoctorReport {
        root,
        checks,
        passed,
    }
}

fn check_known_config(path: &Path, required_properties: &[&str]) -> ConfigDoctorCheck {
    let json = match read_json_value(path) {
        Ok(value) => value,
        Err(message) => return doctor_check(path, false, Some(message)),
    };
    let Some(object) = json.as_object() else {
        return doctor_check(
            path,
            false,
            Some("la racine doit etre un objet JSON".into()),
        );
    };
    let missing = required_properties
        .iter()
        .filter(|property| !object.contains_key(**property))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        doctor_check(path, true, None)
    } else {
        doctor_check(
            path,
            false,
            Some(format!("proprietes manquantes: {}", missing.join(", "))),
        )
    }
}

fn check_jsonc(path: &Path) -> ConfigDoctorCheck {
    match read_jsonc_value(path) {
        Ok(_) => doctor_check(path, true, None),
        Err(message) => doctor_check(path, false, Some(message)),
    }
}

fn check_exists(path: &Path) -> ConfigDoctorCheck {
    doctor_check(
        path,
        path.exists(),
        if path.exists() {
            None
        } else {
            Some("fichier manquant".into())
        },
    )
}

fn doctor_check(path: &Path, passed: bool, message: Option<String>) -> ConfigDoctorCheck {
    ConfigDoctorCheck {
        path: path.display().to_string(),
        passed,
        message,
    }
}
