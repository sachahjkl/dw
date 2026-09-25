package config

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestExpandDollarKeepsUndefinedAndUnterminatedLiterals(t *testing.T) {
	t.Setenv("DW_TEST_DEFINED", "value")
	cases := map[string]string{
		"a/$DW_TEST_DEFINED/b":     "a/value/b",
		"a/${DW_TEST_DEFINED}/b":   "a/value/b",
		"a/$DW_TEST_UNDEFINED_X/b": "a/$DW_TEST_UNDEFINED_X/b",
		"a/${DW_TEST_UNDEFINED_X}": "a/${DW_TEST_UNDEFINED_X}",
		"a/${unterminated":         "a/${unterminated",
		"price$":                   "price$",
		"%DW_TEST_DEFINED%":        "value",
		"%DW_TEST_UNDEFINED_X%":    "%DW_TEST_UNDEFINED_X%",
	}
	for input, want := range cases {
		if got := expandEnvironmentVariables(input); got != want {
			t.Errorf("expandEnvironmentVariables(%q) = %q, want %q", input, got, want)
		}
	}
}

func TestNormalizePathDropsParentAboveRoot(t *testing.T) {
	root := string(filepath.Separator)
	if runtime.GOOS == "windows" {
		root = `C:\`
	}
	got, err := NormalizePath(root + filepath.Join("..", "..", "a"))
	if err != nil {
		t.Fatal(err)
	}
	if got != root+"a" {
		t.Fatalf("NormalizePath = %q, want %q", got, root+"a")
	}
}

func TestExpandHomeAcceptsPlatformSeparator(t *testing.T) {
	home := ResolvePlatformBaseDirs().HomeDir
	if got := expandHome("~/x"); got != appendPath(home, "x") {
		t.Fatalf("expandHome(~/x) = %q", got)
	}
	if runtime.GOOS == "windows" {
		if got := expandHome(`~\x`); got != appendPath(home, "x") {
			t.Fatalf(`expandHome(~\x) = %q`, got)
		}
	}
}

func TestWriteFileAtomicReplacesContent(t *testing.T) {
	path := filepath.Join(t.TempDir(), "settings.json")
	for _, content := range []string{"one", "two"} {
		if err := writeFileAtomic(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(path)
	if err != nil || string(data) != "two" {
		t.Fatalf("content = %q, %v", data, err)
	}
	entries, _ := os.ReadDir(filepath.Dir(path))
	if len(entries) != 1 {
		t.Fatalf("temporary files left: %v", entries)
	}
}
