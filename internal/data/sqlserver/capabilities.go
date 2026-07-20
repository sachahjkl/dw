package sqlserver

import (
	"context"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func (provider *Provider) ValidateRead(_ context.Context, connection data.Connection, query data.NativeQuery) error {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return err
	}
	if _, err := Resolve(resolved.Config, resolved.Defaults); err != nil {
		return err
	}
	guard := ValidateReadOnlySQL(query.Statement)
	if guard.IsAllowed {
		return nil
	}
	reason := l10n.Text("data.error.unknown_reason")
	if guard.Reason != nil {
		reason = *guard.Reason
	}
	return &ProviderError{Kind: ErrorBlockedQuery, Reason: reason}
}

func (provider *Provider) ResolveCredential(ctx context.Context, connection data.Connection, store contract.SecretStore) (contract.SecretValue, error) {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return contract.SecretValue{}, err
	}
	if store == nil {
		store = provider.secrets
	}
	return ResolveConnectionString(ctx, resolved.Config, store)
}

func (provider *Provider) QueryNative(ctx context.Context, connection data.Connection, query data.NativeQuery) (data.Table, error) {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return data.Table{}, err
	}
	if query.TimeoutSeconds > 0 {
		resolved.Config.TimeoutSeconds = intPointer(query.TimeoutSeconds)
	}
	var maximum *int
	if query.MaximumRows != 0 {
		maximum = intPointer(query.MaximumRows)
	}
	result, err := provider.Query(ctx, resolved, query.Statement, maximum)
	if err != nil {
		return data.Table{}, err
	}
	return genericTable(result), nil
}

func (provider *Provider) Catalog(ctx context.Context, connection data.Connection) ([]data.CatalogEntry, error) {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return nil, err
	}
	result, err := provider.CatalogNative(ctx, resolved)
	if err != nil {
		return nil, err
	}
	entries := make([]data.CatalogEntry, 0, len(result.Rows))
	for _, row := range result.Rows {
		if len(row) < 3 || !row[0].Valid || !row[1].Valid {
			continue
		}
		kind := data.CatalogTable
		if row[2].Valid && strings.EqualFold(row[2].Value, "VIEW") {
			kind = data.CatalogView
		}
		entries = append(entries, data.CatalogEntry{Kind: kind, Schema: row[0].Value, Name: row[1].Value})
	}
	return entries, nil
}

func (provider *Provider) Describe(ctx context.Context, connection data.Connection, object data.ObjectRef) (data.Description, error) {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return data.Description{}, err
	}
	schema := object.Schema
	if strings.TrimSpace(schema) == "" {
		schema = "dbo"
	}
	result, err := provider.DescribeNative(ctx, resolved, schema+"."+object.Name)
	if err != nil {
		return data.Description{}, err
	}
	columns := make([]data.Column, 0, len(result.Rows))
	for index, row := range result.Rows {
		if len(row) < 3 || !row[0].Valid {
			continue
		}
		column := data.Column{Name: row[0].Value, Ordinal: index + 1}
		if row[1].Valid {
			column.NativeType = row[1].Value
		}
		column.Nullable = row[2].Valid && strings.EqualFold(row[2].Value, "YES")
		columns = append(columns, column)
	}
	return data.Description{Object: data.ObjectRef{Catalog: object.Catalog, Schema: schema, Name: object.Name}, Columns: columns}, nil
}

// DescribeTable preserves SQL Server's native describe projection while Describe remains the
// provider-neutral metadata capability.
func (provider *Provider) DescribeTable(ctx context.Context, connection data.Connection, object data.ObjectRef) (data.Table, error) {
	resolved, err := provider.resolveGeneric(connection)
	if err != nil {
		return data.Table{}, err
	}
	schema := object.Schema
	if strings.TrimSpace(schema) == "" {
		schema = "dbo"
	}
	result, err := provider.DescribeNative(ctx, resolved, schema+"."+object.Name)
	if err != nil {
		return data.Table{}, err
	}
	return genericTable(result), nil
}

