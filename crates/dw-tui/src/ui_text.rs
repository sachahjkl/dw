use crate::actions::QuickOptionState;
use crate::app::App;
use crate::form::{FormMode, FormState};
use crate::history::RunHistoryEntry;
use crate::model::{ActionRisk, TuiAction, View};

pub(crate) fn help_lines() -> Vec<&'static str> {
    vec![
        "Tab / Shift-Tab: changer de vue",
        "1-7: vues principales    ?: aide",
        "j/k ou flèches: sélectionner une opération",
        "J/K: sélectionner un workspace dans la vue Workspaces",
        "ADO: J/K ou [/]: projet, j/k: work item, n/x/e/c/w: préparer/créer/état/contexte/fiche",
        "PRs: j/k: PR, n/x/N/f/c/d: préparer/créer/formulaire/finaliser/changements/diff",
        "Assistant: les formulaires réutilisent le contexte chargé dans les onglets",
        "Les chargements ADO/PR continuent en arrière-plan quand vous changez d'onglet",
        "h: afficher les lancements; i: afficher état/messages",
        "/: filtrer les opérations",
        "n: ouvrir l’assistant d’opération",
        "o: options rapides agent/config/couleur; O force options depuis Workspaces",
        "Entrée: lancer l'opération sélectionnée",
        "r: recharger les données",
        "q / Esc: quitter",
    ]
}

pub(crate) fn shortcut_bar_line(app: &App) -> String {
    let selected = match app.view {
        View::Dashboard => {
            let items = app.cockpit_items();
            items
                .get(app.selected_cockpit.min(items.len().saturating_sub(1)))
                .map(|item| item.primary_action.display_label())
                .unwrap_or_else(|| "Aucun item cockpit".into())
        }
        View::Composer => match app.action_form.mode {
            FormMode::Selecting => format!(
                "Parcours: {}",
                crate::form::FormTemplate::ALL[app.action_form.template_index].label()
            ),
            FormMode::Editing => app
                .action_form
                .build_action(&app.snapshot.root)
                .map(|action| action.display_label())
                .unwrap_or_else(|| "Action incomplète".into()),
        },
        View::Ado => app
            .selected_ado_action_preview()
            .unwrap_or_else(|| "Aucun work item ADO".into()),
        View::PullRequests => app
            .selected_pull_request_action_preview()
            .unwrap_or_else(|| "Aucune PR".into()),
        View::Db => app
            .selected_database_action_preview()
            .unwrap_or_else(|| "Aucune base DB".into()),
        View::Workspaces => app
            .selected_workspace_action_preview()
            .unwrap_or_else(|| "Aucun workspace".into()),
        _ => app
            .selected_visible_action()
            .map(|(_, action)| action.display_label())
            .unwrap_or_else(|| "Aucune action".into()),
    };
    let running = app
        .running_action_label()
        .map(|label| format!(" | en cours: {label}"))
        .unwrap_or_default();
    format!(
        "{} | {} | h lancements | i état | ? aide | q quitter{}",
        view_hint(app),
        selected,
        running
    )
}

pub(crate) fn state_modal_lines(app: &App) -> Vec<String> {
    let mut lines = vec![
        "Contexte courant".into(),
        shortcut_bar_line(app),
        String::new(),
        "Chargements".into(),
    ];
    lines.extend(app.background_status_lines());
    let queued = app.action_queue_status_lines();
    if !queued.is_empty() {
        lines.push(String::new());
        lines.push("File d'actions".into());
        lines.extend(queued);
    }
    lines.push(String::new());
    lines.push("Messages".into());
    if app.messages.is_empty() {
        lines.push("Aucun message.".into());
    } else {
        lines.extend(app.messages.iter().rev().take(12).rev().cloned());
    }
    lines.push(String::new());
    lines.push("Esc/i: fermer    j/k: scroller    Home/End: début/fin".into());
    lines
}

