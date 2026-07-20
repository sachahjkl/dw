// Package providerapp exposes provider registry introspection without depending
// on any concrete provider implementation.
package providerapp

import (
	"context"
	"fmt"
	"sort"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/work"
)

const (
	ActionList         action.ID = "provider.list"
	ActionShow         action.ID = "provider.show"
	ActionCapabilities action.ID = "provider.capabilities"
)

type Kind string

const (
	KindWork Kind = "work"
	KindData Kind = "data"
)

type Summary struct {
	Name         string   `json:"name"`
	Kinds        []Kind   `json:"kinds"`
	Capabilities []string `json:"capabilities"`
}

type Details struct {
	Name         string   `json:"name"`
	Kinds        []Kind   `json:"kinds"`
	Capabilities []string `json:"capabilities"`
}

type ListRequest struct{}
type ShowRequest struct {
	Provider string `json:"provider"`
}
type CapabilitiesRequest struct {
	Provider string `json:"provider"`
}

type ListReport struct {
	Providers []Summary `json:"providers"`
}
type ShowReport struct {
	Provider Details `json:"provider"`
}
type CapabilitiesReport struct {
	Provider     string   `json:"provider"`
	Kinds        []Kind   `json:"kinds"`
	Capabilities []string `json:"capabilities"`
}

func (ListRequest) ActionID() action.ID         { return ActionList }
func (ShowRequest) ActionID() action.ID         { return ActionShow }
func (CapabilitiesRequest) ActionID() action.ID { return ActionCapabilities }
func (ListReport) ActionID() action.ID          { return ActionList }
func (ShowReport) ActionID() action.ID          { return ActionShow }
func (CapabilitiesReport) ActionID() action.ID  { return ActionCapabilities }

type ProviderNotFoundError struct{ Provider string }

func (e *ProviderNotFoundError) Error() string { return "provider.not-found:" + e.Provider }
func (e *ProviderNotFoundError) Localized() l10n.Message {
	return l10n.M("error.provider-not-found", l10n.A("provider", e.Provider))
}

type Service struct {
	Work *work.Registry
	Data *data.Registry
}

func New(workProviders *work.Registry, dataProviders *data.Registry) *Service {
	return &Service{Work: workProviders, Data: dataProviders}
}

func (service *Service) List() ListReport {
	details := service.providers()
	report := ListReport{Providers: make([]Summary, len(details))}
	for index, provider := range details {
		report.Providers[index] = Summary{Name: provider.Name, Kinds: append([]Kind{}, provider.Kinds...), Capabilities: append([]string{}, provider.Capabilities...)}
	}
	return report
}

func (service *Service) Show(name string) (ShowReport, error) {
	provider, found := service.find(name)
	if !found {
		return ShowReport{}, &ProviderNotFoundError{Provider: name}
	}
	return ShowReport{Provider: provider}, nil
}

func (service *Service) Capabilities(name string) (CapabilitiesReport, error) {
	provider, found := service.find(name)
	if !found {
		return CapabilitiesReport{}, &ProviderNotFoundError{Provider: name}
	}
	return CapabilitiesReport{Provider: provider.Name, Kinds: provider.Kinds, Capabilities: provider.Capabilities}, nil
}

func (service *Service) ProviderNames() []string {
	providers := service.providers()
	names := make([]string, len(providers))
	for index := range providers {
		names[index] = providers[index].Name
	}
	return names
}

func (service *Service) find(name string) (Details, bool) {
	for _, provider := range service.providers() {
		if provider.Name == name {
			return provider, true
		}
	}
	return Details{}, false
}

func (service *Service) providers() []Details {
	providers := make([]Details, 0)
	indexes := make(map[string]int)
	if service != nil && service.Work != nil {
		for _, provider := range service.Work.Providers() {
			name := string(provider.Name())
			indexes[name] = len(providers)
			providers = append(providers, Details{Name: name, Kinds: []Kind{KindWork}, Capabilities: workCapabilities(provider)})
		}
	}
	if service != nil && service.Data != nil {
		for _, provider := range service.Data.Providers() {
			name := string(provider.Name())
			if index, exists := indexes[name]; exists {
				providers[index].Kinds = append(providers[index].Kinds, KindData)
				providers[index].Capabilities = append(providers[index].Capabilities, dataCapabilities(provider)...)
				continue
			}
			indexes[name] = len(providers)
			providers = append(providers, Details{Name: name, Kinds: []Kind{KindData}, Capabilities: dataCapabilities(provider)})
		}
	}
	for index := range providers {
		sort.Strings(providers[index].Capabilities)
	}
	return providers
}

