package web

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/starfederation/datastar-go/datastar"
)

func (server *Server) handleSubmit(writer http.ResponseWriter, request *http.Request) {
	if !server.requireMutation(writer, request) {
		return
	}
	var submission SubmitV1
	if err := decodeRequest(writer, request, &submission, server.deps.Settings.MaxRequestBodyBytes); err != nil {
		return
	}
	if submission.Schema != schemaV1 {
		http.Error(writer, "invalid schema", http.StatusBadRequest)
		return
	}
	idempotencyKey, err := execution.ParseIdempotencyKey(submission.IdempotencyKey)
	if err != nil {
		http.Error(writer, "invalid idempotency key", http.StatusBadRequest)
		return
	}
	typedRequest, root, err := server.buildRequest(submission.CommandKey, submission.Fields)
	if err != nil {
		http.Error(writer, "invalid command", http.StatusBadRequest)
		return
	}
	executionID, err := server.deps.Executor.Submit(request.Context(), execution.Submission{Request: typedRequest, Root: root, Actor: server.deps.Actor, IdempotencyKey: idempotencyKey})
	if err != nil {
		http.Error(writer, "submission failed", http.StatusConflict)
		return
	}
	record, err := server.deps.Executor.Get(request.Context(), server.deps.Actor, executionID)
	if err != nil {
		http.Error(writer, "execution unavailable", http.StatusInternalServerError)
		return
	}
	writeJSON(writer, http.StatusAccepted, ExecutionRefV1{Schema: schemaV1, ExecutionID: executionID.String(), AttemptID: record.AttemptID.String()})
}

func (server *Server) handleGetExecution(writer http.ResponseWriter, request *http.Request) {
	if !server.requireSession(writer, request) {
		return
	}
	record, err := server.executionRecord(request)
	if err != nil {
		http.Error(writer, "execution not found", http.StatusNotFound)
		return
	}
	writeJSON(writer, http.StatusOK, recordV1(record))
}

func (server *Server) handleExecutionEvents(writer http.ResponseWriter, request *http.Request) {
	if !server.requireSession(writer, request) {
		return
	}
	executionID, err := execution.ParseExecutionID(request.PathValue("id"))
	if err != nil {
		http.Error(writer, "invalid execution ID", http.StatusBadRequest)
		return
	}
	after := execution.EventSequence(0)
	if raw := request.URL.Query().Get("after"); raw != "" {
		parsed, parseErr := strconv.ParseUint(raw, 10, 64)
		if parseErr != nil {
			http.Error(writer, "invalid event sequence", http.StatusBadRequest)
			return
		}
		after = execution.EventSequence(parsed)
	}
	subscription, err := server.deps.Executor.Subscribe(request.Context(), server.deps.Actor, executionID, after)
	if err != nil {
		http.Error(writer, "execution not found", http.StatusNotFound)
		return
	}
	sse := datastar.NewSSE(writer, request)
	heartbeat := time.NewTicker(runtimeconfig.Seconds(server.deps.Settings.SSEHeartbeatSeconds))
	defer heartbeat.Stop()
	eventChannel, errorChannel := subscription.Events, subscription.Errors
	for {
		select {
		case event, open := <-eventChannel:
			if !open {
				eventChannel = nil
				if errorChannel == nil {
					return
				}
				continue
			}
			payload, marshalErr := json.Marshal(eventV1(event))
			if marshalErr != nil {
				return
			}
			if sendErr := sse.Send(datastar.EventType("dw-execution"), []string{string(payload)}, datastar.WithSSEEventId(strconv.FormatUint(uint64(event.Sequence), 10))); sendErr != nil {
				return
			}
		case streamErr, open := <-errorChannel:
			if !open {
				errorChannel = nil
				if eventChannel == nil {
					return
				}
				continue
			}
			if streamErr != nil {
				return
			}
		case <-heartbeat.C:
			if err := sse.Send(datastar.EventType("dw-heartbeat"), []string{`{"schema":1}`}); err != nil {
				return
			}
		case <-request.Context().Done():
			return
		}
	}
}

func (server *Server) handleCancelExecution(writer http.ResponseWriter, request *http.Request) {
	if !server.requireMutation(writer, request) {
		return
	}
	executionID, err := execution.ParseExecutionID(request.PathValue("id"))
	if err != nil {
		http.Error(writer, "invalid execution ID", http.StatusBadRequest)
		return
	}
	if err = server.deps.Executor.Cancel(request.Context(), server.deps.Actor, executionID); err != nil {
		http.Error(writer, "cancel failed", http.StatusConflict)
		return
	}
	writer.WriteHeader(http.StatusNoContent)
}

