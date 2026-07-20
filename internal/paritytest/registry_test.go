package paritytest_test

import (
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/providerapp"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/work/ado"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
)

func TestADOStaticProviderAdvertisesEveryImplementedCapability(t *testing.T) {
	provider := ado.NewWithStore(ado.Options{}, nil, nil)
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	registered, err := registry.Get(ado.ProviderName)
	if err != nil {
		t.Fatal(err)
	}
	if registered != provider {
		t.Fatal("registry returned a different provider instance")
	}
	providers := registry.Providers()
	if len(providers) != 1 || providers[0].Name() != ado.ProviderName {
		t.Fatalf("registration order = %#v, want azure-devops", providers)
	}

	assertCapability[work.Authenticator](t, registered, work.CapabilityAuthenticator)
	assertCapability[work.ItemReader](t, registered, work.CapabilityItemReader)
	assertCapability[work.AssignedQuerier](t, registered, work.CapabilityAssignedQuerier)
	assertCapability[work.RelationReader](t, registered, work.CapabilityRelationReader)
	assertCapability[work.StateWriter](t, registered, work.CapabilityStateWriter)
	assertCapability[work.StateClassifier](t, registered, work.CapabilityStateClassifier)
	assertCapability[work.ChildCreator](t, registered, work.CapabilityChildCreator)
	assertCapability[work.PullRequestReader](t, registered, work.CapabilityPullRequestReader)
	assertCapability[work.PullRequestWriter](t, registered, work.CapabilityPullRequestWriter)
	assertCapability[work.RichContextReader](t, registered, work.CapabilityRichContextReader)
	assertCapability[work.RawItemReader](t, registered, work.CapabilityRawItemReader)
}

func assertCapability[T any](t *testing.T, provider work.Provider, capability work.Capability) {
	t.Helper()
	implementation, err := work.Require[T](provider, capability)
	if err != nil {
		t.Fatalf("Require(%s): %v", capability, err)
	}
	if any(implementation) == nil {
		t.Fatalf("Require(%s) returned nil implementation", capability)
	}
}

func TestWorkRegistryRejectsDuplicateStaticProvider(t *testing.T) {
	registry := work.NewRegistry()
	provider := ado.NewWithStore(ado.Options{}, nil, nil)
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	err := registry.Register(provider)
	var duplicate *work.DuplicateProviderError
	if !errors.As(err, &duplicate) || duplicate.Provider != ado.ProviderName {
		t.Fatalf("duplicate registration error = %#v, want typed azure-devops duplicate", err)
	}
	if got := registry.Providers(); len(got) != 1 {
		t.Fatalf("duplicate changed registry contents: %#v", got)
	}
}

func TestMissingWorkProviderUsesTypedError(t *testing.T) {
	_, err := work.NewRegistry().Get(work.ProviderName("missing"))
	var missing *work.ProviderNotFoundError
	if !errors.As(err, &missing) || missing.Provider != work.ProviderName("missing") {
		t.Fatalf("missing provider error = %#v", err)
	}
}

func TestSQLServerStaticProviderCapabilitiesAndUnsupportedErrors(t *testing.T) {
	provider := sqlserver.New(nil)
	registry := data.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	registered, err := registry.Get(data.ProviderName(sqlserver.ProviderName))
	if err != nil {
		t.Fatal(err)
	}
	assertDataCapability[data.Cataloger](t, registered, data.CapabilityCataloger)
	assertDataCapability[data.Describer](t, registered, data.CapabilityDescriber)
	assertDataCapability[data.NativeQuerier](t, registered, data.CapabilityNativeQuerier)
	assertDataCapability[data.TabularReader](t, registered, data.CapabilityTabularReader)
	assertDataCapability[data.ReadPolicy](t, registered, data.CapabilityReadPolicy)
	assertDataCapability[data.CredentialResolver](t, registered, data.CapabilityCredentialResolver)

	_, err = data.Require[data.DocumentReader](registered, data.CapabilityDocumentReader)
	var unsupported *data.UnsupportedCapabilityError
	if !errors.As(err, &unsupported) {
		t.Fatalf("unsupported document capability error = %#v", err)
	}
	if unsupported.Provider != data.ProviderName(sqlserver.ProviderName) || unsupported.Capability != data.CapabilityDocumentReader {
		t.Fatalf("unsupported capability detail = %#v", unsupported)
	}
	if got := registry.Providers(); len(got) != 1 || got[0].Name() != data.ProviderName(sqlserver.ProviderName) {
		t.Fatalf("data registration order = %#v", got)
	}
}

