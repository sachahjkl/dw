package console

import (
	"strings"
	"unicode/utf8"

	"charm.land/lipgloss/v2"
	"github.com/sachahjkl/dw/internal/l10n"
)

type MessageID = l10n.ID
type Localizer = l10n.Localizer

func localize(localizer Localizer, id l10n.ID, args ...l10n.Arg) string {
	if len(args) == 0 {
		return localizer.Text(id)
	}
	return localizer.Render(l10n.M(id, args...))
}

// Theme contains only presentation. It never changes machine output.
type Theme struct {
	enabled bool
	title   lipgloss.Style
	panel   lipgloss.Style
	label   lipgloss.Style
	muted   lipgloss.Style
	command lipgloss.Style
	path    lipgloss.Style
	success lipgloss.Style
	warning lipgloss.Style
	failure lipgloss.Style
}

func NewTheme(enabled bool) Theme {
	plain := lipgloss.NewStyle()
	panel := plain.Border(lipgloss.RoundedBorder()).Padding(0, 1)
	if !enabled {
		return Theme{title: plain, panel: panel, label: plain, muted: plain, command: plain, path: plain, success: plain, warning: plain, failure: plain}
	}
	return Theme{
		enabled: true,
		title:   plain.Bold(true).Foreground(lipgloss.Color("12")),
		panel:   panel.BorderForeground(lipgloss.Color("8")),
		label:   plain.Bold(true).Foreground(lipgloss.Color("14")),
		muted:   plain.Faint(true),
		command: plain.Foreground(lipgloss.Color("13")),
		path:    plain.Foreground(lipgloss.Color("14")),
		success: plain.Bold(true).Foreground(lipgloss.Color("10")),
		warning: plain.Bold(true).Foreground(lipgloss.Color("11")),
		failure: plain.Bold(true).Foreground(lipgloss.Color("9")),
	}
}

func (t Theme) Title(value string) string   { return t.title.Render(value) }
func (t Theme) Panel(value string) string   { return t.panel.Render(value) }
func (t Theme) Label(value string) string   { return t.label.Render(value) }
func (t Theme) Muted(value string) string   { return t.muted.Render(value) }
func (t Theme) Command(value string) string { return t.command.Render(value) }
func (t Theme) Path(value string) string    { return t.path.Render(value) }
func (t Theme) Success(value string) string { return t.success.Render(value) }
func (t Theme) Warning(value string) string { return t.warning.Render(value) }
func (t Theme) Failure(value string) string { return t.failure.Render(value) }

func (t Theme) Badge(status Status, value string) string {
	switch status {
	case StatusSuccess:
		return t.success.Copy().Padding(0, 1).Render(value)
	case StatusWarning:
		return t.warning.Copy().Padding(0, 1).Render(value)
	case StatusFailure:
		return t.failure.Copy().Padding(0, 1).Render(value)
	default:
		return t.label.Copy().Padding(0, 1).Render(value)
	}
}

type Status uint8

const (
	StatusNeutral Status = iota
	StatusSuccess
	StatusWarning
	StatusFailure
)

type Field struct {
	Label MessageID
	Value string
	Style ValueStyle
}

type ValueStyle uint8

const (
	ValuePlain ValueStyle = iota
	ValuePath
	ValueCommand
	ValueSuccess
	ValueWarning
	ValueFailure
	ValueMuted
)

type Table struct {
	Columns     []MessageID
	ColumnNames []string
	Rows        [][]string
}

type Panel struct {
	Title MessageID
	Body  string
}

type Section struct {
	Title  MessageID
	Fields []Field
	Table  *Table
	Panels []Panel
	Items  []string
}

// Page is the provider-neutral human projection accepted by all ordinary result renderers.
type Page struct {
	Title    MessageID
	Badge    MessageID
	Status   Status
	Summary  []Field
	Sections []Section
	Hint     *Field
}

