// Package agent defines deterministic agent selection, configuration, and direct launch contracts.
package agent

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
	dwprocess "github.com/sachahjkl/dw/internal/process"
)

type Agent string

const (
	Opencode Agent = "opencode"
	Cursor   Agent = "cursor"
	Claude   Agent = "claude"
	Codex    Agent = "codex"
	CodexCLI Agent = "codex-cli"
	Copilot  Agent = "copilot"

	DefaultAgent = Opencode
)

var All = [...]Agent{Opencode, Cursor, Claude, Codex, CodexCLI, Copilot}

type ParseError struct{ Value string }

func (problem *ParseError) Message() l10n.Message {
	return l10n.M("agent.unknown", l10n.A("value", problem.Value))
}
func (problem *ParseError) Localized() l10n.Message { return problem.Message() }
func (problem *ParseError) Error() string           { return l10n.Render(problem.Message()) }

func Parse(value string) (Agent, error) {
	candidate := strings.TrimSpace(value)
	for _, available := range All {
		if strings.EqualFold(candidate, string(available)) {
			return available, nil
		}
	}
	return "", &ParseError{Value: value}
}

// ResolveChoice preserves selection precedence: explicit, project default, workflow default,
// followed by opencode when no layer selected an agent.
func ResolveChoice(explicit *Agent, projectDefault, workflowDefault *string) (Agent, error) {
	if explicit != nil {
		return Parse(string(*explicit))
	}
	if projectDefault != nil {
		return Parse(*projectDefault)
	}
	if workflowDefault != nil {
		return Parse(*workflowDefault)
	}
	return DefaultAgent, nil
}

type EnvironmentVariable = dwprocess.EnvironmentVariable

type Launch struct {
	FileName         string
	Arguments        []string
	Environment      []EnvironmentVariable
	WorkingDirectory string
}

func (launch Launch) MarshalJSON() ([]byte, error) {
	var output bytes.Buffer
	output.WriteString(`{"fileName":`)
	fileName, err := json.Marshal(launch.FileName)
	if err != nil {
		return nil, err
	}
	output.Write(fileName)
	output.WriteString(`,"arguments":`)
	arguments, err := json.Marshal(launch.Arguments)
	if err != nil {
		return nil, err
	}
	output.Write(arguments)
	output.WriteString(`,"environment":{`)
	for index, variable := range launch.Environment {
		if index != 0 {
			output.WriteByte(',')
		}
		name, marshalErr := json.Marshal(variable.Name)
		if marshalErr != nil {
			return nil, marshalErr
		}
		value, marshalErr := json.Marshal(variable.Value)
		if marshalErr != nil {
			return nil, marshalErr
		}
		output.Write(name)
		output.WriteByte(':')
		output.Write(value)
	}
	output.WriteString(`},"workingDirectory":`)
	workingDirectory, err := json.Marshal(launch.WorkingDirectory)
	if err != nil {
		return nil, err
	}
	output.Write(workingDirectory)
	output.WriteByte('}')
	return output.Bytes(), nil
}

type OpenRequest struct {
	Root      string
	Workspace string
	Continue  bool
}

func BuildOpenLaunch(selected *Agent, request OpenRequest) Launch {
	choice := DefaultAgent
	if selected != nil {
		choice = *selected
	}
	workspace := request.Workspace
	launch := Launch{Arguments: []string{}, WorkingDirectory: workspace}
	switch choice {
	case Cursor:
		launch.FileName = "agent"
		launch.Arguments = []string{"--workspace", workspace}
		if request.Continue {
			launch.Arguments = append(launch.Arguments, "--continue")
		}
	case Claude:
		launch.FileName = "claude"
		if request.Continue {
			launch.Arguments = []string{"--continue"}
		}
	case Codex, CodexCLI:
		launch.FileName = "codex"
		if request.Continue {
			launch.Arguments = []string{"resume", "--last", "--cd", workspace}
		} else {
			launch.Arguments = []string{"--cd", workspace}
		}
	case Copilot:
		launch.FileName = "copilot"
		if request.Continue {
			launch.Arguments = []string{"--continue"}
		}
	default:
		launch.FileName = "opencode"
		if request.Continue {
			launch.Arguments = []string{"-c", workspace}
		} else {
			launch.Arguments = []string{workspace}
		}
		launch.Environment = []EnvironmentVariable{{
			Name:  "OPENCODE_CONFIG",
			Value: request.Root + "/config/opencode/opencode.jsonc",
		}}
	}
	return launch
}

