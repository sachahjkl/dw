use crate::json::read_json;
use crate::types::UserSettings;
use std::path::{Component, Path, PathBuf};
use std::{env, fs};

pub fn default_root() -> String {
    let home = home_directory().unwrap_or_else(|| "~".into());
    format!("{home}/dev/dw")
}

pub fn user_config_directory() -> String {
    if cfg!(windows)
        && let Some(local_app_data) = env_value("LOCALAPPDATA")
    {
        return format!("{local_app_data}/DevWorkflow");
    }

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME")
        && !xdg.trim().is_empty()
    {
        return format!("{xdg}/DevWorkflow");
    }

    let home = home_directory().unwrap_or_else(|| "~".into());
    format!("{home}/.config/DevWorkflow")
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

pub fn set_user_root(root: &str) -> std::io::Result<String> {
    let root = normalize_path(root)?;
    let mut settings = load_user_settings();
    settings.root = Some(root.clone());
    save_user_settings(&settings)?;
    Ok(root)
}

pub fn set_color_mode(mode: &str) -> std::io::Result<String> {
    let normalized = normalize_color_mode(Some(mode))
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let mut settings = load_user_settings();
    settings.color = Some(normalized.clone());
    save_user_settings(&settings)?;
    Ok(normalized)
}

pub fn normalize_color_mode(mode: Option<&str>) -> Result<String, String> {
    let normalized = mode
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "auto".into());
    match normalized.as_str() {
        "auto" | "always" | "never" => Ok(normalized),
        _ => Err(format!(
            "Mode couleur inconnu: {}. Valeurs autorisées: auto, always, never.",
            mode.unwrap_or_default()
        )),
    }
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
        return home_directory().unwrap_or_else(|| value.into());
    }

    if let Some(stripped) = value.strip_prefix("~/") {
        let home = home_directory().unwrap_or_else(|| "~".into());
        return format!("{home}/{stripped}");
    }

    value.to_string()
}

fn home_directory() -> Option<String> {
    env_value("HOME")
        .or_else(|| env_value("USERPROFILE"))
        .or_else(|| match (env_value("HOMEDRIVE"), env_value("HOMEPATH")) {
            (Some(drive), Some(path)) => Some(format!("{drive}{path}")),
            _ => None,
        })
}

fn env_value(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_root_uses_userprofile_when_home_is_missing() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let home = env::var("HOME").ok();
        let userprofile = env::var("USERPROFILE").ok();
        unsafe {
            env::remove_var("HOME");
            env::set_var("USERPROFILE", "/tmp/windows-home");
        }

        let root = default_root();

        restore_env("HOME", home);
        restore_env("USERPROFILE", userprofile);
        assert_eq!(root, "/tmp/windows-home/dev/dw");
    }

    #[test]
    fn normalize_expands_bare_tilde_with_windows_home_fallback() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let home = env::var("HOME").ok();
        let userprofile = env::var("USERPROFILE").ok();
        unsafe {
            env::remove_var("HOME");
            env::set_var("USERPROFILE", "/tmp/windows-home");
        }

        let path = normalize_path_lossy("~/dev/dw");

        restore_env("HOME", home);
        restore_env("USERPROFILE", userprofile);
        assert_eq!(path, "/tmp/windows-home/dev/dw");
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
