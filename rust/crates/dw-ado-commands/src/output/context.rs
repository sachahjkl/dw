use dw_contracts::AdoAiContextItem;
use dw_ui::TerminalTheme;

pub fn render_context_items(
    items: &[AdoAiContextItem],
    comment_limit: i32,
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("---".into());
            lines.push(String::new());
        }

        lines.push(format_context_header(item, theme));
        lines.push(format!(
            "Assigné à: {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigné")
        ));
        if let Some(metadata) = work_item_metadata(item)
            && !metadata.is_empty()
        {
            lines.push(format!("Métadonnées: {metadata}"));
        }

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            lines.push(String::new());
            lines.push(theme.bold("Description:"));
            lines.push(description.trim().into());
        }

        if let Some(acceptance_criteria) = &item.content.acceptance_criteria
            && !acceptance_criteria.trim().is_empty()
        {
            lines.push(String::new());
            lines.push(theme.bold("Critères d'acceptation:"));
            lines.push(acceptance_criteria.trim().into());
        }

        if !item.attachments.items.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold(&format!(
                "Pièces jointes ({})",
                item.attachments.items.len()
            )));
            lines.push(format!("  Dossier: {}", item.attachments.directory_hint));
            for attachment in &item.attachments.items {
                lines.push(format!(
                    "- {}",
                    attachment
                        .name
                        .as_deref()
                        .or(attachment.url.as_deref())
                        .unwrap_or("pièce jointe sans nom")
                ));
            }
        }

        if !item.relations.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Relations:"));
            for relation in &item.relations {
                lines.push(format!(
                    "- {} {}",
                    relation.kind,
                    relation
                        .work_item_id
                        .as_deref()
                        .or(relation.url.as_deref())
                        .unwrap_or("")
                ));
            }
        }

        if comment_limit != 0 && !item.comments.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Commentaires:"));
            for comment in item.comments.iter().take(comment_limit.max(0) as usize) {
                lines.push(format!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                ));
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "AI context: {}",
            theme.command(&format!(
                "dw ado ai-context {} --project {}",
                item.work_item.id, project
            ))
        ));
    }
    lines.join("\n")
}

fn format_context_header(item: &AdoAiContextItem, theme: &TerminalTheme) -> String {
    format!(
        "{} {} {}",
        theme.success(&format!("#{}", item.work_item.id)),
        theme.dim(&format!(
            "[{} / {}]",
            item.work_item.kind.as_deref().unwrap_or("type inconnu"),
            item.work_item.state.as_deref().unwrap_or("état inconnu")
        )),
        item.work_item.title.as_deref().unwrap_or("(sans titre)")
    )
}

fn work_item_metadata(item: &AdoAiContextItem) -> Option<String> {
    let mut values = Vec::new();
    if let Some(area) = item
        .work_item
        .area_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        values.push(format!("area={area}"));
    }
    if let Some(iteration) = item
        .work_item
        .iteration_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        values.push(format!("iteration={iteration}"));
    }
    if !item.work_item.tags.is_empty() {
        values.push(format!("tags={}", item.work_item.tags.join(", ")));
    }
    (!values.is_empty()).then(|| values.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_render_includes_relations_comments_and_ai_context_command() {
        let output = render_context_items(
            &[AdoAiContextItem {
                schema_version: dw_contracts::AI_CONTEXT_VERSION.into(),
                work_item: dw_contracts::AdoAiContextWorkItem {
                    id: "42".into(),
                    url: None,
                    title: Some("Corriger".into()),
                    kind: Some("Bug".into()),
                    state: Some("Actif".into()),
                    assigned_to: Some("Sacha".into()),
                    area_path: Some("Produit\\Backlog".into()),
                    iteration_path: Some("Sprint 1".into()),
                    tags: vec!["urgent".into()],
                },
                core: dw_contracts::AdoAiContextCore {
                    created_by: None,
                    created_date: None,
                    changed_by: None,
                    changed_date: None,
                    priority: None,
                    value_area: None,
                },
                content: dw_contracts::AdoAiContextContent {
                    description: Some("Description courte".into()),
                    acceptance_criteria: Some("Critère A".into()),
                    product_context: Default::default(),
                },
                links: dw_contracts::AdoAiContextLinks {
                    parent_ids: vec![],
                    child_ids: vec![],
                    predecessor_ids: vec![],
                    successor_ids: vec![],
                },
                attachments: dw_contracts::AdoAiContextAttachments {
                    directory_hint: "attachments/ado/42".into(),
                    items: vec![dw_contracts::AdoAiContextAttachment {
                        name: Some("capture.png".into()),
                        url: Some("https://example.invalid/capture.png".into()),
                        comment: None,
                        directory_hint: "attachments/ado/42".into(),
                    }],
                },
                relations: vec![dw_contracts::AdoAiContextRelation {
                    kind: "Parent".into(),
                    rel: None,
                    work_item_id: Some("1".into()),
                    name: None,
                    url: None,
                    comment: None,
                    artifact: None,
                    display: "Parent #1".into(),
                }],
                comments: vec![dw_contracts::AdoAiContextComment {
                    author: Some("Bob".into()),
                    created_date: None,
                    text: Some("OK".into()),
                }],
            }],
            10,
            "ha",
            &TerminalTheme::plain(),
        );

        assert!(output.contains("#42 [Bug / Actif] Corriger"));
        assert!(
            output
                .contains("Métadonnées: area=Produit\\Backlog | iteration=Sprint 1 | tags=urgent")
        );
        assert!(output.contains("Description courte"));
        assert!(output.contains("Critères d'acceptation:"));
        assert!(output.contains("Critère A"));
        assert!(output.contains("Pièces jointes (1)"));
        assert!(output.contains("capture.png"));
        assert!(output.contains("- Parent 1"));
        assert!(output.contains("- Bob: OK"));
        assert!(output.contains("dw ado ai-context 42 --project ha"));
    }
}
