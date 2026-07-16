use dw_ado_commands::commands::changelog::{
    ChangelogOutputFormat, ChangelogReport, ChangelogSection,
};

pub fn render_ado_changelog_document(report: &ChangelogReport) -> String {
    report
        .sections
        .iter()
        .map(|section| render_changelog_section(report, section))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_changelog_section(report: &ChangelogReport, section: &ChangelogSection) -> String {
    match report.format {
        ChangelogOutputFormat::Raw => render_raw_changelog(report, section),
        ChangelogOutputFormat::Markdown => render_markdown_changelog(report, section),
        ChangelogOutputFormat::Html => render_html_changelog(report, section),
    }
}

fn render_raw_changelog(report: &ChangelogReport, section: &ChangelogSection) -> String {
    let mut blocks = Vec::new();
    if let Some(repository) = &section.repository {
        blocks.push(format!("Changelog ({repository})"));
    }
    blocks.extend(
        section
            .warnings
            .iter()
            .map(|warning| format!("Warning: {}", warning.detail)),
    );
    if let Some(status) = section_status(report, section) {
        blocks.push(status.into());
    } else {
        let content = if report.group_by_parent && !section.groups.is_empty() {
            render_grouped_raw_changelog(section)
        } else {
            section
                .items
                .iter()
                .map(render_raw_changelog_item)
                .collect::<Vec<_>>()
                .join("\n")
        };
        if !content.is_empty() {
            blocks.push(content);
        }
    }
    blocks.join("\n\n")
}

fn render_grouped_raw_changelog(section: &ChangelogSection) -> String {
    section
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

fn render_markdown_changelog(report: &ChangelogReport, section: &ChangelogSection) -> String {
    let mut blocks = vec![format!("# {}", changelog_title(section))];
    blocks.extend(
        section
            .warnings
            .iter()
            .map(|warning| format!("> **Warning:** {}", warning.detail)),
    );
    if let Some(status) = section_status(report, section) {
        blocks.push(format!("> {status}"));
    } else {
        let content = if report.group_by_parent && !section.groups.is_empty() {
            render_grouped_markdown_changelog(report, section)
        } else if report.table {
            render_flat_markdown_changelog_table(report, section)
        } else {
            section
                .items
                .iter()
                .map(|item| format!("- {}", render_markdown_changelog_line(item, report)))
                .collect::<Vec<_>>()
                .join("\n")
        };
        if !content.is_empty() {
            blocks.push(content);
        }
    }
    blocks.join("\n\n")
}

fn render_flat_markdown_changelog_table(
    report: &ChangelogReport,
    section: &ChangelogSection,
) -> String {
    let mut lines = vec![
        "| Work Item | Type | State | Title |".into(),
        "| --- | --- | --- | --- |".into(),
    ];
    lines.extend(
        section
            .items
            .iter()
            .map(|item| render_markdown_table_row(item, report)),
    );
    lines.join("\n")
}

fn render_grouped_markdown_changelog(
    report: &ChangelogReport,
    section: &ChangelogSection,
) -> String {
    section
        .groups
        .iter()
        .map(|group| {
            let mut blocks = vec![format!(
                "## {}",
                render_markdown_changelog_line(&group.parent, report)
            )];
            if report.table {
                let mut lines = vec![
                    "| Work Item | Type | State | Title |".into(),
                    "| --- | --- | --- | --- |".into(),
                ];
                let rows = if group.items.is_empty() {
                    vec![&group.parent]
                } else {
                    group.items.iter().collect::<Vec<_>>()
                };
                lines.extend(
                    rows.into_iter()
                        .map(|item| render_markdown_table_row(item, report)),
                );
                blocks.push(lines.join("\n"));
            } else {
                blocks.push(
                    group
                        .items
                        .iter()
                        .map(|item| format!("- {}", render_markdown_changelog_line(item, report)))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
            blocks
                .into_iter()
                .filter(|block| !block.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_markdown_table_row(item: &dw_ado::WorkItemSnapshot, report: &ChangelogReport) -> String {
    format!(
        "| {} | {} | {} | {} |",
        render_markdown_changelog_link(item, report),
        escape_markdown_table_cell(item.kind.as_deref()),
        escape_markdown_table_cell(item.state.as_deref()),
        escape_markdown_table_cell(item.title.as_deref())
    )
}

fn render_html_changelog(report: &ChangelogReport, section: &ChangelogSection) -> String {
    let mut blocks = vec![format!(
        "<h1>{}</h1>",
        html_escape(&changelog_title(section))
    )];
    blocks.extend(section.warnings.iter().map(|warning| {
        format!(
            "<p><strong>Warning:</strong> {}</p>",
            html_escape(&warning.detail)
        )
    }));
    if let Some(status) = section_status(report, section) {
        blocks.push(format!("<p>{}</p>", html_escape(status)));
    } else {
        let content = if report.group_by_parent && !section.groups.is_empty() {
            render_grouped_html_changelog(report, section)
        } else if report.table {
            render_html_table(&section.items, report)
        } else {
            render_html_list(&section.items, report)
        };
        if !content.is_empty() {
            blocks.push(content);
        }
    }
    blocks.join("\n")
}

fn render_grouped_html_changelog(report: &ChangelogReport, section: &ChangelogSection) -> String {
    section
        .groups
        .iter()
        .map(|group| {
            let mut blocks = vec![format!(
                "<h2>{}</h2>",
                render_html_changelog_line(&group.parent, report)
            )];
            if report.table {
                let rows = if group.items.is_empty() {
                    vec![group.parent.clone()]
                } else {
                    group.items.clone()
                };
                blocks.push(render_html_table(&rows, report));
            } else if !group.items.is_empty() {
                blocks.push(render_html_list(&group.items, report));
            }
            blocks.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_html_list(items: &[dw_ado::WorkItemSnapshot], report: &ChangelogReport) -> String {
    let mut output = String::from("<ul>\n");
    for item in items {
        output.push_str(&format!(
            "  <li>{}</li>\n",
            render_html_changelog_line(item, report)
        ));
    }
    output.push_str("</ul>");
    output
}

fn render_html_table(items: &[dw_ado::WorkItemSnapshot], report: &ChangelogReport) -> String {
    let mut output = String::from(
        "<table>\n  <thead>\n    <tr><th>Work Item</th><th>Type</th><th>State</th><th>Title</th></tr>\n  </thead>\n  <tbody>\n",
    );
    for item in items {
        output.push_str(&format!(
            "    <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            render_html_changelog_link(item, report),
            html_escape(item.kind.as_deref().unwrap_or_default()),
            html_escape(item.state.as_deref().unwrap_or_default()),
            html_escape(item.title.as_deref().unwrap_or_default())
        ));
    }
    output.push_str("  </tbody>\n</table>");
    output
}

fn changelog_title(section: &ChangelogSection) -> String {
    section
        .repository
        .as_ref()
        .map(|repository| format!("Changelog ({repository})"))
        .unwrap_or_else(|| "Changelog".into())
}

fn section_status<'a>(report: &ChangelogReport, section: &'a ChangelogSection) -> Option<&'a str> {
    if section.source_empty {
        Some(if report.from_git {
            "No work item detected in git range commit messages."
        } else if report.from_pr {
            "No work item detected for the given pull requests."
        } else {
            "No work item provided."
        })
    } else if section.resolved_empty {
        Some("No work item resolved in Azure DevOps.")
    } else {
        None
    }
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
    let mut line = render_html_changelog_link(item, report);
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

fn render_html_changelog_link(item: &dw_ado::WorkItemSnapshot, report: &ChangelogReport) -> String {
    format!(
        "<a href=\"{}\">#{}</a>",
        html_escape(&dw_ado::work_item_web_url(
            &report.options,
            item.id.as_str()
        )),
        html_escape(item.id.as_str())
    )
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
        let mut report = base_report(ChangelogOutputFormat::Markdown, true, true);
        report.sections[0].groups = vec![
            dw_ado::WorkItemGroup {
                parent: item("100", "Feature", "Actif", "Parent"),
                items: vec![item("42", "Bug", "Actif", "Child")],
            },
            dw_ado::WorkItemGroup {
                parent: item("200", "Feature", "Nouveau", "Empty parent"),
                items: Vec::new(),
            },
        ];

        let output = render_ado_changelog_document(&report);

        assert!(output.contains(
            "## [#100](https://dev.azure.com/acme/Acme/_workitems/edit/100) [Feature] Actif - Parent"
        ));
        assert!(output.contains(
            "| [#42](https://dev.azure.com/acme/Acme/_workitems/edit/42) | Bug | Actif | Child |"
        ));
        assert!(output.contains("| [#200](https://dev.azure.com/acme/Acme/_workitems/edit/200) | Feature | Nouveau | Empty parent |"));
    }

    #[test]
    fn renders_repository_sections_warnings_and_html_tables() {
        let mut report = base_report(ChangelogOutputFormat::Html, true, false);
        report.from_pr = false;
        report.from_git = true;
        report.sections[0].repository = Some("front".into());
        report.sections.push(ChangelogSection {
            repository: Some("back".into()),
            repository_path: Some("/tmp/back.git".into()),
            work_item_ids: Vec::new(),
            items: Vec::new(),
            groups: Vec::new(),
            source_empty: false,
            resolved_empty: false,
            warnings: vec![dw_ado_commands::commands::changelog::ChangelogWarning {
                detail: "Missing <ref>".into(),
            }],
        });

        let output = render_ado_changelog_document(&report);

        assert!(output.contains("<h1>Changelog (front)</h1>"));
        assert!(output.contains("<table>"));
        assert!(output.contains("<h1>Changelog (back)</h1>"));
        assert!(output.contains("<strong>Warning:</strong> Missing &lt;ref&gt;"));

        report.format = ChangelogOutputFormat::Raw;
        report.table = false;
        let raw = render_ado_changelog_document(&report);
        assert!(raw.contains("Changelog (front)"));
        assert!(raw.contains("Changelog (back)"));
        assert!(raw.contains("Warning: Missing <ref>"));
    }

    fn base_report(
        format: ChangelogOutputFormat,
        table: bool,
        group_by_parent: bool,
    ) -> ChangelogReport {
        let items = vec![
            item("42", "Bug", "Actif", "Corriger la sortie"),
            item("43", "Task", "Prêt", "Tester \\| table"),
        ];
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
            sections: vec![ChangelogSection {
                repository: None,
                repository_path: None,
                work_item_ids: Vec::new(),
                items,
                groups: Vec::new(),
                source_empty: false,
                resolved_empty: false,
                warnings: Vec::new(),
            }],
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
            project: "Acme".into(),
            api_version: "7.1".into(),
        }
    }
}
