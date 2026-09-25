package agent

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestWorkspaceConfigFileMerge(t *testing.T) {
	var file WorkspaceConfigFile
	for _, candidate := range WorkspaceConfigFiles(WorkspaceConfigRequest{Project: "sample", WorkItems: []WorkspaceWorkItemRef{{ID: "42"}}}) {
		if candidate.RelativePath == "AGENTS.md" {
			file = candidate
		}
	}
	if !strings.HasPrefix(file.Content, BlockBegin+"\n") || !strings.HasSuffix(file.Content, BlockEnd+"\n") {
		t.Fatalf("content is not marked: %q", file.Content)
	}
	if got := string(file.Merge(nil, false)); got != file.Content {
		t.Fatalf("new file = %q", got)
	}
	if got := string(file.Merge([]byte(file.Legacy), true)); got != file.Content {
		t.Fatalf("legacy file = %q", got)
	}
	if got := string(file.Merge([]byte("# Mine\n"), true)); got != "# Mine\n\n"+file.Content {
		t.Fatalf("unmarked file = %q", got)
	}
	existing := "before\n" + BlockBegin + "\nold\n" + BlockEnd + "\nafter\n"
	if got := string(file.Merge([]byte(existing), true)); got != "before\n"+file.Content+"after\n" {
		t.Fatalf("marked file = %q", got)
	}
}

func TestWriteWorkspaceConfigFilesPreservesUserContent(t *testing.T) {
	workspace := t.TempDir()
	path := filepath.Join(workspace, "AGENTS.md")
	if err := os.WriteFile(path, []byte("keep me\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	request := WorkspaceConfigRequest{Workspace: workspace, Project: "sample"}
	for range 2 {
		if err := WriteWorkspaceConfigFiles(request); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(string(data), "keep me\n") || strings.Count(string(data), BlockBegin) != 1 {
		t.Fatalf("AGENTS.md = %q", data)
	}
}

func TestCursorCommand(t *testing.T) {
	original := lookPath
	t.Cleanup(func() { lookPath = original })
	cases := []struct {
		paths map[string]string
		want  string
	}{
		{map[string]string{"cursor-agent": "/bin/cursor-agent", "agent": "/bin/agent"}, "cursor-agent"},
		{map[string]string{"agent": "/opt/Cursor/bin/agent"}, "agent"},
		{map[string]string{"agent": "/usr/bin/agent"}, "cursor-agent"},
	}
	for _, test := range cases {
		lookPath = func(name string) (string, error) {
			if path, ok := test.paths[name]; ok {
				return path, nil
			}
			return "", errors.New("not found")
		}
		if got := cursorCommand(); got != test.want {
			t.Errorf("cursorCommand(%v) = %q, want %q", test.paths, got, test.want)
		}
	}
}

func TestOpencodeConfigPathUsesJoin(t *testing.T) {
	selected := Opencode
	launch := BuildOpenLaunch(&selected, OpenRequest{Root: "root", Workspace: "ws"})
	if launch.Environment[0].Value != filepath.Join("root", "config", "opencode", "opencode.jsonc") {
		t.Fatalf("OPENCODE_CONFIG = %q", launch.Environment[0].Value)
	}
}
