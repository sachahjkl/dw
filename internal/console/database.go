package console

import (
	"encoding/base64"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
)

func RenderQuery(result data.Table, policy Policy, localizer Localizer, theme Theme) Output {
	localizer = WithConsoleMessages(localizer)
	if !policy.Streams.StdoutTTY {
		return TextOutput(FormatTSV, RenderQueryTSV(result))
	}
	columns := make([]string, len(result.Columns))
	for i := range result.Columns {
		columns[i] = result.Columns[i].Name
	}
	if len(columns) == 0 {
		columns = []string{localize(localizer, "db.column.result")}
	}
	rows := make([][]string, len(result.Rows))
	for rowIndex, row := range result.Rows {
		rows[rowIndex] = make([]string, len(columns))
		for columnIndex := range columns {
			if columnIndex >= len(row) {
				rows[rowIndex][columnIndex] = localize(localizer, "db.null")
			} else {
				rows[rowIndex][columnIndex] = displayDataValue(row[columnIndex], localize(localizer, "db.null"))
			}
		}
	}
	page := Page{
		Title:    "db.query.title",
		Summary:  []Field{{Label: "db.query.result", Value: localize(localizer, "db.query.rows", l10n.A("count", len(result.Rows))), Style: ValueSuccess}},
		Sections: []Section{{Table: &Table{ColumnNames: columns, Rows: rows}}},
	}
	if result.Truncated {
		page.Status = StatusWarning
		page.Badge = "db.query.truncated.badge"
		page.Hint = &Field{Label: "db.query.truncated", Value: "--max-rows", Style: ValueCommand}
	}
	return TextOutput(FormatHuman, RenderPage(page, localizer, theme))
}

// RenderQueryTSV preserves column and row order and the legacy footer exactly.
func RenderQueryTSV(result data.Table) string {
	columns := make([]string, len(result.Columns))
	for i := range result.Columns {
		columns[i] = result.Columns[i].Name
	}
	lines := make([]string, 0, len(result.Rows)+2)
	lines = append(lines, strings.Join(columns, "\t"))
	for _, row := range result.Rows {
		cells := make([]string, len(row))
		for i := range row {
			cells[i] = displayDataValue(row[i], "NULL")
		}
		lines = append(lines, strings.Join(cells, "\t"))
	}
	footer := "-- " + strconv.Itoa(len(result.Rows)) + " rows"
	if result.Truncated {
		footer += " (truncated)"
	}
	lines = append(lines, footer)
	return strings.Join(lines, "\n")
}

func displayDataValue(value data.Value, null string) string {
	if value.Kind() == data.ValueNull {
		return null
	}
	if text, ok := value.Text(); ok {
		return text
	}
	if boolean, ok := value.Boolean(); ok {
		return strconv.FormatBool(boolean)
	}
	if binary, ok := value.Binary(); ok {
		return base64.StdEncoding.EncodeToString(binary)
	}
	return null
}
