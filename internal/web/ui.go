package web

import (
	"bytes"
	"context"
	"embed"
	"fmt"
	"net/http"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/a-h/templ"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/starfederation/datastar-go/datastar"
)

//go:embed assets/*
var assets embed.FS

type pageView struct {
	CSRF         string
	Snapshot     cockpit.Snapshot
	Work         []cockpit.WorkProject
	PullRequests []cockpit.PullRequest
	Commands     []commandView
	Executions   []executionView
	Error        string
}

type commandView struct {
	Key            string
	Name           string
	Summary        string
	CLI            string
	Disabled       bool
	DisabledReason string
	Submit         string
	Fields         []fieldView
}

type fieldView struct {
	Name       string
	Label      string
	Help       string
	Signal     string
	Kind       spec.ValueKind
	InputType  string
	Default    string
	Allowed    []string
	Required   bool
	Repeatable bool
	Positional bool
	Conflicts  []string
	Requires   []string
	Values     string
}

type executionView struct {
	ID        string
	AttemptID string
	ActionID  string
	Status    execution.Status
	CreatedAt string
	Failure   string
	Prompt    *promptView
	Cancel    string
	Events    []eventView
}

type eventView struct {
	Sequence         execution.EventSequence
	Kind             execution.EventKind
	Message          string
	AuthorizationURL string
	CallbackURI      string
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
	case "datastar.js":
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
	writer.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
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
	ticker := time.NewTicker(runtimeconfig.Milliseconds(server.deps.Settings.PagePollMilliseconds))
	defer ticker.Stop()
	for {
		view := server.loadPage(request.Context(), csrf)
		var buffer bytes.Buffer
		if err := liveSections(view).Render(request.Context(), &buffer); err != nil {
			return
		}
		if err := sse.PatchElements(buffer.String(), datastar.WithSelector("#live-sections"), datastar.WithModeOuter()); err != nil {
			return
		}
		select {
		case <-ticker.C:
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
	view := pageView{CSRF: csrf, Commands: server.commandCatalog(csrf)}
	snapshot, err := server.deps.Cockpit.Snapshot(ctx, server.deps.Config.Root)
	if err != nil {
		view.Error = err.Error()
		return view
	}
	view.Snapshot = snapshot
	view.Work, err = server.deps.Cockpit.Work(ctx, snapshot)
	if err != nil {
		view.Error = err.Error()
	}
	view.PullRequests, err = server.deps.Cockpit.PullRequests(ctx, snapshot)
	if err != nil && view.Error == "" {
		view.Error = err.Error()
	}
	records, listErr := server.deps.Executor.List(ctx, server.deps.Actor, execution.ListFilter{Root: server.deps.Config.Root, Limit: server.deps.Settings.RecentExecutionLimit})
	if listErr != nil && view.Error == "" {
		view.Error = listErr.Error()
	}
	view.Executions = make([]executionView, 0, len(records))
	for _, record := range records {
		item := makeExecutionView(record, csrf)
		item.Events = server.executionEvents(ctx, record.ExecutionID)
		view.Executions = append(view.Executions, item)
	}
	return view
}

func (server *Server) commandCatalog(csrf string) []commandView {
	commands := make([]commandView, 0, len(server.deps.Routes.Keys()))
	var visit func(*spec.Command, []string)
	visit = func(command *spec.Command, path []string) {
		if command == nil || command.Hidden {
			return
		}
		path = append(path, command.Name)
		if len(command.Children) != 0 {
			for _, child := range command.Children {
				visit(child, path)
			}
			return
		}
		if strings.HasPrefix(command.Key, "completion.") || command.Key == "tui" || strings.HasPrefix(command.Key, "web.") {
			return
		}
		route, found := server.deps.Routes.Route(command.Key)
		if !found || route.Build == nil {
			return
		}
		view := commandView{Key: command.Key, Name: strings.Join(path[1:], " "), Summary: command.Text(command.Summary), CLI: strings.Join(path, " ")}
		if command.Key == "agent.open" || command.Key == "workspace.open" || command.Key == "workspace.start" {
			view.Disabled = true
			view.DisabledReason = "This action requires an external terminal."
		}
		arguments := commandArguments(command)
		view.Fields = make([]fieldView, 0, len(arguments))
		for _, argument := range arguments {
			if argument.Hidden {
				continue
			}
			view.Fields = append(view.Fields, makeFieldView(command, argument))
		}
		view.Submit = submitExpression(csrf, command.Key, view.Fields)
		commands = append(commands, view)
	}
	for _, child := range server.deps.Grammar.Children {
		visit(child, []string{server.deps.Grammar.Name})
	}
	sort.SliceStable(commands, func(left, right int) bool { return commands[left].Key < commands[right].Key })
	return commands
}

func makeFieldView(command *spec.Command, argument spec.Argument) fieldView {
	inputType := "text"
	if argument.Kind == spec.Bool {
		inputType = "checkbox"
	} else if argument.Kind == spec.Int || argument.Kind == spec.Count {
		inputType = "number"
	}
	defaultValue := ""
	if argument.Default != nil {
		if argument.Kind == spec.Int || argument.Kind == spec.Count {
			defaultValue = strconv.FormatInt(argument.Default.Int, 10)
		} else {
			defaultValue = argument.Default.String
		}
	}
	signal := "dw_" + strings.NewReplacer(".", "_", "-", "_", " ", "_").Replace(command.Key+"_"+argument.Name)
	values := "($" + signal + " === '' || $" + signal + " == null) ? [] : [String($" + signal + ")]"
	if argument.Kind == spec.Bool {
		values = "[$" + signal + " ? 'true' : 'false']"
	} else if argument.Kind == spec.Int || argument.Kind == spec.Count {
		values = fmt.Sprintf("(document.getElementById(%q).value === '') ? [] : [String($%s)]", signal, signal)
	} else if argument.Kind == spec.Strings || argument.Repeatable {
		values = "$" + signal + ".split('\\n').map(value => value.trim()).filter(value => value !== '')"
	}
	return fieldView{
		Name: argument.Name, Label: argument.Token(), Help: command.Text(argument.Help), Signal: signal,
		Kind: argument.Kind, InputType: inputType, Default: defaultValue, Allowed: append([]string(nil), argument.Allowed...),
		Required: argument.Required, Repeatable: argument.Repeatable || argument.Kind == spec.Strings,
		Positional: argument.Positional(), Conflicts: append([]string(nil), argument.Conflicts...),
		Requires: append([]string(nil), argument.Requires...), Values: values,
	}
}

func submitExpression(csrf, commandKey string, fields []fieldView) string {
	items := make([]string, 0, len(fields))
	for _, field := range fields {
		items = append(items, fmt.Sprintf("{name:%q,values:%s}", field.Name, field.Values))
	}
	return fmt.Sprintf("@post('/executions', {contentType:'json', headers:{'X-DW-CSRF':%q}, payload:{schema:1,idempotencyKey:crypto.randomUUID().replaceAll('-',''),commandKey:%q,fields:[%s]}})", csrf, commandKey, strings.Join(items, ","))
}

func makeExecutionView(record execution.Record, csrf string) executionView {
	view := executionView{ID: record.ExecutionID.String(), AttemptID: record.AttemptID.String(), ActionID: string(record.ActionID), Status: record.Status, CreatedAt: record.CreatedAt.Local().Format(time.DateTime), Cancel: fmt.Sprintf("@post('/executions/%s/cancel', {headers:{'X-DW-CSRF':%q}})", record.ExecutionID.String(), csrf)}
	if record.Failure != nil {
		view.Failure = string(record.Failure.Code) + ": " + string(record.Failure.Message.ID)
	}
	if record.PendingPrompt != nil {
		view.Prompt = decodePromptView(record, csrf)
	}
	return view
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
			view := eventView{Sequence: event.Sequence, Kind: event.Kind, Message: string(event.Message.ID)}
			if workEvent, ok := event.TypedData.(workapp.Event); ok {
				view.AuthorizationURL = workEvent.AuthorizationURL
				view.CallbackURI = workEvent.CallbackURI
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
