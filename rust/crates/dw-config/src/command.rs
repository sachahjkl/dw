use anyhow::Result;
use clap::Subcommand;
use dw_ui::TerminalTheme;

use crate::{config_doctor, config_show, set_color_mode, set_user_root};

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
}
