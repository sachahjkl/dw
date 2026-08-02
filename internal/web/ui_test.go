package web

import (
	"context"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/execution"
)

func TestTemplRendersEveryExecutionStatus(t *testing.T) {
	statuses := []execution.Status{
		execution.StatusQueued,
		execution.StatusRunning,
		execution.StatusWaitingInput,
		execution.StatusCanceling,
		execution.StatusCanceled,
		execution.StatusSucceeded,
		execution.StatusFailed,
		execution.StatusInterrupted,
	}
	items := make([]executionView, 0, len(statuses))
	for _, status := range statuses {
		items = append(items, executionView{ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001", ActionID: "test.action", Status: status, Cancel: "@post('/cancel')"})
	}
	html, err := renderComponent(context.Background(), executionsSection(items))
	if err != nil {
		t.Fatal(err)
	}
	for _, status := range statuses {
		if !strings.Contains(html, ">"+string(status)+"<") {
			t.Errorf("status %q was not rendered", status)
		}
	}
}

func TestTemplRendersStrictPromptControls(t *testing.T) {
	prompts := []promptView{
		{Kind: "text", Label: "Text", Signal: "text", Submit: "@post('/text')"},
		{Kind: "secret", Label: "Secret", Required: true, Submit: "@post('/secret')"},
		{Kind: "confirm", Label: "Confirm", Signal: "confirm", Submit: "@post('/confirm')"},
		{Kind: "select-one", Label: "One", Signal: "one", Submit: "@post('/one')", Choices: []choiceView{{Value: "a", Label: "A"}}},
		{Kind: "select-many", Label: "Many", Signal: "many", Submit: "@post('/many')", Choices: []choiceView{{Value: "a", Label: "A"}}},
	}
	var html strings.Builder
	for _, prompt := range prompts {
		component, err := renderComponent(context.Background(), promptForm("01J00000000000000000000000", prompt))
		if err != nil {
			t.Fatal(err)
		}
		html.WriteString(component)
	}
	value := html.String()
	for _, marker := range []string{`type="text"`, `type="password"`, `type="checkbox"`, `<select`, `> A</label>`} {
		if !strings.Contains(value, marker) {
			t.Errorf("prompt control %q was not rendered", marker)
		}
	}
	secretStart := strings.Index(value, `type="password"`)
	if secretStart < 0 {
		t.Fatal("secret input was not rendered")
	}
	secretEnd := strings.Index(value[secretStart:], "</form>")
	if secretEnd < 0 {
		t.Fatal("secret form did not close")
	}
	if strings.Contains(value[secretStart:secretStart+secretEnd], "data-bind") {
		t.Fatal("secret value was attached to a Datastar signal")
	}
}

func TestExecutionViewRendersInteractiveBrowserLogin(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001",
		ActionID: "provider.auth.login", Status: execution.StatusRunning,
		Events: []eventView{{
			Sequence: 3, Kind: execution.EventProgress, Message: "work.event.browser-login-required",
			AuthorizationURL: "https://login.microsoftonline.com/tenant/oauth2/v2.0/authorize?state=secret",
			CallbackURI:      "http://localhost:43210",
		}},
	}
	html, err := renderComponent(context.Background(), executionsSection([]executionView{item}))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`class="auth-panel"`, `target="_blank"`, `rel="noopener noreferrer"`, `Continue with Microsoft`, `http://localhost:43210`} {
		if !strings.Contains(html, marker) {
			t.Errorf("browser login marker %q was not rendered", marker)
		}
	}
}

func TestVendoredDatastarVersionAndChecksum(t *testing.T) {
	bundle, err := assets.ReadFile("assets/datastar.js")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(string(bundle), "// Datastar v1.0.2") {
		t.Fatal("unexpected Datastar bundle version")
	}
	checksum, err := assets.ReadFile("assets/datastar.js.sha256")
	if err != nil {
		t.Fatal(err)
	}
	if string(checksum) != "2837d87acf6ee0ba8e4e63765926c25a98d63883b02f88be194a86b81d3fd24a  datastar.js\n" {
		t.Fatal("unexpected Datastar checksum")
	}
}
