package bootstrap

import (
	"testing"

	"github.com/sachahjkl/dw/internal/l10n"
)

func TestEnglishCatalogCoversInteractiveWorkPrompts(t *testing.T) {
	catalog, err := englishCatalog()
	if err != nil {
		t.Fatalf("build English catalog: %v", err)
	}
	for _, id := range []l10n.ID{
		"prompt.project",
		"prompt.repositories",
		"prompt.choice.value",
		"prompt.work-item.manual",
		"prompt.work-item",
		"prompt.work-item-id",
	} {
		if !catalog.Has(id) {
			t.Errorf("English catalog is missing %s", id)
		}
	}
}
