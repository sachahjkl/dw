package console

import "testing"

func TestPolicyInteractiveRequiresInputAndDiagnosticTerminals(t *testing.T) {
	for _, test := range []struct {
		stdin  bool
		stderr bool
		want   bool
	}{
		{stdin: true, stderr: true, want: true},
		{stdin: false, stderr: true, want: false},
		{stdin: true, stderr: false, want: false},
		{stdin: false, stderr: false, want: false},
	} {
		policy := Policy{Streams: Streams{StdinTTY: test.stdin, StderrTTY: test.stderr}}
		if got := policy.Interactive(); got != test.want {
			t.Fatalf("Interactive() with stdin=%t stderr=%t = %t, want %t", test.stdin, test.stderr, got, test.want)
		}
	}
}
