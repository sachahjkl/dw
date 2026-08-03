package bootstrap

import (
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/providerapp"
)

func TestProviderListPagePresentsGroupedFeatures(t *testing.T) {
	page := providerListPage(providerapp.ListReport{Providers: []providerapp.Summary{{
		Name:         "example",
		Kinds:        []providerapp.Kind{providerapp.KindWork},
		Capabilities: []string{"item-reader", "assigned-querier", "authenticator"},
	}}})
	rendered := console.RenderPage(page, console.NewEnglishLocalizer(), console.NewTheme(false))
	for _, expected := range []string{"Provider List", "Work items", "Authentication"} {
		if !strings.Contains(rendered, expected) {
			t.Fatalf("provider output missing %q: %s", expected, rendered)
		}
	}
	if strings.Contains(rendered, "item-reader") || strings.Contains(rendered, "assigned-querier") {
		t.Fatalf("provider list leaked internal capability names: %s", rendered)
	}
}

func TestWorkspaceListPageExplainsEmptyState(t *testing.T) {
	page := workspaceListPage(console.ResultWorkspaceList, "/tmp/root", nil)
	rendered := console.RenderPage(page, console.NewEnglishLocalizer(), console.NewTheme(false))
	if !strings.Contains(rendered, "No workspaces found") || strings.Contains(rendered, "dw workspace start") || len(page.Actions) != 0 {
		t.Fatalf("workspace empty output = %s", rendered)
	}
}

func TestScopeTUIRootSetsDoctorRoot(t *testing.T) {
	const root = "/explicit/tui/root"
	request, ok := scopeTUIRoot(doctor.Request{}, root).(doctor.Request)
	if !ok {
		t.Fatalf("scoped request type = %T, want doctor.Request", request)
	}
	if request.Root != root {
		t.Fatalf("scoped Doctor root = %q, want %q", request.Root, root)
	}
}