func workCapabilities(provider work.Provider) []string {
	capabilities := make([]string, 0, 12)
	appendCapability := func(capability work.Capability, supported bool) {
		if supported {
			capabilities = append(capabilities, string(capability))
		}
	}
	_, authenticator := provider.(work.Authenticator)
	appendCapability(work.CapabilityAuthenticator, authenticator)
	_, itemReader := provider.(work.ItemReader)
	appendCapability(work.CapabilityItemReader, itemReader)
	_, assignedQuerier := provider.(work.AssignedQuerier)
	appendCapability(work.CapabilityAssignedQuerier, assignedQuerier)
	_, relationReader := provider.(work.RelationReader)
	appendCapability(work.CapabilityRelationReader, relationReader)
	_, stateWriter := provider.(work.StateWriter)
	appendCapability(work.CapabilityStateWriter, stateWriter)
	_, stateClassifier := provider.(work.StateClassifier)
	appendCapability(work.CapabilityStateClassifier, stateClassifier)
	_, childCreator := provider.(work.ChildCreator)
	appendCapability(work.CapabilityChildCreator, childCreator)
	_, pullRequestReader := provider.(work.PullRequestReader)
	appendCapability(work.CapabilityPullRequestReader, pullRequestReader)
	_, pullRequestWriter := provider.(work.PullRequestWriter)
	appendCapability(work.CapabilityPullRequestWriter, pullRequestWriter)
	_, richContextReader := provider.(work.RichContextReader)
	appendCapability(work.CapabilityRichContextReader, richContextReader)
	_, rawItemReader := provider.(work.RawItemReader)
	appendCapability(work.CapabilityRawItemReader, rawItemReader)
	_, commitReferenceExtractor := provider.(work.CommitReferenceExtractor)
	appendCapability(work.CapabilityCommitReferenceExtractor, commitReferenceExtractor)
	return capabilities
}

func dataCapabilities(provider data.Provider) []string {
	capabilities := make([]string, 0, 9)
	appendCapability := func(capability data.Capability, supported bool) {
		if supported {
			capabilities = append(capabilities, string(capability))
		}
	}
	_, discoverer := provider.(data.Discoverer)
	appendCapability(data.CapabilityDiscoverer, discoverer)
	_, cataloger := provider.(data.Cataloger)
	appendCapability(data.CapabilityCataloger, cataloger)
	_, describer := provider.(data.Describer)
	appendCapability(data.CapabilityDescriber, describer)
	_, nativeQuerier := provider.(data.NativeQuerier)
	appendCapability(data.CapabilityNativeQuerier, nativeQuerier)
	_, tabularReader := provider.(data.TabularReader)
	appendCapability(data.CapabilityTabularReader, tabularReader)
	_, workbookReader := provider.(data.WorkbookReader)
	appendCapability(data.CapabilityWorkbookReader, workbookReader)
	_, documentReader := provider.(data.DocumentReader)
	appendCapability(data.CapabilityDocumentReader, documentReader)
	_, readPolicy := provider.(data.ReadPolicy)
	appendCapability(data.CapabilityReadPolicy, readPolicy)
	_, credentialResolver := provider.(data.CredentialResolver)
	appendCapability(data.CapabilityCredentialResolver, credentialResolver)
	return capabilities
}

type handler struct {
	id      action.ID
	service *Service
}

func Handlers(service *Service) []action.Handler {
	return []action.Handler{
		handler{id: ActionList, service: service},
		handler{id: ActionShow, service: service},
		handler{id: ActionCapabilities, service: service},
	}
}

func (handler handler) ID() action.ID { return handler.id }

func (handler handler) Execute(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	if handler.service == nil {
		return nil, fmt.Errorf("provider.nil-service")
	}
	switch handler.id {
	case ActionList:
		if _, ok := request.(ListRequest); !ok {
			return nil, fmt.Errorf("provider.invalid-request:%s:%T", handler.id, request)
		}
		return handler.service.List(), nil
	case ActionShow:
		value, ok := request.(ShowRequest)
		if !ok {
			return nil, fmt.Errorf("provider.invalid-request:%s:%T", handler.id, request)
		}
		return handler.service.Show(value.Provider)
	case ActionCapabilities:
		value, ok := request.(CapabilitiesRequest)
		if !ok {
			return nil, fmt.Errorf("provider.invalid-request:%s:%T", handler.id, request)
		}
		return handler.service.Capabilities(value.Provider)
	default:
		return nil, fmt.Errorf("provider.unknown-action:%s", handler.id)
	}
}
