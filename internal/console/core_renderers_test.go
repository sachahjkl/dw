package console

import (
	"errors"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/l10n"
)

type canceledActionError struct{}

func (canceledActionError) Error() string           { return "work.execution-canceled" }
func (canceledActionError) Localized() l10n.Message { return l10n.M("work.execution-canceled") }

func TestLocalizedErrorTextDoesNotExposeTechnicalCode(t *testing.T) {
	text := LocalizedErrorText(NewEnglishLocalizer(), errors.New("execution.sqlite-open:detail"))
	if text != "An unexpected internal error occurred." {
		t.Fatalf("error text = %q", text)
	}
}

func TestLocalizedErrorTextRendersCanceledAction(t *testing.T) {
	if text := LocalizedErrorText(NewEnglishLocalizer(), canceledActionError{}); text != "Action canceled." {
		t.Fatalf("error text = %q", text)
	}
}

func TestDoctorExposesStructuredPageProjection(t *testing.T) {
	registry := NewRegistry()
	if err := RegisterCoreRenderers(registry); err != nil {
		t.Fatal(err)
	}
	report := doctor.Report{Root: `S:\dw`, Checks: []doctor.Check{{Kind: doctor.CheckGit, Passed: true}}}
	page, ok, err := registry.ProjectPage(doctor.ActionDoctor, report)
	if err != nil {
		t.Fatal(err)
	}
	if !ok || len(page.Summary) == 0 || len(page.Sections) != 1 || page.Sections[0].Table == nil {
		t.Fatalf("Doctor page projection = %#v", page)
	}
}

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
