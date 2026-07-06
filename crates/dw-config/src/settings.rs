use crate::base_dirs::PlatformBaseDirs;
use crate::json::read_json;
use crate::types::UserSettings;
use dw_core::{ConfigColorMode, DevWorkflowRoot};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::{env, fs};

pub const COLOR_MODE_CHOICES: &[ConfigColorMode] = &ConfigColorMode::ALL;

pub fn default_root() -> String {
    PlatformBaseDirs::resolve()
        .default_root()
        .display()
        .to_string()
}

pub fn user_config_directory() -> String {
    PlatformBaseDirs::resolve()
        .user_config_directory()
        .display()
        .to_string()
}

pub fn user_settings_path() -> String {
    format!("{}/settings.json", user_config_directory())
}

pub fn load_user_settings() -> UserSettings {
    let path = user_settings_path();
    read_json::<UserSettings>(&path).unwrap_or_default()
}

pub fn save_user_settings(settings: &UserSettings) -> std::io::Result<()> {
    let directory = user_config_directory();
    fs::create_dir_all(&directory)?;
    let path = user_settings_path();
    let content = serde_json::to_string_pretty(settings)?;
    fs::write(path, content)
}

pub fn set_user_root(root: &str) -> std::io::Result<DevWorkflowRoot> {
    let root = normalize_path(root)?;
    let mut settings = load_user_settings();
    settings.root = Some(root.clone());
    save_user_settings(&settings)?;
    Ok(DevWorkflowRoot::from(root))
}

pub fn set_color_mode(mode: ConfigColorMode) -> std::io::Result<ConfigColorMode> {
    let normalized = normalize_color_mode(Some(mode))
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let mut settings = load_user_settings();
    settings.color = Some(normalized);
    save_user_settings(&settings)?;
    Ok(normalized)
}

pub fn normalize_color_mode(mode: Option<ConfigColorMode>) -> Result<ConfigColorMode, String> {
    Ok(mode.unwrap_or(ConfigColorMode::Auto))
}

pub fn parse_color_mode(mode: Option<&str>) -> Result<ConfigColorMode, String> {
    mode.filter(|value| !value.trim().is_empty())
        .map(ConfigColorMode::from_str)
        .transpose()
        .map_err(|_| {
            let choices = COLOR_MODE_CHOICES
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Unknown color mode: {}. Allowed values: {}.",
                mode.unwrap_or_default(),
                choices
            )
        })
        .map(|mode| mode.unwrap_or(ConfigColorMode::Auto))
}

pub fn resolve_root(explicit_root: Option<&str>) -> String {
    if let Some(root) = explicit_root.filter(|value| !value.trim().is_empty()) {
        return normalize_path_lossy(root);
    }

    let settings = load_user_settings();
    if let Some(root) = settings.root.filter(|value| !value.trim().is_empty()) {
        return normalize_path_lossy(&root);
    }

    normalize_path_lossy(&default_root())
}

pub(crate) fn normalize_path_lossy(value: &str) -> String {
    normalize_path(value).unwrap_or_else(|_| expand_home(value))
}

fn normalize_path(value: &str) -> std::io::Result<String> {
    let expanded = expand_environment_variables(&expand_home(value));
    let path = PathBuf::from(expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };
    Ok(normalize_components(&absolute).display().to_string())
}

fn expand_home(value: &str) -> String {
    if value == "~" {
        return PlatformBaseDirs::resolve().home_dir.display().to_string();
    }

    if let Some(stripped) = value.strip_prefix("~/") {
        return PlatformBaseDirs::resolve()
            .home_dir
            .join(stripped)
            .display()
            .to_string();
    }

    value.to_string()
}

fn expand_environment_variables(value: &str) -> String {
    let percent_expanded = expand_percent_environment_variables(value);
    expand_dollar_environment_variables(&percent_expanded)
}

fn expand_percent_environment_variables(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            output.push('%');
            output.push_str(after_start);
            return output;
        };
        let key = &after_start[..end];
        output.push_str(&env::var(key).unwrap_or_else(|_| format!("%{key}%")));
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    output
}

fn expand_dollar_environment_variables(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            output.push(ch);
            continue;
        }

        if chars.peek() == Some(&'{') {
            let _ = chars.next();
            let mut key = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                key.push(next);
            }
            output.push_str(&env::var(&key).unwrap_or_else(|_| format!("${{{key}}}")));
            continue;
        }

        let mut key = String::new();
        while let Some(next) = chars.peek().copied() {
            if next == '_' || next.is_ascii_alphanumeric() {
                key.push(next);
                let _ = chars.next();
            } else {
                break;
            }
        }
        if key.is_empty() {
            output.push('$');
        } else {
            output.push_str(&env::var(&key).unwrap_or_else(|_| format!("${key}")));
        }
    }
    output
}

fn normalize_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = normalized.pop();
                if !popped {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[allow(dead_code)]
fn _is_absolute(path: &Path) -> bool {
    path.is_absolute()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::sync::Mutex;

    #[cfg(windows)]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn normalize_expands_tilde_from_platform_home() {
        let path = normalize_path_lossy("~/dev/dw");

        assert_eq!(
            path,
            PlatformBaseDirs::resolve()
                .home_dir
                .join("dev")
                .join("dw")
                .display()
                .to_string()
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_user_config_uses_local_app_data() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let local_app_data = env::var("LOCALAPPDATA").ok();
        unsafe {
            env::set_var("LOCALAPPDATA", "C:/Users/demo/AppData/Local");
        }

        let directory = user_config_directory();

        restore_env("LOCALAPPDATA", local_app_data);
        assert_eq!(directory, "C:/Users/demo/AppData/Local/DevWorkflow");
    }

    #[cfg(windows)]
    fn restore_env(key: &str, previous: Option<String>) {
        unsafe {
            if let Some(value) = previous {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }
}
