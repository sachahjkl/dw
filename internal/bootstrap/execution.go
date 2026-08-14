package bootstrap

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/dataapp"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/providerapp"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/update"
	"github.com/sachahjkl/dw/internal/workapp"
	"sync"
)

func executionRegistries(dispatcher *action.Dispatcher) (*execution.Registry, *execution.EventDataRegistry, error) {
	registry := execution.NewRegistry()
	descriptors := append([]execution.Descriptor{}, controller.ExecutionDescriptors()...)
	descriptors = append(descriptors, bootstrapExecutionDescriptors()...)
	descriptors = append(descriptors, configExecutionDescriptors()...)
	descriptors = append(descriptors, dataExecutionDescriptors()...)
	descriptors = append(descriptors, providerExecutionDescriptors()...)
	descriptors = append(descriptors, doctorExecutionDescriptors()...)
	descriptors = append(descriptors, secretExecutionDescriptors()...)
	descriptors = append(descriptors, workExecutionDescriptors()...)
	descriptors = append(descriptors, updateExecutionDescriptors()...)
	for _, descriptor := range descriptors {
		if err := registry.Register(descriptor); err != nil {
			return nil, nil, err
		}
	}
	if err := registry.ValidateDispatcher(dispatcher); err != nil {
		return nil, nil, err
	}
	events := execution.NewEventDataRegistry()
	if err := execution.RegisterEventData[workapp.Event](events, "work.event"); err != nil {
		return nil, nil, err
	}
	if err := execution.RegisterEventData[update.Event](events, "update.event"); err != nil {
		return nil, nil, err
	}
	return registry, events, nil
}

func bootstrapExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[openURLRequest, externalResult](actionOpenURL, noLock[openURLRequest]),
		execution.NewJSONDescriptor[guideRequest, guideResult](actionGuide, noLock[guideRequest]),
	}
}

func configExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[config.InitRequest, config.InitReport](config.ActionInit, exclusiveRoot(func(request config.InitRequest) string { return request.Root })),
		execution.NewJSONDescriptor[config.RefreshRequest, config.RefreshReport](config.ActionRefresh, exclusiveRoot(func(request config.RefreshRequest) string { return request.Root })),
		execution.NewJSONDescriptor[config.ShowRequest, config.ConfigShow](config.ActionShow, noLock[config.ShowRequest]),
		execution.NewJSONDescriptor[config.DoctorRequest, config.ConfigDoctorReport](config.ActionDoctor, noLock[config.DoctorRequest]),
		execution.NewJSONDescriptor[config.RootSetRequest, config.RootSetReport](config.ActionRootSet, exclusiveResource[config.RootSetRequest]("user-config")),
		execution.NewJSONDescriptor[config.ColorSetRequest, config.ColorSetReport](config.ActionColorSet, exclusiveResource[config.ColorSetRequest]("user-config")),
		execution.NewJSONDescriptor[config.AgentConfigRequest, config.AgentConfigReport](config.ActionAgentConfig, exclusiveRoot(func(request config.AgentConfigRequest) string { return request.Root })),
		execution.NewJSONDescriptor[config.AgentShowRequest, config.AgentShowReport](config.ActionAgentShow, noLock[config.AgentShowRequest]),
		execution.NewJSONDescriptor[config.AgentDefaultSetRequest, config.AgentDefaultSetReport](config.ActionAgentDefaultSet, exclusiveRoot(func(request config.AgentDefaultSetRequest) string { return request.Root })),
	}
}

func dataExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[dataapp.DataSourceListRequest, dataapp.DataSourceListResult](dataapp.ActionDataSourceList, noLock[dataapp.DataSourceListRequest]),
		execution.NewJSONDescriptor[dataapp.DataSourceCollectRequest, dataapp.DataSourceCollectResult](dataapp.ActionDataSourceCollect, func(request dataapp.DataSourceCollectRequest) (execution.LockSpec, error) {
			if !request.Save {
				return execution.LockSpec{Mode: execution.LockNone}, nil
			}
			return execution.LockSpec{Mode: execution.LockExclusive, Key: request.Root}, nil
		}),
		execution.NewJSONDescriptor[dataapp.GuardRequest, dataapp.GuardResult](dataapp.ActionDataGuard, noLock[dataapp.GuardRequest]),
		execution.NewJSONDescriptor[dataapp.CatalogRequest, dataapp.CatalogResult](dataapp.ActionDataCatalog, noLock[dataapp.CatalogRequest]),
		execution.NewJSONDescriptor[dataapp.DescribeRequest, dataapp.DescribeResult](dataapp.ActionDataDescribe, noLock[dataapp.DescribeRequest]),
		execution.NewJSONDescriptor[dataapp.QueryRequest, dataapp.DataQueryResult](dataapp.ActionDataQuery, noLock[dataapp.QueryRequest]),
		execution.NewJSONDescriptor[dataapp.ReadRequest, dataapp.DataReadResult](dataapp.ActionDataRead, noLock[dataapp.ReadRequest]),
	}
}

func providerExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[providerapp.ListRequest, providerapp.ListReport](providerapp.ActionList, noLock[providerapp.ListRequest]),
		execution.NewJSONDescriptor[providerapp.ShowRequest, providerapp.ShowReport](providerapp.ActionShow, noLock[providerapp.ShowRequest]),
		execution.NewJSONDescriptor[providerapp.CapabilitiesRequest, providerapp.CapabilitiesReport](providerapp.ActionCapabilities, noLock[providerapp.CapabilitiesRequest]),
	}
}

func doctorExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[doctor.Request, doctor.Report](doctor.ActionDoctor, func(request doctor.Request) (execution.LockSpec, error) {
			if !request.Fix {
				return execution.LockSpec{Mode: execution.LockNone}, nil
			}
			return execution.LockSpec{Mode: execution.LockExclusive, Key: request.Root}, nil
		}),
		execution.NewJSONDescriptor[doctor.AgentRequest, doctor.AgentReport](doctor.ActionAgentDoctor, noLock[doctor.AgentRequest]),
	}
}

func secretExecutionDescriptors() []execution.Descriptor {
	set := execution.NewJSONDescriptor[secret.SetRequest, secret.SetReport](secret.ActionSet, exclusiveResource[secret.SetRequest]("user-secrets"))
	set.Request = execution.Codec[secret.SetRequest]{
		Encode: encodeSecretSetRequest,
		Decode: func(execution.Encoded) (secret.SetRequest, error) {
			return secret.SetRequest{}, fmt.Errorf("execution.redacted-request-not-resumable:%s", secret.ActionSet)
		},
	}
	return []execution.Descriptor{
		execution.NewJSONDescriptor[secret.ListRequest, secret.ListReport](secret.ActionList, noLock[secret.ListRequest]),
		set,
		execution.NewJSONDescriptor[secret.GetRequest, secret.GetReport](secret.ActionGet, noLock[secret.GetRequest]),
		execution.NewJSONDescriptor[secret.DeleteRequest, secret.DeleteReport](secret.ActionDelete, exclusiveResource[secret.DeleteRequest]("user-secrets")),
	}
}

func encodeSecretSetRequest(request secret.SetRequest) (execution.Encoded, error) {
	source := "prompt"
	var environment *string
	if request.Value != nil {
		source = "provided"
	}
	if request.Environment != nil {
		source = "environment"
		value := string(*request.Environment)
		environment = &value
	}
	encoded, err := json.Marshal(struct {
		Key         string  `json:"key"`
		Source      string  `json:"source"`
		Environment *string `json:"environment,omitempty"`
	}{Key: string(request.Key), Source: source, Environment: environment})
	if err != nil {
		return execution.Encoded{}, err
	}
	return execution.Encoded{Schema: 1, JSON: encoded, Redacted: true}, nil
}

func workExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[workapp.AuthLoginRequest, workapp.AuthLoginReport](workapp.ActionProviderAuthLogin, exclusiveRoot(func(request workapp.AuthLoginRequest) string { return request.Root })),
		execution.NewJSONDescriptor[workapp.AuthStatusRequest, workapp.AuthStatusReport](workapp.ActionProviderAuthStatus, noLock[workapp.AuthStatusRequest]),
		execution.NewJSONDescriptor[workapp.AuthLogoutRequest, workapp.AuthLogoutReport](workapp.ActionProviderAuthLogout, exclusiveRoot(func(request workapp.AuthLogoutRequest) string { return request.Root })),
		execution.NewJSONDescriptor[workapp.AssignedRequest, workapp.AssignedReport](workapp.ActionWorkItemList, noLock[workapp.AssignedRequest]),
		execution.NewJSONDescriptor[workapp.PullRequestsRequest, workapp.PullRequestsReport](workapp.ActionWorkPullRequestList, noLock[workapp.PullRequestsRequest]),
		execution.NewJSONDescriptor[workapp.ChangelogRequest, workapp.ChangelogReport](workapp.ActionWorkChangelog, noLock[workapp.ChangelogRequest]),
		execution.NewJSONDescriptor[workapp.ContextRequest, workapp.ContextReport](workapp.ActionWorkContextShow, noLock[workapp.ContextRequest]),
		execution.NewJSONDescriptor[workapp.AIContextRequest, workapp.AIContextResult](workapp.ActionWorkContextAI, noLock[workapp.AIContextRequest]),
		execution.NewJSONDescriptor[workapp.ItemShowRequest, workapp.ItemShowReport](workapp.ActionWorkItemShow, noLock[workapp.ItemShowRequest]),
		execution.NewJSONDescriptor[workapp.StatePlanRequest, workapp.StatePlanReport](workapp.ActionWorkItemStatePlan, noLock[workapp.StatePlanRequest]),
		execution.NewJSONDescriptor[workapp.StateExecuteRequest, workapp.StateExecutionReport](workapp.ActionWorkItemStateExecute, exclusiveResource[workapp.StateExecuteRequest]("work-provider")),
		execution.NewJSONDescriptor[workapp.StateSetRequest, workapp.StateSetResult](workapp.ActionWorkItemStateSet, exclusiveResource[workapp.StateSetRequest]("work-provider")),
		execution.NewJSONDescriptor[workapp.DoingRequest, workapp.DoingPlanReport](workapp.ActionWorkItemDoingPlan, noLock[workapp.DoingRequest]),
		execution.NewJSONDescriptor[workapp.DoingActionRequest, workapp.DoingActionResult](workapp.ActionWorkItemDoing, exclusiveResource[workapp.DoingActionRequest]("work-provider")),
		execution.NewJSONDescriptor[workapp.DoingExecuteRequest, workapp.DoingExecutionReport](workapp.ActionWorkItemDoingExecute, exclusiveResource[workapp.DoingExecuteRequest]("work-provider")),
		execution.NewJSONDescriptor[workapp.StartRequest, workapp.StartResult](workapp.ActionWorkspaceStart, func(request workapp.StartRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute || request.PromptToExecute), nil
		}),
		execution.NewJSONDescriptor[workapp.ScratchStartRequest, workapp.ScratchStartResult](workapp.ActionWorkspaceScratchStart, func(request workapp.ScratchStartRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute), nil
		}),
		execution.NewJSONDescriptor[workapp.ScratchPromoteRequest, workapp.ScratchPromoteResult](workapp.ActionWorkspaceScratchPromote, func(request workapp.ScratchPromoteRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute), nil
		}),
		execution.NewJSONDescriptor[workapp.StartPullRequestRequest, workapp.StartPullRequestResult](workapp.ActionWorkspacePullRequestStart, func(request workapp.StartPullRequestRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute), nil
		}),
		execution.NewJSONDescriptor[workapp.OpenRequest, workapp.OpenReport](workapp.ActionWorkspaceOpen, noLock[workapp.OpenRequest]),
		execution.NewJSONDescriptor[workapp.SyncRequest, workapp.SyncReport](workapp.ActionWorkspaceSync, exclusiveRoot(func(request workapp.SyncRequest) string { return request.Root })),
		execution.NewJSONDescriptor[workapp.ContextRefreshRequest, workapp.ContextRefreshReport](workapp.ActionWorkspaceContextRefresh, exclusiveRoot(func(request workapp.ContextRefreshRequest) string { return request.Root })),
		execution.NewJSONDescriptor[workapp.ChildRequest, workapp.ChildReport](workapp.ActionWorkItemChildCreate, exclusiveRoot(func(request workapp.ChildRequest) string { return request.Root })),
		execution.NewJSONDescriptor[workapp.PruneRequest, workapp.PruneReport](workapp.ActionWorkspacePrune, func(request workapp.PruneRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute), nil
		}),
		execution.NewJSONDescriptor[workapp.FinishRequest, workapp.FinishReport](workapp.ActionWorkspaceFinish, func(request workapp.FinishRequest) (execution.LockSpec, error) {
			return conditionalExecutionLock(request.Root, request.Execute), nil
		}),
	}
}

func updateExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[update.Request, update.Report](update.ActionID, func(request update.Request) (execution.LockSpec, error) {
			if request.Check {
				return execution.LockSpec{Mode: execution.LockNone}, nil
			}
			return execution.LockSpec{Mode: execution.LockExclusive, Key: request.ExecutablePath}, nil
		}),
	}
}

func noLock[T action.Request](T) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}

func exclusiveRoot[T action.Request](root func(T) string) func(T) (execution.LockSpec, error) {
	return func(request T) (execution.LockSpec, error) {
		return execution.LockSpec{Mode: execution.LockExclusive, Key: root(request)}, nil
	}
}

func exclusiveResource[T action.Request](key string) func(T) (execution.LockSpec, error) {
	return func(T) (execution.LockSpec, error) {
		return execution.LockSpec{Mode: execution.LockExclusive, Key: "resource:" + key}, nil
	}
}

func conditionalExecutionLock(root string, execute bool) execution.LockSpec {
	if !execute {
		return execution.LockSpec{Mode: execution.LockNone}
	}
	return execution.LockSpec{Mode: execution.LockExclusive, Key: root}
}

type lazyExecutor struct {
	factory  func() (execution.Executor, error)
	once     sync.Once
	executor execution.Executor
	err      error
}

func newLazyExecutor(factory func() (execution.Executor, error)) *lazyExecutor {
	return &lazyExecutor{factory: factory}
}

func (executor *lazyExecutor) start() (execution.Executor, error) {
	executor.once.Do(func() {
		if executor.factory == nil {
			executor.err = fmt.Errorf("execution.invalid-factory")
			return
		}
		executor.executor, executor.err = executor.factory()
	})
	return executor.executor, executor.err
}

func (executor *lazyExecutor) Submit(ctx context.Context, submission execution.Submission) (execution.ExecutionID, error) {
	service, err := executor.start()
	if err != nil {
		return execution.ExecutionID{}, err
	}
	return service.Submit(ctx, submission)
}

func (executor *lazyExecutor) Get(ctx context.Context, actor execution.Actor, id execution.ExecutionID) (execution.Record, error) {
	service, err := executor.start()
	if err != nil {
		return execution.Record{}, err
	}
	return service.Get(ctx, actor, id)
}

func (executor *lazyExecutor) List(ctx context.Context, actor execution.Actor, filter execution.ListFilter) ([]execution.Record, error) {
	service, err := executor.start()
	if err != nil {
		return nil, err
	}
	return service.List(ctx, actor, filter)
}

func (executor *lazyExecutor) Cancel(ctx context.Context, actor execution.Actor, id execution.ExecutionID) error {
	service, err := executor.start()
	if err != nil {
		return err
	}
	return service.Cancel(ctx, actor, id)
}

func (executor *lazyExecutor) Respond(ctx context.Context, actor execution.Actor, id execution.ExecutionID, promptID action.PromptID, response action.Response) error {
	service, err := executor.start()
	if err != nil {
		return err
	}
	return service.Respond(ctx, actor, id, promptID, response)
}

func (executor *lazyExecutor) Subscribe(ctx context.Context, actor execution.Actor, id execution.ExecutionID, after execution.EventSequence) (execution.Subscription, error) {
	service, err := executor.start()
	if err != nil {
		return execution.Subscription{}, err
	}
	return service.Subscribe(ctx, actor, id, after)
}

func (executor *lazyExecutor) Wait(ctx context.Context, actor execution.Actor, id execution.ExecutionID) (execution.Record, error) {
	service, err := executor.start()
	if err != nil {
		return execution.Record{}, err
	}
	return service.Wait(ctx, actor, id)
}

func (executor *lazyExecutor) Close(ctx context.Context) error {
	if executor.executor == nil {
		return executor.err
	}
	return executor.executor.Close(ctx)
}
