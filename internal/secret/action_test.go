package secret

import (
	"context"
	"errors"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

func respond(response action.Response) action.Runtime {
	return action.Runtime{Input: action.InputPortFunc(func(context.Context, action.Prompt) (action.Response, error) {
		return response, nil
	})}
}

func requireLocalized(t *testing.T, err error, code string) {
	t.Helper()
	var problem *LocalizedError
	if !errors.As(err, &problem) || problem.Error() != code {
		t.Fatalf("error = %v, want %s", err, code)
	}
	if !l10n.NewEnglish().Has(problem.Localized().ID) {
		t.Fatalf("message %q is missing from the catalog", problem.Localized().ID)
	}
}

func TestResolveSetValueRejectsConflictingSources(t *testing.T) {
	value := contract.NewSecretValue("x")
	name := contract.EnvironmentVariable("DW_SECRET_TEST")
	_, err := resolveSetValue(context.Background(), SetRequest{Key: "k", Value: &value, Environment: &name}, action.Runtime{})
	requireLocalized(t, err, "secret.conflicting-value-sources")
}

func TestResolveSetValueRejectsUnexpectedResponseType(t *testing.T) {
	_, err := resolveSetValue(context.Background(), SetRequest{Key: "k"}, respond(&action.SecretResponse{Value: contract.NewSecretValue("x")}))
	if err == nil {
		t.Fatal("resolveSetValue accepted a pointer response")
	}
}

func TestConfirmDeleteRejectsUnexpectedResponseType(t *testing.T) {
	if err := confirmDelete(context.Background(), "k", respond(&action.ConfirmResponse{Accepted: true})); err == nil {
		t.Fatal("confirmDelete accepted a pointer response")
	}
}

func TestConfirmDeleteCanceled(t *testing.T) {
	err := confirmDelete(context.Background(), "k", respond(action.ConfirmResponse{Accepted: false}))
	requireLocalized(t, err, "secret.delete-canceled")
}

func TestHandlerErrorsAreLocalized(t *testing.T) {
	handler := Handler{action: ActionGet, service: NewService(nil)}
	_, err := handler.Execute(context.Background(), ListRequest{}, action.Runtime{})
	requireLocalized(t, err, "secret.invalid-request")

	handler.action = "secret.unknown"
	_, err = handler.Execute(context.Background(), ListRequest{}, action.Runtime{})
	requireLocalized(t, err, "secret.unknown-action")

	requireLocalized(t, unexpectedResponse(nil), "secret.unexpected-response")
}
