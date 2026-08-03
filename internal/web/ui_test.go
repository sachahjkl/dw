package web

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/workapp"
)

type finalReplayExecutor struct {
	execution.Executor
	events []execution.Event
}

func (executor finalReplayExecutor) Subscribe(context.Context, execution.Actor, execution.ExecutionID, execution.EventSequence) (execution.Subscription, error) {
	events := make(chan execution.Event)
	errors := make(chan error)
	go func() {
		defer close(events)
		defer close(errors)
		for _, event := range executor.events {
			time.Sleep(3 * time.Millisecond)
			events <- event
		}
	}()
	return execution.Subscription{Events: events, Errors: errors}, nil
}

func (executor finalReplayExecutor) Get(context.Context, execution.Actor, execution.ExecutionID) (execution.Record, error) {
	return execution.Record{Status: execution.StatusSucceeded}, nil
}

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
		items = append(items, executionView{ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001", Title: "Test action", Status: status, StatusLabel: statusLabel(status), Cancel: "/cancel"})
	}
	html, err := renderComponent(context.Background(), executionsSection(items, 4))
	if err != nil {
		t.Fatal(err)
	}
	for _, status := range statuses {
		if !strings.Contains(html, ">"+statusLabel(status)+"<") {
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
	for _, marker := range []string{`type="text"`, `type="password"`, `type="checkbox"`, `<select`, `>A</span>`} {
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
		Title: "Sign in to a work provider", Status: execution.StatusRunning, StatusLabel: "Running", Active: true,
		Events: []eventView{{
			Sequence: 3, Kind: execution.EventProgress, Message: "work.event.browser-login-required",
			AuthorizationURL: "https://login.microsoftonline.com/tenant/oauth2/v2.0/authorize?state=secret",
			CallbackURI:      "http://localhost:43210",
		}},
	}
	html, err := renderComponent(context.Background(), executionsSection([]executionView{item}, 1))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`class="auth-panel"`, `target="_blank"`, `rel="noopener noreferrer"`, `Continue with Microsoft`, `http://localhost:43210`} {
		if !strings.Contains(html, marker) {
			t.Errorf("browser login marker %q was not rendered", marker)
		}
	}
}

func TestExecutionViewRendersRemoteDeviceLogin(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001",
		Title: "Sign in to a work provider", Status: execution.StatusRunning, StatusLabel: "Running", Active: true,
		Events: []eventView{{
			Sequence: 4, Kind: execution.EventProgress, Message: "work.event.device-login-required",
			VerificationURI: "https://microsoft.com/devicelogin", UserCode: "ABCD-EFGH",
		}},
	}
	html, err := renderComponent(context.Background(), executionsSection([]executionView{item}, 1))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`Open Microsoft sign-in`, `https://microsoft.com/devicelogin`, `ABCD-EFGH`} {
		if !strings.Contains(html, marker) {
			t.Errorf("device login marker %q was not rendered", marker)
		}
	}
	if strings.Contains(html, "localhost") {
		t.Fatalf("device login rendered a loopback callback: %s", html)
	}
}

func TestWorkSectionGuidesObviousRecovery(t *testing.T) {
	tests := []struct {
		name    string
		problem string
		markers []string
	}{
		{
			name:    "credential store",
			problem: "OS credential storage unavailable: secret.store-unavailable",
			markers: []string{`OS credential store unavailable`, `Start a Secret Service provider`, `GNOME Keyring`, `KeePassXC`},
		},
		{
			name:    "provider sign-in",
			problem: "ado.error:missing-auth",
			markers: []string{`Connect Azure DevOps`, `Use Sign in above`, `work items will refresh`},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			view := pageView{Work: []cockpit.WorkProject{{
				Label: "default", Provider: "azure-devops", Error: test.problem,
			}}}
			html, err := renderComponent(context.Background(), workSection(view))
			if err != nil {
				t.Fatal(err)
			}
			markers := append([]string{`class="guided-error"`, `<summary>Technical details</summary>`}, test.markers...)
			for _, marker := range markers {
				if !strings.Contains(html, marker) {
					t.Errorf("recovery marker %q was not rendered", marker)
				}
			}
		})
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
	fields := strings.Fields(string(checksum))
	if len(fields) != 2 || fields[1] != "datastar.js" {
		t.Fatalf("unexpected Datastar checksum file: %q", checksum)
	}
	digest := sha256.Sum256(bundle)
	if actual := hex.EncodeToString(digest[:]); actual != fields[0] {
		t.Fatalf("Datastar checksum = %s, want %s", actual, fields[0])
	}
}