pub(crate) fn guide_detail_lines() -> Vec<String> {
    vec![
        "Guide de démarrage DevWorkflow".into(),
        String::new(),
        "1. Vérifier l'environnement".into(),
        "   Ouvrir Config puis lancer les diagnostics configuration et agents.".into(),
        "   Corriger les points bloquants avant de créer des workspaces.".into(),
        String::new(),
        "2. Lire le cockpit".into(),
        "   Le Dashboard priorise les PR sans workspace, workspaces actifs, work items assignés et alertes.".into(),
        "   Entrée lance l'opération primaire de la ligne sélectionnée.".into(),
        String::new(),
        "3. Traiter ADO et PRs".into(),
        "   Onglet ADO: sélectionner un projet et un work item, puis préparer, contextualiser ou ouvrir la fiche.".into(),
        "   Onglet PRs: charger les PR actives, créer un workspace, finir ou ouvrir la PR.".into(),
        String::new(),
        "4. Travailler un workspace".into(),
        "   Onglet Workspaces: ouvrir l'agent, vérifier, synchroniser, préparer handoff ou finaliser.".into(),
        "   Les actions destructives passent par une confirmation TUI explicite.".into(),
        String::new(),
        "5. Explorer les données".into(),
        "   Onglet DB: explorer le schéma, décrire une table ou lancer une requête guidée en lecture seule.".into(),
        "   Les résultats longs s'ouvrent dans la modale Lancements.".into(),
        String::new(),
        "6. Construire une opération avancée".into(),
        "   Onglet Composer: choisir un parcours, remplir les champs, appliquer les suggestions.".into(),
        "   La preview affiche l’intention TUI et son niveau de risque, pas une commande à recopier.".into(),
        String::new(),
        "Dans le TUI".into(),
        "   Les panneaux modaux affichent les résultats de lecture.".into(),
        "   Les lancements en arrière-plan restent dans le panneau Lancements.".into(),
        "   Esc: fermer    j/k: scroller    Home/End: début/fin".into(),
    ]
}

pub(crate) fn history_marker(entry: &RunHistoryEntry) -> &'static str {
    if entry.status == "en cours" {
        "..."
    } else if entry.success {
        "OK"
    } else {
        "KO"
    }
}

pub(crate) fn view_hint(app: &App) -> &'static str {
    match app.view {
        View::Dashboard => "j/k: cockpit    Entrée: décision    r: recharger    h: lancements",
        View::Workspaces => {
            "J/K: workspace    o: ouvrir    p: vérifier    s: sync    l: latest    v: handoff    c: commit    f/F: finir    t/x: supprimer"
        }
        View::Ado if app.assigned_loading() => {
            "Chargement ADO en arrière-plan; vous pouvez changer d'onglet."
        }
        View::Ado => {
            "n: préparer workspace    x: créer workspace    e/E: état workflow    c: contexte    w: fiche    u: ouvrir ADO"
        }
        View::PullRequests if app.pull_requests_loading() => {
            "Chargement PRs en arrière-plan; vous pouvez changer d'onglet."
        }
        View::PullRequests => {
            "n: préparer workspace    x: créer workspace    N: formulaire PR    f/F: finaliser    c: changements    d: diff    u: ouvrir PR"
        }
        View::Db => "Entrée/s: explorer schéma    d: décrire table    e: requête guidée",
        View::Config => {
            "s/d/f/g/a: config show/doctor/refresh/guide/agent doctor    o: options    r: reload"
        }
        View::Composer => {
            "Entrée: éditer/lancer    j/k/Tab: sélectionner    Ctrl+Espace: suggestion    Esc: parcours"
        }
        View::Help => "Tab: revenir aux vues    o: options    q: quitter",
    }
}

