// Package paritytest provides black-box helpers for comparing dw executables.
// It deliberately treats stdout, stderr, and exit status as separate contracts.
package paritytest

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"time"
)

const DefaultTimeout = 15 * time.Second

// Sandbox isolates every user-writable location used by dw.
type Sandbox struct {
	Base string
	Home string
	Root string
	Temp string
}

// NewSandbox creates an isolated HOME, DevWorkflow root, and temporary directory.
func NewSandbox(parent, name string) (Sandbox, error) {
	base, err := os.MkdirTemp(parent, "dw-parity-"+safeName(name)+"-")
	if err != nil {
		return Sandbox{}, fmt.Errorf("create parity sandbox: %w", err)
	}
	sandbox := Sandbox{
		Base: base,
		Home: filepath.Join(base, "home"),
		Root: filepath.Join(base, "root"),
		Temp: filepath.Join(base, "tmp"),
	}
	for _, directory := range []string{sandbox.Home, sandbox.Root, sandbox.Temp} {
		if err := os.MkdirAll(directory, 0o700); err != nil {
			_ = os.RemoveAll(base)
			return Sandbox{}, fmt.Errorf("create parity directory %q: %w", directory, err)
		}
	}
	return sandbox, nil
}

func safeName(name string) string {
	if name == "" {
		return "case"
	}
	return strings.Map(func(r rune) rune {
		if r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || r == '-' || r == '_' {
			return r
		}
		return '-'
	}, name)
}

// Cleanup removes all files owned by the sandbox.
func (s Sandbox) Cleanup() error { return os.RemoveAll(s.Base) }

// Env returns a deterministic process environment. PATH is preserved so that
// commands exercising the real Git executable remain possible.
func (s Sandbox) Env(overrides map[string]string) []string {
	values := map[string]string{
		"HOME":                s.Home,
		"USERPROFILE":         s.Home,
		"HOMEDRIVE":           filepath.VolumeName(s.Home),
		"HOMEPATH":            strings.TrimPrefix(s.Home, filepath.VolumeName(s.Home)),
		"XDG_CONFIG_HOME":     filepath.Join(s.Home, ".config"),
		"XDG_DATA_HOME":       filepath.Join(s.Home, ".local", "share"),
		"XDG_CACHE_HOME":      filepath.Join(s.Home, ".cache"),
		"XDG_STATE_HOME":      filepath.Join(s.Home, ".local", "state"),
		"LOCALAPPDATA":        filepath.Join(s.Home, "AppData", "Local"),
		"APPDATA":             filepath.Join(s.Home, "AppData", "Roaming"),
		"TMPDIR":              s.Temp,
		"TMP":                 s.Temp,
		"TEMP":                s.Temp,
		"NO_COLOR":            "1",
		"CLICOLOR":            "0",
		"CLICOLOR_FORCE":      "0",
		"TERM":                "dumb",
		"DW_PARITY_ROOT":      s.Root,
		"GIT_CONFIG_NOSYSTEM": "1",
		"GIT_TERMINAL_PROMPT": "0",
		"GCM_INTERACTIVE":     "never",
		"GIT_ASKPASS":         "",
		"SSH_ASKPASS":         "",
	}
	for _, key := range []string{"PATH", "SystemRoot", "ComSpec", "PATHEXT", "WINDIR"} {
		if value, ok := os.LookupEnv(key); ok {
			values[key] = value
		}
	}
	for key, value := range overrides {
		values[key] = value
	}
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	environment := make([]string, 0, len(keys))
	for _, key := range keys {
		environment = append(environment, key+"="+values[key])
	}
	return environment
}

// Command is one process invocation. Args and environment values may contain
// ${BASE}, ${HOME}, ${ROOT}, and ${TEMP} placeholders.
type Command struct {
	Executable string
	Args       []string
	Env        map[string]string
	Dir        string
	Stdin      []byte
	Timeout    time.Duration
}

// Outcome is the complete observable process contract.
type Outcome struct {
	ExitCode int
	Stdout   []byte
	Stderr   []byte
}

