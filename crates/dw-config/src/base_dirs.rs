use directories::BaseDirs;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformBaseDirs {
    pub home_dir: PathBuf,
    pub cache_dir: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub data_local_dir: Option<PathBuf>,
    pub executable_dir: Option<PathBuf>,
    pub preference_dir: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
}

impl PlatformBaseDirs {
    pub fn resolve() -> Self {
        if let Some(base_dirs) = BaseDirs::new() {
            return Self {
                home_dir: base_dirs.home_dir().to_path_buf(),
                cache_dir: Some(base_dirs.cache_dir().to_path_buf()),
                config_dir: Some(base_dirs.config_dir().to_path_buf()),
                data_dir: Some(base_dirs.data_dir().to_path_buf()),
                data_local_dir: Some(base_dirs.data_local_dir().to_path_buf()),
                executable_dir: base_dirs.executable_dir().map(PathBuf::from),
                preference_dir: Some(base_dirs.preference_dir().to_path_buf()),
                runtime_dir: base_dirs.runtime_dir().map(PathBuf::from),
                state_dir: base_dirs.state_dir().map(PathBuf::from),
            };
        }

        let home_dir = fallback_home_dir();
        Self {
            cache_dir: fallback_cache_dir(&home_dir),
            config_dir: fallback_config_dir(&home_dir),
            data_dir: fallback_data_dir(&home_dir),
            data_local_dir: fallback_data_local_dir(&home_dir),
            executable_dir: None,
            preference_dir: fallback_preference_dir(&home_dir),
            runtime_dir: None,
            state_dir: fallback_state_dir(&home_dir),
            home_dir,
        }
    }

    pub fn default_root(&self) -> PathBuf {
        self.home_dir.join("dev").join("dw")
    }

    pub fn user_config_directory(&self) -> PathBuf {
        if cfg!(windows) {
            return self
                .data_local_dir
                .clone()
                .or_else(|| self.config_dir.clone())
                .unwrap_or_else(|| self.home_dir.clone())
                .join("DevWorkflow");
        }

        self.config_dir
            .clone()
            .unwrap_or_else(|| self.home_dir.join(".config"))
            .join("DevWorkflow")
    }
}

fn fallback_home_dir() -> PathBuf {
    env_value("HOME")
        .or_else(|| env_value("USERPROFILE"))
        .or_else(|| match (env_value("HOMEDRIVE"), env_value("HOMEPATH")) {
            (Some(drive), Some(path)) => Some(format!("{drive}{path}")),
            _ => None,
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
}

fn fallback_cache_dir(home_dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        return env_value("LOCALAPPDATA").map(PathBuf::from);
    }
    env_value("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir.join(".cache")))
}

fn fallback_config_dir(home_dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        return env_value("APPDATA")
            .or_else(|| env_value("LOCALAPPDATA"))
            .map(PathBuf::from);
    }
    env_value("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir.join(".config")))
}

fn fallback_data_dir(home_dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        return env_value("APPDATA")
            .or_else(|| env_value("LOCALAPPDATA"))
            .map(PathBuf::from);
    }
    env_value("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir.join(".local").join("share")))
}

fn fallback_data_local_dir(home_dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        return env_value("LOCALAPPDATA").map(PathBuf::from);
    }
    fallback_data_dir(home_dir)
}

fn fallback_preference_dir(home_dir: &Path) -> Option<PathBuf> {
    fallback_config_dir(home_dir)
}

fn fallback_state_dir(home_dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    env_value("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir.join(".local").join("state")))
}

fn env_value(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_base_dirs_include_home_and_config() {
        let dirs = PlatformBaseDirs::resolve();

        assert!(!dirs.home_dir.as_os_str().is_empty());
        assert!(!dirs.user_config_directory().as_os_str().is_empty());
    }
}