pub(crate) fn confirmation_lines(action: &TuiAction) -> Vec<String> {
    let mut lines = vec![
        format!("Risque      : {}", action.kind.risk_label()),
        format!("Opération   : {}", action.display_label()),
        format!("Description : {}", action.description),
    ];
    if matches!(action.kind, ActionRisk::Destructive) && action.bypasses_cli_confirmation() {
        lines.push("Confirmation destructive déjà portée par la requête TUI.".into());
    }
    lines.extend([
        String::new(),
        action.display_label(),
        String::new(),
        action.kind.confirmation_hint().into(),
    ]);
    lines
}

pub(crate) fn options_summary_lines(app: &App) -> Vec<String> {
    vec![
        format!("Root: {}", app.snapshot.root),
        format!(
            "Config: {} projets · {} repositories · {} DB · doctor {}",
            app.snapshot.project_count(),
            app.snapshot.repository_count(),
            app.snapshot.database_count(),
            if app.snapshot.config_doctor.passed {
                "OK"
            } else {
                "KO"
            }
        ),
        format!(
            "Agent: {}    Couleur: {}",
            app.snapshot.default_agent(),
            app.snapshot.color_mode
        ),
    ]
}

pub(crate) fn history_output_lines(app: &App) -> Vec<String> {
    let Some(entry) = app.history.selected_entry() else {
        return vec!["Aucun lancement sélectionné.".into()];
    };
    let marker = history_marker(entry);
    let mut lines = vec![
        format!(
            "Lancement   : {}/{}",
            app.history.selected_entry + 1,
            app.history.entries.len()
        ),
        format!("{marker} {} ({})", entry.request_label, entry.status),
        if entry.status == "en cours" {
            "Sortie en cours de capture; vous pouvez fermer cette modale sans interrompre l’action."
                .into()
        } else {
            "Esc/h: fermer    [/]: lancement    j/k: scroller    Home/End: début/fin".into()
        },
        String::new(),
    ];
    if entry.output_lines.is_empty() {
        if entry.status == "en cours" {
            lines.push("En attente de la première ligne de sortie...".into());
        } else {
            lines.push("Aucune sortie capturée pour ce lancement.".into());
        }
    } else {
        lines.extend(entry.output_lines.clone());
    }
    lines
}

pub(crate) fn option_active(
    state: QuickOptionState,
    current_agent: &str,
    current_color: &str,
) -> bool {
    match state {
        QuickOptionState::Agent(agent) => current_agent.eq_ignore_ascii_case(agent),
        QuickOptionState::Color(color) => current_color.eq_ignore_ascii_case(color),
        QuickOptionState::None => false,
    }
}

pub(crate) fn form_preview_lines(app: &App) -> Vec<String> {
    let Some(form) = &app.form else {
        return vec!["Aucun formulaire ouvert.".into()];
    };
    form_preview_lines_for(form, app)
}

pub(crate) fn action_builder_preview_lines(app: &App) -> Vec<String> {
    form_preview_lines_for(&app.action_form, app)
}

