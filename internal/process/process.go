// Package process executes explicitly named programs without invoking a general-purpose shell.
package process

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"os/exec"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

// EnvironmentVariable is ordered so callers can preserve deterministic launch contracts.
type EnvironmentVariable struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

// Command is a direct executable invocation. Arguments are never interpolated by this package.
type Command struct {
	FileName         string                `json:"fileName"`
	Arguments        []string              `json:"arguments"`
	Environment      []EnvironmentVariable `json:"environment,omitempty"`
	WorkingDirectory string                `json:"workingDirectory,omitempty"`
	Input            []byte                `json:"-"`
}

// ResolvedCommand is one platform-specific direct launch candidate.
type ResolvedCommand struct {
	FileName  string   `json:"fileName"`
	Arguments []string `json:"arguments"`
	kind      candidateKind
}

// Result contains captured output. The original command line is intentionally absent so secrets
// can never appear through error or result formatting.
type Result struct {
	Stdout   []byte
	Stderr   []byte
	ExitCode int
}

// ExitError is returned only after a process starts and exits unsuccessfully.
type ExitError struct {
	FileName string
	Code     int
	Stderr   string
	cause    error
}

func (problem *ExitError) Message() l10n.Message {
	return l10n.M("process.exit-failed",
		l10n.A("command", problem.FileName),
		l10n.A("code", problem.Code),
		l10n.A("detail", problem.Stderr),
	)
}

func (problem *ExitError) Localized() l10n.Message { return problem.Message() }
func (problem *ExitError) Error() string           { return l10n.Render(problem.Message()) }

func (problem *ExitError) Unwrap() error { return problem.cause }

// StartError is returned when none of the platform candidates can be started.
type StartError struct {
	FileName string
	cause    error
}

func (problem *StartError) Message() l10n.Message {
	return l10n.M("process.start-failed",
		l10n.A("command", problem.FileName),
		l10n.A("detail", problem.cause),
	)
}

func (problem *StartError) Localized() l10n.Message { return problem.Message() }
func (problem *StartError) Error() string           { return l10n.Render(problem.Message()) }

func (problem *StartError) Unwrap() error { return problem.cause }

// CommandCandidates returns candidates in Rust-compatible order: the original executable, then a
// .cmd file, then a PowerShell .ps1 invocation when the platform supports script fallbacks.
func CommandCandidates(fileName string, arguments []string) []ResolvedCommand {
	args := cloneStrings(arguments)
	candidates := []ResolvedCommand{{FileName: fileName, Arguments: args, kind: candidateDirect}}
	return appendPlatformCandidates(candidates, fileName, arguments)
}

// Output executes a direct program and captures stdout and stderr.
func Output(ctx context.Context, command Command) (Result, error) {
	var lastNotFound error
	for _, candidate := range CommandCandidates(command.FileName, command.Arguments) {
		prepared, prepareErr := prepareCandidate(candidate)
		if prepareErr != nil {
			if isNotFound(prepareErr) {
				lastNotFound = prepareErr
				continue
			}
			return Result{ExitCode: -1}, &StartError{FileName: command.FileName, cause: prepareErr}
		}
		var stdout, stderr bytes.Buffer
		cmd := executableCommand(ctx, prepared)
		var stdin io.Reader
		if len(command.Input) != 0 {
			stdin = bytes.NewReader(command.Input)
		}
		configure(cmd, command, &stdout, &stderr, stdin)
		err := cmd.Run()
		if err == nil {
			return Result{Stdout: stdout.Bytes(), Stderr: stderr.Bytes(), ExitCode: 0}, nil
		}
		if isNotFound(err) {
			lastNotFound = err
			continue
		}
		code := -1
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			code = exit.ExitCode()
		}
		return Result{Stdout: stdout.Bytes(), Stderr: stderr.Bytes(), ExitCode: code}, &ExitError{
			FileName: command.FileName,
			Code:     code,
			Stderr:   stderr.String(),
			cause:    err,
		}
	}
	if lastNotFound == nil {
		lastNotFound = exec.ErrNotFound
	}
	return Result{ExitCode: -1}, &StartError{FileName: command.FileName, cause: lastNotFound}
}

// Run executes a direct program with inherited or caller-supplied streams.
func Run(ctx context.Context, command Command, stdin io.Reader, stdout, stderr io.Writer) error {
	var lastNotFound error
	for _, candidate := range CommandCandidates(command.FileName, command.Arguments) {
		prepared, prepareErr := prepareCandidate(candidate)
		if prepareErr != nil {
			if isNotFound(prepareErr) {
				lastNotFound = prepareErr
				continue
			}
			return &StartError{FileName: command.FileName, cause: prepareErr}
		}
		cmd := executableCommand(ctx, prepared)
		configure(cmd, command, stdout, stderr, stdin)
		err := cmd.Run()
		if err == nil {
			return nil
		}
		if isNotFound(err) {
			lastNotFound = err
			continue
		}
		code := -1
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			code = exit.ExitCode()
		}
		return &ExitError{FileName: command.FileName, Code: code, cause: err}
	}
	if lastNotFound == nil {
		lastNotFound = exec.ErrNotFound
	}
	return &StartError{FileName: command.FileName, cause: lastNotFound}
}

// Available reports whether the executable can run the supplied probe successfully.
func Available(ctx context.Context, fileName string, arguments ...string) bool {
	_, err := Output(ctx, Command{FileName: fileName, Arguments: arguments})
	return err == nil
}

func configure(cmd *exec.Cmd, command Command, stdout, stderr io.Writer, stdin io.Reader) {
	cmd.Dir = command.WorkingDirectory
	cmd.Stdin = stdin
	cmd.Stdout = stdout
	cmd.Stderr = stderr
	if len(command.Environment) == 0 {
		return
	}
	cmd.Env = append([]string(nil), os.Environ()...)
	for _, variable := range command.Environment {
		assignment := variable.Name + "=" + variable.Value
		replaced := false
		write := 0
		for _, existing := range cmd.Env {
			name, _, _ := strings.Cut(existing, "=")
			if environmentNameEqual(name, variable.Name) {
				if !replaced {
					cmd.Env[write] = assignment
					write++
					replaced = true
				}
				continue
			}
			cmd.Env[write] = existing
			write++
		}
		cmd.Env = cmd.Env[:write]
		if !replaced {
			cmd.Env = append(cmd.Env, assignment)
		}
	}
}

func isNotFound(err error) bool {
	return errors.Is(err, exec.ErrNotFound) || errors.Is(err, os.ErrNotExist)
}

func cloneStrings(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	return append([]string(nil), values...)
}

type candidateKind uint8

const (
	candidateDirect candidateKind = iota
	candidateCommandScript
)
