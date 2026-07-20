package dataapp

import (
	"context"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
)

type Service struct {
	providers *data.Registry
	secrets   contract.SecretStore
}

func NewService(providers *data.Registry, secrets contract.SecretStore) *Service {
	return &Service{providers: providers, secrets: secrets}
}

func (service *Service) List(explicitRoot, selectedProvider string) (DataSourceListReport, error) {
	report, err := Inventory(config.ResolveRoot(explicitRoot))
	if err != nil || service.providers == nil {
		return report, err
	}
	if selectedProvider != "" {
		providerName := registryProviderName(selectedProvider)
		if _, providerErr := service.providers.Get(providerName); providerErr != nil {
			return DataSourceListReport{}, providerErr
		}
		entries := report.Entries[:0]
		for _, entry := range report.Entries {
			if registryProviderName(entry.Provider) == providerName {
				entries = append(entries, entry)
			}
		}
		report.Entries = entries
	}
	unsupported := l10n.Text("data.warning.unsupported")
	for index := range report.Entries {
		if _, providerErr := service.providers.Get(registryProviderName(report.Entries[index].Provider)); providerErr != nil {
			report.Entries[index].Warnings = append(report.Entries[index].Warnings, unsupported)
		}
	}
	return report, nil
}

type Selection struct {
	Provider string
	Root     string
	Project  string
	Source   string
	Env      string
}

func (selection Selection) resolved() (root, project, source string) {
	root = config.ResolveRoot(selection.Root)
	project = selection.Project
	if strings.TrimSpace(project) == "" {
		project = "default"
	}
	source = selection.Source
	if strings.TrimSpace(source) == "" {
		source = selection.Env
	}
	if strings.TrimSpace(source) == "" {
		source = "dev"
	}
	return
}

func (service *Service) Resolve(selection Selection) (data.Connection, error) {
	root, project, source := selection.resolved()
	catalog, err := LoadCatalog(filepath.Join(root, "config", "databases.json"))
	if err != nil {
		return data.Connection{}, err
	}
	connection, err := catalog.Resolve(project, source)
	if err == nil && strings.TrimSpace(selection.Provider) != "" {
		connection.Source.Provider = data.ProviderName(strings.TrimSpace(selection.Provider))
	}
	return connection, err
}

func (service *Service) selectedProvider(name string) (data.Provider, error) {
	if service == nil || service.providers == nil {
		return nil, &data.ProviderNotFoundError{Provider: data.ProviderName(strings.TrimSpace(name))}
	}
	if strings.TrimSpace(name) != "" {
		return service.providers.Get(registryProviderName(name))
	}
	providers := service.providers.Providers()
	if len(providers) == 0 {
		return nil, &data.ProviderNotFoundError{}
	}
	return providers[0], nil
}

func (service *Service) provider(connection data.Connection) (data.Provider, error) {
	return service.selectedProvider(string(connection.Source.Provider))
}

func (service *Service) Guard(ctx context.Context, providerName, statement string) (GuardReport, error) {
	provider, err := service.selectedProvider(providerName)
	if err != nil {
		return GuardReport{}, err
	}
	policy, err := data.Require[data.ReadPolicy](provider, data.CapabilityReadPolicy)
	if err != nil {
		return GuardReport{}, err
	}
	connection := data.Connection{Source: data.Source{Provider: provider.Name()}}
	if err := policy.ValidateRead(ctx, connection, data.NativeQuery{Statement: statement}); err != nil {
		reason := err.Error()
		return GuardReport{IsAllowed: false, Reason: &reason}, nil
	}
	return GuardReport{IsAllowed: true}, nil
}

