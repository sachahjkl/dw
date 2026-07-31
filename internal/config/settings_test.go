package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSettersDoNotOverwriteInvalidSettings(t *testing.T) {
	configHome := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", configHome)
	t.Setenv("LOCALAPPDATA", configHome)
	t.Setenv("APPDATA", configHome)
	path := filepath.Join(configHome, "DevWorkflow", "settings.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	original := []byte(`{"root":`)
	if err := os.WriteFile(path, original, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := SetUserRoot(t.TempDir()); err == nil {
		t.Fatal("SetUserRoot succeeded with invalid settings")
	}
	if _, err := SetColorMode(ColorAlways); err == nil {
		t.Fatal("SetColorMode succeeded with invalid settings")
	}
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != string(original) {
		t.Fatalf("settings changed to %q", content)
	}
}