func assertDataCapability[T any](t *testing.T, provider data.Provider, capability data.Capability) {
	t.Helper()
	implementation, err := data.Require[T](provider, capability)
	if err != nil {
		t.Fatalf("Require(%s): %v", capability, err)
	}
	if any(implementation) == nil {
		t.Fatalf("Require(%s) returned nil implementation", capability)
	}
}

type namedWorkProvider string

func (provider namedWorkProvider) Name() work.ProviderName { return work.ProviderName(provider) }

type namedDataProvider string

func (provider namedDataProvider) Name() data.ProviderName { return data.ProviderName(provider) }

func TestProviderReportsFollowRegistryOrderAndCoalesceKinds(t *testing.T) {
	workRegistry := work.NewRegistry()
	dataRegistry := data.NewRegistry()
	for _, provider := range []work.Provider{namedWorkProvider("zeta"), namedWorkProvider("shared")} {
		if err := workRegistry.Register(provider); err != nil {
			t.Fatal(err)
		}
	}
	for _, provider := range []data.Provider{namedDataProvider("shared"), namedDataProvider("alpha")} {
		if err := dataRegistry.Register(provider); err != nil {
			t.Fatal(err)
		}
	}

	service := providerapp.New(workRegistry, dataRegistry)
	want := []providerapp.Summary{
		{Name: "zeta", Kinds: []providerapp.Kind{providerapp.KindWork}, Capabilities: []string{}},
		{Name: "shared", Kinds: []providerapp.Kind{providerapp.KindWork, providerapp.KindData}, Capabilities: []string{}},
		{Name: "alpha", Kinds: []providerapp.Kind{providerapp.KindData}, Capabilities: []string{}},
	}
	if got := service.List().Providers; !reflect.DeepEqual(got, want) {
		t.Fatalf("ordered provider list = %#v, want %#v", got, want)
	}
	if got, wantNames := service.ProviderNames(), []string{"zeta", "shared", "alpha"}; !reflect.DeepEqual(got, wantNames) {
		t.Fatalf("provider names = %#v, want %#v", got, wantNames)
	}
	show, err := service.Show("shared")
	if err != nil {
		t.Fatal(err)
	}
	if got, wantKinds := show.Provider.Kinds, []providerapp.Kind{providerapp.KindWork, providerapp.KindData}; !reflect.DeepEqual(got, wantKinds) {
		t.Fatalf("shared provider kinds = %#v, want %#v", got, wantKinds)
	}
	_, err = service.Capabilities("missing")
	var missing *providerapp.ProviderNotFoundError
	if !errors.As(err, &missing) || missing.Provider != "missing" {
		t.Fatalf("missing provider report error = %#v", err)
	}
}

