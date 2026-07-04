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
        "Guide de démarrage pas à pas".into(),
        String::new(),
        "1. Vérifier l'installation".into(),
        format!("   {}", theme.command("dw version")),
        format!("   {}", theme.command("dw doctor")),
        "   Corriger les prérequis signalés avant de créer des workspaces.".into(),
        String::new(),
        "2. Initialiser le root DevWorkflow".into(),
        format!("   {}", theme.command("dw init")),
        format!("   {}", theme.command("dw config show")),
        "   Le root contient config, schemas, cache, projets, workspaces et contextes agents.".into(),
        "   Pour choisir un chemin explicite:".into(),
        format!("   {}", theme.command("dw init --root ~/dev/dw")),
        String::new(),
        "3. Brancher Azure DevOps".into(),
        format!("   {}", theme.command("dw auth login")),
        format!("   {}", theme.command("dw auth status")),
        format!("   {}", theme.command("dw ado assigned")),
        "   Sans --project, dw propose les projets configurés quand le terminal est interactif.".into(),
        String::new(),
        "4. Créer un workspace de travail".into(),
        format!("   {}", theme.command("dw task start <work-item-id>")),
        "   Sans --execute, dw affiche le plan: branche, repositories, worktrees, handoffs.".into(),
        format!("   {}", theme.command("dw task start <work-item-id> --execute")),
        format!("   {}", theme.command("dw task open --continue")),
        "   L'agent configuré s'ouvre dans le workspace avec le contexte DevWorkflow.".into(),
        String::new(),
        "5. Boucle quotidienne".into(),
        format!("   {}", theme.command("dw task status")),
        format!("   {}", theme.command("dw task list")),
        format!("   {}", theme.command("dw task current")),
        format!("   {}", theme.command("dw task preflight --continue")),
        format!("   {}", theme.command("dw task sync --continue")),
        "   Utiliser preflight avant l'implémentation et sync pour rafraîchir task.json depuis ADO.".into(),
        String::new(),
        "6. Gérer le contenu du workspace".into(),
        format!("   {}", theme.command("dw task add-work-item --continue")),
        format!("   {}", theme.command("dw task remove-work-item --continue")),
        format!("   {}", theme.command("dw task add-repo --continue")),
        format!("   {}", theme.command("dw task repo-latest --continue")),
        "   Les commandes interactives proposent les valeurs locales quand elles sont disponibles.".into(),
        String::new(),
        "7. Préparer la fin de tâche".into(),
        format!("   {}", theme.command("dw task handoff-validate --continue")),
        format!("   {}", theme.command("dw task commit --continue")),
        format!("   {}", theme.command("dw task finish --continue")),
        "   Ces commandes sont en preview par défaut. Ajouter --execute seulement après lecture du plan.".into(),
        format!("   {}", theme.command("dw task finish --continue --execute")),
        String::new(),
        "8. Nettoyer".into(),
        format!("   {}", theme.command("dw task teardown --continue")),
        format!("   {}", theme.command("dw task prune")),
        "   teardown et prune suppriment seulement avec --execute, et demandent confirmation en interactif.".into(),
        String::new(),
        "9. ADO, DB et contexte IA".into(),
        format!("   {}", theme.command("dw ado work-item <id>")),
        format!("   {}", theme.command("dw ado context <id>")),
        format!("   {}", theme.command("dw ado changelog <ids>")),
        format!("   {}", theme.command("dw db schema")),
        format!("   {}", theme.command("dw db describe <table>")),
        format!("   {}", theme.command("dw db query --sql \"select top 20 * from ...\"")),
        format!("   {}", theme.command("dw agent context")),
        "   Les accès DB sont protégés par la garde read-only.".into(),
        String::new(),
        "10. Productivité shell".into(),
        format!("   {}", theme.command("dw completion show")),
        format!("   {}", theme.command("dw completion install fish")),
        format!("   {}", theme.command("dw completion install powershell")),
        "   Les complétions proposent options, projets, repositories, workspaces, databases et descriptions.".into(),
        String::new(),
        "Diagnostic rapide".into(),
        format!("   {}", theme.command("dw doctor --fix")),
        format!("   {}", theme.command("dw config doctor")),
        format!("   {}", theme.command("dw refresh")),
        "   refresh régénère schemas et contextes agents sans écraser les fichiers utilisateur.".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_renders_version_and_next_steps() {
        let lines = render_guide("2026.07.02.3+54011f0", &TerminalTheme::plain());

        assert_eq!(lines[0], "Dev Workflow 2026.07.02.3+54011f0");
        assert!(lines.contains(&"Guide de démarrage pas à pas".into()));
        assert!(lines.iter().any(|line| line.contains("dw init")));
        assert!(lines.iter().any(|line| line.contains("dw doctor")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task start <work-item-id>"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task finish --continue"))
        );
        assert!(lines.iter().any(|line| line.contains("dw db query")));
        assert!(lines.iter().any(|line| line.contains("dw ado assigned")));
        assert!(lines.iter().any(|line| line.contains("dw completion show")));
    }
}
