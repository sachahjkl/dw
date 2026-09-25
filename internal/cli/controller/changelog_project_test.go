package controller

import (
	"os"
	"path/filepath"
	"testing"
)

func TestInferProjectFromProjectsTree(t *testing.T) {
	root := t.TempDir()
	nested := filepath.Join(root, "projects", "acme", "repositories", "front.git")
	if err := os.MkdirAll(nested, 0o755); err != nil {
		t.Fatal(err)
	}
	if project := inferProject(root, nested); project != "acme" {
		t.Fatalf("project = %q, want acme", project)
	}
	for _, directory := range []string{root, filepath.Join(root, "projects"), t.TempDir()} {
		if project := inferProject(root, directory); project != "" {
			t.Fatalf("inferProject(%q) = %q, want none", directory, project)
		}
	}
}