// RunLaunch executes the launch directly through the typed process resolver, including Windows
// .cmd and .ps1 compatibility. It never uses a caller-provided shell command line.
func RunLaunch(ctx context.Context, launch Launch, stdin io.Reader, stdout, stderr io.Writer) error {
	return dwprocess.Run(ctx, dwprocess.Command{
		FileName:         launch.FileName,
		Arguments:        launch.Arguments,
		Environment:      launch.Environment,
		WorkingDirectory: launch.WorkingDirectory,
	}, stdin, stdout, stderr)
}

type WorkspaceWorkItemRef struct {
	ID    string  `json:"id"`
	Kind  *string `json:"kind,omitempty"`
	Title *string `json:"title,omitempty"`
}

type WorkspaceConfigRequest struct {
	Workspace string
	WorkItems []WorkspaceWorkItemRef
	Project   string
}

type WorkspaceConfigFile struct {
	RelativePath string `json:"relativePath"`
	Content      string `json:"content"`
}

func WorkspaceConfigFiles(request WorkspaceConfigRequest) []WorkspaceConfigFile {
	instructions := workspaceInstructions(request.WorkItems, request.Project)
	return []WorkspaceConfigFile{
		{RelativePath: "AGENTS.md", Content: instructions},
		{RelativePath: "CLAUDE.md", Content: instructions},
		{RelativePath: ".claude/CLAUDE.md", Content: instructions},
		{RelativePath: ".cursor/rules/devworkflow.mdc", Content: "---\nalwaysApply: true\n---\n\n" + instructions},
		{RelativePath: ".codex/config.toml", Content: l10n.Text("agent.codex-config")},
		{RelativePath: ".github/copilot-instructions.md", Content: instructions},
	}
}

type ConfigWriteError struct {
	Path  string
	cause error
}

func (problem *ConfigWriteError) Message() l10n.Message {
	return l10n.M("agent.config-write-failed", l10n.A("path", problem.Path), l10n.A("detail", problem.cause))
}
func (problem *ConfigWriteError) Localized() l10n.Message { return problem.Message() }
func (problem *ConfigWriteError) Error() string           { return l10n.Render(problem.Message()) }

func (problem *ConfigWriteError) Unwrap() error { return problem.cause }

func WriteWorkspaceConfigFiles(request WorkspaceConfigRequest) error {
	for _, file := range WorkspaceConfigFiles(request) {
		path := filepath.Join(request.Workspace, filepath.FromSlash(file.RelativePath))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			return &ConfigWriteError{Path: path, cause: err}
		}
		if err := os.WriteFile(path, []byte(file.Content), 0o644); err != nil {
			return &ConfigWriteError{Path: path, cause: err}
		}
	}
	return nil
}

type ContextReport struct {
	Root string `json:"root"`
}

func Context(root string) ContextReport { return ContextReport{Root: root} }

type DoctorCheck struct {
	Agent     Agent  `json:"agent"`
	Command   string `json:"command"`
	Available bool   `json:"available"`
}

type DoctorReport struct {
	Checks []DoctorCheck `json:"checks"`
}

func (report DoctorReport) AvailableCount() int {
	count := 0
	for _, check := range report.Checks {
		if check.Available {
			count++
		}
	}
	return count
}

func (report DoctorReport) TotalCount() int { return len(report.Checks) }
func (report DoctorReport) Passed() bool    { return report.AvailableCount() == report.TotalCount() }

func Doctor(ctx context.Context, requested *Agent) DoctorReport {
	agents := All[:]
	if requested != nil {
		agents = []Agent{*requested}
	}
	checks := make([]DoctorCheck, 0, len(agents))
	for _, selected := range agents {
		launch := BuildOpenLaunch(&selected, OpenRequest{Root: ".", Workspace: "."})
		checks = append(checks, DoctorCheck{
			Agent:     selected,
			Command:   launch.FileName,
			Available: dwprocess.Available(ctx, launch.FileName, "--help"),
		})
	}
	return DoctorReport{Checks: checks}
}

func workspaceInstructions(workItems []WorkspaceWorkItemRef, project string) string {
	items := make([]string, 0, len(workItems))
	for _, item := range workItems {
		suffix := ""
		if item.Kind != nil || item.Title != nil {
			kind := "?"
			if item.Kind != nil {
				kind = *item.Kind
			}
			title := ""
			if item.Title != nil {
				title = *item.Title
			}
			suffix = strings.TrimRight(" ["+kind+"] "+title, " ")
		}
		items = append(items, "  - `#"+item.ID+"`"+suffix)
	}
	return l10n.Render(l10n.M("agent.workspace-instructions",
		l10n.A("project", project),
		l10n.A("items", strings.Join(items, "\n")),
	))
}
