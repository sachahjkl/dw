use anyhow::Result;
use clap::Subcommand;
use dw_ui::TerminalTheme;

use crate::{
    InitReport, InitRequest, RefreshReport, RefreshRequest, config_doctor, config_show, init_root,
    refresh_root, resolve_root, set_color_mode, set_user_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitCommandArgs {
    pub profile: String,
    pub root: Option<String>,
    pub dry_run: bool,
    pub no_save: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshCommandArgs {
    pub root: Option<String>,
    pub profile: String,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Show {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    SetRoot {
        path: String,
    },
    SetColor {
        mode: String,
    },
}

pub fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show { root, json } => {
            let report = config_show(root.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_styled(&format!("Root: {}", report.root));
                print_styled(&format!("Color: {}", report.color));
            }
        }
        ConfigCommand::Doctor { root, json } => {
            let report = config_doctor(root.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for line in config_doctor_lines(&report) {
                    print_styled(&line);
                }
            }
            if !report.passed {
                std::process::exit(1);
            }
        }
        ConfigCommand::SetRoot { path } => {
            print_styled(&format!("Root: {}", set_user_root(&path)?));
        }
        ConfigCommand::SetColor { mode } => {
            print_styled(&format!("Color: {}", set_color_mode(&mode)?));
        }
    }
    Ok(())
}

pub fn handle_init(args: InitCommandArgs) -> Result<()> {
    let report = init_root(InitRequest {
        root: args.root,
        profile: args.profile,
        no_save: args.no_save,
        dry_run: args.dry_run,
    })?;
    for line in init_report_lines(&report) {
        print_styled(&line);
    }
    Ok(())
}

pub fn handle_refresh(args: RefreshCommandArgs) -> Result<()> {
    let root = resolve_root(args.root.as_deref());
    let report = refresh_root(RefreshRequest {
        root,
        profile: Some(args.profile),
    })?;
    for line in refresh_report_lines(&report) {
        print_styled(&line);
    }
    Ok(())
}

fn init_report_lines(report: &InitReport) -> Vec<String> {
    if report.dry_run {
        let mut lines = vec![
            format!("Dry-run init DevWorkflow: {}", report.root),
            format!("Profil: {}", report.profile),
        ];
        lines.extend(
            report
                .planned_paths
                .iter()
                .map(|path| format!("  would create/write: {path}")),
        );
        lines.push(if report.no_save {
            "  would not modify user settings (--no-save).".into()
        } else {
            format!("  would save user root: {}", report.root)
        });
        return lines;
    }

    let mut lines = vec![
        format!("Root DevWorkflow initialise: {}", report.root),
        format!("Profil: {}", report.profile),
    ];
    if report.no_save {
        lines.push("Settings utilisateur non modifies (--no-save).".into());
    }
    lines.push("Prochaine etape conseillee: dw doctor".into());
    lines
}

fn refresh_report_lines(report: &RefreshReport) -> Vec<String> {
    vec![
        format!("Root rafraichi: {}", report.root),
        format!("Profil: {}", report.profile),
        "Schemas et contextes agents regeneres.".into(),
        "Fichiers utilisateurs preserves: projects.json, workflow.json, databases.json, plan.md."
            .into(),
    ]
}

fn config_doctor_lines(report: &crate::ConfigDoctorReport) -> Vec<String> {
    let mut lines = Vec::new();
    for check in &report.checks {
        lines.push(format!(
            "{} {}",
            if check.passed { "[OK]  " } else { "[WARN]" },
            check.path
        ));
        if let Some(message) = &check.message {
            lines.push(format!("      {message}"));
        }
    }
    lines
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_doctor_lines_include_status_and_message() {
        let report = crate::ConfigDoctorReport {
            root: "/tmp/dw".into(),
            passed: false,
            checks: vec![crate::ConfigDoctorCheck {
                path: "/tmp/dw/config/projects.json".into(),
                passed: false,
                message: Some("fichier absent".into()),
            }],
        };

        let lines = config_doctor_lines(&report);

        assert_eq!(lines[0], "[WARN] /tmp/dw/config/projects.json");
        assert_eq!(lines[1], "      fichier absent");
    }

    #[test]
    fn init_report_lines_keep_dry_run_paths_and_save_hint() {
        let report = InitReport {
            root: "/tmp/dw".into(),
            profile: "business".into(),
            dry_run: true,
            no_save: false,
            planned_paths: vec!["/tmp/dw/config/projects.json".into()],
        };

        let lines = init_report_lines(&report);

        assert_eq!(lines[0], "Dry-run init DevWorkflow: /tmp/dw");
        assert!(lines.contains(&"  would create/write: /tmp/dw/config/projects.json".into()));
        assert!(lines.contains(&"  would save user root: /tmp/dw".into()));
    }

    #[test]
    fn refresh_report_lines_include_preserved_user_files() {
        let report = RefreshReport {
            root: "/tmp/dw".into(),
            profile: "business".into(),
        };

        let lines = refresh_report_lines(&report);

        assert!(lines.contains(&"Root rafraichi: /tmp/dw".into()));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Fichiers utilisateurs preserves"))
        );
    }
}
