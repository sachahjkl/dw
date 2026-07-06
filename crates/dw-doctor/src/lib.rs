use anyhow::Result;
use dw_config::{
    InitRequest, default_agent, init_root, load_user_settings, resolve_root, user_settings_path,
};
use dw_core::DevWorkflowRoot;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub root: String,
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
    pub name: String,
    pub passed: bool,
    pub detail: Option<String>,
    pub remediation: String,
}

pub fn run_doctor(fix: bool) -> Result<DoctorReport> {
    let settings = load_user_settings();
    let root = resolve_root(settings.root.as_deref());
    let mut checks = vec![
        DoctorCheck {
            name: "Root DevWorkflow".into(),
            passed: Path::new(&root).is_dir(),
            detail: Some(root.clone()),
            remediation: format!("Initialiser le root DevWorkflow: {root}"),
        },
        DoctorCheck {
            name: "Configuration utilisateur".into(),
            passed: Path::new(&user_settings_path()).is_file(),
            detail: Some(user_settings_path()),
            remediation: "Exécuter: dw init".into(),
        },
        check_default_agent(&root),
        check_command(
            "git",
            &["--version"],
            "Git",
            "Installer Git puis relancer dw doctor",
            |_| true,
        ),
        check_node_package_manager(),
        check_command(
            "opencode",
            &["--version"],
            "OpenCode",
            "Installer OpenCode selon la procédure d'équipe, puis vérifier le PATH",
            |_| true,
        ),
    ];

    if fix && !Path::new(&root).is_dir() {
        init_root(InitRequest {
            root: Some(root.clone()),
            profile: "business".into(),
            no_save: false,
            dry_run: false,
        })?;
        checks[0].passed = true;
    }

    Ok(DoctorReport { root, checks })
}

fn check_default_agent(root: &str) -> DoctorCheck {
    let agent = default_agent(&DevWorkflowRoot::from(root));
    DoctorCheck {
        name: "Agent par défaut".into(),
        passed: true,
        detail: Some(agent.to_string()),
        remediation: "Configurer: dw agent config set-default opencode".into(),
    }
}

fn check_command(
    file_name: &str,
    arguments: &[&str],
    name: &str,
    remediation: &str,
    validator: impl Fn(&str) -> bool,
) -> DoctorCheck {
    let Ok(output) = dw_process::output(file_name, arguments.iter().copied()) else {
        return failed_check(name, remediation);
    };
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() && validator(&combined) {
        DoctorCheck {
            name: name.into(),
            passed: true,
            detail: first_non_empty_line(&combined).or_else(|| Some(file_name.into())),
            remediation: remediation.into(),
        }
    } else {
        failed_check(name, remediation)
    }
}

fn check_node_package_manager() -> DoctorCheck {
    let remediation = "Installer pnpm, ou Node.js/npm si pnpm est indisponible.";
    for (file_name, detail_prefix) in [("pnpm", "pnpm"), ("npm", "npm")] {
        let Ok(output) = dw_process::output(file_name, ["--version"]) else {
            continue;
        };
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() {
            let detail = first_non_empty_line(&combined)
                .map(|line| format!("{detail_prefix} {line}"))
                .unwrap_or_else(|| detail_prefix.into());
            return DoctorCheck {
                name: "pnpm/npm".into(),
                passed: true,
                detail: Some(detail),
                remediation: remediation.into(),
            };
        }
    }
    failed_check("pnpm/npm", remediation)
}

fn failed_check(name: &str, remediation: &str) -> DoctorCheck {
    DoctorCheck {
        name: name.into(),
        passed: false,
        detail: None,
        remediation: remediation.into(),
    }
}

fn first_non_empty_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_report_counts_failures() {
        let report = DoctorReport {
            root: "/tmp/dw".into(),
            checks: vec![
                DoctorCheck {
                    name: "Root DevWorkflow".into(),
                    passed: true,
                    detail: Some("/tmp/dw".into()),
                    remediation: "dw init".into(),
                },
                DoctorCheck {
                    name: "Git".into(),
                    passed: false,
                    detail: None,
                    remediation: "Installer Git".into(),
                },
            ],
        };

        assert_eq!(report.passed_count(), 1);
        assert_eq!(report.failed_count(), 1);
        assert!(!report.passed());
    }
}