// Run executes one command without merging output channels. Non-zero exit
// status is returned in Outcome and is not itself an error.
func Run(ctx context.Context, sandbox Sandbox, command Command) (Outcome, error) {
	if command.Executable == "" {
		return Outcome{}, errors.New("parity command has no executable")
	}
	timeout := command.Timeout
	if timeout <= 0 {
		timeout = DefaultTimeout
	}
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	expand := sandbox.expander()
	arguments := make([]string, len(command.Args))
	for index, argument := range command.Args {
		arguments[index] = expand(argument)
	}
	overrides := make(map[string]string, len(command.Env))
	for key, value := range command.Env {
		overrides[key] = expand(value)
	}

	process := exec.CommandContext(ctx, expand(command.Executable), arguments...)
	process.Env = sandbox.Env(overrides)
	process.Dir = expand(command.Dir)
	if process.Dir == "" {
		process.Dir = sandbox.Root
	}
	process.Stdin = bytes.NewReader(command.Stdin)
	var stdout, stderr bytes.Buffer
	process.Stdout = &stdout
	process.Stderr = &stderr

	err := process.Run()
	outcome := Outcome{Stdout: stdout.Bytes(), Stderr: stderr.Bytes()}
	if err == nil {
		return outcome, nil
	}
	if ctx.Err() != nil {
		return Outcome{}, fmt.Errorf("run %q: %w", command.Executable, ctx.Err())
	}
	var exitError *exec.ExitError
	if errors.As(err, &exitError) {
		outcome.ExitCode = exitError.ExitCode()
		return outcome, nil
	}
	return Outcome{}, fmt.Errorf("start %q: %w", command.Executable, err)
}

func (s Sandbox) expander() func(string) string {
	replacer := strings.NewReplacer(
		"${BASE}", s.Base,
		"${HOME}", s.Home,
		"${ROOT}", s.Root,
		"${TEMP}", s.Temp,
	)
	return replacer.Replace
}

// Substitution replaces one explicitly declared volatile value. Callers must
// not substitute ordering, output channels, or exit status.
type Substitution struct {
	Value string
	Token string
}

// SandboxSubstitutions returns only filesystem locations created by the
// harness. Longer paths are normalized first to avoid partial replacements.
func SandboxSubstitutions(s Sandbox) []Substitution {
	return []Substitution{
		{Value: s.Home, Token: "<HOME>"},
		{Value: s.Root, Token: "<ROOT>"},
		{Value: s.Temp, Token: "<TEMP>"},
		{Value: s.Base, Token: "<SANDBOX>"},
	}
}

// Normalize performs only caller-declared literal substitutions.
func Normalize(outcome Outcome, substitutions ...Substitution) Outcome {
	ordered := append([]Substitution(nil), substitutions...)
	sort.SliceStable(ordered, func(i, j int) bool { return len(ordered[i].Value) > len(ordered[j].Value) })
	normalize := func(input []byte) []byte {
		result := string(input)
		for _, substitution := range ordered {
			if substitution.Value == "" {
				continue
			}
			result = strings.ReplaceAll(result, substitution.Value, substitution.Token)
			if runtime.GOOS == "windows" {
				result = strings.ReplaceAll(result, filepath.ToSlash(substitution.Value), substitution.Token)
			}
		}
		return []byte(result)
	}
	outcome.Stdout = normalize(outcome.Stdout)
	outcome.Stderr = normalize(outcome.Stderr)
	return outcome
}

// Difference returns a channel-specific mismatch description.
func Difference(want, got Outcome) error {
	if want.ExitCode != got.ExitCode {
		return fmt.Errorf("exit status differs: reference=%d subject=%d", want.ExitCode, got.ExitCode)
	}
	if !bytes.Equal(want.Stdout, got.Stdout) {
		return fmt.Errorf("stdout differs:\n%s", byteDifference(want.Stdout, got.Stdout))
	}
	if !bytes.Equal(want.Stderr, got.Stderr) {
		return fmt.Errorf("stderr differs:\n%s", byteDifference(want.Stderr, got.Stderr))
	}
	return nil
}

func byteDifference(want, got []byte) string {
	return fmt.Sprintf("reference (%d bytes): %q\nsubject (%d bytes): %q", len(want), want, len(got), got)
}