func TestAssetsUseContentVersionedCache(t *testing.T) {
	server := &Server{}
	unversioned := httptest.NewRequest(http.MethodGet, "/assets/app.css", nil)
	unversioned.SetPathValue("name", "app.css")
	unversionedResponse := httptest.NewRecorder()
	server.handleAsset(unversionedResponse, unversioned)
	if got := unversionedResponse.Header().Get("Cache-Control"); got != "no-cache" {
		t.Fatalf("unversioned cache policy = %q", got)
	}

	versioned := httptest.NewRequest(http.MethodGet, assetURL("app.css"), nil)
	versioned.SetPathValue("name", "app.css")
	versionedResponse := httptest.NewRecorder()
	server.handleAsset(versionedResponse, versioned)
	if got := versionedResponse.Header().Get("Cache-Control"); got != "public, max-age=31536000, immutable" {
		t.Fatalf("versioned cache policy = %q", got)
	}
}

func TestActionToastsReportActiveActionsAndPrompts(t *testing.T) {
	prompt := promptView{}
	now := time.Date(2026, 8, 3, 12, 0, 0, 0, time.UTC)
	toasts := actionToasts([]executionView{
		{Title: "Sign in", Events: []eventView{{AuthorizationURL: "https://login.example.test"}}},
		{Title: "Refresh work", Active: true},
		{Title: "Confirm deletion", Prompt: &prompt},
		{Title: "Doctor", Status: execution.StatusSucceeded, FinishedAt: timePointer(now.Add(-time.Second)), Result: ansiToSpans("Doctor report\nPassed 6/6")},
		{Title: "Refresh", Status: execution.StatusFailed, FinishedAt: timePointer(now.Add(-time.Second)), Failure: "Network unavailable"},
		{Title: "Old action", Status: execution.StatusSucceeded, FinishedAt: timePointer(now.Add(-time.Minute)), Result: ansiToSpans("Old result")},
	}, now)
	if len(toasts) != 4 || toasts[0].Title != "Action running" || toasts[1].Title != "Input required" || toasts[2].Title != "Action completed" || toasts[2].Detail != "Doctor report" || toasts[3].Title != "Action failed" {
		t.Fatalf("toasts = %#v", toasts)
	}
}

func timePointer(value time.Time) *time.Time { return &value }

func TestExecutionViewRendersProjectedResult(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001",
		Title: "Doctor", Status: execution.StatusSucceeded, StatusLabel: "Completed",
		Result: ansiToSpans("Doctor\nRoot  S:\\dw\nPassed  6/6"),
	}
	html, err := renderComponent(context.Background(), liveSections(pageView{Executions: []executionView{item}}))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`class="view-result"`, `class="result-dialog"`, `class="terminal-result"`, `Root  S:\dw`, `Passed  6/6`} {
		if !strings.Contains(html, marker) {
			t.Errorf("result marker %q was not rendered: %s", marker, html)
		}
	}
}

func TestExecutionViewRendersStructuredResultDialog(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", AttemptID: "01J00000000000000000000001",
		Title: "Doctor", Status: execution.StatusSucceeded, StatusLabel: "Completed",
		ResultPage: &resultPageView{
			Title: "Doctor", Badge: "Passed", Status: "success",
			Summary:  []resultFieldView{{Label: "Root", Value: `S:\dw`, Style: "path"}},
			Sections: []resultSectionView{{Table: &resultTableView{Columns: []string{"Check", "Status"}, Rows: [][]string{{"Git", "Passed"}}}}},
		},
	}
	html, err := renderComponent(context.Background(), liveSections(pageView{Executions: []executionView{item}}))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`class="view-result"`, `<dialog id="result-01J`, `class="structured-result"`, `class="result-table"`, `>Git<`, `>Passed<`} {
		if !strings.Contains(html, marker) {
			t.Errorf("structured result marker %q was not rendered: %s", marker, html)
		}
	}
	if strings.Contains(html, `class="terminal-result"`) {
		t.Fatalf("structured result used terminal fallback: %s", html)
	}
}

