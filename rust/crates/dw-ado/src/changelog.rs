use crate::{
    AdoError, AdoToken, AzureDevOpsOptions, WorkItemSnapshot, get_related_work_item_ids,
    get_work_item_snapshot_authenticated, get_work_item_snapshots_authenticated,
    try_get_pull_request_work_item_ids, work_item_web_url,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RELATION_HIERARCHY_REVERSE: &str = "System.LinkTypes.Hierarchy-Reverse";
pub const RELATION_HIERARCHY_FORWARD: &str = "System.LinkTypes.Hierarchy-Forward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangelogFormat {
    Raw,
    Markdown,
    Html,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemGroup {
    pub parent: WorkItemSnapshot,
    pub items: Vec<WorkItemSnapshot>,
}

pub fn parse_changelog_format(format: Option<&str>) -> Result<ChangelogFormat, AdoError> {
    match format
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "raw" => Ok(ChangelogFormat::Raw),
        "markdown" => Ok(ChangelogFormat::Markdown),
        "html" => Ok(ChangelogFormat::Html),
        _ => Err(AdoError::InvalidInput(format!(
            "Format de changelog inconnu: {}",
            format.unwrap_or_default()
        ))),
    }
}

pub fn extract_work_item_ids_from_commit_messages(commit_log: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for (index, _) in commit_log.match_indices('#') {
        let id = commit_log[index + 1..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if !id.is_empty() && !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    ids
}

pub fn parse_id_set(source: &str) -> Vec<String> {
    source
        .split([',', ' ', '\n', '\t', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('#').to_string())
        .collect()
}

pub fn render_flat_changelog(
    items: &[WorkItemSnapshot],
    format: ChangelogFormat,
    options: &AzureDevOpsOptions,
    markdown_table: bool,
) -> String {
    match format {
        ChangelogFormat::Raw => items
            .iter()
            .map(render_raw_line)
            .collect::<Vec<_>>()
            .join("\n"),
        ChangelogFormat::Markdown if markdown_table => render_flat_markdown_table(items, options),
        ChangelogFormat::Markdown => render_flat_markdown(items, options),
        ChangelogFormat::Html => render_flat_html(items, options),
    }
}

pub fn render_grouped_changelog(
    groups: &[WorkItemGroup],
    format: ChangelogFormat,
    options: &AzureDevOpsOptions,
    markdown_table: bool,
) -> String {
    match format {
        ChangelogFormat::Raw => render_grouped_raw(groups),
        ChangelogFormat::Markdown if markdown_table => {
            render_grouped_markdown_table(groups, options)
        }
        ChangelogFormat::Markdown => render_grouped_markdown(groups, options),
        ChangelogFormat::Html => render_grouped_html(groups, options),
    }
}

pub fn group_work_items_by_parent(
    options: &AzureDevOpsOptions,
    items: &[WorkItemSnapshot],
    token: &AdoToken,
) -> Result<Vec<WorkItemGroup>, AdoError> {
    let mut groups: BTreeMap<String, Vec<WorkItemSnapshot>> = BTreeMap::new();
    let mut parents: BTreeMap<String, WorkItemSnapshot> = BTreeMap::new();

    for item in items {
        let parent_id =
            get_related_work_item_ids(options, &item.id, RELATION_HIERARCHY_REVERSE, token)?
                .into_iter()
                .next()
                .unwrap_or_else(|| item.id.clone());
        if parent_id == item.id {
            parents.insert(parent_id.clone(), item.clone());
        } else if !parents.contains_key(&parent_id) {
            parents.insert(
                parent_id.clone(),
                get_work_item_snapshot_authenticated(options, &parent_id, token)?,
            );
        }

        let children = groups.entry(parent_id.clone()).or_default();
        if parent_id != item.id {
            children.push(item.clone());
        }
    }

    Ok(groups
        .into_iter()
        .filter_map(|(parent_id, mut items)| {
            items.sort_by(|left, right| left.id.cmp(&right.id));
            Some(WorkItemGroup {
                parent: parents.remove(&parent_id)?,
                items,
            })
        })
        .collect())
}

pub fn get_work_item_ids_from_pull_requests(
    options: &AzureDevOpsOptions,
    repositories: &[String],
    source: &str,
    token: &AdoToken,
) -> Result<Vec<String>, AdoError> {
    if repositories.is_empty() {
        return Err(AdoError::InvalidInput(
            "Le mode PR requiert --repo, ou un --project avec des repositories AzureDevOpsRepository configurés.".into(),
        ));
    }

    let mut ids = Vec::new();
    for pull_request_id in parse_id_set(source) {
        let numeric_pull_request_id = pull_request_id.parse::<i64>().map_err(|_| {
            AdoError::InvalidInput(format!("ID de pull request invalide: {pull_request_id}"))
        })?;
        let mut matches = Vec::new();
        for repository in repositories {
            if let Some(work_item_ids) = try_get_pull_request_work_item_ids(
                options,
                repository,
                numeric_pull_request_id,
                token,
            )? {
                matches.push((repository.clone(), work_item_ids));
            }
        }

        match matches.len() {
            0 => {
                return Err(AdoError::Request(format!(
                    "Pull request #{pull_request_id} introuvable dans les repos Azure DevOps testes: {}",
                    repositories.join(", ")
                )));
            }
            1 => ids.extend(matches.remove(0).1),
            _ => {
                return Err(AdoError::InvalidInput(format!(
                    "Pull request #{pull_request_id} trouvee dans plusieurs repos ({}). Preciser --repo.",
                    matches
                        .into_iter()
                        .map(|item| item.0)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }
    }

    let mut seen = BTreeSet::new();
    Ok(ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect())
}

pub fn load_changelog_items(
    options: &AzureDevOpsOptions,
    work_item_ids: &[String],
    token: &AdoToken,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    get_work_item_snapshots_authenticated(options, work_item_ids, token)
}

fn render_grouped_raw(groups: &[WorkItemGroup]) -> String {
    groups
        .iter()
        .map(|group| {
            let mut lines = vec![render_raw_line(&group.parent)];
            lines.extend(
                group
                    .items
                    .iter()
                    .map(|item| format!("  - {}", render_raw_line(item))),
            );
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_flat_markdown(items: &[WorkItemSnapshot], options: &AzureDevOpsOptions) -> String {
    std::iter::once("# Changelog".to_string())
        .chain(std::iter::once(String::new()))
        .chain(
            items
                .iter()
                .map(|item| format!("- {}", render_markdown_line(item, options))),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_flat_markdown_table(items: &[WorkItemSnapshot], options: &AzureDevOpsOptions) -> String {
    let mut lines = vec![
        "# Changelog".into(),
        String::new(),
        "| Work Item | Type | Etat | Titre |".into(),
        "| --- | --- | --- | --- |".into(),
    ];
    lines.extend(items.iter().map(|item| {
        format!(
            "| {} | {} | {} | {} |",
            render_markdown_link(item, options),
            escape_markdown_table_cell(item.kind.as_deref()),
            escape_markdown_table_cell(item.state.as_deref()),
            escape_markdown_table_cell(item.title.as_deref())
        )
    }));
    lines.join("\n")
}

fn render_grouped_markdown(groups: &[WorkItemGroup], options: &AzureDevOpsOptions) -> String {
    let mut output = String::from("# Changelog\n\n");
    for (index, group) in groups.iter().enumerate() {
        output.push_str(&format!(
            "## {}\n",
            render_markdown_line(&group.parent, options)
        ));
        for item in &group.items {
            output.push_str(&format!("- {}\n", render_markdown_line(item, options)));
        }
        if index < groups.len() - 1 {
            output.push('\n');
        }
    }
    output.trim_end().to_string()
}

fn render_grouped_markdown_table(groups: &[WorkItemGroup], options: &AzureDevOpsOptions) -> String {
    let mut output = String::from("# Changelog\n\n");
    for (index, group) in groups.iter().enumerate() {
        output.push_str(&format!(
            "## {}\n\n",
            render_markdown_line(&group.parent, options)
        ));
        output.push_str("| Work Item | Type | Etat | Titre |\n");
        output.push_str("| --- | --- | --- | --- |\n");
        let rows = if group.items.is_empty() {
            vec![&group.parent]
        } else {
            group.items.iter().collect::<Vec<_>>()
        };
        for item in rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                render_markdown_link(item, options),
                escape_markdown_table_cell(item.kind.as_deref()),
                escape_markdown_table_cell(item.state.as_deref()),
                escape_markdown_table_cell(item.title.as_deref())
            ));
        }
        if index < groups.len() - 1 {
            output.push('\n');
        }
    }
    output.trim_end().to_string()
}

fn render_flat_html(items: &[WorkItemSnapshot], options: &AzureDevOpsOptions) -> String {
    let mut output = String::from("<h1>Changelog</h1>\n<ul>\n");
    for item in items {
        output.push_str(&format!("  <li>{}</li>\n", render_html_line(item, options)));
    }
    output.push_str("</ul>");
    output
}

fn render_grouped_html(groups: &[WorkItemGroup], options: &AzureDevOpsOptions) -> String {
    let mut output = String::from("<h1>Changelog</h1>\n");
    for group in groups {
        output.push_str(&format!(
            "<h2>{}</h2>\n",
            render_html_line(&group.parent, options)
        ));
        if group.items.is_empty() {
            continue;
        }
        output.push_str("<ul>\n");
        for item in &group.items {
            output.push_str(&format!("  <li>{}</li>\n", render_html_line(item, options)));
        }
        output.push_str("</ul>\n");
    }
    output.trim_end().to_string()
}

fn render_raw_line(item: &WorkItemSnapshot) -> String {
    let mut line = format!("#{}", item.id);
    if let Some(kind) = item
        .kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" [{kind}]"));
    }
    if let Some(state) = item
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" {state}"));
    }
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" - {title}"));
    }
    line
}

