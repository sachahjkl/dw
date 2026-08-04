package bootstrap

import (
	"reflect"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/update"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
	"github.com/sachahjkl/dw/internal/workspaceapp"
)

func TestExecutionDescriptorsMatchHandlersAndRoundTrip(t *testing.T) {
	services, err := newServices()
	if err != nil {
		t.Fatal(err)
	}
	dispatcher := action.NewDispatcher()
	if err := registerHandlers(dispatcher, services); err != nil {
		t.Fatal(err)
	}
	registry, _, err := executionRegistries(dispatcher)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(dispatcher.IDs(), registry.IDs()) {
		t.Fatalf("handler IDs = %v, descriptor IDs = %v", dispatcher.IDs(), registry.IDs())
	}

	for _, id := range registry.IDs() {
		t.Run(string(id), func(t *testing.T) {
			descriptor, ok := registry.Descriptor(id)
			if !ok {
				t.Fatalf("missing descriptor %s", id)
			}
			request := zeroDescriptorValue(t, descriptor, "Request").(action.Request)
			encodedRequest, err := descriptor.EncodeRequest(request)
			if err != nil {
				t.Fatalf("EncodeRequest: %v", err)
			}
			if !encodedRequest.Redacted {
				decodedRequest, err := descriptor.DecodeRequest(encodedRequest)
				if err != nil {
					t.Fatalf("DecodeRequest: %v", err)
				}
				if reflect.TypeOf(decodedRequest) != reflect.TypeOf(request) {
					t.Fatalf("request type = %T, want %T", decodedRequest, request)
				}
			}

			result := zeroResult(id, zeroDescriptorValue(t, descriptor, "Result").(action.Result))
			encodedResult, err := descriptor.EncodeResult(result)
			if err != nil {
				t.Fatalf("EncodeResult: %v", err)
			}
			decodedResult, err := descriptor.DecodeResult(encodedResult)
			if err != nil {
				t.Fatalf("DecodeResult: %v", err)
			}
			if reflect.TypeOf(decodedResult) != reflect.TypeOf(result) || decodedResult.ActionID() != id {
				t.Fatalf("result = %T/%s, want %T/%s", decodedResult, decodedResult.ActionID(), result, id)
			}
		})
	}
}

func zeroDescriptorValue(t *testing.T, descriptor execution.Descriptor, codecName string) any {
	t.Helper()
	value := reflect.ValueOf(descriptor)
	if value.Kind() != reflect.Pointer || value.Elem().Kind() != reflect.Struct {
		t.Fatalf("descriptor %T is not a JSON descriptor", descriptor)
	}
	codec := value.Elem().FieldByName(codecName)
	if !codec.IsValid() {
		t.Fatalf("descriptor %T has no %s codec", descriptor, codecName)
	}
	encode := codec.FieldByName("Encode")
	if !encode.IsValid() || encode.Type().NumIn() != 1 {
		t.Fatalf("descriptor %T has invalid %s encoder", descriptor, codecName)
	}
	return reflect.Zero(encode.Type().In(0)).Interface()
}

func TestSecretSetDescriptorRedactsSecretValue(t *testing.T) {
	value := contract.NewSecretValue("top-secret-marker")
	encoded, err := encodeSecretSetRequest(secret.SetRequest{Key: "token", Value: &value})
	if err != nil {
		t.Fatal(err)
	}
	if !encoded.Redacted || strings.Contains(string(encoded.JSON), "top-secret-marker") {
		t.Fatalf("encoded secret request = %s, redacted = %t", encoded.JSON, encoded.Redacted)
	}
}

func zeroResult(id action.ID, result action.Result) action.Result {
	switch id {
	case "workspace.item.add", "workspace.item.remove":
		return workspaceapp.ItemUpdateResult{Operation: id}
	case workapp.ActionWorkspaceFinish:
		return workapp.FinishReport{Execution: &workspace.FinishExecutionReport{Events: []workspace.ActionEvent{{Type: "verifyingFinish", RepositoryCount: 1}}}}
	case workapp.ActionWorkspaceOpen:
		return workapp.OpenReport{Workspace: "workspace", Launch: &agent.Launch{FileName: "opencode", Arguments: []string{"workspace"}, Environment: []agent.EnvironmentVariable{{Name: "OPENCODE_CONFIG", Value: "config.jsonc"}}, WorkingDirectory: "workspace"}, Events: []workapp.Event{}}
	case update.ActionID:
		return update.Report{Kind: "check", Check: &update.CheckReport{}}
	default:
		return result
	}
}
