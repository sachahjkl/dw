use crate::WorkspaceManifest;

pub(crate) fn plan_markdown(manifest: &WorkspaceManifest) -> String {
    format!(
        "# Plan - Work items {}\n\nProjet: `{}`\n\n## Résumé fonctionnel\n\nTODO\n\n## Repositories impactés\n\n{}\n\n## Analyse code\n\nTODO\n\n## Plan technique\n\nTODO\n\n## Risques\n\nTODO\n\n## Vérification\n\nTODO\n",
        manifest
            .parent_work_items()
            .iter()
            .map(|item| format!("#{}", item.id))
            .collect::<Vec<_>>()
            .join(", "),
        manifest.project,
        manifest
            .repositories
            .iter()
            .map(|repo| format!("- {repo}: TODO"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub(crate) fn handoff_markdown(manifest: &WorkspaceManifest, repository: &str) -> String {
    format!(
        "# Handoff {repository}\n\n## Contexte\n\n- Projet: `{}`\n- Repository: `{repository}`\n- Branche: `{}`\n- Work items parents: {}\n- Child tasks connus: (aucune)\n\n## Entrées déterministes à relire\n\n1. `task.json`\n2. `plan.md`\n3. `AGENTS.md`\n4. `dw ado ai-context <id> --project {}` pour chaque work item parent\n5. `dw task preflight --continue`\n\n## Objectif du lot\n\nDécrire ici, dans `plan.md`, ce qui relève de `{repository}` et ce qui doit être traité par ce handoff.\n\n## Contraintes\n\n- Préserver les labels métier exacts\n- Tout texte utilisateur/projet en français\n- Traiter les screenshots, mockups et attachments comme sources factuelles\n- Demander à l'utilisateur au lieu de deviner si le contexte manque\n- Vérifier les impacts API et les contrats front/back quand pertinent\n\n## Travail attendu\n\n- Limiter le travail à `{repository}`\n- Lister clairement les fichiers/zones impactés\n- Signaler les dépendances vers d'autres domaines\n- Mettre à jour la synthèse structurée ci-dessous\n\n## Synthèse structurée attendue\n\nRemplir ce bloc sans changer les labels.\n\n```yaml\nstatus: todo\nrepository: {repository}\nsummary:\n  done: []\n  decisions: []\n  risks: []\n  blockers: []\n  follow_up: []\nverification:\n  commands: []\n  manual_checks: []\nartifacts:\n  files: []\n  screenshots: []\n  attachments: []\n```\n",
        manifest.project,
        manifest.branch_name,
        manifest
            .parent_work_items()
            .iter()
            .map(|item| format!("`#{}`", item.id))
            .collect::<Vec<_>>()
            .join(", "),
        manifest.project,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_template_is_french_and_lists_repositories() {
        let manifest = manifest();

        let text = plan_markdown(&manifest);

        assert!(text.starts_with("# Plan - Work items #42"));
        assert!(text.contains("Projet: `ha`"));
        assert!(text.contains("## Résumé fonctionnel"));
        assert!(text.contains("- front: TODO"));
        assert!(text.contains("- back: TODO"));
        assert!(!text.contains("Functional Summary"));
    }

    #[test]
    fn handoff_template_preserves_summary_contract() {
        let manifest = manifest();

        let text = handoff_markdown(&manifest, "front");

        assert!(text.contains("# Handoff front"));
        assert!(text.contains("- Repository: `front`"));
        assert!(text.contains("dw ado ai-context <id> --project ha"));
        assert!(text.contains("status: todo"));
        assert!(text.contains("manual_checks: []"));
    }

    fn manifest() -> WorkspaceManifest {
        WorkspaceManifest {
            schema: 1,
            work_item_id: "42".into(),
            task_id: None,
            project: "ha".into(),
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/42-demo".into(),
            created_at: "2026-07-03T10:00:00Z".into(),
            repositories: vec!["front".into(), "back".into()],
            status: "created".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Titre".into()),
            work_item_state: Some("New".into()),
            child_task_ids: None,
            child_tasks: None,
            work_items: None,
        }
    }
}
