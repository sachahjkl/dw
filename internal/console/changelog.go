package console

import (
	"html"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

type ChangelogFormat uint8

const (
	ChangelogRaw ChangelogFormat = iota
	ChangelogMarkdown
	ChangelogHTML
)

type ChangelogItem struct {
	ID    string
	URL   string
	Type  string
	State string
	Title string
}

type ChangelogWarning struct{ Detail string }

type ChangelogGroup struct {
	Parent ChangelogItem
	Items  []ChangelogItem
}

type ChangelogSection struct {
	Repository    string
	Warnings      []ChangelogWarning
	Items         []ChangelogItem
	Groups        []ChangelogGroup
	SourceEmpty   bool
	ResolvedEmpty bool
}

type ChangelogReport struct {
	Format        ChangelogFormat
	GroupByParent bool
	Table         bool
	FromGit       bool
	FromPR        bool
	IDsOnly       bool
	WorkItemIDs   []string
	Sections      []ChangelogSection
}

func RenderChangelog(report ChangelogReport, localizer Localizer) string {
	localizer = WithConsoleMessages(localizer)
	if report.IDsOnly {
		return strings.Join(report.WorkItemIDs, " ")
	}
	sections := make([]string, len(report.Sections))
	for i := range report.Sections {
		switch report.Format {
		case ChangelogMarkdown:
			sections[i] = renderChangelogMarkdown(report, report.Sections[i], localizer)
		case ChangelogHTML:
			sections[i] = renderChangelogHTML(report, report.Sections[i], localizer)
		default:
			sections[i] = renderChangelogRaw(report, report.Sections[i], localizer)
		}
	}
	return strings.Join(sections, "\n\n")
}

func renderChangelogRaw(report ChangelogReport, section ChangelogSection, localizer Localizer) string {
	blocks := make([]string, 0, 2+len(section.Warnings))
	if section.Repository != "" {
		blocks = append(blocks, localize(localizer, "changelog.title.repository", l10n.A("repository", section.Repository)))
	}
	for _, warning := range section.Warnings {
		blocks = append(blocks, localize(localizer, "changelog.warning", l10n.A("detail", warning.Detail)))
	}
	if status := changelogStatus(report, section, localizer); status != "" {
		blocks = append(blocks, status)
	} else if report.GroupByParent && len(section.Groups) != 0 {
		groups := make([]string, len(section.Groups))
		for i, group := range section.Groups {
			lines := []string{rawChangelogItem(group.Parent)}
			for _, item := range group.Items {
				lines = append(lines, "  - "+rawChangelogItem(item))
			}
			groups[i] = strings.Join(lines, "\n")
		}
		blocks = append(blocks, strings.Join(groups, "\n\n"))
	} else if len(section.Items) != 0 {
		lines := make([]string, len(section.Items))
		for i, item := range section.Items {
			lines[i] = rawChangelogItem(item)
		}
		blocks = append(blocks, strings.Join(lines, "\n"))
	}
	return strings.Join(blocks, "\n\n")
}

func renderChangelogMarkdown(report ChangelogReport, section ChangelogSection, localizer Localizer) string {
	blocks := []string{"# " + changelogTitle(section, localizer)}
	for _, warning := range section.Warnings {
		blocks = append(blocks, "> **"+localizer.Text("changelog.warning.label")+":** "+warning.Detail)
	}
	if status := changelogStatus(report, section, localizer); status != "" {
		blocks = append(blocks, "> "+status)
	} else if report.GroupByParent && len(section.Groups) != 0 {
		groups := make([]string, len(section.Groups))
		for i, group := range section.Groups {
			groups[i] = markdownChangelogGroup(report, group, localizer)
		}
		blocks = append(blocks, strings.Join(groups, "\n\n"))
	} else if report.Table {
		blocks = append(blocks, markdownChangelogTable(section.Items, report, localizer))
	} else if len(section.Items) != 0 {
		lines := make([]string, len(section.Items))
		for i, item := range section.Items {
			lines[i] = "- " + markdownChangelogLine(item)
		}
		blocks = append(blocks, strings.Join(lines, "\n"))
	}
	return strings.Join(blocks, "\n\n")
}

func markdownChangelogGroup(report ChangelogReport, group ChangelogGroup, localizer Localizer) string {
	blocks := []string{"## " + markdownChangelogLine(group.Parent)}
	if report.Table {
		items := group.Items
		if len(items) == 0 {
			items = []ChangelogItem{group.Parent}
		}
		blocks = append(blocks, markdownChangelogTable(items, report, localizer))
	} else if len(group.Items) != 0 {
		lines := make([]string, len(group.Items))
		for i, item := range group.Items {
			lines[i] = "- " + markdownChangelogLine(item)
		}
		blocks = append(blocks, strings.Join(lines, "\n"))
	}
	return strings.Join(blocks, "\n\n")
}

func markdownChangelogTable(items []ChangelogItem, _ ChangelogReport, localizer Localizer) string {
	lines := []string{
		"| " + localizer.Text("changelog.column.work-item") + " | " + localizer.Text("changelog.column.type") + " | " + localizer.Text("changelog.column.state") + " | " + localizer.Text("changelog.column.title") + " |",
		"| --- | --- | --- | --- |",
	}
	for _, item := range items {
		lines = append(lines, "| "+markdownChangelogLink(item)+" | "+escapeMarkdownCell(item.Type)+" | "+escapeMarkdownCell(item.State)+" | "+escapeMarkdownCell(item.Title)+" |")
	}
	return strings.Join(lines, "\n")
}

func renderChangelogHTML(report ChangelogReport, section ChangelogSection, localizer Localizer) string {
	blocks := []string{"<h1>" + html.EscapeString(changelogTitle(section, localizer)) + "</h1>"}
	for _, warning := range section.Warnings {
		blocks = append(blocks, "<p><strong>"+html.EscapeString(localizer.Text("changelog.warning.label"))+":</strong> "+html.EscapeString(warning.Detail)+"</p>")
	}
	if status := changelogStatus(report, section, localizer); status != "" {
		blocks = append(blocks, "<p>"+html.EscapeString(status)+"</p>")
	} else if report.GroupByParent && len(section.Groups) != 0 {
		for _, group := range section.Groups {
			blocks = append(blocks, "<h2>"+htmlChangelogLine(group.Parent)+"</h2>")
			if report.Table {
				items := group.Items
				if len(items) == 0 {
					items = []ChangelogItem{group.Parent}
				}
				blocks = append(blocks, htmlChangelogTable(items, localizer))
			} else if len(group.Items) != 0 {
				blocks = append(blocks, htmlChangelogList(group.Items))
			}
		}
	} else if report.Table {
		blocks = append(blocks, htmlChangelogTable(section.Items, localizer))
	} else {
		blocks = append(blocks, htmlChangelogList(section.Items))
	}
	return strings.Join(blocks, "\n")
}

func htmlChangelogList(items []ChangelogItem) string {
	var output strings.Builder
	output.WriteString("<ul>\n")
	for _, item := range items {
		output.WriteString("  <li>" + htmlChangelogLine(item) + "</li>\n")
	}
	output.WriteString("</ul>")
	return output.String()
}

func htmlChangelogTable(items []ChangelogItem, localizer Localizer) string {
	var output strings.Builder
	output.WriteString("<table>\n  <thead>\n    <tr><th>")
	output.WriteString(html.EscapeString(localizer.Text("changelog.column.work-item")))
	output.WriteString("</th><th>" + html.EscapeString(localizer.Text("changelog.column.type")) + "</th><th>" + html.EscapeString(localizer.Text("changelog.column.state")) + "</th><th>" + html.EscapeString(localizer.Text("changelog.column.title")) + "</th></tr>\n  </thead>\n  <tbody>\n")
	for _, item := range items {
		output.WriteString("    <tr><td>" + htmlChangelogLink(item) + "</td><td>" + html.EscapeString(item.Type) + "</td><td>" + html.EscapeString(item.State) + "</td><td>" + html.EscapeString(item.Title) + "</td></tr>\n")
	}
	output.WriteString("  </tbody>\n</table>")
	return output.String()
}

func changelogTitle(section ChangelogSection, localizer Localizer) string {
	if section.Repository != "" {
		return localize(localizer, "changelog.title.repository", l10n.A("repository", section.Repository))
	}
	return localize(localizer, "changelog.title")
}

func changelogStatus(report ChangelogReport, section ChangelogSection, localizer Localizer) string {
	if section.SourceEmpty {
		if report.FromGit {
			return localizer.Text("changelog.empty.git")
		}
		if report.FromPR {
			return localizer.Text("changelog.empty.pr")
		}
		return localizer.Text("changelog.empty.input")
	}
	if section.ResolvedEmpty {
		return localizer.Text("changelog.empty.resolved")
	}
	return ""
}

func rawChangelogItem(item ChangelogItem) string {
	line := "#" + item.ID
	if strings.TrimSpace(item.Type) != "" {
		line += " [" + item.Type + "]"
	}
	if strings.TrimSpace(item.State) != "" {
		line += " " + item.State
	}
	if strings.TrimSpace(item.Title) != "" {
		line += " - " + item.Title
	}
	return line
}

func markdownChangelogLine(item ChangelogItem) string {
	line := markdownChangelogLink(item)
	if strings.TrimSpace(item.Type) != "" {
		line += " [" + item.Type + "]"
	}
	if strings.TrimSpace(item.State) != "" {
		line += " " + item.State
	}
	if strings.TrimSpace(item.Title) != "" {
		line += " - " + item.Title
	}
	return line
}

func markdownChangelogLink(item ChangelogItem) string { return "[#" + item.ID + "](" + item.URL + ")" }

func htmlChangelogLine(item ChangelogItem) string {
	line := htmlChangelogLink(item)
	if strings.TrimSpace(item.Type) != "" {
		line += " [" + html.EscapeString(item.Type) + "]"
	}
	if strings.TrimSpace(item.State) != "" {
		line += " " + html.EscapeString(item.State)
	}
	if strings.TrimSpace(item.Title) != "" {
		line += " - " + html.EscapeString(item.Title)
	}
	return line
}

func htmlChangelogLink(item ChangelogItem) string {
	return "<a href=\"" + html.EscapeString(item.URL) + "\">#" + html.EscapeString(item.ID) + "</a>"
}

func escapeMarkdownCell(value string) string {
	return strings.NewReplacer("|", "\\|", "\r\n", "<br />", "\n", "<br />").Replace(value)
}