func (server *Server) handleResponse(writer http.ResponseWriter, request *http.Request) {
	if !server.requireMutation(writer, request) {
		return
	}
	executionID, err := execution.ParseExecutionID(request.PathValue("id"))
	if err != nil {
		http.Error(writer, "invalid execution ID", http.StatusBadRequest)
		return
	}
	record, err := server.deps.Executor.Get(request.Context(), server.deps.Actor, executionID)
	if err != nil || record.PendingPrompt == nil || string(record.PendingPrompt.ID) != request.PathValue("promptID") {
		http.Error(writer, "prompt not found", http.StatusNotFound)
		return
	}
	response, err := decodePromptResponse(writer, request, record.PendingPrompt.Kind, server.deps.Settings.MaxRequestBodyBytes)
	if err != nil {
		return
	}
	if err = server.deps.Executor.Respond(request.Context(), server.deps.Actor, executionID, record.PendingPrompt.ID, response); err != nil {
		http.Error(writer, "response rejected", http.StatusConflict)
		return
	}
	writer.WriteHeader(http.StatusNoContent)
}

func (server *Server) buildRequest(commandKey string, fields []FieldV1) (action.Request, string, error) {
	command := commandByKey(server.deps.Grammar, commandKey)
	if command == nil || command.Hidden || strings.HasPrefix(command.Key, "completion.") || command.Key == "tui" || strings.HasPrefix(command.Key, "web.") {
		return nil, "", fmt.Errorf("web.unknown-command:%s", commandKey)
	}
	route, ok := server.deps.Routes.Route(command.Key)
	if !ok || route.Build == nil {
		return nil, "", fmt.Errorf("web.command-unavailable:%s", commandKey)
	}
	arguments := commandArguments(command)
	declared := make(map[string]spec.Argument, len(arguments))
	for _, argument := range arguments {
		declared[argument.Name] = argument
	}
	seen := make(map[string]struct{}, len(fields))
	argv := commandPath(command)
	root := config.ResolveRoot(server.deps.Config.Root)
	for _, field := range fields {
		argument, known := declared[field.Name]
		if !known {
			return nil, "", fmt.Errorf("web.unknown-field:%s", field.Name)
		}
		if _, duplicate := seen[field.Name]; duplicate {
			return nil, "", fmt.Errorf("web.duplicate-field:%s", field.Name)
		}
		seen[field.Name] = struct{}{}
		if !argument.Repeatable && len(field.Values) > 1 {
			return nil, "", fmt.Errorf("web.duplicate-field:%s", field.Name)
		}
		if field.Name == "root" && len(field.Values) == 1 {
			requestedRoot := config.ResolveRoot(field.Values[0])
			if requestedRoot != root {
				return nil, "", fmt.Errorf("web.root-mismatch:%s", requestedRoot)
			}
		}
		var fieldErr error
		argv, fieldErr = appendField(argv, argument, field.Values)
		if fieldErr != nil {
			return nil, "", fieldErr
		}
	}
	if _, provided := seen["root"]; !provided {
		if argument, declaredRoot := declared["root"]; declaredRoot {
			var fieldErr error
			argv, fieldErr = appendField(argv, argument, []string{server.deps.Config.Root})
			if fieldErr != nil {
				return nil, "", fieldErr
			}
		}
	}
	invocation, err := parse.Parse(server.deps.Grammar, argv)
	if err != nil || invocation.Command.Key != command.Key {
		return nil, "", fmt.Errorf("web.invalid-command:%s", commandKey)
	}
	typedRequest, buildErr := route.Build(invocation)
	return typedRequest, root, buildErr
}

func appendField(argv []string, argument spec.Argument, values []string) ([]string, error) {
	if argument.Kind == spec.Bool {
		if len(values) != 1 || (values[0] != "true" && values[0] != "false") {
			return nil, fmt.Errorf("web.invalid-boolean-field:%s", argument.Name)
		}
		if values[0] == "true" {
			argv = append(argv, argument.Token())
		}
		return argv, nil
	}
	if argument.Required && len(values) == 0 {
		return nil, fmt.Errorf("web.required-field:%s", argument.Name)
	}
	if argument.Positional() {
		return append(argv, values...), nil
	}
	for _, value := range values {
		argv = append(argv, argument.Token(), value)
	}
	return argv, nil
}

func commandByKey(root *spec.Command, key string) *spec.Command {
	if root.Key == key {
		return root
	}
	for _, child := range root.Children {
		if result := commandByKey(child, key); result != nil {
			return result
		}
	}
	return nil
}

func commandPath(command *spec.Command) []string {
	var reversed []string
	for current := command; current != nil && current.Parent() != nil; current = current.Parent() {
		reversed = append(reversed, current.Name)
	}
	path := make([]string, len(reversed))
	for index := range reversed {
		path[len(reversed)-1-index] = reversed[index]
	}
	return path
}