// Pair runs a reference and subject with equivalent fresh sandboxes. Seed may
// populate each sandbox with identical real filesystem state.
func Pair(ctx context.Context, parent, name string, reference, subject Command, seed func(Sandbox) error, substitutions ...Substitution) error {
	referenceSandbox, err := NewSandbox(parent, name+"-reference")
	if err != nil {
		return err
	}
	defer referenceSandbox.Cleanup()
	subjectSandbox, err := NewSandbox(parent, name+"-subject")
	if err != nil {
		return err
	}
	defer subjectSandbox.Cleanup()

	for _, sandbox := range []Sandbox{referenceSandbox, subjectSandbox} {
		if seed != nil {
			if err := seed(sandbox); err != nil {
				return fmt.Errorf("seed %q: %w", name, err)
			}
		}
	}
	referenceOutcome, err := Run(ctx, referenceSandbox, reference)
	if err != nil {
		return fmt.Errorf("reference: %w", err)
	}
	subjectOutcome, err := Run(ctx, subjectSandbox, subject)
	if err != nil {
		return fmt.Errorf("subject: %w", err)
	}
	referenceSubs := append(SandboxSubstitutions(referenceSandbox), substitutions...)
	subjectSubs := append(SandboxSubstitutions(subjectSandbox), substitutions...)
	return Difference(Normalize(referenceOutcome, referenceSubs...), Normalize(subjectOutcome, subjectSubs...))
}

// Fixture describes a reusable CLI process case. Exact expected values are
// optional because the same fixture can drive differential and golden tests.
type Fixture struct {
	Name     string            `json:"name"`
	Args     []string          `json:"args"`
	Env      map[string]string `json:"env,omitempty"`
	Stdin    string            `json:"stdin,omitempty"`
	Expected *Expected         `json:"expected,omitempty"`
}

type Expected struct {
	ExitCode int    `json:"exitCode"`
	Stdout   string `json:"stdout"`
	Stderr   string `json:"stderr"`
}

func (f Fixture) Command(executable string) Command {
	return Command{Executable: executable, Args: append([]string(nil), f.Args...), Env: cloneMap(f.Env), Stdin: []byte(f.Stdin)}
}

func (f Fixture) ExpectedOutcome() (Outcome, bool) {
	if f.Expected == nil {
		return Outcome{}, false
	}
	return Outcome{ExitCode: f.Expected.ExitCode, Stdout: []byte(f.Expected.Stdout), Stderr: []byte(f.Expected.Stderr)}, true
}

func cloneMap(input map[string]string) map[string]string {
	if input == nil {
		return nil
	}
	output := make(map[string]string, len(input))
	for key, value := range input {
		output[key] = value
	}
	return output
}

// LoadFixtures decodes a fixture array and rejects unknown fields.
func LoadFixtures(path string) ([]Fixture, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	var fixtures []Fixture
	if err := decoder.Decode(&fixtures); err != nil {
		return nil, fmt.Errorf("decode parity fixtures %q: %w", path, err)
	}
	if len(fixtures) == 0 {
		return nil, fmt.Errorf("parity fixtures %q are empty", path)
	}
	seen := make(map[string]struct{}, len(fixtures))
	for index, fixture := range fixtures {
		if fixture.Name == "" {
			return nil, fmt.Errorf("parity fixture %d has no name", index)
		}
		if _, exists := seen[fixture.Name]; exists {
			return nil, fmt.Errorf("duplicate parity fixture name %q", fixture.Name)
		}
		seen[fixture.Name] = struct{}{}
	}
	return fixtures, nil
}

// Snapshot records regular files, directories, symlinks, modes, and contents
// in stable lexical order for filesystem side-effect comparisons.
type SnapshotEntry struct {
	Path    string
	Mode    fs.FileMode
	Link    string
	Content []byte
}

func Snapshot(root string) ([]SnapshotEntry, error) {
	var entries []SnapshotEntry
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == root {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		record := SnapshotEntry{Path: filepath.ToSlash(relative), Mode: info.Mode()}
		switch {
		case info.Mode()&os.ModeSymlink != 0:
			record.Link, err = os.Readlink(path)
		case info.Mode().IsRegular():
			record.Content, err = os.ReadFile(path)
		}
		if err != nil {
			return err
		}
		entries = append(entries, record)
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("snapshot %q: %w", root, err)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Path < entries[j].Path })
	return entries, nil
}

