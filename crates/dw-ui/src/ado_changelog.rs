use dw_ado_commands::commands::changelog::{ChangelogOutputFormat, ChangelogReport};

pub fn render_ado_changelog_document(report: &ChangelogReport) -> String {
    if report.group_by_parent {
        render_grouped_changelog(report)
    } else {
        render_flat_changelog(report)
    }
}

fn render_flat_changelog(report: &ChangelogReport) -> String {
    match report.format {
        ChangelogOutputFormat::Raw => report
            .items
            .iter()
            .map(render_raw_changelog_item)
            .collect::<Vec<_>>()
            .join("\n"),
        ChangelogOutputFormat::Markdown if report.table => {
            render_flat_markdown_changelog_table(report)
        }
        ChangelogOutputFormat::Markdown => render_flat_markdown_changelog(report),
        ChangelogOutputFormat::Html => render_flat_html_changelog(report),
    }
}

fn render_grouped_changelog(report: &ChangelogReport) -> String {
    match report.format {
        ChangelogOutputFormat::Raw => render_grouped_raw_changelog(report),
        ChangelogOutputFormat::Markdown if report.table => {
            render_grouped_markdown_changelog_table(report)
        }
        ChangelogOutputFormat::Markdown => render_grouped_markdown_changelog(report),
        ChangelogOutputFormat::Html => render_grouped_html_changelog(report),
    }
}