fn form_preview_lines_for(form: &FormState, app: &App) -> Vec<String> {
    let mut lines = Vec::new();
    match form.build_action(&app.snapshot.root) {
        Some(action) => {
            lines.push(action.display_label());
            lines.push(format!("Risque: {}", action.kind.risk_label()));
            if matches!(action.kind, ActionRisk::Destructive) && action.bypasses_cli_confirmation()
            {
                lines.push(
                    "Confirmation destructive portée par la requête TUI; confirmation TUI requise."
                        .into(),
                );
            }
        }
        None => lines.push("Action incomplète".into()),
    }
    if let Some(field) = form.fields.get(form.selected_field) {
        let value = if field.value.trim().is_empty() {
            "<vide>"
        } else {
            field.value.as_str()
        };
        lines.push(format!("Champ: {} = {}", field.label, value));
        if !field.help.trim().is_empty() {
            lines.push(format!("Aide: {}", field.help));
        }
    }
    if let Some(value) = form.selected_suggestion(&app.snapshot) {
        lines.push(format!("Suggestion: {value}"));
    }
    lines.push(
        "Entrée: lancer    Tab/flèches: champ    Ctrl+Espace: suggestion    Espace: toggle    Esc: annuler"
            .into(),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{FormState, FormTemplate};

    #[test]
    fn dashboard_hint_points_to_accelerators() {
        let app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        assert!(view_hint(&app).contains("cockpit"));
        assert!(view_hint(&app).contains("décision"));
    }

    #[test]
    fn dashboard_hint_stays_actionable_while_snapshot_loads() {
        let app = App::new(Some("/tmp/missing-dw-root".into()));

        assert!(app.snapshot_loading());
        assert!(view_hint(&app).contains("cockpit"));
        assert!(view_hint(&app).contains("recharger"));
    }

    #[test]
    fn actions_hint_targets_builder_not_cli_catalog() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Composer;

        assert!(view_hint(&app).contains("éditer/lancer"));
        assert!(shortcut_bar_line(&app).contains("Parcours: Créer workspace"));
    }

    #[test]
    fn ado_hint_lists_native_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Ado;

        let hint = view_hint(&app);

        assert!(hint.contains("préparer workspace"));
        assert!(hint.contains("état workflow"));
        assert!(hint.contains("e/E"));
        assert!(hint.contains("contexte"));
        assert!(hint.contains("ouvrir ADO"));
    }

    #[test]
    fn workspace_hint_lists_native_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Workspaces;

        let hint = view_hint(&app);

        assert!(hint.contains("vérifier"));
        assert!(hint.contains("sync"));
        assert!(hint.contains("latest"));
        assert!(hint.contains("handoff"));
        assert!(hint.contains("finir"));
        assert!(hint.contains("supprimer"));
    }

    #[test]
    fn pull_request_hint_lists_review_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::PullRequests;

        let hint = view_hint(&app);

        assert!(hint.contains("préparer workspace"));
        assert!(hint.contains("formulaire PR"));
        assert!(hint.contains("finaliser"));
        assert!(hint.contains("changements"));
        assert!(hint.contains("diff"));
        assert!(hint.contains("ouvrir PR"));
    }

    #[test]
    fn config_hint_lists_native_accelerators() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Config;

        let hint = view_hint(&app);

        assert!(hint.contains("config show/doctor"));
        assert!(!hint.contains("completion"));
        assert!(hint.contains("agent doctor"));
    }

    #[test]
    fn help_mentions_start_pr_form_prefill() {
        let help = help_lines().join("\n");

        assert!(help.contains("Assistant"));
        assert!(help.contains("contexte chargé"));
    }

    #[test]
    fn help_mentions_direct_view_shortcuts_and_help_key() {
        let help = help_lines().join("\n");

        assert!(help.contains("1-7: vues principales"));
        assert!(help.contains("?: aide"));
    }

    #[test]
    fn options_summary_lines_show_loaded_config_counts() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.projects = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" },
        "back": { "url": "", "defaultBranch": "main" }
      }
    }
  }
}"#,
        )
        .expect("projects config");
        app.snapshot.databases.globals.insert(
            "shared".into(),
            serde_json::json!({"provider": "sqlserver"}),
        );
        app.snapshot.color_mode = "always".into();
        app.snapshot.config_doctor.passed = true;

        let lines = options_summary_lines(&app);

        assert!(lines[0].contains("/tmp/missing-dw-root"));
        assert!(lines[1].contains("1 projets"));
        assert!(lines[1].contains("2 repositories"));
        assert!(lines[1].contains("1 DB"));
        assert!(lines[1].contains("doctor OK"));
        assert!(lines[2].contains("Couleur: always"));
    }

    #[test]
    fn confirmation_lines_include_risk_action_and_hint() {
        let action = TuiAction {
            label: "Teardown execute".into(),
            request: crate::model::TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some("/tmp/ws".into()),
                root: None,
                project: None,
                work_item: None,
                r#continue: false,
                positional_work_item: None,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
            }),
            description: "Supprimer le workspace".into(),
            kind: ActionRisk::Destructive,
        };

        let lines = confirmation_lines(&action);

        assert!(lines.iter().any(|line| line.contains("Risque")));
        assert!(lines.iter().any(|line| line.contains("Teardown execute")));
        assert!(lines.iter().any(|line| line.contains("destructive")));
        assert!(lines.iter().any(|line| line.contains("requête TUI")));
    }

    #[test]
    fn form_preview_lines_show_risk_and_injected_yes() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::AdoSetState)
            .expect("ado set-state template");
        form.begin_editing(&app.snapshot);
        for field in &mut form.fields {
            match field.label.as_str() {
                "Work items" => field.value = "42".into(),
                "Projet" => field.value = "ha".into(),
                "State" => field.value = "En réalisation".into(),
                _ => {}
            }
        }
        app.form = Some(form);

        let lines = form_preview_lines(&app);

        assert!(lines[0].contains("Assistant · Changer état ADO"));
        assert!(lines.iter().any(|line| line.contains("Risque:")));
        assert!(lines.iter().any(|line| line.contains("requête TUI")));
        assert!(lines.iter().any(|line| line.contains("Champ: Work items")));
        assert!(lines.iter().any(|line| line.contains("Aide: IDs ADO")));
    }

    #[test]
    fn form_preview_lines_explain_incomplete_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbQuery)
            .expect("db query template");
        form.begin_editing(&app.snapshot);
        form.fields
            .iter_mut()
            .find(|field| field.label == "SQL")
            .expect("sql field")
            .value
            .clear();
        app.form = Some(form);

        let lines = form_preview_lines(&app);

        assert_eq!(lines[0], "Action incomplète");
        assert!(lines.iter().any(|line| line == "Champ: Projet = <vide>"));
        assert!(
            lines
                .iter()
                .any(|line| line == "Aide: Projet configuré optionnel")
        );
        assert!(lines.last().is_some_and(|line| line.contains("Entrée")));
    }

    #[test]
    fn history_marker_distinguishes_running_success_and_failure() {
        let running = RunHistoryEntry {
            request_label: "Task finish".into(),
            status: "en cours".into(),
            success: true,
            output_preview: Vec::new(),
            output_lines: Vec::new(),
        };
        let success = RunHistoryEntry {
            status: "exit 0".into(),
            ..running.clone()
        };
        let failure = RunHistoryEntry {
            status: "exit 1".into(),
            success: false,
            ..running.clone()
        };

        assert_eq!(history_marker(&running), "...");
        assert_eq!(history_marker(&success), "OK");
        assert_eq!(history_marker(&failure), "KO");
    }

    #[test]
    fn history_output_lines_explain_running_capture() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.start_running("Task finish".into());

        let lines = history_output_lines(&app);

        assert!(lines[1].contains("en cours"));
        assert!(lines[2].contains("Sortie en cours de capture"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("première ligne de sortie"))
        );
    }

    #[test]
    fn history_output_lines_keep_empty_finished_output_clear() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.push(RunHistoryEntry {
            request_label: "Doctor".into(),
            status: "exit 0".into(),
            success: true,
            output_preview: Vec::new(),
            output_lines: Vec::new(),
        });

        let lines = history_output_lines(&app);

        assert!(lines[2].contains("Esc/h"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Aucune sortie capturée"))
        );
    }

    #[test]
    fn option_active_matches_agent_and_color() {
        assert!(option_active(
            QuickOptionState::Agent("codex"),
            "codex",
            "auto"
        ));
        assert!(option_active(
            QuickOptionState::Color("always"),
            "codex",
            "always"
        ));
        assert!(!option_active(
            QuickOptionState::Agent("opencode"),
            "codex",
            "auto"
        ));
        assert!(!option_active(QuickOptionState::None, "codex", "auto"));
    }
}
