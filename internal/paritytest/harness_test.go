package paritytest_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/paritytest"
)

func TestHelperProcess(t *testing.T) {
	mode := os.Getenv("DW_PARITY_HELPER")
	if mode == "" {
		return
	}
	switch mode {
	case "streams":
		fmt.Fprint(os.Stdout, "stdout without implicit newline")
		fmt.Fprintln(os.Stderr, "stderr line")
		os.Exit(7)
	case "paths":
		fmt.Fprintf(os.Stdout, "HOME=%s\nROOT=%s\n", os.Getenv("HOME"), os.Getenv("DW_PARITY_ROOT"))
		os.Exit(0)
	case "stdin":
		contents, err := os.ReadFile(os.Getenv("DW_PARITY_INPUT"))
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(2)
		}
		os.Stdout.Write(contents)
		os.Exit(0)
	case "write":
		path := filepath.Join(os.Getenv("DW_PARITY_ROOT"), os.Getenv("DW_PARITY_FILE"))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(2)
		}
		if err := os.WriteFile(path, []byte("home="+os.Getenv("HOME")+"\n"), 0o600); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(2)
		}
		fmt.Fprintln(os.Stdout, path)
		os.Exit(0)
	default:
		fmt.Fprintln(os.Stderr, "unknown helper mode")
		os.Exit(3)
	}
}

func helperCommand(mode string) paritytest.Command {
	executable, err := os.Executable()
	if err != nil {
		panic(err)
	}
	return paritytest.Command{
		Executable: executable,
		Args:       []string{"-test.run=^TestHelperProcess$"},
		Env:        map[string]string{"DW_PARITY_HELPER": mode},
	}
}

func TestRunPreservesExitAndSeparateStreams(t *testing.T) {
	sandbox, err := paritytest.NewSandbox(t.TempDir(), t.Name())
	if err != nil {
		t.Fatal(err)
	}
	defer sandbox.Cleanup()

	outcome, err := paritytest.Run(context.Background(), sandbox, helperCommand("streams"))
	if err != nil {
		t.Fatal(err)
	}
	want := paritytest.Outcome{
		ExitCode: 7,
		Stdout:   []byte("stdout without implicit newline"),
		Stderr:   []byte("stderr line\n"),
	}
	if err := paritytest.Difference(want, outcome); err != nil {
		t.Fatal(err)
	}
}

func TestPairSubstitutesIndependentSandboxPaths(t *testing.T) {
	command := helperCommand("paths")
	if err := paritytest.Pair(context.Background(), t.TempDir(), t.Name(), command, command, nil); err != nil {
		t.Fatal(err)
	}
}

func TestPairSequenceComparesOutputsAndFilesystem(t *testing.T) {
	first := helperCommand("write")
	first.Env["DW_PARITY_FILE"] = "config/first.txt"
	second := helperCommand("write")
	second.Env["DW_PARITY_FILE"] = "config/second.txt"
	commands := []paritytest.Command{first, second}
	if err := paritytest.PairSequence(context.Background(), t.TempDir(), t.Name(), commands, commands, nil); err != nil {
		t.Fatal(err)
	}
}

func TestCommandExpandsRootInArgumentsAndEnvironment(t *testing.T) {
	sandbox, err := paritytest.NewSandbox(t.TempDir(), t.Name())
	if err != nil {
		t.Fatal(err)
	}
	defer sandbox.Cleanup()
	content := []byte("fixture bytes\n")
	input := filepath.Join(sandbox.Root, "input.txt")
	if err := os.WriteFile(input, content, 0o600); err != nil {
		t.Fatal(err)
	}
	command := helperCommand("stdin")
	command.Env["DW_PARITY_INPUT"] = "${ROOT}/input.txt"
	outcome, err := paritytest.Run(context.Background(), sandbox, command)
	if err != nil {
		t.Fatal(err)
	}
	if err := paritytest.Difference(paritytest.Outcome{Stdout: content}, outcome); err != nil {
		t.Fatal(err)
	}
}

func TestDifferenceNeverMergesChannels(t *testing.T) {
	want := paritytest.Outcome{Stdout: []byte("same bytes"), Stderr: []byte("different")}
	got := paritytest.Outcome{Stdout: []byte("same bytesdifferent")}
	err := paritytest.Difference(want, got)
	if err == nil || !strings.Contains(err.Error(), "stdout differs") {
		t.Fatalf("Difference() = %v, want stdout-specific mismatch", err)
	}
}

func TestNormalizeOnlyDeclaredVolatileValues(t *testing.T) {
	input := paritytest.Outcome{
		ExitCode: 9,
		Stdout:   []byte("/tmp/run-42 then /tmp/run\nordered=b,a"),
		Stderr:   []byte("pid=123"),
	}
	got := paritytest.Normalize(input,
		paritytest.Substitution{Value: "/tmp/run", Token: "<BASE>"},
		paritytest.Substitution{Value: "/tmp/run-42", Token: "<RUN>"},
	)
	want := paritytest.Outcome{
		ExitCode: 9,
		Stdout:   []byte("<RUN> then <BASE>\nordered=b,a"),
		Stderr:   []byte("pid=123"),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Normalize() = %#v, want %#v", got, want)
	}
}

func TestSnapshotIsStableAndIncludesFileKinds(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, "b"), 0o750); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "b", "file.txt"), []byte("contents"), 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(filepath.Join(root, "a"), 0o700); err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS != "windows" {
		if err := os.Symlink("b/file.txt", filepath.Join(root, "link")); err != nil {
			t.Fatal(err)
		}
	}
	first, err := paritytest.Snapshot(root)
	if err != nil {
		t.Fatal(err)
	}
	second, err := paritytest.Snapshot(root)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(first, second) {
		t.Fatalf("snapshots differ:\nfirst=%#v\nsecond=%#v", first, second)
	}
	for index := 1; index < len(first); index++ {
		if first[index-1].Path >= first[index].Path {
			t.Fatalf("snapshot is not lexically ordered: %#v", first)
		}
	}
}

func TestGreenfieldCLIFixturesCoverStableProcessContracts(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "contract", "cli-cases.json")
	fixtures, err := paritytest.LoadFixtures(path)
	if err != nil {
		t.Fatal(err)
	}
	byName := make(map[string]paritytest.Fixture, len(fixtures))
	for _, fixture := range fixtures {
		byName[fixture.Name] = fixture
	}
	for _, required := range []string{
		"root-help-long",
		"deep-help-work-item-show",
		"deep-help-workspace-open",
		"deep-help-data-query",
		"deep-help-provider-capabilities",
		"provider-auth-positional-selection",
		"informational-version-long",
		"runtime-version-command",
		"completion-bash-root",
		"completion-fish-root",
		"completion-zsh-root",
		"completion-json-root",
		"completion-json-work-options",
		"completion-json-provider-auth",
		"completion-install-elvish",
	} {
		if _, exists := byName[required]; !exists {
			t.Errorf("missing greenfield CLI fixture %q", required)
		}
	}
}
