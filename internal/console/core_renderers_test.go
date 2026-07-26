package console

import (
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
)

func TestActionPageUsesReadableTitleWithoutInternalActionField(t *testing.T) {
	rendered := RenderPage(actionPage(ResultWorkspaceList, Field{Label: "result.items", Value: "0"}), NewEnglishLocalizer(), NewTheme(false))
	if !strings.HasPrefix(rendered, "Workspace List\n") {
		t.Fatalf("rendered title = %q", rendered)
	}
	if strings.Contains(rendered, "Dev Workflow result") || strings.Contains(rendered, "Action  workspace.list") {
		t.Fatalf("rendered internal presentation = %q", rendered)
	}
}

func TestRenderDataTableUsesContextualTitleAndEmptyStateOnTTY(t *testing.T) {
	output := RenderDataTable(data.Table{}, Policy{Streams: Streams{StdoutTTY: true}}, NewEnglishLocalizer(), NewTheme(false), "data.catalog.title")
	rendered := string(output.Body)
	if output.Format != FormatHuman || !strings.Contains(rendered, "Data catalog") || !strings.Contains(rendered, "No rows returned") {
		t.Fatalf("rendered output = %#v %q", output.Format, rendered)
	}
}

func TestRenderDataTablePreservesTSVForPipes(t *testing.T) {
	table := data.Table{Columns: []data.Column{{Name: "name"}}, Rows: [][]data.Value{{data.StringValue("Ada")}}}
	output := RenderDataTable(table, Policy{}, NewEnglishLocalizer(), NewTheme(false), "data.read.title")
	if output.Format != FormatTSV || string(output.Body) != "name\nAda\n-- 1 rows" {
		t.Fatalf("piped output = %#v %q", output.Format, output.Body)
	}
}
