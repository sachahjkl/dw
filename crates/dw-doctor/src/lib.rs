use anyhow::Result;
use dw_config::{
    InitRequest, default_agent, init_root, load_user_settings, resolve_root, user_settings_path,
};
use dw_core::{Agent, DevWorkflowRoot};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub root: DevWorkflowRoot,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn passed_count(&self) -> usize {
        self.checks.iter().filter(|check| check.passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.checks.len().saturating_sub(self.passed_count())
    }

    pub fn passed(&self) -> bool {
        self.failed_count() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub kind: DoctorCheckKind,
    pub passed: bool,
    pub detail: Option<DoctorCheckDetail>,
    pub remediation: DoctorRemediation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoctorCheckKind {
    DevWorkflowRoot,
    UserConfiguration,
    DefaultAgent,
    Git,
    NodePackageManager,
    OpenCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DoctorCheckDetail {
    Path {
        path: DoctorPath,
    },
    Agent {
        agent: Agent,
    },
    ProcessOutput {
        line: DoctorOutputLine,
    },
    PackageManagerVersion {
        manager: PackageManager,
        version: DoctorOutputLine,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageManager {
    Pnpm,
    Npm,
}

impl PackageManager {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm",
            Self::Npm => "npm",
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.executable())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DoctorRemediation {
    InitRoot { root: DevWorkflowRoot },
    RunInit,
    ConfigureDefaultAgent { agent: Agent },
    InstallGit,
    InstallNodePackageManager,
    InstallOpenCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DoctorPath(String);

impl DoctorPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DoctorPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DoctorPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DoctorPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DoctorOutputLine(String);

impl DoctorOutputLine {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DoctorOutputLine {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DoctorOutputLine {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DoctorOutputLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub fn run_doctor(fix: bool) -> Result<DoctorReport> {
    let settings = load_user_settings();
    let root = resolve_root(settings.root.as_deref());
    let root = DevWorkflowRoot::from(root);
    let mut checks = vec![
        DoctorCheck {
            kind: DoctorCheckKind::DevWorkflowRoot,
            passed: Path::new(root.as_str()).is_dir(),
            detail: Some(DoctorCheckDetail::Path {
                path: DoctorPath::from(root.to_string()),
            }),
            remediation: DoctorRemediation::InitRoot { root: root.clone() },
        },
        DoctorCheck {
            kind: DoctorCheckKind::UserConfiguration,
            passed: Path::new(&user_settings_path()).is_file(),
            detail: Some(DoctorCheckDetail::Path {
                path: DoctorPath::from(user_settings_path()),
            }),
            remediation: DoctorRemediation::RunInit,
        },
        check_default_agent(&root),
        check_command(
            "git",
            &["--version"],
            DoctorCheckKind::Git,
            DoctorRemediation::InstallGit,
            |_| true,
        ),
        check_node_package_manager(),
        check_command(
            "opencode",
            &["--version"],
            DoctorCheckKind::OpenCode,
            DoctorRemediation::InstallOpenCode,
            |_| true,
        ),
    ];

    if fix && !Path::new(root.as_str()).is_dir() {
        init_root(InitRequest {
            root: Some(root.to_string()),
            profile: "business".into(),
            no_save: false,
            dry_run: false,
        })?;
        checks[0].passed = true;
    }

    Ok(DoctorReport { root, checks })
}

fn check_default_agent(root: &DevWorkflowRoot) -> DoctorCheck {
    let agent = default_agent(root);
    DoctorCheck {
        kind: DoctorCheckKind::DefaultAgent,
        passed: true,
        detail: Some(DoctorCheckDetail::Agent { agent }),
        remediation: DoctorRemediation::ConfigureDefaultAgent {
            agent: dw_agent::DEFAULT_AGENT,
        },
    }
}

fn check_command(
    file_name: &str,
    arguments: &[&str],
    kind: DoctorCheckKind,
    remediation: DoctorRemediation,
    validator: impl Fn(&str) -> bool,
) -> DoctorCheck {
    let Ok(output) = dw_process::output(file_name, arguments.iter().copied()) else {
        return failed_check(kind, remediation);
    };
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() && validator(&combined) {
        DoctorCheck {
            kind,
            passed: true,
            detail: first_non_empty_line(&combined)
                .or_else(|| Some(DoctorOutputLine::from(file_name)))
                .map(|line| DoctorCheckDetail::ProcessOutput { line }),
            remediation,
        }
    } else {
        failed_check(kind, remediation)
    }
}

fn check_node_package_manager() -> DoctorCheck {
    let remediation = DoctorRemediation::InstallNodePackageManager;
    for manager in [PackageManager::Pnpm, PackageManager::Npm] {
        let Ok(output) = dw_process::output(manager.executable(), ["--version"]) else {
            continue;
        };
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() {
            let version = first_non_empty_line(&combined)
                .unwrap_or_else(|| DoctorOutputLine::from(manager.executable()));
            return DoctorCheck {
                kind: DoctorCheckKind::NodePackageManager,
                passed: true,
                detail: Some(DoctorCheckDetail::PackageManagerVersion { manager, version }),
                remediation,
            };
        }
    }
    failed_check(DoctorCheckKind::NodePackageManager, remediation)
}

fn failed_check(kind: DoctorCheckKind, remediation: DoctorRemediation) -> DoctorCheck {
    DoctorCheck {
        kind,
        passed: false,
        detail: None,
        remediation,
    }
}

fn first_non_empty_line(output: &str) -> Option<DoctorOutputLine> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(DoctorOutputLine::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_report_counts_failures() {
        let report = DoctorReport {
            root: DevWorkflowRoot::from("/tmp/dw"),
            checks: vec![
                DoctorCheck {
                    kind: DoctorCheckKind::DevWorkflowRoot,
                    passed: true,
                    detail: Some(DoctorCheckDetail::Path {
                        path: DoctorPath::from("/tmp/dw"),
                    }),
                    remediation: DoctorRemediation::RunInit,
                },
                DoctorCheck {
                    kind: DoctorCheckKind::Git,
                    passed: false,
                    detail: None,
                    remediation: DoctorRemediation::InstallGit,
                },
            ],
        };

        assert_eq!(report.passed_count(), 1);
        assert_eq!(report.failed_count(), 1);
        assert!(!report.passed());
    }
}
