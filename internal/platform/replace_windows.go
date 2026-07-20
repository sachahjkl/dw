//go:build windows

package platform

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
)

const (
	cleanupMarkerEnv = "DW_UPDATE_CLEANUP"
	cleanupParentEnv = "DW_UPDATE_CLEANUP_PARENT"
	cleanupOldEnv    = "DW_UPDATE_CLEANUP_OLD"
)

var (
	kernel32            = syscall.NewLazyDLL("kernel32.dll")
	openProcess         = kernel32.NewProc("OpenProcess")
	waitForSingleObject = kernel32.NewProc("WaitForSingleObject")
	closeHandle         = kernel32.NewProc("CloseHandle")
)

// CleanupExitCode performs the cleanup half of deferred Windows replacement.
// The bool is false during normal execution; when true the caller must return
// the code from its single process-exit boundary.
func CleanupExitCode() (code int, cleanup bool) {
	if os.Getenv(cleanupMarkerEnv) != "1" {
		return 0, false
	}
	parentText := os.Getenv(cleanupParentEnv)
	parent, err := strconv.ParseUint(parentText, 10, 32)
	if err != nil || parent == 0 {
		return 2, true
	}
	old, valid := validatedCleanupTarget(parentText)
	if !valid {
		return 2, true
	}
	waitForProcess(uint32(parent))
	for attempt := 0; attempt < 100; attempt++ {
		if err := os.Remove(old); err == nil || os.IsNotExist(err) {
			return 0, true
		}
		time.Sleep(50 * time.Millisecond)
	}
	return 1, true
}

func validatedCleanupTarget(parent string) (string, bool) {
	old, err := filepath.Abs(os.Getenv(cleanupOldEnv))
	if err != nil {
		return "", false
	}
	executable, err := os.Executable()
	if err != nil {
		return "", false
	}
	executable, err = filepath.Abs(executable)
	if err != nil || !strings.EqualFold(filepath.Dir(old), filepath.Dir(executable)) {
		return "", false
	}
	stem := strings.TrimSuffix(filepath.Base(executable), filepath.Ext(executable))
	prefix := "." + stem + "." + parent + "-"
	name := filepath.Base(old)
	const suffix = ".__relocated__.exe"
	if len(name) < len(prefix)+len(suffix) || !strings.EqualFold(name[:len(prefix)], prefix) || !strings.EqualFold(name[len(name)-len(suffix):], suffix) {
		return "", false
	}
	nonce := name[len(prefix) : len(name)-len(suffix)]
	if nonce == "" {
		return "", false
	}
	for index := range len(nonce) {
		if nonce[index] < '0' || nonce[index] > '9' {
			return "", false
		}
	}
	return old, true
}

func waitForProcess(pid uint32) {
	const (
		synchronize = 0x00100000
		infinite    = 0xffffffff
	)
	handle, _, _ := openProcess.Call(synchronize, 0, uintptr(pid))
	if handle == 0 {
		return
	}
	_, _, _ = waitForSingleObject.Call(handle, infinite)
	_, _, _ = closeHandle.Call(handle)
}

// ReplaceExecutable moves the running executable aside, installs replacement,
// and starts the installed binary in cleanup mode. Windows permits renaming a
// running executable but keeps the relocated file locked until this process exits.
func ReplaceExecutable(executable, replacement string) (Replacement, error) {
	target, err := filepath.EvalSymlinks(executable)
	if err != nil {
		return Replacement{}, fmt.Errorf("platform: resolve-executable: %w", err)
	}
	staged, err := copyWindowsBeside(target, replacement)
	if err != nil {
		return Replacement{}, err
	}
	old := filepath.Join(filepath.Dir(target), fmt.Sprintf(".%s.%d-%d.__relocated__.exe", strings.TrimSuffix(filepath.Base(target), filepath.Ext(target)), os.Getpid(), time.Now().UnixNano()))
	if err := os.Rename(target, old); err != nil {
		_ = os.Remove(staged)
		return Replacement{}, fmt.Errorf("platform: relocate-locked-executable: %w", err)
	}
	if err := os.Rename(staged, target); err != nil {
		_ = os.Rename(old, target)
		_ = os.Remove(staged)
		return Replacement{}, fmt.Errorf("platform: install-replacement: %w", err)
	}
	if err := startCleanupProcess(target, old); err != nil {
		_ = os.Remove(target)
		_ = os.Rename(old, target)
		return Replacement{}, fmt.Errorf("platform: start-deferred-cleanup: %w", err)
	}
	_ = os.Remove(replacement)
	return Replacement{ExecutablePath: executable, DeferredWindowsReplacement: true}, nil
}

func copyWindowsBeside(target, source string) (staged string, resultErr error) {
	input, err := os.Open(source)
	if err != nil {
		return "", fmt.Errorf("platform: open-replacement: %w", err)
	}
	defer input.Close()
	output, err := os.CreateTemp(filepath.Dir(target), ".dw.__temp__.*.exe")
	if err != nil {
		return "", fmt.Errorf("platform: create-staged-executable: %w", err)
	}
	staged = output.Name()
	defer func() {
		if resultErr != nil {
			_ = output.Close()
			_ = os.Remove(staged)
		}
	}()
	if _, err := io.Copy(output, input); err != nil {
		return "", fmt.Errorf("platform: copy-replacement: %w", err)
	}
	if err := output.Sync(); err != nil {
		return "", fmt.Errorf("platform: sync-replacement: %w", err)
	}
	if err := output.Close(); err != nil {
		return "", fmt.Errorf("platform: close-replacement: %w", err)
	}
	return staged, nil
}

func startCleanupProcess(executable, old string) error {
	command := exec.Command(executable)
	environment := os.Environ()
	filtered := environment[:0]
	for _, variable := range environment {
		name, _, _ := strings.Cut(variable, "=")
		if strings.EqualFold(name, cleanupMarkerEnv) || strings.EqualFold(name, cleanupParentEnv) || strings.EqualFold(name, cleanupOldEnv) {
			continue
		}
		filtered = append(filtered, variable)
	}
	command.Env = append(filtered,
		cleanupMarkerEnv+"=1",
		cleanupParentEnv+"="+strconv.Itoa(os.Getpid()),
		cleanupOldEnv+"="+old,
	)
	if err := command.Start(); err != nil {
		return err
	}
	_ = command.Process.Release()
	return nil
}
