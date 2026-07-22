package dataapp

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
)

const (
	ActionDataSourceList    action.ID = "data.source.list"
	ActionDataSourceCollect action.ID = "data.source.collect"
	ActionDataGuard         action.ID = "data.guard"
	ActionDataCatalog       action.ID = "data.catalog"
	ActionDataDescribe      action.ID = "data.describe"
	ActionDataQuery         action.ID = "data.query"
	ActionDataRead          action.ID = "data.read"
)

type DataSourceListRequest struct {
	Provider string `json:"provider,omitempty"`
	Root     string `json:"root,omitempty"`
}
type DataSourceCollectRequest struct {
	Provider string `json:"provider,omitempty"`
	Root     string `json:"root,omitempty"`
	Save     bool   `json:"save"`
}
type GuardRequest struct {
	Provider string `json:"provider,omitempty"`
	Query    string `json:"query"`
}
type CatalogRequest struct {
	Selection Selection `json:"selection"`
}
type DescribeRequest struct {
	Selection Selection `json:"selection"`
	Object    string    `json:"object"`
}
type QueryRequest struct {
	Selection Selection `json:"selection"`
	Query     string    `json:"query"`
	MaxRows   *int      `json:"maxRows,omitempty"`
}
type ReadRequest struct {
	Selection Selection `json:"selection"`
	Object    string    `json:"object,omitempty"`
	Worksheet string    `json:"worksheet,omitempty"`
	Range     string    `json:"range,omitempty"`
	Columns   []string  `json:"columns,omitempty"`
	MaxRows   *int      `json:"maxRows,omitempty"`
}

func (DataSourceListRequest) ActionID() action.ID    { return ActionDataSourceList }
func (DataSourceCollectRequest) ActionID() action.ID { return ActionDataSourceCollect }
func (GuardRequest) ActionID() action.ID             { return ActionDataGuard }
func (CatalogRequest) ActionID() action.ID           { return ActionDataCatalog }
func (DescribeRequest) ActionID() action.ID          { return ActionDataDescribe }
func (QueryRequest) ActionID() action.ID             { return ActionDataQuery }
func (ReadRequest) ActionID() action.ID              { return ActionDataRead }

type DataSourceListResult struct{ DataSourceListReport }
type DataSourceCollectResult struct{ DataSourceCollectReport }
type GuardResult struct{ GuardReport }
type CatalogResult struct{ NativeQueryReport }
type DataQueryResult struct{ NativeQueryReport }
type DescribeResult struct{ Result *NativeQueryReport }
type DataReadResult struct{ NativeQueryReport }

func (DataSourceListResult) ActionID() action.ID    { return ActionDataSourceList }
func (DataSourceCollectResult) ActionID() action.ID { return ActionDataSourceCollect }
func (GuardResult) ActionID() action.ID             { return ActionDataGuard }
func (CatalogResult) ActionID() action.ID           { return ActionDataCatalog }
func (DescribeResult) ActionID() action.ID          { return ActionDataDescribe }
func (DataQueryResult) ActionID() action.ID         { return ActionDataQuery }
func (DataReadResult) ActionID() action.ID          { return ActionDataRead }

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
		Handler{id: ActionDataSourceList, service: service}, Handler{id: ActionDataSourceCollect, service: service},
		Handler{id: ActionDataGuard, service: service}, Handler{id: ActionDataCatalog, service: service},
		Handler{id: ActionDataDescribe, service: service}, Handler{id: ActionDataQuery, service: service},
		Handler{id: ActionDataRead, service: service},
	}
}

func (handler Handler) ID() action.ID { return handler.id }

func (handler Handler) Execute(ctx context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	if handler.service == nil {
		return nil, fmt.Errorf("data.nil-service")
	}
	switch handler.id {
	case ActionDataSourceList:
		value, ok := asDataSourceListRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.List(value.Root, value.Provider)
		return DataSourceListResult{report}, err
	case ActionDataSourceCollect:
		value, ok := asDataSourceCollectRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.CollectDiscovered(ctx, value.Root, value.Provider, value.Save)
		return DataSourceCollectResult{report}, err
	case ActionDataGuard:
		value, ok := asGuardRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Guard(ctx, value.Provider, value.Query)
		return GuardResult{report}, err
	case ActionDataCatalog:
		value, ok := asCatalogRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Catalog(ctx, value.Selection)
		return CatalogResult{report}, err
	case ActionDataDescribe:
		value, ok := asDescribeRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Describe(ctx, value.Selection, value.Object)
		return DescribeResult{Result: report}, err
	case ActionDataQuery:
		value, ok := asQueryRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Query(ctx, value.Selection, value.Query, value.MaxRows)
		return DataQueryResult{report}, err
	case ActionDataRead:
		value, ok := asReadRequest(request)
		if !ok {
			return nil, requestTypeError(handler.id)
		}
		report, err := handler.service.Read(ctx, value.Selection, value.Object, value.Worksheet, value.Range, value.Columns, value.MaxRows)
		return DataReadResult{report}, err
	default:
		return nil, fmt.Errorf("data.unknown-action:%s", handler.id)
	}
}

func requestTypeError(id action.ID) error { return fmt.Errorf("data.invalid-request:%s", id) }
func asDataSourceListRequest(request action.Request) (DataSourceListRequest, bool) {
	switch value := request.(type) {
	case DataSourceListRequest:
		return value, true
	case *DataSourceListRequest:
		if value == nil {
			return DataSourceListRequest{}, false
		}
		return *value, true
	default:
		return DataSourceListRequest{}, false
	}
}
func asDataSourceCollectRequest(request action.Request) (DataSourceCollectRequest, bool) {
	switch value := request.(type) {
	case DataSourceCollectRequest:
		return value, true
	case *DataSourceCollectRequest:
		if value == nil {
			return DataSourceCollectRequest{}, false
		}
		return *value, true
	default:
		return DataSourceCollectRequest{}, false
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
func asCatalogRequest(request action.Request) (CatalogRequest, bool) {
	switch value := request.(type) {
	case CatalogRequest:
		return value, true
	case *CatalogRequest:
		if value == nil {
			return CatalogRequest{}, false
		}
		return *value, true
	default:
		return CatalogRequest{}, false
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

func asReadRequest(request action.Request) (ReadRequest, bool) {
	switch value := request.(type) {
	case ReadRequest:
		return value, true
	case *ReadRequest:
		if value == nil {
			return ReadRequest{}, false
		}
		return *value, true
	default:
		return ReadRequest{}, false
	}
}
