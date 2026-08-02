package secret

import (
	"context"
	"errors"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	ActionList   action.ID = "secret.list"
	ActionSet    action.ID = "secret.set"
	ActionGet    action.ID = "secret.get"
	ActionDelete action.ID = "secret.delete"
)

type ListRequest struct {
	Root string `json:"root,omitempty"`
}
type SetRequest struct {
	Key         contract.SecretKey            `json:"key"`
	Value       *contract.SecretValue         `json:"-"`
	Environment *contract.EnvironmentVariable `json:"environment,omitempty"`
}
type GetRequest struct {
	Key contract.SecretKey `json:"key"`
}
type DeleteRequest struct {
	Key       contract.SecretKey `json:"key"`
	Confirmed bool               `json:"confirmed"`
}

func (ListRequest) ActionID() action.ID   { return ActionList }
func (SetRequest) ActionID() action.ID    { return ActionSet }
func (GetRequest) ActionID() action.ID    { return ActionGet }
func (DeleteRequest) ActionID() action.ID { return ActionDelete }
func (ListReport) ActionID() action.ID    { return ActionList }
func (SetReport) ActionID() action.ID     { return ActionSet }
func (GetReport) ActionID() action.ID     { return ActionGet }
func (DeleteReport) ActionID() action.ID  { return ActionDelete }

type RootResolver func(string) string

type Handler struct {
	action  action.ID
	service *Service
	root    RootResolver
}

func Handlers(service *Service, resolveRoot RootResolver) []action.Handler {
	if service == nil {
		service = NewService(nil)
	}
	return []action.Handler{
		Handler{action: ActionList, service: service, root: resolveRoot},
		Handler{action: ActionSet, service: service},
		Handler{action: ActionGet, service: service},
		Handler{action: ActionDelete, service: service},
	}
}

func (handler Handler) ID() action.ID { return handler.action }

func (handler Handler) Execute(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
	switch handler.action {
	case ActionList:
		typed, err := listRequest(request)
		if err != nil {
			return nil, err
		}
		root := typed.Root
		if handler.root != nil {
			root = handler.root(root)
		}
		return handler.service.List(ctx, root)
	case ActionSet:
		typed, err := setRequest(request)
		if err != nil {
			return nil, err
		}
		value, err := resolveSetValue(ctx, typed, runtime)
		if err != nil {
			return nil, err
		}
		return handler.service.Set(ctx, typed.Key, value)
	case ActionGet:
		typed, err := getRequest(request)
		if err != nil {
			return nil, err
		}
		return handler.service.Get(ctx, typed.Key)
	case ActionDelete:
		typed, err := deleteRequest(request)
		if err != nil {
			return nil, err
		}
		if !typed.Confirmed {
			if err := confirmDelete(ctx, typed.Key, runtime); err != nil {
				return nil, err
			}
		}
		return handler.service.Delete(ctx, typed.Key)
	default:
		return nil, fmt.Errorf("secret.unknown-action:%s", handler.action)
	}
}

func resolveSetValue(ctx context.Context, request SetRequest, runtime action.Runtime) (contract.SecretValue, error) {
	if request.Value != nil && request.Environment != nil {
		return contract.SecretValue{}, errors.New("secret.conflicting-value-sources")
	}
	if request.Value != nil {
		return *request.Value, nil
	}
	if request.Environment != nil {
		return FromEnvironment(*request.Environment)
	}
	response, err := runtime.Ask(ctx, action.SecretPrompt{
		Meta: action.PromptMeta{
			ID:    action.PromptID("secret-set:" + string(request.Key)),
			Label: l10n.M("secret.prompt-value", l10n.A("key", request.Key)),
			Help:  messagePointer(l10n.M("secret.prompt-value-help")),
		},
		Required: true,
	})
	if err != nil {
		return contract.SecretValue{}, err
	}
	return response.(action.SecretResponse).Value, nil
}

func confirmDelete(ctx context.Context, key contract.SecretKey, runtime action.Runtime) error {
	response, err := runtime.Ask(ctx, action.ConfirmPrompt{
		Meta: action.PromptMeta{
			ID:    action.PromptID("secret-delete:" + string(key)),
			Label: l10n.M("secret.prompt-delete", l10n.A("key", key)),
			Help:  messagePointer(l10n.M("secret.prompt-delete-help")),
		},
		Default: false,
	})
	if err != nil {
		return err
	}
	if !response.(action.ConfirmResponse).Accepted {
		return newLocalizedError("secret.delete-canceled", l10n.M("secret.delete-canceled"), nil)
	}
	return nil
}

func messagePointer(message l10n.Message) *l10n.Message { return &message }

func listRequest(request action.Request) (ListRequest, error) {
	value, ok := request.(ListRequest)
	if !ok {
		return ListRequest{}, fmt.Errorf("secret.invalid-list-request:%T", request)
	}
	return value, nil
}
func setRequest(request action.Request) (SetRequest, error) {
	value, ok := request.(SetRequest)
	if !ok {
		return SetRequest{}, fmt.Errorf("secret.invalid-set-request:%T", request)
	}
	return value, nil
}
func getRequest(request action.Request) (GetRequest, error) {
	value, ok := request.(GetRequest)
	if !ok {
		return GetRequest{}, fmt.Errorf("secret.invalid-get-request:%T", request)
	}
	return value, nil
}
func deleteRequest(request action.Request) (DeleteRequest, error) {
	value, ok := request.(DeleteRequest)
	if !ok {
		return DeleteRequest{}, fmt.Errorf("secret.invalid-delete-request:%T", request)
	}
	return value, nil
}
