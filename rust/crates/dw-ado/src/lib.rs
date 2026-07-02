use dw_contracts::{
    AI_CONTEXT_VERSION, ATTACHMENT_DIRECTORY_PREFIX, AdoAiContextAttachment,
    AdoAiContextAttachments, AdoAiContextComment, AdoAiContextContent, AdoAiContextCore,
    AdoAiContextItem, AdoAiContextLinks, AdoAiContextRelation, AdoAiContextWorkItem,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

pub const DEFAULT_API_VERSION: &str = "7.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AzureDevOpsOptions {
    pub organization: String,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    EnvironmentPat,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    pub source: AuthSource,
    pub variable_name: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum AdoError {
    #[error("Azure DevOps auth indisponible. Definir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT.")]
    MissingAuth,
}

pub fn detect_env_auth() -> AuthStatus {
    if std::env::var("DW_ADO_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
    {
        return AuthStatus {
            source: AuthSource::EnvironmentPat,
            variable_name: Some("DW_ADO_TOKEN"),
        };
    }

    if std::env::var("AZURE_DEVOPS_EXT_PAT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
    {
        return AuthStatus {
            source: AuthSource::EnvironmentPat,
            variable_name: Some("AZURE_DEVOPS_EXT_PAT"),
        };
    }

    AuthStatus {
        source: AuthSource::Missing,
        variable_name: None,
    }
}

pub fn expanded_work_item_url(options: &AzureDevOpsOptions, work_item_id: &str) -> String {
    format!(
        "{}/{}/_apis/wit/workitems/{}?$expand=all&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id,
        DEFAULT_API_VERSION
    )
}

pub fn work_item_comments_url(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    top: u32,
) -> String {
    format!(
        "{}/{}/_apis/wit/workItems/{}/comments?$top={}&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id,
        top,
        DEFAULT_API_VERSION
    )
}

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
            url: Some(format!(
                "{}/{}/_workitems/edit/{}",
                azure_dev_ops.organization.trim_end_matches('/'),
                azure_dev_ops.project,
                id
            )),
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

fn field_text(fields: &Value, name: &str) -> Option<String> {
    fields.get(name).and_then(|value| element_text(Some(value)))
}

fn identity_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    value
        .get("displayName")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| element_text(Some(value)))
}

fn element_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Object(object) => object
            .get("displayName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        Value::Null => None,
        other => Some(other.to_string()),
    }
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

fn work_item_id_from_relation_url(url: &str) -> Option<String> {
    let marker = "/workItems/";
    let index = url.find(marker)?;
    let id = &url[index + marker.len()..];
    Some(id.split(['/', '?']).next()?.to_string())
}

fn clean_text(value: Option<String>) -> Option<String> {
    let value = value?;
    let mut in_tag = false;
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let trimmed = out.replace("&nbsp;", " ").trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_url_matches_dotnet_shape() {
        let options = AzureDevOpsOptions {
            organization: "https://dev.azure.com/org".into(),
            project: "Project X".into(),
        };

        assert_eq!(
            expanded_work_item_url(&options, "12345"),
            "https://dev.azure.com/org/Project X/_apis/wit/workitems/12345?$expand=all&api-version=7.1"
        );
    }

    #[test]
    fn map_ai_context_item_matches_expected_contract_shape() {
        let root: Value = serde_json::from_str(
            r#"{
  "id": 55201,
  "fields": {
    "System.Title": "Demande transport SOMOTHA",
    "System.WorkItemType": "User Story",
    "System.State": "En realisation",
    "System.AssignedTo": { "displayName": "Alice Martin" },
    "System.AreaPath": "HA\\Transport",
    "System.IterationPath": "HA\\Sprint 42",
    "System.Tags": "transport; somotha",
    "System.Description": "<p>Verifier la maquette</p>",
    "System.CreatedBy": { "displayName": "Bob" },
    "System.CreatedDate": "2026-07-01T10:00:00Z",
    "System.ChangedBy": { "displayName": "Claire" },
    "System.ChangedDate": "2026-07-02T09:00:00Z",
    "Microsoft.VSTS.Common.Priority": 1,
    "Microsoft.VSTS.Common.ValueArea": "Business",
    "Microsoft.VSTS.Common.AcceptanceCriteria": "<div>Respecter le libelle SOMOTHA</div>",
    "Custom.ProductContext": "<div>Ecran existant</div>"
  },
  "relations": [
    { "rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/54000" },
    { "rel": "System.LinkTypes.Hierarchy-Forward", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/55202" },
    { "rel": "System.LinkTypes.Dependency-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/55199" },
    { "rel": "AttachedFile", "url": "https://dev.azure.com/org/_apis/wit/attachments/123", "attributes": { "name": "maquette transport somotha.png", "comment": "<p>Source ecran</p>" } }
  ]
}"#,
        )
        .expect("json should parse");

        let context = map_ai_context_item(
            &root,
            &AzureDevOpsOptions {
                organization: "https://dev.azure.com/org".into(),
                project: "Project".into(),
            },
            false,
            vec![AdoAiContextComment {
                author: Some("Alice Martin".into()),
                created_date: Some("2026-07-02T08:00:00Z".into()),
                text: Some("Verifier le screenshot".into()),
            }],
        );

        assert_eq!(context.schema_version, AI_CONTEXT_VERSION);
        assert_eq!(context.work_item.id, "55201");
        assert_eq!(
            context.work_item.title.as_deref(),
            Some("Demande transport SOMOTHA")
        );
        assert_eq!(context.work_item.tags, vec!["transport", "somotha"]);
        assert_eq!(
            context.content.description.as_deref(),
            Some("Verifier la maquette")
        );
        assert_eq!(
            context.content.acceptance_criteria.as_deref(),
            Some("Respecter le libelle SOMOTHA")
        );
        assert_eq!(
            context
                .content
                .product_context
                .get("ProductContext")
                .map(String::as_str),
            Some("Ecran existant")
        );
        assert_eq!(context.links.parent_ids, vec!["54000"]);
        assert_eq!(context.links.child_ids, vec!["55202"]);
        assert_eq!(context.links.predecessor_ids, vec!["55199"]);
        assert_eq!(context.attachments.directory_hint, "attachments/ado/55201");
        assert_eq!(
            context.attachments.items[0].name.as_deref(),
            Some("maquette transport somotha.png")
        );
        assert_eq!(
            context.attachments.items[0].comment.as_deref(),
            Some("Source ecran")
        );
        assert_eq!(
            context.comments[0].text.as_deref(),
            Some("Verifier le screenshot")
        );
        assert!(
            context
                .relations
                .iter()
                .any(|relation| relation.kind == "attachment")
        );
    }

    #[test]
    fn map_ai_context_summary_mode_keeps_links_and_hides_relations() {
        let root: Value = serde_json::from_str(
            r#"{
  "id": 55201,
  "fields": {
    "System.Title": "Titre",
    "System.WorkItemType": "Task",
    "System.State": "New"
  },
  "relations": [
    { "rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/org/Project/_apis/wit/workItems/54000" },
    { "rel": "AttachedFile", "url": "https://dev.azure.com/org/_apis/wit/attachments/123", "attributes": { "name": "mockup.png" } }
  ]
}"#,
        )
        .expect("json should parse");

        let context = map_ai_context_item(
            &root,
            &AzureDevOpsOptions {
                organization: "https://dev.azure.com/org".into(),
                project: "Project".into(),
            },
            true,
            vec![],
        );

        assert_eq!(context.links.parent_ids, vec!["54000"]);
        assert_eq!(context.attachments.items.len(), 1);
        assert!(context.relations.is_empty());
    }
}
