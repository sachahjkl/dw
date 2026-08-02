package dataapp

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"

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

func (result *DescribeResult) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		result.Result = nil
		return nil
	}
	var value NativeQueryReport
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return fmt.Errorf("data.invalid-describe-result-json")
	}
	result.Result = &value
	return nil
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
	value, ok := request.(DataSourceListRequest)
	return value, ok
}
func asDataSourceCollectRequest(request action.Request) (DataSourceCollectRequest, bool) {
	value, ok := request.(DataSourceCollectRequest)
	return value, ok
}
func asGuardRequest(request action.Request) (GuardRequest, bool) {
	value, ok := request.(GuardRequest)
	return value, ok
}
func asCatalogRequest(request action.Request) (CatalogRequest, bool) {
	value, ok := request.(CatalogRequest)
	return value, ok
}
func asDescribeRequest(request action.Request) (DescribeRequest, bool) {
	value, ok := request.(DescribeRequest)
	return value, ok
}
func asQueryRequest(request action.Request) (QueryRequest, bool) {
	value, ok := request.(QueryRequest)
	return value, ok
}

func asReadRequest(request action.Request) (ReadRequest, bool) {
	value, ok := request.(ReadRequest)
	return value, ok
}