func commandArguments(command *spec.Command) []spec.Argument {
	return append([]spec.Argument(nil), command.Arguments...)
}

func (server *Server) executionRecord(request *http.Request) (execution.Record, error) {
	id, err := execution.ParseExecutionID(request.PathValue("id"))
	if err != nil {
		return execution.Record{}, err
	}
	return server.deps.Executor.Get(request.Context(), server.deps.Actor, id)
}

func recordV1(record execution.Record) RecordV1 {
	return RecordV1{Schema: schemaV1, ExecutionID: record.ExecutionID.String(), AttemptID: record.AttemptID.String(), ActionID: string(record.ActionID), Status: record.Status, Root: record.Root, Origin: record.Origin, Result: record.Result, Failure: record.Failure, PendingPrompt: record.PendingPrompt, CreatedAt: record.CreatedAt, StartedAt: record.StartedAt, FinishedAt: record.FinishedAt}
}

func eventV1(event execution.Event) EventV1 {
	value := EventV1{Schema: schemaV1, ExecutionID: event.ExecutionID.String(), AttemptID: event.AttemptID.String(), Sequence: event.Sequence, At: event.At, Kind: event.Kind, ActionID: string(event.ActionID), Message: event.Message}
	if event.Payload != nil {
		value.Payload = &PayloadV1{Type: string(event.Payload.Type), Schema: uint16(event.Payload.Schema), JSON: event.Payload.JSON}
	}
	return value
}

func decodePromptResponse(writer http.ResponseWriter, request *http.Request, kind action.PromptKind, maxBytes int64) (action.Response, error) {
	switch kind {
	case action.PromptText:
		var value TextResponseV1
		if err := decodeRequest(writer, request, &value, maxBytes); err != nil {
			return nil, err
		}
		if err := validateResponseSchema(writer, value.Schema); err != nil {
			return nil, err
		}
		return action.TextResponse{Value: value.Value}, nil
	case action.PromptSecret:
		value, err := decodeSecretForm(writer, request, maxBytes)
		if err != nil {
			return nil, err
		}
		return action.SecretResponse{Value: contract.NewSecretValue(value)}, nil
	case action.PromptConfirm:
		var value ConfirmResponseV1
		if err := decodeRequest(writer, request, &value, maxBytes); err != nil {
			return nil, err
		}
		if err := validateResponseSchema(writer, value.Schema); err != nil {
			return nil, err
		}
		return action.ConfirmResponse{Accepted: value.Accepted}, nil
	case action.PromptSelectOne:
		var value SelectOneResponseV1
		if err := decodeRequest(writer, request, &value, maxBytes); err != nil {
			return nil, err
		}
		if err := validateResponseSchema(writer, value.Schema); err != nil {
			return nil, err
		}
		return action.SelectOneResponse{Value: action.ChoiceValue(value.Value)}, nil
	case action.PromptSelectMany:
		var value SelectManyResponseV1
		if err := decodeRequest(writer, request, &value, maxBytes); err != nil {
			return nil, err
		}
		if err := validateResponseSchema(writer, value.Schema); err != nil {
			return nil, err
		}
		values := make([]action.ChoiceValue, len(value.Values))
		for index := range value.Values {
			values[index] = action.ChoiceValue(value.Values[index])
		}
		return action.SelectManyResponse{Values: values}, nil
	default:
		http.Error(writer, "unsupported prompt", http.StatusBadRequest)
		return nil, fmt.Errorf("web.unsupported-prompt:%s", kind)
	}
}

func validateResponseSchema(writer http.ResponseWriter, schema uint16) error {
	if schema == schemaV1 {
		return nil
	}
	http.Error(writer, "invalid response", http.StatusBadRequest)
	return fmt.Errorf("web.invalid-response-schema:%d", schema)
}

func decodeSecretForm(writer http.ResponseWriter, request *http.Request, maxBytes int64) (string, error) {
	if request.URL.RawQuery != "" || !strings.HasPrefix(request.Header.Get("Content-Type"), "application/x-www-form-urlencoded") {
		http.Error(writer, "invalid request", http.StatusBadRequest)
		return "", fmt.Errorf("web.invalid-secret-form")
	}
	request.Body = http.MaxBytesReader(writer, request.Body, maxBytes)
	if err := request.ParseForm(); err != nil {
		http.Error(writer, "invalid request", http.StatusBadRequest)
		return "", fmt.Errorf("web.invalid-secret-form")
	}
	if len(request.PostForm) != 2 || len(request.PostForm["schema"]) != 1 || len(request.PostForm["value"]) != 1 || request.PostForm.Get("schema") != strconv.Itoa(int(schemaV1)) {
		http.Error(writer, "invalid request", http.StatusBadRequest)
		return "", fmt.Errorf("web.invalid-secret-form")
	}
	return request.PostForm.Get("value"), nil
}
