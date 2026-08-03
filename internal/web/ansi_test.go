package web

import (
	"context"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/execution"
)

func TestANSIResultIsStyledAndEscaped(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001",
		Title: "Doctor", Status: execution.StatusSucceeded, StatusLabel: "Completed",
		Result: ansiToSpans("\x1b[1;94mDoctor\x1b[0m\n\x1b[96mRoot\x1b[0m  <script>alert(1)</script>"),
	}
	html, err := renderComponent(context.Background(), executionsSection([]executionView{item}, 0))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`class="ansi-bold ansi-fg-bright-4"`, `class="ansi-fg-bright-6"`, `&lt;script&gt;alert(1)&lt;/script&gt;`} {
		if !strings.Contains(html, marker) {
			t.Errorf("ANSI marker %q is missing: %s", marker, html)
		}
	}
	if strings.Contains(html, "\x1b[") || strings.Contains(html, "<script>alert") {
		t.Fatalf("unsafe terminal output rendered: %s", html)
	}
}
