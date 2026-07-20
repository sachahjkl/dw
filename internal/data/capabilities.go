package data

import (
	"context"

	"github.com/sachahjkl/dw/internal/contract"
)

// Provider supplies identity only. Operations are optional capabilities.
type Provider interface{ Name() ProviderName }

type Capability string

const (
	CapabilityDiscoverer         Capability = "discoverer"
	CapabilityCataloger          Capability = "cataloger"
	CapabilityDescriber          Capability = "describer"
	CapabilityNativeQuerier      Capability = "native-querier"
	CapabilityTabularReader      Capability = "tabular-reader"
	CapabilityWorkbookReader     Capability = "workbook-reader"
	CapabilityDocumentReader     Capability = "document-reader"
	CapabilityReadPolicy         Capability = "read-policy"
	CapabilityCredentialResolver Capability = "credential-resolver"
)

type Discoverer interface {
	Provider
	Discover(context.Context, DiscoveryRequest) (DiscoveryReport, error)
}

type Cataloger interface {
	Provider
	Catalog(context.Context, Connection) ([]CatalogEntry, error)
}

type Describer interface {
	Provider
	Describe(context.Context, Connection, ObjectRef) (Description, error)
}

type NativeQuerier interface {
	Provider
	QueryNative(context.Context, Connection, NativeQuery) (Table, error)
}

type TabularReader interface {
	Provider
	ReadTable(context.Context, Connection, TabularRead) (Table, error)
}

type WorkbookReader interface {
	Provider
	ReadWorkbook(context.Context, Connection, WorkbookRead) (Table, error)
}

type DocumentReader interface {
	Provider
	ReadDocument(context.Context, Connection, DocumentRead) (Document, error)
}

// ReadPolicy is a mandatory pre-execution check for providers that accept a
// native query language. A provider may implement both policy and execution.
type ReadPolicy interface {
	Provider
	ValidateRead(context.Context, Connection, NativeQuery) error
}

type CredentialResolver interface {
	Provider
	ResolveCredential(context.Context, Connection, contract.SecretStore) (contract.SecretValue, error)
}
