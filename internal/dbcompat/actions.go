package dbcompat

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
)

const (
	ActionList     action.ID = "db.list"
	ActionCollect  action.ID = "db.collect"
	ActionGuard    action.ID = "db.guard"
	ActionSchema   action.ID = "db.schema"
	ActionDescribe action.ID = "db.describe"
	ActionQuery    action.ID = "db.query"
)

type ListRequest struct {
	Root string `json:"root,omitempty"`
}
type CollectRequest struct {
	Root string `json:"root,omitempty"`
	Save bool   `json:"save"`
}
type GuardRequest struct {
	SQL string `json:"sql"`
}
type SchemaRequest struct {
	Selection Selection `json:"selection"`
}
type DescribeRequest struct {
	Selection Selection `json:"selection"`
	Table     string    `json:"table"`
}
type QueryRequest struct {
	Selection Selection `json:"selection"`
	SQL       string    `json:"sql"`
	MaxRows   *int      `json:"maxRows,omitempty"`
}

func (ListRequest) ActionID() action.ID     { return ActionList }
func (CollectRequest) ActionID() action.ID  { return ActionCollect }
func (GuardRequest) ActionID() action.ID    { return ActionGuard }
func (SchemaRequest) ActionID() action.ID   { return ActionSchema }
func (DescribeRequest) ActionID() action.ID { return ActionDescribe }
func (QueryRequest) ActionID() action.ID    { return ActionQuery }

type ListResult struct{ DatabaseListReport }
type CollectResult struct{ DatabaseCollectReport }
type GuardResult struct{ SQLGuardResult }
type SchemaResult struct{ QueryResult }
type QueryActionResult struct{ QueryResult }
type DescribeResult struct{ Result *QueryResult }

func (ListResult) ActionID() action.ID        { return ActionList }
func (CollectResult) ActionID() action.ID     { return ActionCollect }
func (GuardResult) ActionID() action.ID       { return ActionGuard }
func (SchemaResult) ActionID() action.ID      { return ActionSchema }
func (DescribeResult) ActionID() action.ID    { return ActionDescribe }
func (QueryActionResult) ActionID() action.ID { return ActionQuery }

func (result DescribeResult) MarshalJSON() ([]byte, error) {
	if result.Result == nil {
		return []byte("null"), nil
	}
	return json.Marshal(result.Result)
}

type Handler struct {
	id      action.ID
	service *Service
}

func Handlers(service *Service) []action.Handler {
	return []action.Handler{
		Handler{id: ActionList, service: service}, Handler{id: ActionCollect, service: service},
		Handler{id: ActionGuard, service: service}, Handler{id: ActionSchema, service: service},
		Handler{id: ActionDescribe, service: service}, Handler{id: ActionQuery, service: service},
	}
}

func (handler Handler) ID() action.ID { return handler.id }

func (handler Handler) Execute(ctx context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	if handler.service == nil {
		return nil, fmt.Errorf("db.nil-service")
	}
	switch handler.id {
	case ActionList:
		value, ok := asListRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.List(value.Root)
		return ListResult{report}, err
	case ActionCollect:
		value, ok := asCollectRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.CollectDiscovered(ctx, value.Root, value.Save)
		return CollectResult{report}, err
	case ActionGuard:
		value, ok := asGuardRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		return GuardResult{handler.service.Guard(value.SQL)}, nil
	case ActionSchema:
		value, ok := asSchemaRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Schema(ctx, value.Selection)
		return SchemaResult{report}, err
	case ActionDescribe:
		value, ok := asDescribeRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Describe(ctx, value.Selection, value.Table)
		return DescribeResult{Result: report}, err
	case ActionQuery:
		value, ok := asQueryRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Query(ctx, value.Selection, value.SQL, value.MaxRows)
		return QueryActionResult{report}, err
	default:
		return nil, fmt.Errorf("db.unknown-action:%s", handler.id)
	}
}

func requestTypeError(id action.ID) error { return fmt.Errorf("db.invalid-request:%s", id) }
func asListRequest(request action.Request) (ListRequest, bool) {
	switch value := request.(type) {
	case ListRequest:
		return value, true
	case *ListRequest:
		if value == nil {
			return ListRequest{}, false
		}
		return *value, true
	default:
		return ListRequest{}, false
	}
}
func asCollectRequest(request action.Request) (CollectRequest, bool) {
	switch value := request.(type) {
	case CollectRequest:
		return value, true
	case *CollectRequest:
		if value == nil {
			return CollectRequest{}, false
		}
		return *value, true
	default:
		return CollectRequest{}, false
	}
}
func asGuardRequest(request action.Request) (GuardRequest, bool) {
	switch value := request.(type) {
	case GuardRequest:
		return value, true
	case *GuardRequest:
		if value == nil {
			return GuardRequest{}, false
		}
		return *value, true
	default:
		return GuardRequest{}, false
	}
}
func asSchemaRequest(request action.Request) (SchemaRequest, bool) {
	switch value := request.(type) {
	case SchemaRequest:
		return value, true
	case *SchemaRequest:
		if value == nil {
			return SchemaRequest{}, false
		}
		return *value, true
	default:
		return SchemaRequest{}, false
	}
}
func asDescribeRequest(request action.Request) (DescribeRequest, bool) {
	switch value := request.(type) {
	case DescribeRequest:
		return value, true
	case *DescribeRequest:
		if value == nil {
			return DescribeRequest{}, false
		}
		return *value, true
	default:
		return DescribeRequest{}, false
	}
}
func asQueryRequest(request action.Request) (QueryRequest, bool) {
	switch value := request.(type) {
	case QueryRequest:
		return value, true
	case *QueryRequest:
		if value == nil {
			return QueryRequest{}, false
		}
		return *value, true
	default:
		return QueryRequest{}, false
	}
}
