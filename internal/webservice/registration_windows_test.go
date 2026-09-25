//go:build windows

package webservice

import "testing"

func TestScheduledTaskCommandRejectsQuotes(t *testing.T) {
	command, err := scheduledTaskCommand(`C:\Program Files\dw\dw.exe`)
	if err != nil || command != `"C:\Program Files\dw\dw.exe" web serve` {
		t.Fatalf("command = %q, err = %v", command, err)
	}
	for _, executable := range []string{``, `C:\dw" & calc.exe & "`, "C:\\dw\r\n.exe"} {
		if _, err := scheduledTaskCommand(executable); err == nil {
			t.Fatalf("executable %q was accepted", executable)
		}
	}
}

func TestTaskStateRunning(t *testing.T) {
	if !taskStateRunning("Running\r\n") || taskStateRunning("Ready\r\n") || taskStateRunning("") {
		t.Fatal("unexpected task state parsing")
	}
}

func TestDecodeCodePage(t *testing.T) {
	if got := decodeCodePage(850, []byte{'e', 'r', 'r', 'e', 'u', 'r', ' ', 0x82}); got != "erreur é" {
		t.Fatalf("decoded = %q", got)
	}
}