fn render_markdown_line(item: &WorkItemSnapshot, options: &AzureDevOpsOptions) -> String {
    let mut line = render_markdown_link(item, options);
    if let Some(kind) = item
        .kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" [{kind}]"));
    }
    if let Some(state) = item
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" {state}"));
    }
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" - {title}"));
    }
    line
}

fn render_markdown_link(item: &WorkItemSnapshot, options: &AzureDevOpsOptions) -> String {
    format!("[#{}]({})", item.id, work_item_web_url(options, &item.id))
}

fn render_html_line(item: &WorkItemSnapshot, options: &AzureDevOpsOptions) -> String {
    let mut line = format!(
        "<a href=\"{}\">#{}</a>",
        html_escape(&work_item_web_url(options, &item.id)),
        html_escape(&item.id)
    );
    if let Some(kind) = item
        .kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" [{}]", html_escape(kind)));
    }
    if let Some(state) = item
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" {}", html_escape(state)));
    }
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        line.push_str(&format!(" - {}", html_escape(title)));
    }
    line
}

fn escape_markdown_table_cell(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .replace('|', "\\|")
        .replace("\r\n", "<br />")
        .replace('\n', "<br />")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_api_version;

    fn options() -> AzureDevOpsOptions {
        AzureDevOpsOptions {
            organization: "https://dev.azure.com/digital-factory-ogf".into(),
            project: "HOMMAGE AGENCE".into(),
            api_version: default_api_version(),
        }
    }

    #[test]
    fn extract_work_item_ids_from_commit_messages_reads_ids_in_order_and_dedupes() {
        let commit_log =
            "fix(#53115 #53312): corriger le calcul\u{1e}refactor(#53115 #54000): simplifier";

        let ids = extract_work_item_ids_from_commit_messages(commit_log);

        assert_eq!(ids, vec!["53115", "53312", "54000"]);
        assert_eq!(ids.join(" "), "53115 53312 54000");
    }

    #[test]
    fn render_flat_changelog_markdown_adds_links_on_work_item_numbers() {
        let items = vec![WorkItemSnapshot {
            id: "53115".into(),
            kind: Some("Bug".into()),
            state: Some("En développement".into()),
            title: Some("Corriger le calcul".into()),
            url: None,
        }];

        let markdown = render_flat_changelog(&items, ChangelogFormat::Markdown, &options(), false);

        assert!(markdown.contains(
            "[#53115](https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115)"
        ));
        assert!(markdown.contains("[Bug] En développement - Corriger le calcul"));
    }

    #[test]
    fn render_flat_changelog_markdown_table_renders_columns_and_links() {
        let items = vec![WorkItemSnapshot {
            id: "53115".into(),
            kind: Some("Bug".into()),
            state: Some("En développement".into()),
            title: Some("Corriger le calcul".into()),
            url: None,
        }];

        let markdown = render_flat_changelog(&items, ChangelogFormat::Markdown, &options(), true);

        assert!(markdown.contains("| Work Item | Type | Etat | Titre |"));
        assert!(markdown.contains("| [#53115](https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115) | Bug | En développement | Corriger le calcul |"));
    }

    #[test]
    fn render_grouped_changelog_html_adds_links_on_parent_and_children() {
        let groups = vec![WorkItemGroup {
            parent: WorkItemSnapshot {
                id: "53115".into(),
                kind: Some("User Story".into()),
                state: Some("En réalisation".into()),
                title: Some("Parent".into()),
                url: None,
            },
            items: vec![WorkItemSnapshot {
                id: "53312".into(),
                kind: Some("Task".into()),
                state: Some("En développement".into()),
                title: Some("Enfant".into()),
                url: None,
            }],
        }];

        let html = render_grouped_changelog(&groups, ChangelogFormat::Html, &options(), false);

        assert!(html.contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115\">#53115</a>"));
        assert!(html.contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53312\">#53312</a>"));
        assert!(html.contains("[Task] En développement - Enfant"));
    }
}