func TestLivePageExposesIndependentDatastarPatchTargets(t *testing.T) {
	html, err := renderComponent(context.Background(), page(pageView{}))
	if err != nil {
		t.Fatal(err)
	}
	for _, id := range []string{`id="tab-actions"`, `id="action-notifications"`, `id="resource-sections"`, `id="actions"`, `id="action-results"`} {
		if !strings.Contains(html, id) {
			t.Errorf("Datastar patch target %s is missing", id)
		}
	}
}

func TestStructuredResultRendersSemanticNextAction(t *testing.T) {
	item := executionView{
		ID: "01J00000000000000000000000", Title: "Doctor", SubjectKey: "root-key", Status: execution.StatusSucceeded,
		ResultPage: &resultPageView{Title: "Doctor", Actions: []resultActionView{{Relation: "doctor-fix", Label: "Doctor fix"}}},
	}
	html, err := renderComponent(context.Background(), resultDialogs([]executionView{item}))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{`data-next-operation="doctor-fix"`, `data-operation-subject="root-key"`, `>Doctor fix</button>`} {
		if !strings.Contains(html, marker) {
			t.Errorf("semantic action marker %q is missing: %s", marker, html)
		}
	}
	if strings.Contains(html, "dw doctor") {
		t.Fatalf("web result leaked CLI syntax: %s", html)
	}
}

func TestActiveExecutionDoesNotExposeResultDialog(t *testing.T) {
	item := executionView{ID: "01J00000000000000000000000", Title: "Doctor", Status: execution.StatusRunning}
	html, err := renderComponent(context.Background(), liveSections(pageView{Executions: []executionView{item}}))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(html, `class="result-dialog"`) || strings.Contains(html, `class="view-result"`) {
		t.Fatalf("active execution exposed a result: %s", html)
	}
}

func TestFinalExecutionReplayWaitsForCompleteBacklog(t *testing.T) {
	events := make([]execution.Event, 5)
	for index := range events {
		events[index] = execution.Event{
			Sequence: execution.EventSequence(index + 1), Kind: execution.EventProgress,
			Message: execution.MessageV1{Schema: execution.MessageSchemaV1, ID: "execution.event.started"},
		}
	}
	settings := runtimeconfig.Default().Web
	settings.EventSettleMilliseconds = 1
	server := &Server{deps: Dependencies{Executor: finalReplayExecutor{events: events}, Localizer: console.NewEnglishLocalizer(), Settings: settings}}
	projected := server.executionEvents(context.Background(), execution.ExecutionID{})
	if len(projected) != len(events) || projected[len(projected)-1].Sequence != events[len(events)-1].Sequence {
		t.Fatalf("final replay = %d events, want %d", len(projected), len(events))
	}
}

func TestWebDefaultsProviderLoginToDeviceCode(t *testing.T) {
	request := applyWebDefaults(workapp.AuthLoginRequest{Provider: "azure-devops"})
	login, ok := request.(workapp.AuthLoginRequest)
	if !ok || login.Mode != workapp.AuthLoginDeviceCode {
		t.Fatalf("web login request = %#v", request)
	}
}

func TestAppHeaderContainsOnlyBrandAndActions(t *testing.T) {
	html, err := renderComponent(context.Background(), appHeader(2))
	if err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{"DevWorkflow", "Actions", "2"} {
		if !strings.Contains(html, marker) {
			t.Errorf("header marker %q was not rendered", marker)
		}
	}
	for _, removed := range []string{"Local control center", "Root"} {
		if strings.Contains(html, removed) {
			t.Errorf("header still contains %q: %s", removed, html)
		}
	}
}