func (service *Service) Catalog(ctx context.Context, selection Selection) (NativeQueryReport, error) {
	connection, err := service.Resolve(selection)
	if err != nil {
		return NativeQueryReport{}, err
	}
	provider, err := service.provider(connection)
	if err != nil {
		return NativeQueryReport{}, err
	}
	cataloger, err := data.Require[data.Cataloger](provider, data.CapabilityCataloger)
	if err != nil {
		return NativeQueryReport{}, err
	}
	entries, err := cataloger.Catalog(ctx, connection)
	if err != nil {
		return NativeQueryReport{}, err
	}
	table := data.Table{Rows: make([][]data.Value, 0, len(entries))}
	if len(entries) > 0 {
		table.Columns = []data.Column{{Name: "TABLE_SCHEMA", Ordinal: 1}, {Name: "TABLE_NAME", Ordinal: 2}, {Name: "TABLE_TYPE", Ordinal: 3}}
	}
	for _, entry := range entries {
		typeName := "BASE TABLE"
		if entry.Kind == data.CatalogView {
			typeName = "VIEW"
		}
		table.Rows = append(table.Rows, []data.Value{data.StringValue(entry.Schema), data.StringValue(entry.Name), data.StringValue(typeName)})
	}
	return ProjectTable(table), nil
}

type tableDescriber interface {
	DescribeTable(context.Context, data.Connection, data.ObjectRef) (data.Table, error)
}

func (service *Service) Describe(ctx context.Context, selection Selection, tableName string) (*NativeQueryReport, error) {
	if strings.TrimSpace(tableName) == "" {
		return nil, &MissingTableError{}
	}
	connection, err := service.Resolve(selection)
	if err != nil {
		return nil, err
	}
	provider, err := service.provider(connection)
	if err != nil {
		return nil, err
	}
	describer, err := data.Require[data.Describer](provider, data.CapabilityDescriber)
	if err != nil {
		return nil, err
	}
	schema, name := "dbo", tableName
	if before, after, found := strings.Cut(tableName, "."); found {
		schema, name = before, after
	}
	object := data.ObjectRef{Schema: schema, Name: name}
	if detailed, ok := provider.(tableDescriber); ok {
		table, detailErr := detailed.DescribeTable(ctx, connection, object)
		if detailErr != nil {
			return nil, detailErr
		}
		result := ProjectTable(table)
		return &result, nil
	}
	description, err := describer.Describe(ctx, connection, object)
	if err != nil {
		return nil, err
	}
	table := data.Table{Rows: make([][]data.Value, 0, len(description.Columns))}
	if len(description.Columns) > 0 {
		table.Columns = []data.Column{{Name: "COLUMN_NAME", Ordinal: 1}, {Name: "DATA_TYPE", Ordinal: 2}, {Name: "IS_NULLABLE", Ordinal: 3}, {Name: "CHARACTER_MAXIMUM_LENGTH", Ordinal: 4}}
	}
	for _, column := range description.Columns {
		nullable := "NO"
		if column.Nullable {
			nullable = "YES"
		}
		table.Rows = append(table.Rows, []data.Value{data.StringValue(column.Name), data.StringValue(column.NativeType), data.StringValue(nullable), data.NullValue()})
	}
	result := ProjectTable(table)
	return &result, nil
}

func (service *Service) Query(ctx context.Context, selection Selection, statement string, maximumRows *int) (NativeQueryReport, error) {
	connection, err := service.Resolve(selection)
	if err != nil {
		return NativeQueryReport{}, err
	}
	provider, err := service.provider(connection)
	if err != nil {
		return NativeQueryReport{}, err
	}
	query := data.NativeQuery{Statement: statement}
	if maximumRows != nil {
		query.MaximumRows = *maximumRows
	}
	policy, err := data.Require[data.ReadPolicy](provider, data.CapabilityReadPolicy)
	if err != nil {
		return NativeQueryReport{}, err
	}
	if err := policy.ValidateRead(ctx, connection, query); err != nil {
		return NativeQueryReport{}, err
	}
	querier, err := data.Require[data.NativeQuerier](provider, data.CapabilityNativeQuerier)
	if err != nil {
		return NativeQueryReport{}, err
	}
	table, err := querier.QueryNative(ctx, connection, query)
	if err != nil {
		return NativeQueryReport{}, err
	}
	return ProjectTable(table), nil
}

type MissingTableError struct{}

func (*MissingTableError) Error() string { return l10n.Text("data.error.missing_table") }
