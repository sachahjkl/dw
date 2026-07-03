use anyhow::Result;
use dw_agent::parse_agent_kind;
use dw_config::{
    InitRequest, default_agent, init_root, load_user_settings, resolve_root, user_settings_path,
};
use dw_ui::{ColorMode, TerminalTheme};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DoctorCheck {
    pub(crate) name: String,
    pub(crate) passed: bool,
    pub(crate) detail: Option<String>,
    pub(crate) remediation: String,
}

pub(crate) fn run_doctor(fix: bool) -> Result<()> {
    let settings = load_user_settings();
    let root = resolve_root(settings.root.as_deref());
    let theme = theme_from_settings(settings.color.as_deref());
    let mut checks = vec![
        DoctorCheck {
            name: "Root DevWorkflow".into(),
            passed: Path::new(&root).is_dir(),
            detail: Some(root.clone()),
            remediation: format!("Exécuter: dw init --root \"{root}\""),
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
            "Installer OpenCode selon la procedure d'equipe, puis verifier le PATH",
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

    println!("{}", render_doctor_report(&checks, &theme));
    if checks.iter().all(|check| check.passed) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("doctor a détecté des points à corriger."))
    }
}

fn render_doctor_report(checks: &[DoctorCheck], theme: &TerminalTheme) -> String {
    let passed_count = checks.iter().filter(|check| check.passed).count();
    let total_count = checks.len();
    let mut lines = vec![
        theme.command("Diagnostic Dev Workflow"),
        format!(
            "{} {passed_count}/{total_count} checks OK",
            if passed_count == total_count {
                theme.success("✓")
            } else {
                theme.warning("!")
            }
        ),
        String::new(),
    ];
    lines.extend(render_checks(checks, theme));
    lines.join("\n")
}

fn render_checks(checks: &[DoctorCheck], theme: &TerminalTheme) -> Vec<String> {
    let mut lines = Vec::new();
    for check in checks {
        let status = if check.passed {
            theme.success("✓ OK")
        } else {
            theme.warning("! WARN")
        };
        lines.push(format!("{:<8} {}", status, check.name));
        if let Some(detail) = check
            .detail
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(format!("         {}", theme.path(detail)));
        }
        if !check.passed {
            lines.push(format!("         {}", theme.command(&check.remediation)));
        }
    }
    lines
}

fn theme_from_settings(color: Option<&str>) -> TerminalTheme {
    let mode = match color.unwrap_or("auto").to_ascii_lowercase().as_str() {
        "always" => ColorMode::Always,
        "never" => ColorMode::Never,
        _ => ColorMode::Auto,
    };
    TerminalTheme::stdout(mode)
}

fn check_default_agent(root: &str) -> DoctorCheck {
    let agent = default_agent(root);
    match parse_agent_kind(Some(&agent)) {
        Ok(kind) => DoctorCheck {
            name: "Agent par defaut".into(),
            passed: true,
            detail: Some(kind.name().into()),
            remediation: "Configurer: dw agent config set-default opencode".into(),
        },
        Err(error) => DoctorCheck {
            name: "Agent par defaut".into(),
            passed: false,
            detail: Some(agent),
            remediation: error.to_string(),
        },
    }
}

fn check_command(
    file_name: &str,
    arguments: &[&str],
    name: &str,
    remediation: &str,
    validator: impl Fn(&str) -> bool,
) -> DoctorCheck {
    let Ok(output) = Command::new(file_name).args(arguments).output() else {
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
        let Ok(output) = Command::new(file_name).arg("--version").output() else {
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
    fn doctor_report_includes_summary_details_and_remediation() {
        let checks = vec![
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
        ];

        let report = render_doctor_report(&checks, &TerminalTheme::plain());

        assert!(report.contains("Diagnostic Dev Workflow"));
        assert!(report.contains("! 1/2 checks OK"));
        assert!(report.contains("✓ OK"));
        assert!(report.contains("/tmp/dw"));
        assert!(report.contains("Installer Git"));
    }
}
