use crate::commands::project::{resolve_ado_options, resolve_cli_ado_options};
use crate::commands::work_item::parse_work_item_ids_as_strings;
use crate::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{get_ai_context, get_work_item_expanded};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};

#[derive(Debug, Clone)]
pub struct ContextArgs {
    pub id: String,
    pub root: Option<String>,
    pub project: Option<String>,
    pub summary: bool,
    pub comments: i32,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct AiContextArgs {
    pub root: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub id: String,
    pub summary: bool,
    pub include_comments: bool,
}

pub fn handle_context(args: ContextArgs) -> Result<()> {
    let ContextArgs {
        id,
        root,
        project,
        summary,
        comments,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado context requiert --project configure."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let token = require_token(load_auth_options(Some(&root))?)?;
    let ids = parse_work_item_ids_as_strings(&id)?;
    if json {
        let payloads = ids
            .iter()
            .map(|item_id| get_work_item_expanded(&options, item_id, &token))
            .collect::<Result<Vec<_>, _>>()?;
        println!("{}", serde_json::to_string_pretty(&payloads)?);
    } else {
        let items = ids
            .iter()
            .map(|item_id| get_ai_context(&options, item_id, summary, &token))
            .collect::<Result<Vec<_>, _>>()?;
        print_context_items(&items, comments, &project_key);
    }
    Ok(())
}

pub fn handle_ai_context(args: AiContextArgs) -> Result<()> {
    let AiContextArgs {
        root,
        organization,
        project,
        id,
        summary,
        include_comments,
    } = args;
    let root = resolve_root(root.as_deref());
    let options = resolve_cli_ado_options(&root, organization, project)?;
    let token = require_token(load_auth_options(Some(&root))?)?;
    let contexts = parse_work_item_ids_as_strings(&id)?
        .iter()
        .map(|item_id| get_ai_context(&options, item_id, summary, &token))
        .map(|context| {
            context.map(|context| {
                if include_comments {
                    context
                } else {
                    dw_contracts::AdoAiContextItem {
                        comments: vec![],
                        ..context
                    }
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    println!("{}", serde_json::to_string_pretty(&contexts)?);
    Ok(())
}

fn print_context_items(
    items: &[dw_contracts::AdoAiContextItem],
    comment_limit: i32,
    project: &str,
) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
            println!();
        }

        println!("#{}", item.work_item.id);
        println!(
            "Type: {}",
            item.work_item.kind.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Etat: {}",
            item.work_item.state.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Titre: {}",
            item.work_item.title.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Assigne a: {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigne")
        );

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            println!();
            println!("Description:");
            println!("{}", description.trim());
        }

        if !item.relations.is_empty() {
            println!();
            println!("Relations:");
            for relation in &item.relations {
                println!(
                    "- {} {}",
                    relation.kind,
                    relation
                        .work_item_id
                        .as_deref()
                        .or(relation.url.as_deref())
                        .unwrap_or("")
                );
            }
        }

        if comment_limit != 0 && !item.comments.is_empty() {
            println!();
            println!("Commentaires:");
            for comment in item.comments.iter().take(comment_limit.max(0) as usize) {
                println!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                );
            }
        }

        println!();
        println!(
            "AI context: dw ado ai-context {} --project {}",
            item.work_item.id, project
        );
    }
}
