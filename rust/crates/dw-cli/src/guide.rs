use dw_ui::TerminalTheme;

pub(crate) fn print_guide(version: &str) {
    print_styled_lines(&render_guide(version, &TerminalTheme::stdout_auto()));
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

fn print_styled_lines(lines: &[String]) {
    for line in lines {
        print_styled(line);
    }
}

fn render_guide(version: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command(&format!("Dev Workflow {version}")),
        "Parcours recommandé".into(),
        format!("  1. {}", theme.command("dw init")),
        format!("  2. {}", theme.command("dw doctor")),
        format!("  3. {}", theme.command("dw task start <work-item-id>")),
        String::new(),
        "Commandes utiles".into(),
        format!("  - {}", theme.command("dw ado assigned")),
        format!("  - {}", theme.command("dw task current")),
        format!("  - {}", theme.command("dw completion show")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_renders_version_and_next_steps() {
        let lines = render_guide("2026.07.02.3+54011f0", &TerminalTheme::plain());

        assert_eq!(lines[0], "Dev Workflow 2026.07.02.3+54011f0");
        assert!(lines.contains(&"Parcours recommandé".into()));
        assert!(lines.iter().any(|line| line.contains("dw init")));
        assert!(lines.iter().any(|line| line.contains("dw doctor")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task start <work-item-id>"))
        );
        assert!(lines.iter().any(|line| line.contains("dw ado assigned")));
        assert!(lines.iter().any(|line| line.contains("dw completion show")));
    }
}
