package workspace

import (
	"errors"
	"path/filepath"
	"strings"
	"testing"
)

func TestResolveUsesCurrentOrOnlyMatchingWorkspace(t *testing.T) {
	root := t.TempDir()
	first := writeResolverWorkspace(t, root, "alpha", "one", "1", "2026-08-01T00:00:00Z")
	if found, ok := Root(first); !ok || found != root {
		t.Fatalf("workspace root = %q, %t", found, ok)
	}
	currentChild := filepath.Join(first, "repository", "src")
	if path, err := Resolve(root, "", "", nil, false, currentChild); err != nil || path != first {
		t.Fatalf("current resolution = %q, %v", path, err)
	}
	if path, err := Resolve(root, "", "alpha", []string{"1"}, false, root); err != nil || path != first {
		t.Fatalf("selector resolution = %q, %v", path, err)
	}
	if path, err := Resolve(root, "", "", nil, false, root); err != nil || path != first {
		t.Fatalf("single resolution = %q, %v", path, err)
	}
}

func TestResolveRejectsAmbiguityAndContinueSelectsLatest(t *testing.T) {
	root := t.TempDir()
	writeResolverWorkspace(t, root, "alpha", "old", "1", "2026-08-01T00:00:00Z")
	latest := writeResolverWorkspace(t, root, "alpha", "new", "2", "2026-08-02T00:00:00Z")
	if _, err := Resolve(root, "", "", nil, false, root); err == nil || !strings.Contains(err.Error(), "Multiple task workspaces") {
		t.Fatalf("ambiguous resolution error = %v", err)
	}
	if path, err := Resolve(root, "", "", nil, true, root); err != nil || path != latest {
		t.Fatalf("latest resolution = %q, %v", path, err)
	}
	if _, err := Resolve(filepath.Join(root, "missing"), "", "", nil, false, root); !errors.Is(err, ErrNoWorkspace) {
		t.Fatalf("missing resolution error = %v", err)
	}
}

func writeResolverWorkspace(t *testing.T, root, project, name, id, created string) string {
	t.Helper()
	path := filepath.Join(root, "projects", project, "workspaces", name)
	if err := WriteManifest(filepath.Join(path, ManifestFile), Manifest{Project: project, WorkItemID: id, CreatedAt: created}); err != nil {
		t.Fatal(err)
	}
	return path
}
