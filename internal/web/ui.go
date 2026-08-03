package web

import (
	"bytes"
	"context"
	"crypto/sha256"
	"embed"
	"encoding/hex"
	"fmt"
	"net/http"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/a-h/templ"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/starfederation/datastar-go/datastar"
)

//go:embed assets/*
var assets embed.FS

var assetVersion = func() string {
	hash := sha256.New()
	for _, name := range []string{"app.css", "app.js", "datastar.js"} {
		content, err := assets.ReadFile("assets/" + name)
		if err != nil {
			panic(err)
		}
		_, _ = hash.Write(content)
	}
	return hex.EncodeToString(hash.Sum(nil)[:8])
}()

func assetURL(name string) string {
	return "/assets/" + name + "?v=" + assetVersion
}

type pageView struct {
	CSRF              string
	Snapshot          cockpit.Snapshot
	Work              []cockpit.WorkProject
	PullRequests      []cockpit.PullRequest
	Executions        []executionView
	Toasts            []toastView
	ActiveActionCount int
	Error             string
}

type operationView struct {
	Key            string
	SubjectKey     string
	Label          string
	Relation       cockpit.Relation
	Description    string
	Risk           cockpit.Risk
	Disabled       bool
	DisabledReason string
	Submit         string
	Inputs         []operationInputView
}

type operationInputView struct {
	ID       string
	Name     string
	Label    string
	Signal   string
	Kind     cockpit.InputKind
	Required bool
	Options  []cockpit.InputOption
}

type executionView struct {
	ID           string
	AttemptID    string
	Relation     string
	OperationKey string
	SubjectKey   string
	Title        string
	Status       execution.Status
	StatusLabel  string
	CreatedAt    string
	FinishedAt   *time.Time
	Summary      string
	Result       []ansiSpan
	ResultPage   *resultPageView
	Failure      string
	Prompt       *promptView
	Cancel       string
	Events       []eventView
	Subject      *execution.Subject
	Active       bool
}

type ansiSpan struct {
	Text  string
	Class string
}

type resultPageView struct {
	Title    string
	Badge    string
	Status   string
	Summary  []resultFieldView
	Sections []resultSectionView
	Hint     *resultFieldView
	Actions  []resultActionView
}

type resultActionView struct {
	Relation string
	Label    string
}

type resultFieldView struct {
	Label string
	Value string
	Style string
}

type resultSectionView struct {
	Title  string
	Fields []resultFieldView
	Table  *resultTableView
	Panels []resultPanelView
	Items  []string
}

type resultTableView struct {
	Columns []string
	Rows    [][]string
}

type resultPanelView struct {
	Title string
	Body  string
}

type eventView struct {
	Sequence         execution.EventSequence
	Kind             execution.EventKind
	KindLabel        string
	At               string
	Message          string
	AuthorizationURL string
	CallbackURI      string
	VerificationURI  string
	UserCode         string
}

type toastView struct {
	Title  string
	Detail string
	Target string
}

type workRecoveryView struct {
	Title  string
	Detail string
}

type blockerView struct {
	Title  string
	Detail string
}

type promptView struct {
	ID       string
	Kind     string
	Label    string
	Help     string
	Required bool
	Choices  []choiceView
	Submit   string
	Signal   string
}

type choiceView struct {
	Value  string
	Label  string
	Signal string
}

func (server *Server) renderIndex(writer http.ResponseWriter, request *http.Request, csrf string) {
	view := server.loadPage(request.Context(), csrf)
	writer.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := page(view).Render(request.Context(), writer); err != nil {
		http.Error(writer, "render failed", http.StatusInternalServerError)
	}
}

func (server *Server) handleAsset(writer http.ResponseWriter, request *http.Request) {
	name := request.PathValue("name")
	contentType := ""
	switch name {
	case "datastar.js", "app.js":
		contentType = "text/javascript; charset=utf-8"
	case "app.css":
		contentType = "text/css; charset=utf-8"
	case "datastar-LICENSE.md", "datastar.js.sha256":
		contentType = "text/plain; charset=utf-8"
	default:
		http.NotFound(writer, request)
		return
	}
	content, err := assets.ReadFile("assets/" + name)
	if err != nil {
		http.NotFound(writer, request)
		return
	}
	writer.Header().Set("Content-Type", contentType)
	if request.URL.Query().Get("v") == assetVersion {
		writer.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
	} else {
		writer.Header().Set("Cache-Control", "no-cache")
	}
	_, _ = writer.Write(content)
}