func (provider *Provider) ReadTable(ctx context.Context, connection data.Connection, request data.TabularRead) (data.Table, error) {
	schema := request.Object.Schema
	if strings.TrimSpace(schema) == "" {
		schema = "dbo"
	}
	columns := "*"
	if len(request.Columns) > 0 {
		quoted := make([]string, len(request.Columns))
		for index, column := range request.Columns {
			quoted[index] = quoteIdentifier(column)
		}
		columns = strings.Join(quoted, ", ")
	}
	statement := "select " + columns + " from " + quoteIdentifier(schema) + "." + quoteIdentifier(request.Object.Name)
	return provider.QueryNative(ctx, connection, data.NativeQuery{Statement: statement, MaximumRows: request.MaximumRows})
}

func (provider *Provider) resolveGeneric(connection data.Connection) (ResolvedConnection, error) {
	providerName := string(connection.Source.Provider)
	if providerName == "" {
		providerName = ProviderName
	}
	config := ConnectionConfig{Provider: providerName}
	if secret, found := connection.Secret.Get(); found {
		config.ConnectionString = secret
	}
	if credential, found := connection.CredentialKey.Get(); found {
		config.CredentialKey = string(credential)
	}
	options := connection.Source.Options
	if value, found := optionString(&options, "connectionStringEnvironmentVariable"); found {
		config.ConnectionStringEnvironmentVariable = value
	}
	if value, found := optionString(&options, "credentialKey"); found && config.CredentialKey == "" {
		config.CredentialKey = value
	}
	if value, found := optionBool(&options, "readonly"); found {
		config.ReadOnly = boolPointer(value)
	}
	if value, found := optionInt(&options, "maxRows"); found {
		config.MaxRows = intPointer(value)
	}
	if value, found := optionInt(&options, "timeoutSeconds"); found {
		config.TimeoutSeconds = intPointer(value)
	}
	defaults := DefaultSettings()
	if value, found := optionBool(&options, "defaultReadonly"); found {
		defaults.ReadOnly = value
	}
	if value, found := optionInt(&options, "defaultMaxRows"); found {
		defaults.MaxRows = value
	}
	if value, found := optionInt(&options, "defaultTimeoutSeconds"); found {
		defaults.TimeoutSeconds = value
	}
	return Resolve(config, defaults)
}

func genericTable(result NativeQueryReport) data.Table {
	columns := make([]data.Column, len(result.Columns))
	for index, name := range result.Columns {
		columns[index] = data.Column{Name: name, Ordinal: index + 1}
	}
	rows := make([][]data.Value, len(result.Rows))
	for rowIndex, sourceRow := range result.Rows {
		row := make([]data.Value, len(sourceRow))
		for columnIndex, cell := range sourceRow {
			if cell.Valid {
				row[columnIndex] = data.StringValue(cell.Value)
			} else {
				row[columnIndex] = data.NullValue()
			}
		}
		rows[rowIndex] = row
	}
	return data.Table{Columns: columns, Rows: rows, Truncated: result.Truncated}
}

func optionString(options *wirejson.Value, name string) (string, bool) {
	value, found := options.Lookup(name)
	if !found {
		return "", false
	}
	text, ok := value.AsString()
	return text, ok && strings.TrimSpace(text) != ""
}

func optionBool(options *wirejson.Value, name string) (bool, bool) {
	value, found := options.Lookup(name)
	if !found {
		return false, false
	}
	return value.AsBool()
}

func optionInt(options *wirejson.Value, name string) (int, bool) {
	value, found := options.Lookup(name)
	if !found {
		return 0, false
	}
	lexeme, ok := value.AsNumber()
	if !ok {
		return 0, false
	}
	parsed, err := strconv.ParseInt(lexeme, 10, 64)
	if err != nil || parsed < 0 || int64(int(parsed)) != parsed {
		return 0, false
	}
	return int(parsed), true
}

func quoteIdentifier(value string) string { return "[" + strings.ReplaceAll(value, "]", "]]") + "]" }
func intPointer(value int) *int           { return &value }
func boolPointer(value bool) *bool        { return &value }

var (
	_ data.Provider           = (*Provider)(nil)
	_ data.Cataloger          = (*Provider)(nil)
	_ data.Describer          = (*Provider)(nil)
	_ data.NativeQuerier      = (*Provider)(nil)
	_ data.TabularReader      = (*Provider)(nil)
	_ data.ReadPolicy         = (*Provider)(nil)
	_ data.CredentialResolver = (*Provider)(nil)
)
