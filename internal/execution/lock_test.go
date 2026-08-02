package execution

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

func TestCanonicalRootResolvesExistingLinks(t *testing.T) {
	realRoot := filepath.Join(t.TempDir(), "real")
	if err := os.Mkdir(realRoot, 0o700); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(t.TempDir(), "link")
	if err := os.Symlink(realRoot, link); err != nil {
		t.Skipf("symbolic links are unavailable: %v", err)
	}
	canonical, err := CanonicalRoot(filepath.Join(link, "missing"))
	if err != nil {
		t.Fatal(err)
	}
	resolvedRoot, err := filepath.EvalSymlinks(realRoot)
	if err != nil {
		t.Fatal(err)
	}
	want := normalizePlatformRoot(filepath.Join(resolvedRoot, "missing"))
	if canonical != want {
		t.Fatalf("canonical root = %q, want %q", canonical, want)
	}
}

func TestRootLockerObservesCancellation(t *testing.T) {
	locker, err := NewRootLocker(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	first, err := locker.Acquire(context.Background(), LockSpec{Mode: LockExclusive, Key: root})
	if err != nil {
		t.Fatal(err)
	}
	defer first.Release()
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	_, err = locker.Acquire(ctx, LockSpec{Mode: LockExclusive, Key: root})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("Acquire error = %v, want deadline exceeded", err)
	}
}

func TestRootLockerCoordinatesProcesses(t *testing.T) {
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	root := t.TempDir()
	first := startLockHelper(t, executable, directory, root)
	defer first.stop(t)
	first.waitLocked(t, time.Second)

	second := startLockHelper(t, executable, directory, root)
	defer second.stop(t)
	if line, ok := second.readLine(150 * time.Millisecond); ok {
		t.Fatalf("second process acquired early: %q", line)
	}
	first.release(t)
	second.waitLocked(t, time.Second)
	second.release(t)
}

func TestRootLockerProcessHelper(t *testing.T) {
	if os.Getenv("DW_LOCK_HELPER") != "1" {
		return
	}
	locker, err := NewRootLocker(os.Getenv("DW_LOCK_DIRECTORY"))
	if err != nil {
		t.Fatal(err)
	}
	handle, err := locker.Acquire(context.Background(), LockSpec{Mode: LockExclusive, Key: os.Getenv("DW_LOCK_ROOT")})
	if err != nil {
		t.Fatal(err)
	}
	fmt.Println("locked")
	if _, err := bufio.NewReader(os.Stdin).ReadString('\n'); err != nil {
		t.Fatal(err)
	}
	if err := handle.Release(); err != nil {
		t.Fatal(err)
	}
}

type lockHelper struct {
	command *exec.Cmd
	input   *os.File
	lines   chan string
	errors  chan error
}

func startLockHelper(t *testing.T, executable, directory, root string) *lockHelper {
	t.Helper()
	command := exec.Command(executable, "-test.run=^TestRootLockerProcessHelper$")
	command.Env = append(os.Environ(), "DW_LOCK_HELPER=1", "DW_LOCK_DIRECTORY="+directory, "DW_LOCK_ROOT="+root)
	stdin, err := command.StdinPipe()
	if err != nil {
		t.Fatal(err)
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	command.Stderr = os.Stderr
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	helper := &lockHelper{command: command, input: stdin.(*os.File), lines: make(chan string, 1), errors: make(chan error, 1)}
	go func() {
		line, readErr := bufio.NewReader(stdout).ReadString('\n')
		if readErr != nil {
			helper.errors <- readErr
			return
		}
		helper.lines <- line
	}()
	return helper
}

func (helper *lockHelper) readLine(timeout time.Duration) (string, bool) {
	timer := time.NewTimer(timeout)
	defer timer.Stop()
	select {
	case line := <-helper.lines:
		return line, true
	case <-helper.errors:
		return "", false
	case <-timer.C:
		return "", false
	}
}

func (helper *lockHelper) waitLocked(t *testing.T, timeout time.Duration) {
	t.Helper()
	line, ok := helper.readLine(timeout)
	if !ok || line != "locked\n" {
		t.Fatalf("helper output = %q, received=%t", line, ok)
	}
}

func (helper *lockHelper) release(t *testing.T) {
	t.Helper()
	if _, err := helper.input.WriteString("release\n"); err != nil {
		t.Fatal(err)
	}
	if err := helper.command.Wait(); err != nil {
		t.Fatal(err)
	}
}

func (helper *lockHelper) stop(t *testing.T) {
	t.Helper()
	if helper.command.ProcessState != nil {
		return
	}
	_ = helper.input.Close()
	_ = helper.command.Process.Kill()
	_ = helper.command.Wait()
}