func TestProviderCapabilitiesAreDerivedFromInterfacesAndSorted(t *testing.T) {
	workRegistry := work.NewRegistry()
	dataRegistry := data.NewRegistry()
	if err := workRegistry.Register(ado.NewWithStore(ado.Options{}, nil, nil)); err != nil {
		t.Fatal(err)
	}
	if err := dataRegistry.Register(sqlserver.New(nil)); err != nil {
		t.Fatal(err)
	}
	providers := providerapp.New(workRegistry, dataRegistry).List().Providers
	want := []providerapp.Summary{
		{
			Name:  string(ado.ProviderName),
			Kinds: []providerapp.Kind{providerapp.KindWork},
			Capabilities: []string{
				"assigned-querier", "authenticator", "child-creator", "commit-reference-extractor", "item-reader",
				"pull-request-reader", "pull-request-writer", "raw-item-reader", "relation-reader",
				"rich-context-reader", "state-classifier", "state-writer",
			},
		},
		{
			Name:  sqlserver.ProviderName,
			Kinds: []providerapp.Kind{providerapp.KindData},
			Capabilities: []string{
				"cataloger", "credential-resolver", "describer", "discoverer", "native-querier", "read-policy", "tabular-reader",
			},
		},
	}
	if !reflect.DeepEqual(providers, want) {
		t.Fatalf("provider capability report = %#v, want %#v", providers, want)
	}
}

type recordingAuthenticator struct {
	name        work.ProviderName
	statusCalls int
}

func (provider *recordingAuthenticator) Name() work.ProviderName { return provider.name }
func (provider *recordingAuthenticator) AuthStatus(context.Context, work.ProjectRef) (work.AuthStatus, error) {
	provider.statusCalls++
	return work.AuthStatus{}, nil
}
func (*recordingAuthenticator) Login(context.Context, work.ProjectRef, work.AuthMode, func(work.DeviceLogin) error) (work.AuthStatus, error) {
	return work.AuthStatus{}, nil
}
func (*recordingAuthenticator) Logout(context.Context, work.ProjectRef) (bool, error) {
	return false, nil
}

func TestProviderAuthRequestSelectsNamedProvider(t *testing.T) {
	registry := work.NewRegistry()
	first := &recordingAuthenticator{name: "first"}
	selected := &recordingAuthenticator{name: "selected"}
	if err := registry.Register(first); err != nil {
		t.Fatal(err)
	}
	if err := registry.Register(selected); err != nil {
		t.Fatal(err)
	}
	service := workapp.New(registry)
	if _, err := service.AuthStatus(context.Background(), workapp.AuthStatusRequest{Provider: "selected"}); err != nil {
		t.Fatal(err)
	}
	if first.statusCalls != 0 || selected.statusCalls != 1 {
		t.Fatalf("auth dispatch calls: first=%d selected=%d", first.statusCalls, selected.statusCalls)
	}
}

type recordingItemProvider struct {
	name    work.ProviderName
	calls   int
	project work.ProjectRef
}

func (provider *recordingItemProvider) Name() work.ProviderName { return provider.name }
func (provider *recordingItemProvider) ReadItems(_ context.Context, project work.ProjectRef, ids []work.ItemID, _ work.ReadOptions) ([]work.Item, error) {
	provider.calls++
	provider.project = project
	return []work.Item{{ID: ids[0], Title: "selected"}}, nil
}

func TestWorkspaceWorkPortResolvesProviderFromRegistry(t *testing.T) {
	registry := work.NewRegistry()
	first := &recordingItemProvider{name: "first"}
	selected := &recordingItemProvider{name: "selected"}
	if err := registry.Register(first); err != nil {
		t.Fatal(err)
	}
	if err := registry.Register(selected); err != nil {
		t.Fatal(err)
	}
	port := workspace.CapabilityWorkPort{
		Providers: registry,
		ResolveProvider: func(context.Context, string) work.ProviderName {
			return selected.Name()
		},
	}
	items, err := port.GetWorkItems(context.Background(), "sample", []string{"42"})
	if err != nil {
		t.Fatal(err)
	}
	if first.calls != 0 || selected.calls != 1 {
		t.Fatalf("provider calls = first:%d selected:%d", first.calls, selected.calls)
	}
	if selected.project.Project != "sample" || len(items) != 1 || items[0].ID != "42" {
		t.Fatalf("resolved project/items = %#v, %#v", selected.project, items)
	}
}
