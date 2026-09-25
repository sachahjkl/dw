//go:build windows

package process

import (
	"context"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"

	"golang.org/x/sys/windows"
)

// systemExecutables never fall back to .cmd or .ps1 scripts found on PATH.
var systemExecutables = map[string]bool{
	"git":        true,
	"ssh":        true,
	"cmd":        true,
	"powershell": true,
	"pwsh":       true,
}

func appendPlatformCandidates(candidates []ResolvedCommand, fileName string, arguments []string) []ResolvedCommand {
	if strings.ContainsAny(fileName, `/\\`) || filepath.Ext(fileName) != "" || systemExecutables[strings.ToLower(fileName)] {
		return candidates
	}
	candidates = append(candidates, ResolvedCommand{
		FileName:  fileName + ".cmd",
		Arguments: cloneStrings(arguments),
		kind:      candidateCommandScript,
	})
	powershellArguments := []string{"-NoProfile", "-ExecutionPolicy", "Bypass", "-File", fileName + ".ps1"}
	powershellArguments = append(powershellArguments, arguments...)
	return append(candidates, ResolvedCommand{FileName: powerShellPath(), Arguments: powershellArguments, kind: candidatePowerShellScript})
}

func powerShellPath() string {
	systemDirectory, err := windows.GetSystemDirectory()
	if err != nil {
		return "powershell"
	}
	return filepath.Join(systemDirectory, "WindowsPowerShell", "v1.0", "powershell.exe")
}

func prepareCandidate(candidate ResolvedCommand) (ResolvedCommand, error) {
	if candidate.kind == candidatePowerShellScript && len(candidate.Arguments) >= 5 && candidate.Arguments[3] == "-File" {
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

// lookPath refuses executables resolved relative to the current directory (exec.ErrDot).
func lookPath(fileName string) (string, error) {
	resolved, err := exec.LookPath(fileName)
	if err != nil {
		return "", err
	}
	return resolved, nil
}

func executableCommand(ctx context.Context, candidate ResolvedCommand, hidden bool) *exec.Cmd {
	var command *exec.Cmd
	attributes := &syscall.SysProcAttr{}
	if hidden {
		attributes.CreationFlags = windows.CREATE_NO_WINDOW
		attributes.HideWindow = true
	}
	if candidate.kind != candidateCommandScript {
		command = exec.CommandContext(ctx, candidate.FileName, candidate.Arguments...)
	} else {
		systemDirectory, err := windows.GetSystemDirectory()
		if err != nil {
			command = exec.CommandContext(ctx, "cmd.exe")
		} else {
			command = exec.CommandContext(ctx, filepath.Join(systemDirectory, "cmd.exe"))
		}
		interpreter := command.Path
		attributes.CmdLine = `"` + interpreter + `" /d /s /c "` + batchCommandLine(candidate) + `"`
	}
	command.SysProcAttr = attributes
	return command
}

// batchCommandLine keeps metacharacters inside double quotes, neutralises percent expansion, and
// does not use CALL (which would perform a dangerous second expansion). Delayed expansion is
// disabled by default because /v is not supplied.
func batchCommandLine(candidate ResolvedCommand) string {
	var command strings.Builder
	appendBatchArgument(&command, candidate.FileName)
	for _, argument := range candidate.Arguments {
		command.WriteByte(' ')
		appendBatchArgument(&command, argument)
	}
	return command.String()
}

// appendBatchArgument writes a quoted argument. On a cmd /c command line "%%" is not an escape;
// "%%cd:~,%" yields a literal percent followed by an always-empty substring expansion, so no
// %NAME% sequence can reach the environment.
func appendBatchArgument(command *strings.Builder, value string) {
	command.WriteByte('"')
	for _, character := range value {
		switch character {
		case '"':
			command.WriteString(`""`)
		case '%':
			command.WriteString("%%cd:~,%")
		case '\r', '\n':
			command.WriteByte(' ')
		default:
			command.WriteRune(character)
		}
	}
	command.WriteByte('"')
}

func environmentNameEqual(left, right string) bool { return strings.EqualFold(left, right) }
