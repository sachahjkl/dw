package paritytest_test

import (
	"errors"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/work/ado"
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
