package update

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

const ActionID action.ID = "upgrade.run"

func (Request) ActionID() action.ID { return ActionID }
func (Report) ActionID() action.ID  { return ActionID }

type Handler struct {
	Service *Service
}

func NewHandler(service *Service) Handler { return Handler{Service: service} }
func (Handler) ID() action.ID             { return ActionID }

func (handler Handler) Execute(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
	var updateRequest Request
	switch value := request.(type) {
	case Request:
		updateRequest = value
	case *Request:
		if value == nil {
			return nil, fmt.Errorf("update: nil-request")
		}
		updateRequest = *value
	default:
		return nil, fmt.Errorf("update: invalid-request-type %T", request)
	}
	service := handler.Service
	if service == nil {
		service = NewService(nil)
	}
	var sequence uint64
	report, err := service.Run(ctx, updateRequest, func(event Event) {
		sequence++
		_ = runtime.Emit(ctx, action.EventEnvelope{
			Action:   ActionID,
			Kind:     eventEnvelopeKind(event.Kind),
			Sequence: sequence,
			Message:  eventMessage(event),
			Data:     event,
		})
	})
	if err != nil {
		return nil, err
	}
	return report, nil
}

func eventEnvelopeKind(kind string) action.EventKind {
	switch kind {
	case "checking-host":
		return action.EventStarted
	case "downloaded-asset-bytes":
		return action.EventProgress
	case "completed":
		return action.EventCompleted
	default:
		return action.EventProgress
	}
}

func eventMessage(event Event) l10n.Message {
	id := l10n.ID(event.ActionID())
	switch event.Kind {
	case "fetching-release":
		return l10n.M(id, l10n.A("owner", event.Owner), l10n.A("repository", event.Repository))
	case "fetching-manifest":
		return l10n.M(id, l10n.A("asset_name", event.AssetName))
	case "selecting-asset":
		return l10n.M(id, l10n.A("rid", event.RID))
	case "downloading-asset", "verifying-checksum":
		return l10n.M(id, l10n.A("file_name", event.FileName))
	case "downloaded-asset-bytes":
		return l10n.M(id, l10n.A("received", event.Received), l10n.A("file_name", event.FileName))
	case "preparing-executable":
		return l10n.M(id, l10n.A("file_name", event.FileName), l10n.A("rid", event.RID))
	case "replacing-executable":
		return l10n.M(id, l10n.A("executable_path", event.ExecutablePath))
	case "completed":
		return l10n.M(id, l10n.A("version", event.Version))
	default:
		return l10n.M(id)
	}
}
