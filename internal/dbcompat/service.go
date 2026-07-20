package dbcompat

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

func (service *Service) List(explicitRoot string) (DatabaseListReport, error) {
	report, err := Inventory(config.ResolveRoot(explicitRoot))
	if err != nil || service.providers == nil {
		return report, err
	}
	unsupported := l10n.Text("db.warning.unsupported")
	for index := range report.Entries {
		if _, providerErr := service.providers.Get(registryProviderName(report.Entries[index].Provider)); providerErr != nil {
			continue
		}
		warnings := report.Entries[index].Warnings[:0]
		for _, warning := range report.Entries[index].Warnings {
			if warning != unsupported {
				warnings = append(warnings, warning)
			}
		}
		report.Entries[index].Warnings = warnings
	}
	return report, nil
}

func (service *Service) Collect(ctx context.Context, explicitRoot string, workspaces []Workspace, save bool) (DatabaseCollectReport, error) {
	mode := Preview
	if save {
		mode = Save
	}
	return CollectAppSettings(ctx, config.ResolveRoot(explicitRoot), workspaces, mode, service.secrets)
}

type Selection struct {
	Root     string
	Project  string
	Database string
	Env      string
}

func (selection Selection) resolved() (root, project, database string) {
	root = config.ResolveRoot(selection.Root)
	project = selection.Project
	if strings.TrimSpace(project) == "" {
		project = "default"
	}
	database = selection.Database
	if strings.TrimSpace(database) == "" {
		database = selection.Env
	}
	if strings.TrimSpace(database) == "" {
		database = "dev"
	}
	return
}

func (service *Service) Resolve(selection Selection) (data.Connection, error) {
	root, project, database := selection.resolved()
	catalog, err := LoadCatalog(filepath.Join(root, "config", "databases.json"))
	if err != nil {
		return data.Connection{}, err
	}
	return catalog.Resolve(project, database)
}

func (service *Service) provider(connection data.Connection) (data.Provider, error) {
	if service.providers == nil {
		return nil, &data.ProviderNotFoundError{Provider: connection.Source.Provider}
	}
	return service.providers.Get(connection.Source.Provider)
}

func (service *Service) Guard(statement string) SQLGuardResult { return Guard(statement) }

func (service *Service) Schema(ctx context.Context, selection Selection) (QueryResult, error) {
	connection, err := service.Resolve(selection)
	if err != nil {
		return QueryResult{}, err
	}
	provider, err := service.provider(connection)
	if err != nil {
		return QueryResult{}, err
	}
	cataloger, err := data.Require[data.Cataloger](provider, data.CapabilityCataloger)
	if err != nil {
		return QueryResult{}, err
	}
	entries, err := cataloger.Catalog(ctx, connection)
	if err != nil {
		return QueryResult{}, err
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

func (service *Service) Describe(ctx context.Context, selection Selection, tableName string) (*QueryResult, error) {
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

func (service *Service) Query(ctx context.Context, selection Selection, statement string, maximumRows *int) (QueryResult, error) {
	connection, err := service.Resolve(selection)
	if err != nil {
		return QueryResult{}, err
	}
	provider, err := service.provider(connection)
	if err != nil {
		return QueryResult{}, err
	}
	query := data.NativeQuery{Statement: statement}
	if maximumRows != nil {
		query.MaximumRows = *maximumRows
	}
	policy, err := data.Require[data.ReadPolicy](provider, data.CapabilityReadPolicy)
	if err != nil {
		return QueryResult{}, err
	}
	if err := policy.ValidateRead(ctx, connection, query); err != nil {
		return QueryResult{}, err
	}
	querier, err := data.Require[data.NativeQuerier](provider, data.CapabilityNativeQuerier)
	if err != nil {
		return QueryResult{}, err
	}
	table, err := querier.QueryNative(ctx, connection, query)
	if err != nil {
		return QueryResult{}, err
	}
	return ProjectTable(table), nil
}

type MissingTableError struct{}

func (*MissingTableError) Error() string { return l10n.Text("db.error.missing_table") }
