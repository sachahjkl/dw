use crate::json::{
    clean_text, element_text, field_text, identity_text, work_item_id_from_relation_url,
};
use crate::{AzureDevOpsOptions, work_item_web_url};
use dw_contracts::{
    AI_CONTEXT_VERSION, ATTACHMENT_DIRECTORY_PREFIX, AdoAiContextAttachment,
    AdoAiContextAttachments, AdoAiContextComment, AdoAiContextContent, AdoAiContextCore,
    AdoAiContextItem, AdoAiContextLinks, AdoAiContextRelation, AdoAiContextWorkItem,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn map_ai_context_item(
    root: &Value,
    azure_dev_ops: &AzureDevOpsOptions,
    summary_only: bool,
    comments: Vec<AdoAiContextComment>,
) -> AdoAiContextItem {
    let fields = root.get("fields").cloned().unwrap_or(Value::Null);
    let id = element_text(root.get("id")).unwrap_or_default();
    let relations = parse_relations(root);
    let attachment_directory = format!("{ATTACHMENT_DIRECTORY_PREFIX}{id}");
    let attachment_items = relations
        .iter()
        .filter(|relation| relation.kind == "attachment")
        .map(|relation| AdoAiContextAttachment {
            name: relation.name.clone(),
            url: relation.url.clone(),
            comment: relation.comment.clone(),
            directory_hint: attachment_directory.clone(),
        })
        .collect::<Vec<_>>();

    AdoAiContextItem {
        schema_version: AI_CONTEXT_VERSION.into(),
        work_item: AdoAiContextWorkItem {
            id: id.clone(),
            url: Some(work_item_web_url(azure_dev_ops, &id)),
            title: field_text(&fields, "System.Title"),
            kind: field_text(&fields, "System.WorkItemType"),
            state: field_text(&fields, "System.State"),
            assigned_to: identity_text(fields.get("System.AssignedTo")),
            area_path: field_text(&fields, "System.AreaPath"),
            iteration_path: field_text(&fields, "System.IterationPath"),
            tags: split_tags(field_text(&fields, "System.Tags")),
        },
        core: AdoAiContextCore {
            created_by: identity_text(fields.get("System.CreatedBy")),
            created_date: field_text(&fields, "System.CreatedDate"),
            changed_by: identity_text(fields.get("System.ChangedBy")),
            changed_date: field_text(&fields, "System.ChangedDate"),
            priority: field_text(&fields, "Microsoft.VSTS.Common.Priority"),
            value_area: field_text(&fields, "Microsoft.VSTS.Common.ValueArea"),
        },
        content: AdoAiContextContent {
            description: clean_text(field_text(&fields, "System.Description")),
            acceptance_criteria: clean_text(field_text(
                &fields,
                "Microsoft.VSTS.Common.AcceptanceCriteria",
            )),
            product_context: extract_product_context(&fields),
        },
        links: AdoAiContextLinks {
            parent_ids: distinct_relation_ids(&relations, "parent"),
            child_ids: distinct_relation_ids(&relations, "child"),
            predecessor_ids: distinct_relation_ids(&relations, "predecessor"),
            successor_ids: distinct_relation_ids(&relations, "successor"),
        },
        attachments: AdoAiContextAttachments {
            directory_hint: attachment_directory,
            items: attachment_items,
        },
        relations: if summary_only { vec![] } else { relations },
        comments,
    }
}

fn parse_relations(root: &Value) -> Vec<AdoAiContextRelation> {
    root.get("relations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|relation| parse_relation(&relation))
        .collect()
}

fn parse_relation(relation: &Value) -> AdoAiContextRelation {
    let rel = relation
        .get("rel")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let url = relation
        .get("url")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let name = relation
        .get("attributes")
        .and_then(|attributes| attributes.get("name"))
        .and_then(|value| element_text(Some(value)));
    let comment = relation
        .get("attributes")
        .and_then(|attributes| attributes.get("comment"))
        .and_then(|value| element_text(Some(value)))
        .and_then(|value| clean_text(Some(value)));
    let work_item_id = url.as_deref().and_then(work_item_id_from_relation_url);
    let kind = relation_kind(rel.as_deref(), work_item_id.as_deref(), url.as_deref());
    let display = describe_relation_target(
        rel.as_deref(),
        work_item_id.as_deref(),
        name.as_deref(),
        url.as_deref(),
    );

    AdoAiContextRelation {
        kind: kind.into(),
        rel,
        work_item_id,
        name,
        url,
        comment,
        artifact: None,
        display,
    }
}

fn relation_kind(rel: Option<&str>, related_id: Option<&str>, url: Option<&str>) -> &'static str {
    if let Some(rel) = rel {
        if rel.contains("Hierarchy-Reverse") {
            return "parent";
        }
        if rel.contains("Hierarchy-Forward") {
            return "child";
        }
        if rel.contains("Dependency-Reverse") {
            return "predecessor";
        }
        if rel.contains("Dependency-Forward") {
            return "successor";
        }
        if rel.contains("AttachedFile") {
            return "attachment";
        }
    }
    if related_id.is_some() {
        return "work-item";
    }
    if url.is_some() {
        return "artifact";
    }
    "relation"
}

fn describe_relation_target(
    rel: Option<&str>,
    related_id: Option<&str>,
    name: Option<&str>,
    url: Option<&str>,
) -> String {
    if let Some(related_id) = related_id {
        return format!(
            "#{related_id} {}",
            name.unwrap_or(rel.unwrap_or("(relation)"))
        );
    }
    if rel.is_some_and(|value| value.contains("AttachedFile")) && name.is_some() && url.is_some() {
        return format!("{} ({})", name.unwrap_or_default(), url.unwrap_or_default());
    }
    name.or(url).unwrap_or("(url absente)").to_string()
}

fn distinct_relation_ids(relations: &[AdoAiContextRelation], kind: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for relation in relations {
        if relation.kind != kind {
            continue;
        }
        if let Some(id) = &relation.work_item_id
            && !ids.iter().any(|existing| existing == id)
        {
            ids.push(id.clone());
        }
    }
    ids
}

fn split_tags(tags: Option<String>) -> Vec<String> {
    tags.unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_product_context(fields: &Value) -> BTreeMap<String, String> {
    fields
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|(field_name, _)| is_context_field(field_name))
        .filter_map(|(field_name, value)| {
            let text = clean_text(element_text(Some(&value)))?;
            Some((friendly_field_name(&field_name), text))
        })
        .collect()
}

fn is_context_field(field_name: &str) -> bool {
    let normalized = field_name.replace(['.', '_', ' '], "").to_ascii_lowercase();
    normalized.contains("acceptance")
        || normalized.contains("productowner")
        || normalized.contains("product")
        || normalized.contains("businessvalue")
        || field_name.eq_ignore_ascii_case("Microsoft.VSTS.Common.AcceptanceCriteria")
}

fn friendly_field_name(field_name: &str) -> String {
    field_name
        .replace("System.", "")
        .replace("Microsoft.VSTS.Common.", "")
        .replace("Custom.", "")
}