// RunSequence executes commands in order against one persistent sandbox.
func RunSequence(ctx context.Context, sandbox Sandbox, commands []Command) ([]Outcome, error) {
	outcomes := make([]Outcome, 0, len(commands))
	for index, command := range commands {
		outcome, err := Run(ctx, sandbox, command)
		if err != nil {
			return nil, fmt.Errorf("command %d: %w", index, err)
		}
		outcomes = append(outcomes, outcome)
	}
	return outcomes, nil
}

// PairSequence compares every process outcome and the resulting HOME/root
// filesystem trees. This is intended for init/refresh and local Git lifecycles.
func PairSequence(ctx context.Context, parent, name string, reference, subject []Command, seed func(Sandbox) error, substitutions ...Substitution) error {
	if len(reference) != len(subject) {
		return fmt.Errorf("sequence length differs: reference=%d subject=%d", len(reference), len(subject))
	}
	referenceSandbox, err := NewSandbox(parent, name+"-reference")
	if err != nil {
		return err
	}
	defer referenceSandbox.Cleanup()
	subjectSandbox, err := NewSandbox(parent, name+"-subject")
	if err != nil {
		return err
	}
	defer subjectSandbox.Cleanup()
	for _, sandbox := range []Sandbox{referenceSandbox, subjectSandbox} {
		if seed != nil {
			if err := seed(sandbox); err != nil {
				return fmt.Errorf("seed %q: %w", name, err)
			}
		}
	}
	referenceOutcomes, err := RunSequence(ctx, referenceSandbox, reference)
	if err != nil {
		return fmt.Errorf("reference: %w", err)
	}
	subjectOutcomes, err := RunSequence(ctx, subjectSandbox, subject)
	if err != nil {
		return fmt.Errorf("subject: %w", err)
	}
	referenceSubs := append(SandboxSubstitutions(referenceSandbox), substitutions...)
	subjectSubs := append(SandboxSubstitutions(subjectSandbox), substitutions...)
	for index := range referenceOutcomes {
		if err := Difference(Normalize(referenceOutcomes[index], referenceSubs...), Normalize(subjectOutcomes[index], subjectSubs...)); err != nil {
			return fmt.Errorf("command %d: %w", index, err)
		}
	}
	for _, tree := range []struct {
		name      string
		reference string
		subject   string
	}{
		{name: "HOME", reference: referenceSandbox.Home, subject: subjectSandbox.Home},
		{name: "root", reference: referenceSandbox.Root, subject: subjectSandbox.Root},
	} {
		want, err := Snapshot(tree.reference)
		if err != nil {
			return err
		}
		got, err := Snapshot(tree.subject)
		if err != nil {
			return err
		}
		want = normalizeSnapshot(want, referenceSubs)
		got = normalizeSnapshot(got, subjectSubs)
		if err := snapshotDifference(want, got); err != nil {
			return fmt.Errorf("%s filesystem: %w", tree.name, err)
		}
	}
	return nil
}

func normalizeSnapshot(entries []SnapshotEntry, substitutions []Substitution) []SnapshotEntry {
	result := make([]SnapshotEntry, len(entries))
	for index, entry := range entries {
		result[index] = entry
		result[index].Content = Normalize(Outcome{Stdout: entry.Content}, substitutions...).Stdout
		result[index].Link = string(Normalize(Outcome{Stdout: []byte(entry.Link)}, substitutions...).Stdout)
	}
	return result
}

func snapshotDifference(want, got []SnapshotEntry) error {
	if len(want) != len(got) {
		return fmt.Errorf("entry count differs: reference=%d subject=%d", len(want), len(got))
	}
	for index := range want {
		left, right := want[index], got[index]
		if left.Path != right.Path || left.Mode != right.Mode || left.Link != right.Link || !bytes.Equal(left.Content, right.Content) {
			return fmt.Errorf("entry %d differs: reference=%#v subject=%#v", index, left, right)
		}
	}
	return nil
}
