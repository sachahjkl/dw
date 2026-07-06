use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{ExitStatus, Output};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    pub file_name: OsString,
    pub arguments: Vec<OsString>,
}

impl ResolvedCommand {
    fn new(file_name: impl Into<OsString>, arguments: Vec<OsString>) -> Self {
        Self {
            file_name: file_name.into(),
            arguments,
        }
    }
}

pub fn command_candidates(
    file_name: impl AsRef<OsStr>,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Vec<ResolvedCommand> {
    let file_name = file_name.as_ref().to_os_string();
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_os_string())
        .collect::<Vec<_>>();

    let mut candidates = vec![ResolvedCommand::new(file_name.clone(), arguments.clone())];
    if should_add_windows_script_fallbacks(&file_name) {
        let base = file_name.to_string_lossy();
        candidates.push(ResolvedCommand::new(
            format!("{base}.cmd"),
            arguments.clone(),
        ));

        let mut powershell_arguments = vec![
            OsString::from("-NoProfile"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-File"),
            OsString::from(format!("{base}.ps1")),
        ];
        powershell_arguments.extend(arguments);
        candidates.push(ResolvedCommand::new("powershell", powershell_arguments));
    }
    candidates
}

pub fn output(
    file_name: impl AsRef<OsStr>,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> std::io::Result<Output> {
    output_in(file_name, arguments, None::<&Path>)
}

pub fn output_in(
    file_name: impl AsRef<OsStr>,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    current_dir: Option<impl AsRef<Path>>,
) -> std::io::Result<Output> {
    let current_dir = current_dir.map(|path| path.as_ref().to_path_buf());
    let mut last_error = None;
    for candidate in command_candidates(file_name, arguments) {
        let mut command = std::process::Command::new(&candidate.file_name);
        command.args(&candidate.arguments);
        if let Some(current_dir) = &current_dir {
            command.current_dir(current_dir);
        }
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)))
}

pub fn status(
    file_name: impl AsRef<OsStr>,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    current_dir: Option<impl AsRef<Path>>,
    environment: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> std::io::Result<ExitStatus> {
    let current_dir = current_dir.map(|path| path.as_ref().to_path_buf());
    let environment = environment
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_os_string(), value.as_ref().to_os_string()))
        .collect::<Vec<_>>();

    let mut last_error = None;
    for candidate in command_candidates(file_name.as_ref(), arguments) {
        let mut command = std::process::Command::new(&candidate.file_name);
        command.args(&candidate.arguments);
        if let Some(current_dir) = &current_dir {
            command.current_dir(current_dir);
        }
        for (key, value) in &environment {
            command.env(key, value);
        }
        match command.status() {
            Ok(status) => return Ok(status),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)))
}

pub fn command_available(file_name: &str, arguments: &[&str]) -> bool {
    output(file_name, arguments.iter().copied()).is_ok_and(|output| output.status.success())
}

fn should_add_windows_script_fallbacks(file_name: &OsStr) -> bool {
    if !cfg!(windows) {
        return false;
    }

    let value = file_name.to_string_lossy();
    if value.contains('/') || value.contains('\\') {
        return false;
    }

    Path::new(value.as_ref()).extension().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn candidates_keep_original_only_outside_windows() {
        let candidates = command_candidates("pnpm", ["--version"]);

        assert_eq!(
            candidates,
            vec![ResolvedCommand {
                file_name: "pnpm".into(),
                arguments: vec!["--version".into()],
            }]
        );
    }

    #[cfg(windows)]
    #[test]
    fn candidates_add_windows_script_fallbacks_for_plain_commands() {
        let candidates = command_candidates("pnpm", ["--version"]);

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].file_name, OsString::from("pnpm"));
        assert_eq!(candidates[1].file_name, OsString::from("pnpm.cmd"));
        assert_eq!(candidates[2].file_name, OsString::from("powershell"));
        assert_eq!(
            candidates[2].arguments,
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-ExecutionPolicy"),
                OsString::from("Bypass"),
                OsString::from("-File"),
                OsString::from("pnpm.ps1"),
                OsString::from("--version"),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn candidates_do_not_add_fallbacks_for_paths_or_extensions() {
        assert_eq!(command_candidates("tools/pnpm", ["--version"]).len(), 1);
        assert_eq!(command_candidates("pnpm.exe", ["--version"]).len(), 1);
    }
}
