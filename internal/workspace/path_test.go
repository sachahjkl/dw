package workspace

import (
	"context"
	"path/filepath"
	"testing"
)

func TestPlanStartRejectsEscapingPathComponents(t *testing.T) {
	t.Parallel()
	root := t.TempDir()
	engine := &Engine{}
	tests := []StartRequest{
		{Root: root, Project: "../../outside", WorkItemIDs: []string{"1"}},
		{Root: root, Project: "project", WorkItemIDs: []string{"../1"}},
		{Root: root, Project: "project", WorkItemIDs: []string{"1"}, Type: "../../type"},
	}
	for _, request := range tests {
		if _, err := engine.PlanStart(context.Background(), request); err == nil {
			t.Fatalf("PlanStart(%+v) succeeded", request)
		}
	}
}

func TestEnsurePathWithinRejectsSibling(t *testing.T) {
	t.Parallel()
	root := t.TempDir()
	if err := ensurePathWithin(root, filepath.Join(root, "..", "outside")); err == nil {
		t.Fatal("ensurePathWithin accepted a sibling path")
	}
}

func TestValidateRelativePathAllowsConfinedNestedFolder(t *testing.T) {
	t.Parallel()
	if err := validateRelativePath("repository folder", "apps/front"); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{"../front", "apps/../../front", `/tmp/front`, `apps\front`} {
		if err := validateRelativePath("repository folder", path); err == nil {
			t.Fatalf("validateRelativePath accepted %q", path)
		}
	}
}
