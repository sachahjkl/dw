//go:build windows

package process

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"

	"golang.org/x/sys/windows"
)

func appendPlatformCandidates(candidates []ResolvedCommand, fileName string, arguments []string) []ResolvedCommand {
	if strings.ContainsAny(fileName, `/\\`) || filepath.Ext(fileName) != "" {
		return candidates
	}
	candidates = append(candidates, ResolvedCommand{
		FileName:  fileName + ".cmd",
		Arguments: cloneStrings(arguments),
		kind:      candidateCommandScript,
	})
	powershellArguments := []string{"-NoProfile", "-ExecutionPolicy", "Bypass", "-File", fileName + ".ps1"}
	powershellArguments = append(powershellArguments, arguments...)
	return append(candidates, ResolvedCommand{FileName: "powershell", Arguments: powershellArguments})
}

func prepareCandidate(candidate ResolvedCommand) (ResolvedCommand, error) {
	if strings.EqualFold(filepath.Base(candidate.FileName), "powershell") && len(candidate.Arguments) >= 5 && candidate.Arguments[3] == "-File" {
		script, err := lookPath(candidate.Arguments[4])
		if err != nil {
			return ResolvedCommand{}, err
		}
		candidate.Arguments = cloneStrings(candidate.Arguments)
		candidate.Arguments[4] = script
	}
	if candidate.kind == candidateDirect && filepath.Ext(candidate.FileName) == "" {
		if executable, err := lookPath(candidate.FileName + ".exe"); err == nil {
			candidate.FileName = executable
			return candidate, nil
		}
	}
	resolved, err := lookPath(candidate.FileName)
	if err != nil {
		return ResolvedCommand{}, err
	}
	extension := strings.ToLower(filepath.Ext(resolved))
	if candidate.kind == candidateDirect && filepath.Ext(candidate.FileName) == "" {
		switch extension {
		case ".exe", ".com":
		default:
			return ResolvedCommand{}, exec.ErrNotFound
		}
	}
	if candidate.kind == candidateDirect && (extension == ".cmd" || extension == ".bat") {
		candidate.kind = candidateCommandScript
	}
	candidate.FileName = resolved
	return candidate, nil
}

func lookPath(fileName string) (string, error) {
	resolved, err := exec.LookPath(fileName)
	if err == nil || errors.Is(err, exec.ErrDot) && resolved != "" {
		return resolved, nil
	}
	return "", err
}

func executableCommand(ctx context.Context, candidate ResolvedCommand) *exec.Cmd {
	var command *exec.Cmd
	attributes := &syscall.SysProcAttr{CreationFlags: windows.CREATE_NO_WINDOW, HideWindow: true}
	if candidate.kind != candidateCommandScript {
		command = exec.CommandContext(ctx, candidate.FileName, candidate.Arguments...)
	} else {
		interpreter := os.Getenv("ComSpec")
		if interpreter == "" {
			interpreter = "cmd.exe"
		}
		command = exec.CommandContext(ctx, interpreter)
		attributes.CmdLine = `"` + interpreter + `" /d /s /c "` + batchCommandLine(candidate) + `"`
	}
	command.SysProcAttr = attributes
	return command
}

// batchCommandLine keeps metacharacters inside double quotes, disables percent expansion, and does
// not use CALL (which would perform a dangerous second expansion). Delayed expansion is disabled by
// default because /v is not supplied.
func batchCommandLine(candidate ResolvedCommand) string {
	var command strings.Builder
	appendBatchArgument(&command, candidate.FileName)
	for _, argument := range candidate.Arguments {
		command.WriteByte(' ')
		appendBatchArgument(&command, argument)
	}
	return command.String()
}

func appendBatchArgument(command *strings.Builder, value string) {
	command.WriteByte('"')
	for _, character := range value {
		switch character {
		case '"':
			command.WriteString(`""`)
		case '%':
			command.WriteString("%%")
		case '\r', '\n':
			command.WriteByte(' ')
		default:
			command.WriteRune(character)
		}
	}
	command.WriteByte('"')
}

func environmentNameEqual(left, right string) bool { return strings.EqualFold(left, right) }