func (server *Server) handlePageEvents(writer http.ResponseWriter, request *http.Request) {
	if !server.requireSession(writer, request) {
		return
	}
	csrf, ok := server.sessionCSRF(request)
	if !ok {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	sse := datastar.NewSSE(writer, request)
	actionTicker := time.NewTicker(250 * time.Millisecond)
	defer actionTicker.Stop()
	resourceTicker := time.NewTicker(runtimeconfig.Milliseconds(server.deps.Settings.PagePollMilliseconds))
	defer resourceTicker.Stop()
	resourceUpdates := make(chan pageView, 1)
	resourceRequests := make(chan struct{}, 1)
	go func() {
		for {
			select {
			case <-resourceRequests:
				view := server.loadResourcePage(request.Context(), csrf)
				select {
				case resourceUpdates <- view:
				case <-request.Context().Done():
					return
				}
			case <-request.Context().Done():
				return
			}
		}
	}()
	requestResources := func() {
		select {
		case resourceRequests <- struct{}{}:
		default:
		}
	}
	requestResources()
	var previousResources, previousActions, previousResults, previousShortcut, previousToasts string
	for {
		select {
		case <-actionTicker.C:
			executions, activeCount, err := server.loadExecutionViews(request.Context(), csrf)
			if err != nil {
				continue
			}
			toasts := actionToasts(executions, time.Now())
			var actionsBuffer, resultsBuffer, shortcutBuffer, toastBuffer bytes.Buffer
			if executionsSection(executions, activeCount).Render(request.Context(), &actionsBuffer) != nil ||
				resultDialogs(executions).Render(request.Context(), &resultsBuffer) != nil ||
				actionsShortcut(activeCount).Render(request.Context(), &shortcutBuffer) != nil ||
				notifications(toasts).Render(request.Context(), &toastBuffer) != nil {
				return
			}
			if html := actionsBuffer.String(); html != previousActions {
				if sse.PatchElements(html, datastar.WithSelector("#actions"), datastar.WithModeOuter()) != nil {
					return
				}
				previousActions = html
			}
			if html := resultsBuffer.String(); html != previousResults {
				if sse.PatchElements(html, datastar.WithSelector("#action-results"), datastar.WithModeOuter()) != nil {
					return
				}
				previousResults = html
			}
			if html := shortcutBuffer.String(); html != previousShortcut {
				if sse.PatchElements(html, datastar.WithSelector("#tab-actions"), datastar.WithModeOuter()) != nil {
					return
				}
				previousShortcut = html
			}
			if html := toastBuffer.String(); html != previousToasts {
				if sse.PatchElements(html, datastar.WithSelector("#action-notifications"), datastar.WithModeOuter()) != nil {
					return
				}
				previousToasts = html
			}
		case view := <-resourceUpdates:
			var buffer bytes.Buffer
			if resourceSections(view).Render(request.Context(), &buffer) != nil {
				return
			}
			if html := buffer.String(); html != previousResources {
				if sse.PatchElements(html, datastar.WithSelector("#resource-sections"), datastar.WithModeOuter()) != nil {
					return
				}
				previousResources = html
			}
		case <-resourceTicker.C:
			requestResources()
		case <-request.Context().Done():
			return
		}
	}
}

func (server *Server) sessionCSRF(request *http.Request) (string, bool) {
	value, ok := server.auth.authenticate(request)
	if !ok {
		return "", false
	}
	return encodeToken(value.csrf), true
}

func (server *Server) loadPage(ctx context.Context, csrf string) pageView {
	view := server.loadResourcePage(ctx, csrf)
	var err error
	view.Executions, view.ActiveActionCount, err = server.loadExecutionViews(ctx, csrf)
	if err != nil && view.Error == "" {
		view.Error = console.LocalizedErrorText(server.deps.Localizer, err)
	}
	view.Toasts = actionToasts(view.Executions, time.Now())
	return view
}

func (server *Server) loadResourcePage(ctx context.Context, csrf string) pageView {
	view := pageView{CSRF: csrf}
	snapshot, err := server.deps.Cockpit.Snapshot(ctx, server.deps.Config.Root)
	view.Snapshot = snapshot
	if err != nil {
		view.Error = console.LocalizedErrorText(server.deps.Localizer, err)
		return view
	}
	view.Work, err = server.deps.Cockpit.Work(ctx, snapshot)
	if err != nil {
		view.Error = console.LocalizedErrorText(server.deps.Localizer, err)
	}
	view.PullRequests, err = server.deps.Cockpit.PullRequests(ctx, snapshot)
	if err != nil && view.Error == "" {
		view.Error = console.LocalizedErrorText(server.deps.Localizer, err)
	}
	return view
}

func (server *Server) loadExecutionViews(ctx context.Context, csrf string) ([]executionView, int, error) {
	records, err := server.deps.Executor.List(ctx, server.deps.Actor, execution.ListFilter{Root: server.deps.Config.Root, Limit: server.deps.Settings.RecentExecutionLimit})
	if err != nil {
		return nil, 0, err
	}
	items := make([]executionView, 0, len(records))
	activeCount := 0
	var events sync.WaitGroup
	for index, record := range records {
		item := makeExecutionView(record, csrf, server.deps.Localizer, server.deps.ProjectResult, server.deps.ProjectPage)
		if item.Active {
			activeCount++
		}
		items = append(items, item)
		events.Add(1)
		go func(index int, id execution.ExecutionID) {
			defer events.Done()
			items[index].Events = server.executionEvents(ctx, id)
			if len(items[index].Events) != 0 {
				items[index].Summary = items[index].Events[len(items[index].Events)-1].Message
			}
		}(index, record.ExecutionID)
	}
	events.Wait()
	sort.SliceStable(items, func(left, right int) bool {
		return items[left].Active && !items[right].Active
	})
	return items, activeCount, nil
}

func operationViews(csrf string, operations []cockpit.Operation) []operationView {
	items := make([]operationView, 0, len(operations))
	for _, operation := range operations {
		if operation.Risk == cockpit.RiskExternal {
			continue
		}
		item := operationView{
			Key:        operationKey(operation.Subject.Kind, operation.Subject.Project, operation.Subject.Key, operation.Relation),
			SubjectKey: operationSubjectKey(operation.Subject.Kind, operation.Subject.Project, operation.Subject.Key),
			Label:      operation.Label, Relation: operation.Relation, Description: operation.Description, Risk: operation.Risk,
			Disabled: !operation.Active, DisabledReason: operation.DisabledReason,
		}
		if err := operation.Validate(); err != nil {
			item.Disabled = true
			if item.DisabledReason == "" {
				item.DisabledReason = "This operation is unavailable."
			}
		}
		item.Inputs = make([]operationInputView, 0, len(operation.Inputs))
		for _, input := range operation.Inputs {
			hash := sha256.Sum256([]byte(string(operation.Subject.Kind) + "\x00" + operation.Subject.Key + "\x00" + string(operation.Relation) + "\x00" + input.Name))
			signal := "dw_operation_" + hex.EncodeToString(hash[:6])
			item.Inputs = append(item.Inputs, operationInputView{
				ID: signal, Name: input.Name, Label: input.Label, Signal: signal,
				Kind: input.Kind, Required: input.Required, Options: input.Options,
			})
		}
		item.Submit = operationSubmitExpression(csrf, operation, item.Inputs)
		items = append(items, item)
	}
	return items
}

func operationSubjectKey(kind cockpit.ResourceKind, project, key string) string {
	hash := sha256.Sum256([]byte(string(kind) + "\x00" + project + "\x00" + key))
	return hex.EncodeToString(hash[:8])
}

func operationKey(kind cockpit.ResourceKind, project, key string, relation cockpit.Relation) string {
	hash := sha256.Sum256([]byte(operationSubjectKey(kind, project, key) + "\x00" + string(relation)))
	return hex.EncodeToString(hash[:8])
}

func operationSubmitExpression(csrf string, operation cockpit.Operation, inputs []operationInputView) string {
	values := make([]string, 0, len(inputs))
	for _, input := range inputs {
		value := fmt.Sprintf("String($%s ?? '')", input.Signal)
		if input.Kind == cockpit.InputBoolean {
			value = fmt.Sprintf("$%s ? 'true' : 'false'", input.Signal)
		}
		values = append(values, fmt.Sprintf("{name:%q,value:%s}", input.Name, value))
	}
	resource := operation.Subject
	return fmt.Sprintf(
		"@post('/operations', {contentType:'json', headers:{'X-DW-CSRF':%q}, payload:{schema:1,idempotencyKey:crypto.randomUUID().replaceAll('-',''),resource:{kind:%q,root:%q,project:%q,key:%q},relation:%q,inputs:[%s]}})",
		csrf, resource.Kind, resource.Root, resource.Project, resource.Key, operation.Relation, strings.Join(values, ","),
	)
}

func executionForResource(executions []executionView, reference cockpit.ResourceRef) *executionView {
	for index := range executions {
		subject := executions[index].Subject
		if subject == nil {
			continue
		}
		if subject.Kind == string(reference.Kind) && subject.Project == reference.Project && subject.Key == reference.Key {
			return &executions[index]
		}
	}
	return nil
}

func makeExecutionView(record execution.Record, csrf string, localizer l10n.Localizer, projectResult func(action.Result) []string, projectPage func(action.Result) (console.Page, bool, error)) executionView {
	title := humanLabel(string(record.ActionID))
	relation := ""
	if record.Subject != nil {
		relation = record.Subject.Relation
		title = humanLabel(relation)
	}
	view := executionView{
		ID: record.ExecutionID.String(), AttemptID: record.AttemptID.String(),
		Relation: relation, Title: title, Status: record.Status, StatusLabel: statusLabel(record.Status),
		CreatedAt: record.CreatedAt.Local().Format(time.RFC3339), Subject: record.Subject, Active: activeStatus(record.Status),
		Cancel: fmt.Sprintf("@post('/executions/%s/cancel', {headers:{'X-DW-CSRF':%q}})", record.ExecutionID.String(), csrf),
	}
	if record.Subject != nil {
		view.SubjectKey = operationSubjectKey(cockpit.ResourceKind(record.Subject.Kind), record.Subject.Project, record.Subject.Key)
		view.OperationKey = operationKey(cockpit.ResourceKind(record.Subject.Kind), record.Subject.Project, record.Subject.Key, cockpit.Relation(record.Subject.Relation))
	}
	view.FinishedAt = record.FinishedAt
	if record.Failure != nil {
		view.Failure = console.LocalizedErrorText(localizer, execution.NewFailureError(*record.Failure))
	}
	if record.TypedResult != nil {
		if page, ok, err := projectPage(record.TypedResult); err == nil && ok {
			projected := webResultPage(page, localizer)
			view.ResultPage = &projected
		} else {
			view.Result = ansiToSpans(strings.Join(projectResult(record.TypedResult), "\n"))
		}
	}
	if record.PendingPrompt != nil {
		view.Prompt = decodePromptView(record, csrf, localizer)
	}
	return view
}

func webResultPage(page console.Page, localizer l10n.Localizer) resultPageView {
	localizer = console.WithConsoleMessages(localizer)
	result := resultPageView{Title: page.TitleText, Status: resultStatusClass(page.Status)}
	if result.Title == "" && page.Title != "" {
		result.Title = localizer.Text(page.Title)
	}
	if page.Badge != "" {
		result.Badge = localizer.Text(page.Badge)
	}
	result.Summary = webResultFields(page.Summary, localizer)
	if page.Hint != nil {
		hint := webResultField(*page.Hint, localizer)
		result.Hint = &hint
	}
	for _, section := range page.Sections {
		projected := resultSectionView{Fields: webResultFields(section.Fields, localizer), Items: append([]string(nil), section.Items...)}
		if section.Title != "" {
			projected.Title = localizer.Text(section.Title)
		}
		if section.Table != nil {
			columns := append([]string(nil), section.Table.ColumnNames...)
			for _, id := range section.Table.Columns {
				columns = append(columns, localizer.Text(id))
			}
			projected.Table = &resultTableView{Columns: columns, Rows: section.Table.Rows}
		}
		for _, panel := range section.Panels {
			title := ""
			if panel.Title != "" {
				title = localizer.Text(panel.Title)
			}
			projected.Panels = append(projected.Panels, resultPanelView{Title: title, Body: panel.Body})
		}
		result.Sections = append(result.Sections, projected)
	}
	for _, action := range page.Actions {
		result.Actions = append(result.Actions, resultActionView{Relation: string(action.Relation), Label: humanLabel(string(action.Relation))})
	}
	return result
}

func webResultFields(fields []console.Field, localizer l10n.Localizer) []resultFieldView {
	result := make([]resultFieldView, len(fields))
	for index, field := range fields {
		result[index] = webResultField(field, localizer)
	}
	return result
}

func webResultField(field console.Field, localizer l10n.Localizer) resultFieldView {
	label := ""
	if field.Label != "" {
		label = localizer.Text(field.Label)
	}
	return resultFieldView{Label: label, Value: field.Value, Style: resultValueClass(field.Style)}
}

func resultStatusClass(status console.Status) string {
	return []string{"neutral", "success", "warning", "failure"}[int(status)]
}

func resultValueClass(style console.ValueStyle) string {
	classes := []string{"plain", "path", "command", "success", "warning", "failure", "muted"}
	if int(style) >= len(classes) {
		return "plain"
	}
	return classes[int(style)]
}

func renderComponent(ctx context.Context, component templ.Component) (string, error) {
	var buffer bytes.Buffer
	if err := component.Render(ctx, &buffer); err != nil {
		return "", err
	}
	return buffer.String(), nil
}
func (server *Server) executionEvents(ctx context.Context, id execution.ExecutionID) []eventView {
	streamContext, cancel := context.WithCancel(ctx)
	defer cancel()
	subscription, err := server.deps.Executor.Subscribe(streamContext, server.deps.Actor, id, 0)
	if err != nil {
		return nil
	}
	timer := time.NewTimer(runtimeconfig.Milliseconds(server.deps.Settings.EventSettleMilliseconds))
	defer timer.Stop()
	events := make([]eventView, 0, 16)
	eventChannel, errorChannel := subscription.Events, subscription.Errors
	for {
		select {
		case event, open := <-eventChannel:
			if !open {
				eventChannel = nil
				if errorChannel == nil {
					return events
				}
				continue
			}
			view := eventView{
				Sequence: event.Sequence, Kind: event.Kind, KindLabel: eventKindLabel(event.Kind),
				At: event.At.Local().Format(time.RFC3339), Message: localizedExecutionMessage(server.deps.Localizer, event.Message),
			}
			if workEvent, ok := event.TypedData.(workapp.Event); ok {
				view.AuthorizationURL = workEvent.AuthorizationURL
				view.CallbackURI = workEvent.CallbackURI
				view.VerificationURI = workEvent.VerificationURI
				view.UserCode = workEvent.UserCode
			}
			events = append(events, view)
		case streamErr, open := <-errorChannel:
			if !open {
				errorChannel = nil
				if eventChannel == nil {
					return events
				}
				continue
			}
			if streamErr != nil {
				return events
			}
		case <-timer.C:
			return events
		case <-ctx.Done():
			return events
		}
	}
}

func localizedExecutionMessage(localizer l10n.Localizer, encoded execution.MessageV1) string {
	message, err := execution.DecodeMessage(encoded)
	if err != nil || localizer == nil {
		return humanLabel(string(encoded.ID))
	}
	localizer = console.WithConsoleMessages(localizer)
	if !localizer.Has(message.ID) {
		return humanLabel(string(message.ID))
	}
	return localizer.Render(message)
}

func statusLabel(status execution.Status) string {
	switch status {
	case execution.StatusQueued:
		return "Queued"
	case execution.StatusRunning:
		return "Running"
	case execution.StatusWaitingInput:
		return "Needs input"
	case execution.StatusCanceling:
		return "Canceling"
	case execution.StatusCanceled:
		return "Canceled"
	case execution.StatusSucceeded:
		return "Completed"
	case execution.StatusFailed:
		return "Failed"
	case execution.StatusInterrupted:
		return "Interrupted"
	default:
		return "Unknown"
	}
}

func eventKindLabel(kind execution.EventKind) string {
	switch kind {
	case execution.EventQueued:
		return "Queued"
	case execution.EventStarted:
		return "Started"
	case execution.EventProgress:
		return "Progress"
	case execution.EventWarning:
		return "Warning"
	case execution.EventLog:
		return "Log"
	case execution.EventInputRequired:
		return "Input required"
	case execution.EventCanceling:
		return "Canceling"
	case execution.EventCanceled:
		return "Canceled"
	case execution.EventSucceeded:
		return "Completed"
	case execution.EventFailed:
		return "Failed"
	case execution.EventInterrupted:
		return "Interrupted"
	default:
		return "Update"
	}
}

func activeStatus(status execution.Status) bool {
	return status == execution.StatusQueued || status == execution.StatusRunning || status == execution.StatusWaitingInput || status == execution.StatusCanceling
}

func actionToasts(executions []executionView, now time.Time) []toastView {
	toasts := make([]toastView, 0)
	for _, item := range executions {
		switch {
		case item.Prompt != nil:
			toasts = append(toasts, toastView{Title: "Input required", Detail: item.Title, Target: "actions"})
		case item.Active:
			toasts = append(toasts, toastView{Title: "Action running", Detail: item.Title, Target: "actions"})
		case item.Status == execution.StatusSucceeded && recentlyFinished(item, now):
			detail := item.Title
			if text := firstResultLine(item.Result); text != "" {
				detail = text
			}
			toasts = append(toasts, toastView{Title: "Action completed", Detail: detail, Target: "actions"})
		case item.Status == execution.StatusFailed && recentlyFinished(item, now):
			detail := item.Failure
			if detail == "" {
				detail = item.Title
			}
			toasts = append(toasts, toastView{Title: "Action failed", Detail: detail, Target: "actions"})
		}
	}
	return toasts
}

func firstResultLine(spans []ansiSpan) string {
	var text strings.Builder
	for _, span := range spans {
		if before, _, found := strings.Cut(span.Text, "\n"); found {
			text.WriteString(before)
			break
		}
		text.WriteString(span.Text)
	}
	return strings.TrimSpace(text.String())
}

func recentlyFinished(item executionView, now time.Time) bool {
	return item.FinishedAt != nil && !item.FinishedAt.After(now) && now.Sub(*item.FinishedAt) <= 8*time.Second
}

func activeExecutions(executions []executionView) []executionView {
	items := make([]executionView, 0)
	for _, item := range executions {
		if item.Active {
			items = append(items, item)
		}
	}
	return items
}

func recentOutcomes(executions []executionView) []executionView {
	items := make([]executionView, 0, 5)
	for _, item := range executions {
		if item.Active {
			continue
		}
		items = append(items, item)
		if len(items) == 5 {
			break
		}
	}
	return items
}

func pageBlockers(view pageView) []blockerView {
	items := make([]blockerView, 0)
	if view.Snapshot.NeedsInit {
		items = append(items, blockerView{Title: "Initialization required", Detail: "Initialize this root before you start work."})
	}
	if !view.Snapshot.DoctorOK {
		items = append(items, blockerView{Title: "Doctor requires attention", Detail: "Run Doctor and inspect its result."})
	}
	for _, project := range view.Work {
		if project.Error == "" {
			continue
		}
		if recovery := workRecovery(project); recovery != nil {
			items = append(items, blockerView{Title: recovery.Title, Detail: recovery.Detail})
		} else {
			items = append(items, blockerView{Title: project.Label + " unavailable", Detail: project.Error})
		}
	}
	return items
}

func workRecovery(project cockpit.WorkProject) *workRecoveryView {
	switch {
	case strings.Contains(project.Error, "secret.store-unavailable"):
		return &workRecoveryView{
			Title:  "OS credential store unavailable",
			Detail: "Start a Secret Service provider, such as GNOME Keyring or KeePassXC, then retry.",
		}
	case strings.Contains(project.Error, "ado.error:missing-auth"),
		strings.Contains(project.Error, "github.token-required"),
		strings.Contains(project.Error, "atlassian.credentials-required"):
		return &workRecoveryView{
			Title:  "Connect " + providerLabel(project.Provider),
			Detail: "Use Sign in above to connect this provider, then work items will refresh.",
		}
	default:
		return nil
	}
}

func providerLabel(value string) string {
	switch strings.ToLower(value) {
	case "azure-devops":
		return "Azure DevOps"
	case "sqlserver":
		return "SQL Server"
	default:
		return humanLabel(value)
	}
}

func humanLabel(value string) string {
	words := strings.Fields(strings.NewReplacer(".", " ", "-", " ", "_", " ").Replace(value))
	for index, word := range words {
		if word == "" {
			continue
		}
		words[index] = strings.ToUpper(word[:1]) + word[1:]
	}
	return strings.Join(words, " ")
}
