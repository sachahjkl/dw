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
    #[command(about = "Affiche le root, le mode couleur et les chemins de configuration.")]
    Show {
        #[arg(long, help = "Root DevWorkflow à inspecter.")]
        root: Option<String>,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Vérifie les fichiers de configuration et les schémas locaux.")]
    Doctor {
        #[arg(long, help = "Root DevWorkflow à vérifier.")]
        root: Option<String>,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Enregistre le root DevWorkflow utilisateur.")]
    SetRoot {
        #[arg(help = "Chemin du root DevWorkflow à enregistrer.")]
        path: String,
    },
    #[command(about = "Configure le mode couleur: auto, always ou never.")]
    SetColor {
        #[arg(help = "Mode couleur à enregistrer: auto, always ou never.")]
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
                for line in config_show_lines(&report) {
                    print_styled(&line);
                }
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
            format!("Prévisualisation init DevWorkflow: {}", report.root),
            format!("Profil: {}", report.profile),
        ];
        lines.extend(
            report
                .planned_paths
                .iter()
                .map(|path| format!("  + créer/mettre à jour: {path}")),
        );
        lines.push(if report.no_save {
            "  - settings utilisateur inchangés (--no-save).".into()
        } else {
            format!("  + enregistrer le root utilisateur: {}", report.root)
        });
        return lines;
    }

    let mut lines = vec![
        format!("Root DevWorkflow initialisé: {}", report.root),
        format!("Profil: {}", report.profile),
    ];
    if report.no_save {
        lines.push("Settings utilisateur non modifiés (--no-save).".into());
    }
    lines.push("Prochaine étape conseillée: dw doctor".into());
    lines
}

fn refresh_report_lines(report: &RefreshReport) -> Vec<String> {
    vec![
        format!("Root rafraîchi: {}", report.root),
        format!("Profil: {}", report.profile),
        "Schémas et contextes agents régénérés.".into(),
        "Fichiers utilisateurs préservés: projects.json, workflow.json, databases.json, plan.md."
            .into(),
    ]
}

fn config_show_lines(report: &crate::ConfigShow) -> Vec<String> {
    vec![
        format!("Root: {}", report.root),
        format!("Couleur: {}", report.color),
        format!("Settings: {}", report.settings_path),
        format!(
            "{} {}",
            if report.projects_exists { "✓" } else { "!" },
            report.projects_path
        ),
        format!(
            "{} {}",
            if report.workflow_exists { "✓" } else { "!" },
            report.workflow_path
        ),
        format!(
            "{} {}",
            if report.databases_exists { "✓" } else { "!" },
            report.databases_path
        ),
    ]
}

fn config_doctor_lines(report: &crate::ConfigDoctorReport) -> Vec<String> {
    let mut lines = vec![format!("Root: {}", report.root)];
    for check in &report.checks {
        lines.push(format!(
            "{} {}",
            if check.passed { "✓" } else { "!" },
            check.path
        ));
        if let Some(message) = &check.message {
            lines.push(format!("  -> {message}"));
        }
    }
    lines.push(if report.passed {
        "Configuration valide.".into()
    } else {
        "Configuration incomplète: corriger les points signalés puis relancer `dw config doctor`."
            .into()
    });
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

        assert_eq!(lines[0], "Root: /tmp/dw");
        assert_eq!(lines[1], "! /tmp/dw/config/projects.json");
        assert_eq!(lines[2], "  -> fichier absent");
        assert_eq!(
            lines[3],
            "Configuration incomplète: corriger les points signalés puis relancer `dw config doctor`."
        );
    }

    #[test]
    fn config_show_lines_include_paths_and_status() {
        let report = crate::ConfigShow {
            root: "/tmp/dw".into(),
            color: "auto".into(),
            settings_path: "/tmp/settings.json".into(),
            workflow_path: "/tmp/dw/config/workflow.json".into(),
            projects_path: "/tmp/dw/config/projects.json".into(),
            databases_path: "/tmp/dw/config/databases.json".into(),
            workflow_exists: true,
            projects_exists: false,
            databases_exists: true,
        };

        let lines = config_show_lines(&report);

        assert_eq!(lines[0], "Root: /tmp/dw");
        assert_eq!(lines[1], "Couleur: auto");
        assert_eq!(lines[2], "Settings: /tmp/settings.json");
        assert_eq!(lines[3], "! /tmp/dw/config/projects.json");
        assert_eq!(lines[4], "✓ /tmp/dw/config/workflow.json");
        assert_eq!(lines[5], "✓ /tmp/dw/config/databases.json");
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

        assert_eq!(lines[0], "Prévisualisation init DevWorkflow: /tmp/dw");
        assert!(lines.contains(&"  + créer/mettre à jour: /tmp/dw/config/projects.json".into()));
        assert!(lines.contains(&"  + enregistrer le root utilisateur: /tmp/dw".into()));
    }

    #[test]
    fn refresh_report_lines_include_preserved_user_files() {
        let report = RefreshReport {
            root: "/tmp/dw".into(),
            profile: "business".into(),
        };

        let lines = refresh_report_lines(&report);

        assert!(lines.contains(&"Root rafraîchi: /tmp/dw".into()));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Fichiers utilisateurs préservés"))
        );
    }
}
