use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::get_work_item_snapshots_authenticated;
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_git::{worktree_prune, worktree_remove};
use dw_workspace::{
    WorkspaceSummary, display_work_items, execute_task_sync, execute_task_teardown,
    filter_workspaces, find_workspaces, plan_task_prune, plan_task_teardown,
};

use crate::render::print_styled;

pub struct PruneArgs {
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub execute: bool,
    pub yes: bool,
    pub no_sync: bool,
    pub json: bool,
}

pub fn handle(args: PruneArgs) -> Result<()> {
    let PruneArgs {
        root,
        project,
        work_item,
        execute,
        yes,
        no_sync,
        json,
    } = args;

    let root = resolve_root(root.as_deref());
    if !no_sync {
        let workspaces = filter_workspaces(
            find_workspaces(&root),
            project.as_deref(),
            work_item.as_deref(),
        );
        sync_workspaces(&root, &workspaces, json);
    }

    let candidates = plan_task_prune(&root, project.as_deref(), work_item.as_deref());
    if json {
        println!("{}", serde_json::to_string_pretty(&candidates)?);
    } else if candidates.is_empty() {
        print_styled("Aucun workspace éligible au prune.");
    } else {
        for candidate in &candidates {
            print_styled(&prune_candidate_line(candidate));
        }
    }

    if candidates.is_empty() || !execute {
        if !candidates.is_empty() && !json {
            print_styled("");
            print_styled(
                "Prévisualisation uniquement. Relancer avec --execute --yes pour supprimer les workspaces éligibles.",
            );
        }
        return Ok(());
    }
    if !yes {
        return Err(anyhow::anyhow!(
            "Suppression destructive refusee: ajouter --yes avec --execute."
        ));
    }

    let projects = load_projects_config(&root);
    for candidate in candidates {
        let (_manifest, steps) = plan_task_teardown(&root, &projects, &candidate.path)?;
        execute_task_teardown(&candidate.path, &steps, |git_dir, args| match args {
            ["worktree", "remove", "--force", target] => {
                worktree_remove(git_dir, target).map_err(|error| error.to_string())
            }
            ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
            _ => Err(format!("commande git non supportée: {}", args.join(" "))),
        })?;
        if !json {
            print_styled(&format!("Workspace supprime: {}", candidate.path));
        }
    }

    Ok(())
}

fn sync_workspaces(root: &str, workspaces: &[WorkspaceSummary], json: bool) {
    let projects = load_projects_config(root);
    let workflow = load_workflow_config(root);
    let auth_options = match load_auth_options(Some(root)) {
        Ok(options) => options,
        Err(error) => {
            if !json {
                print_styled(&format!("Sync ignorée (auth indisponible): {error}"));
            }
            return;
        }
    };

    for workspace in workspaces {
        let result = (|| -> Result<()> {
            let mut options =
                resolve_ado_options(&projects, &workflow, &workspace.manifest.project)?;
            if options.project.trim().is_empty() {
                options.project = workspace.manifest.project.clone();
            }
            let token = require_token(auth_options.clone())?;
            let ids = workspace
                .manifest
                .parent_work_items()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>();
            let snapshots = get_work_item_snapshots_authenticated(&options, &ids, &token)?;
            let updated = execute_task_sync(&workspace.path, &snapshots)?;
            if !json {
                print_styled(&format!(
                    "Sync: {}",
                    display_work_items(&updated.parent_work_items(), true)
                ));
            }
            Ok(())
        })();

        if let Err(error) = result
            && !json
        {
            print_styled(&format!(
                "Sync ignorée [{}]: {}",
                display_work_items(&workspace.manifest.parent_work_items(), false),
                error
            ));
        }
    }
}

fn prune_candidate_line(candidate: &WorkspaceSummary) -> String {
    format!(
        "{} / {}: {}",
        candidate.manifest.project,
        display_work_items(&candidate.manifest.parent_work_items(), true),
        candidate.path
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_workspace::WorkspaceManifest;

    #[test]
    fn prune_no_sync_dry_run_does_not_require_auth() {
        let root = unique_temp_root();
        let workspace = root.join("projects/ha/workspaces/feat-1-done");
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        std::fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"1","taskId":null,"project":"ha","type":"feat","slug":"done","branchName":"feat/1-done","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"1","type":"User Story","title":"Done","state":"Valide"}]}"#,
        )
        .expect("manifest should be written");

        handle(PruneArgs {
            root: Some(root.display().to_string()),
            project: Some("ha".into()),
            work_item: None,
            execute: false,
            yes: false,
            no_sync: true,
            json: true,
        })
        .expect("offline dry-run should not require auth");

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn prune_candidate_line_includes_project_items_and_path() {
        let candidate = WorkspaceSummary {
            path: "/tmp/dw/projects/ha/workspaces/feat-1-done".into(),
            manifest: WorkspaceManifest {
                schema: 1,
                work_item_id: "1".into(),
                task_id: None,
                project: "ha".into(),
                kind: "feat".into(),
                slug: "done".into(),
                branch_name: "feat/1-done".into(),
                created_at: "2026-07-02T10:00:00Z".into(),
                repositories: vec!["front".into()],
                status: "created".into(),
                work_item_type: Some("User Story".into()),
                work_item_title: Some("Done".into()),
                work_item_state: Some("Valide".into()),
                child_task_ids: None,
                child_tasks: None,
                work_items: None,
            },
        };

        assert_eq!(
            prune_candidate_line(&candidate),
            "ha / #1 Done [Valide]: /tmp/dw/projects/ha/workspaces/feat-1-done"
        );
    }

    fn unique_temp_root() -> std::path::PathBuf {
        let suffix = format!(
            "dw-prune-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(suffix)
    }
}