fn render_grouped_raw_changelog(report: &ChangelogReport) -> String {
    report
        .groups
        .iter()
        .map(|group| {
            let mut lines = vec![render_raw_changelog_item(&group.parent)];
            lines.extend(
                group
                    .items
                    .iter()
                    .map(|item| format!("  - {}", render_raw_changelog_item(item))),
            );
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_flat_markdown_changelog(report: &ChangelogReport) -> String {
    std::iter::once("# Changelog".to_string())
        .chain(std::iter::once(String::new()))
        .chain(
            report
                .items
                .iter()
                .map(|item| format!("- {}", render_markdown_changelog_line(item, report))),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_flat_markdown_changelog_table(report: &ChangelogReport) -> String {
    let mut lines = vec![
        "# Changelog".into(),
        String::new(),
        "| Work Item | Type | State | Title |".into(),
        "| --- | --- | --- | --- |".into(),
    ];
    lines.extend(report.items.iter().map(|item| {
        format!(
            "| {} | {} | {} | {} |",
            render_markdown_changelog_link(item, report),
            escape_markdown_table_cell(item.kind.as_deref()),
            escape_markdown_table_cell(item.state.as_deref()),
            escape_markdown_table_cell(item.title.as_deref())
        )
    }));
    lines.join("\n")
}

fn render_grouped_markdown_changelog(report: &ChangelogReport) -> String {
    let mut output = String::from("# Changelog\n\n");
    for (index, group) in report.groups.iter().enumerate() {
        output.push_str(&format!(
            "## {}\n",
            render_markdown_changelog_line(&group.parent, report)
        ));
        for item in &group.items {
            output.push_str(&format!(
                "- {}\n",
                render_markdown_changelog_line(item, report)
            ));
        }
        if index < report.groups.len() - 1 {
            output.push('\n');
        }
    }
    output.trim_end().to_string()
}

fn render_grouped_markdown_changelog_table(report: &ChangelogReport) -> String {
    let mut output = String::from("# Changelog\n\n");
    for (index, group) in report.groups.iter().enumerate() {
        output.push_str(&format!(
            "## {}\n\n",
            render_markdown_changelog_line(&group.parent, report)
        ));
        output.push_str("| Work Item | Type | State | Title |\n");
        output.push_str("| --- | --- | --- | --- |\n");
        let rows = if group.items.is_empty() {
            vec![&group.parent]
        } else {
            group.items.iter().collect::<Vec<_>>()
        };
        for item in rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                render_markdown_changelog_link(item, report),
                escape_markdown_table_cell(item.kind.as_deref()),
                escape_markdown_table_cell(item.state.as_deref()),
                escape_markdown_table_cell(item.title.as_deref())
            ));
        }
        if index < report.groups.len() - 1 {
            output.push('\n');
        }
    }
    output.trim_end().to_string()
}

fn render_flat_html_changelog(report: &ChangelogReport) -> String {
    let mut output = String::from("<h1>Changelog</h1>\n<ul>\n");
    for item in &report.items {
        output.push_str(&format!(
            "  <li>{}</li>\n",
            render_html_changelog_line(item, report)
        ));
    }
    output.push_str("</ul>");
    output
}

fn render_grouped_html_changelog(report: &ChangelogReport) -> String {
    let mut output = String::from("<h1>Changelog</h1>\n");
    for group in &report.groups {
        output.push_str(&format!(
            "<h2>{}</h2>\n",
            render_html_changelog_line(&group.parent, report)
        ));
        if group.items.is_empty() {
            continue;
        }
        output.push_str("<ul>\n");
        for item in &group.items {
            output.push_str(&format!(
                "  <li>{}</li>\n",
                render_html_changelog_line(item, report)
            ));
        }
        output.push_str("</ul>\n");
    }
    output.trim_end().to_string()
}

fn render_raw_changelog_item(item: &dw_ado::WorkItemSnapshot) -> String {
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

fn render_markdown_changelog_line(
    item: &dw_ado::WorkItemSnapshot,
    report: &ChangelogReport,
) -> String {
    let mut line = render_markdown_changelog_link(item, report);
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

fn render_markdown_changelog_link(
    item: &dw_ado::WorkItemSnapshot,
    report: &ChangelogReport,
) -> String {
    format!(
        "[#{}]({})",
        item.id,
        dw_ado::work_item_web_url(&report.options, item.id.as_str())
    )
}

fn render_html_changelog_line(item: &dw_ado::WorkItemSnapshot, report: &ChangelogReport) -> String {
    let mut line = format!(
        "<a href=\"{}\">#{}</a>",
        html_escape(&dw_ado::work_item_web_url(
            &report.options,
            item.id.as_str()
        )),
        html_escape(item.id.as_str())
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

    #[test]
    fn renders_raw_markdown_table_and_html_documents() {
        let report = base_report(ChangelogOutputFormat::Raw, false, false);

        assert_eq!(
            render_ado_changelog_document(&report),
            "#42 [Bug] Actif - Corriger la sortie\n#43 [Task] Prêt - Tester \\| table"
        );

        let markdown = ChangelogReport {
            format: ChangelogOutputFormat::Markdown,
            ..report.clone()
        };
        assert_eq!(
            render_ado_changelog_document(&markdown),
            "# Changelog\n\n- [#42](https://dev.azure.com/acme/Acme/_workitems/edit/42) [Bug] Actif - Corriger la sortie\n- [#43](https://dev.azure.com/acme/Acme/_workitems/edit/43) [Task] Prêt - Tester \\| table"
        );

        let table = ChangelogReport {
            format: ChangelogOutputFormat::Markdown,
            table: true,
            ..report.clone()
        };
        assert_eq!(
            render_ado_changelog_document(&table),
            "# Changelog\n\n| Work Item | Type | State | Title |\n| --- | --- | --- | --- |\n| [#42](https://dev.azure.com/acme/Acme/_workitems/edit/42) | Bug | Actif | Corriger la sortie |\n| [#43](https://dev.azure.com/acme/Acme/_workitems/edit/43) | Task | Prêt | Tester \\\\| table |"
        );

        let html = ChangelogReport {
            format: ChangelogOutputFormat::Html,
            ..report
        };
        assert_eq!(
            render_ado_changelog_document(&html),
            "<h1>Changelog</h1>\n<ul>\n  <li><a href=\"https://dev.azure.com/acme/Acme/_workitems/edit/42\">#42</a> [Bug] Actif - Corriger la sortie</li>\n  <li><a href=\"https://dev.azure.com/acme/Acme/_workitems/edit/43\">#43</a> [Task] Prêt - Tester \\| table</li>\n</ul>"
        );
    }

    #[test]
    fn renders_grouped_documents_with_parent_fallback_rows() {
        let report = ChangelogReport {
            group_by_parent: true,
            groups: vec![
                dw_ado::WorkItemGroup {
                    parent: item("100", "Feature", "Actif", "Parent"),
                    items: vec![item("42", "Bug", "Actif", "Child")],
                },
                dw_ado::WorkItemGroup {
                    parent: item("200", "Feature", "Nouveau", "Empty parent"),
                    items: Vec::new(),
                },
            ],
            ..base_report(ChangelogOutputFormat::Markdown, true, false)
        };

        let output = render_ado_changelog_document(&report);

        assert!(output.contains(
            "## [#100](https://dev.azure.com/acme/Acme/_workitems/edit/100) [Feature] Actif - Parent"
        ));
        assert!(output.contains(
            "| [#42](https://dev.azure.com/acme/Acme/_workitems/edit/42) | Bug | Actif | Child |"
        ));
        assert!(output.contains("| [#200](https://dev.azure.com/acme/Acme/_workitems/edit/200) | Feature | Nouveau | Empty parent |"));
    }

    fn base_report(
        format: ChangelogOutputFormat,
        table: bool,
        group_by_parent: bool,
    ) -> ChangelogReport {
        ChangelogReport {
            root: "/tmp/dw".into(),
            project: "acme".into(),
            from_pr: true,
            from_git: false,
            group_by_parent,
            format,
            table,
            options: ado_options(),
            ids_only: false,
            work_item_ids: Vec::new(),
            items: vec![
                item("42", "Bug", "Actif", "Corriger la sortie"),
                item("43", "Task", "Prêt", "Tester \\| table"),
            ],
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: false,
            events: Vec::new(),
        }
    }

    fn item(id: &str, kind: &str, state: &str, title: &str) -> dw_ado::WorkItemSnapshot {
        dw_ado::WorkItemSnapshot {
            id: id.into(),
            kind: Some(kind.into()),
            state: Some(state.into()),
            title: Some(title.into()),
            url: None,
        }
    }

    fn ado_options() -> dw_ado::AzureDevOpsOptions {
        dw_ado::AzureDevOpsOptions {
            organization: "https://dev.azure.com/acme".into(),
            project: "acme".into(),
            api_version: "7.1".into(),
        }
    }
}