func RenderPage(page Page, localizer Localizer, theme Theme) string {
	localizer = WithConsoleMessages(localizer)
	var blocks []string
	title := localizer.Text(page.Title)
	if page.Badge != "" {
		title += "  " + theme.Badge(page.Status, localizer.Text(page.Badge))
	}
	blocks = append(blocks, theme.Title(title))
	if len(page.Summary) != 0 {
		blocks = append(blocks, renderFields(page.Summary, localizer, theme))
	}
	for _, section := range page.Sections {
		var body []string
		if len(section.Fields) != 0 {
			body = append(body, renderFields(section.Fields, localizer, theme))
		}
		if section.Table != nil {
			body = append(body, RenderTable(*section.Table, localizer, theme))
		}
		for _, panel := range section.Panels {
			content := panel.Body
			if panel.Title != "" {
				content = theme.Label(localizer.Text(panel.Title)) + "\n" + content
			}
			body = append(body, theme.Panel(content))
		}
		for _, item := range section.Items {
			body = append(body, "• "+item)
		}
		if len(body) == 0 {
			continue
		}
		if section.Title != "" {
			blocks = append(blocks, theme.Label(localizer.Text(section.Title))+"\n"+strings.Join(body, "\n"))
		} else {
			blocks = append(blocks, strings.Join(body, "\n"))
		}
	}
	if page.Hint != nil {
		blocks = append(blocks, theme.Muted(localizer.Text(page.Hint.Label)+": ")+styleValue(page.Hint.Value, page.Hint.Style, theme))
	}
	return strings.Join(blocks, "\n\n")
}

func renderFields(fields []Field, localizer Localizer, theme Theme) string {
	width := 0
	labels := make([]string, len(fields))
	for i, field := range fields {
		labels[i] = localizer.Text(field.Label)
		width = max(width, utf8.RuneCountInString(labels[i]))
	}
	lines := make([]string, len(fields))
	for i, field := range fields {
		padding := strings.Repeat(" ", width-utf8.RuneCountInString(labels[i]))
		lines[i] = theme.Label(labels[i]+padding) + "  " + styleValue(field.Value, field.Style, theme)
	}
	return strings.Join(lines, "\n")
}

func styleValue(value string, style ValueStyle, theme Theme) string {
	switch style {
	case ValuePath:
		return theme.Path(value)
	case ValueCommand:
		return theme.Command(value)
	case ValueSuccess:
		return theme.Success(value)
	case ValueWarning:
		return theme.Warning(value)
	case ValueFailure:
		return theme.Failure(value)
	case ValueMuted:
		return theme.Muted(value)
	default:
		return value
	}
}

// RenderTable preserves caller order. Width calculation is deterministic and bounded.
func RenderTable(table Table, localizer Localizer, theme Theme) string {
	columnCount := len(table.Columns)
	if len(table.ColumnNames) != 0 {
		columnCount = len(table.ColumnNames)
	}
	if columnCount == 0 {
		return ""
	}
	headers := append([]string(nil), table.ColumnNames...)
	if len(headers) == 0 {
		headers = make([]string, len(table.Columns))
		for i, id := range table.Columns {
			headers[i] = localizer.Text(id)
		}
	}
	widths := make([]int, len(headers))
	for i := range headers {
		widths[i] = clampWidth(utf8.RuneCountInString(headers[i]))
	}
	for _, row := range table.Rows {
		for i := range widths {
			if i < len(row) {
				widths[i] = max(widths[i], clampWidth(utf8.RuneCountInString(singleLine(row[i]))))
			}
		}
	}
	top := tableSeparator(widths, "┌", "┬", "┐")
	middle := tableSeparator(widths, "├", "┼", "┤")
	bottom := tableSeparator(widths, "└", "┴", "┘")
	lines := []string{top, tableRow(headers, widths, theme, true), middle}
	for _, row := range table.Rows {
		lines = append(lines, tableRow(row, widths, theme, false))
	}
	lines = append(lines, bottom)
	return strings.Join(lines, "\n")
}

func tableSeparator(widths []int, left, middle, right string) string {
	cells := make([]string, len(widths))
	for i, width := range widths {
		cells[i] = strings.Repeat("─", width+2)
	}
	return left + strings.Join(cells, middle) + right
}

func tableRow(row []string, widths []int, theme Theme, header bool) string {
	cells := make([]string, len(widths))
	for i, width := range widths {
		value := ""
		if i < len(row) {
			value = truncateCell(singleLine(row[i]), width)
		}
		value += strings.Repeat(" ", width-utf8.RuneCountInString(value))
		if header {
			value = theme.Label(value)
		}
		cells[i] = " " + value + " "
	}
	return "│" + strings.Join(cells, "│") + "│"
}

func clampWidth(width int) int {
	if width < 1 {
		return 1
	}
	if width > 48 {
		return 48
	}
	return width
}

func singleLine(value string) string {
	return strings.NewReplacer("\r\n", " ", "\r", " ", "\n", " ").Replace(value)
}

func truncateCell(value string, width int) string {
	if utf8.RuneCountInString(value) <= width {
		return value
	}
	if width <= 1 {
		return "…"
	}
	runes := []rune(value)
	return string(runes[:width-1]) + "…"
}
