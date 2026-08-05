//go:build windows

package process

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"golang.org/x/sys/windows"
)

func TestPrepareCandidateSkipsExtensionlessWindowsShim(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "tool"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "tool.cmd"), []byte("@exit /b 0\r\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", directory)

	candidates := CommandCandidates("tool", []string{"--version"})
	if _, err := prepareCandidate(candidates[0]); !errors.Is(err, exec.ErrNotFound) {
		t.Fatalf("extensionless candidate error = %v, want executable not found", err)
	}
	prepared, err := prepareCandidate(candidates[1])
	if err != nil {
		t.Fatalf("prepare .cmd candidate: %v", err)
	}
	if filepath.Ext(prepared.FileName) != ".cmd" {
		t.Fatalf("prepared candidate = %q, want .cmd", prepared.FileName)
	}
}

func TestExecutableCommandDoesNotCreateWindow(t *testing.T) {
	command := executableCommand(context.Background(), ResolvedCommand{FileName: "tool.exe"}, true)
	if command.SysProcAttr == nil || command.SysProcAttr.CreationFlags&windows.CREATE_NO_WINDOW == 0 || !command.SysProcAttr.HideWindow {
		t.Fatalf("Windows process attributes = %#v, want hidden process without console", command.SysProcAttr)
	}
}

func TestInteractiveExecutableCommandUsesCurrentConsole(t *testing.T) {
	command := executableCommand(context.Background(), ResolvedCommand{FileName: "tool.exe"}, false)
	if command.SysProcAttr == nil || command.SysProcAttr.CreationFlags&windows.CREATE_NO_WINDOW != 0 || command.SysProcAttr.HideWindow {
		t.Fatalf("Windows process attributes = %#v, want current console", command.SysProcAttr)
	}
}

func TestCommandShimIgnoresComSpecOverride(t *testing.T) {
	t.Setenv("ComSpec", `C:\attacker\cmd.exe`)
	command := executableCommand(context.Background(), ResolvedCommand{FileName: `C:\tool.cmd`, kind: candidateCommandScript}, true)
	if command.Path == `C:\attacker\cmd.exe` || !filepath.IsAbs(command.Path) {
		t.Fatalf("command interpreter = %q", command.Path)
	}
}

func TestOutputFallsBackToCommandShim(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "tool"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "tool.cmd"), []byte("@echo 1.2.3\r\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", directory+string(os.PathListSeparator)+filepath.Join(os.Getenv("SystemRoot"), "System32"))

	result, err := Output(context.Background(), Command{FileName: "tool", Arguments: []string{"--version"}})
	if err != nil {
		t.Fatalf("execute .cmd fallback: %v", err)
	}
	if string(result.Stdout) != "1.2.3\r\n" {
		t.Fatalf("stdout = %q, want version", result.Stdout)
	}
}

func TestCommandShimDoesNotInterpretArgumentMetacharacters(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "tool.cmd"), []byte("@exit /b 0\r\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	injected := filepath.Join(directory, "injected.txt")
	t.Setenv("PATH", directory+string(os.PathListSeparator)+filepath.Join(os.Getenv("SystemRoot"), "System32"))

	argument := `value"&echo injected>"` + injected
	if _, err := Output(context.Background(), Command{FileName: "tool", Arguments: []string{argument}}); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(injected); !os.IsNotExist(err) {
		t.Fatalf("argument was interpreted as a command: %v", err)
	}
}

func TestOutputFallsBackToPowerShellShim(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "tool"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "tool.ps1"), []byte("'4.5.6'\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	powerShellDirectory := filepath.Join(os.Getenv("SystemRoot"), "System32", "WindowsPowerShell", "v1.0")
	t.Setenv("PATH", directory+string(os.PathListSeparator)+powerShellDirectory)

	result, err := Output(context.Background(), Command{FileName: "tool", Arguments: []string{"--version"}})
	if err != nil {
		t.Fatalf("execute .ps1 fallback: %v", err)
	}
	if string(result.Stdout) != "4.5.6\r\n" {
		t.Fatalf("stdout = %q, want version", result.Stdout)
	}
}
