package dbcompat

import (
	"encoding/json"
	"os"
	"sort"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

type catalogWire struct {
	Defaults json.RawMessage            `json:"defaults"`
	Globals  map[string]json.RawMessage `json:"globals"`
	Projects map[string]json.RawMessage `json:"projects"`
}

type projectWire struct {
	Databases map[string]json.RawMessage `json:"databases"`
}

type connectionWire struct {
	Provider                            string               `json:"provider"`
	ConnectionString                    *string              `json:"connectionString"`
	InlineSecret                        contract.SecretValue `json:"-"`
	ConnectionStringEnvironmentVariable *string              `json:"connectionStringEnvironmentVariable"`
	CredentialKey                       *string              `json:"credentialKey"`
	ReadOnly                            *bool                `json:"readonly"`
	MaxRows                             *int                 `json:"maxRows"`
	TimeoutSeconds                      *int                 `json:"timeoutSeconds"`
	Options                             []wirejson.Member    `json:"-"`
}

type catalogDefaults struct {
	ReadOnly       bool
	MaxRows        int
	TimeoutSeconds int
}

type Catalog struct {
	defaults catalogDefaults
	globals  map[string]connectionWire
	projects map[string]map[string]connectionWire
}

func LoadCatalog(path string) (Catalog, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return Catalog{}, localized("db.error.config_read", l10n.A("path", path), l10n.A("error", err))
	}
	return ParseCatalog(content, path)
}

func ParseCatalog(content []byte, source string) (Catalog, error) {
	var wire catalogWire
	if err := json.Unmarshal(content, &wire); err != nil {
		return Catalog{}, localized("db.error.config_parse", l10n.A("path", source), l10n.A("error", err))
	}
	catalog := Catalog{
		defaults: catalogDefaults{ReadOnly: true, MaxRows: sqlserver.DefaultMaxRows, TimeoutSeconds: sqlserver.DefaultTimeoutSeconds},
		globals:  make(map[string]connectionWire, len(wire.Globals)),
		projects: make(map[string]map[string]connectionWire, len(wire.Projects)),
	}
	if len(wire.Defaults) > 0 && string(wire.Defaults) != "null" {
		var defaults struct {
			ReadOnly       *bool `json:"readonly"`
			MaxRows        *int  `json:"maxRows"`
			TimeoutSeconds *int  `json:"timeoutSeconds"`
		}
		if err := json.Unmarshal(wire.Defaults, &defaults); err == nil {
			if defaults.ReadOnly != nil {
				catalog.defaults.ReadOnly = *defaults.ReadOnly
			}
			if defaults.MaxRows != nil && *defaults.MaxRows >= 0 {
				catalog.defaults.MaxRows = *defaults.MaxRows
			}
			if defaults.TimeoutSeconds != nil && *defaults.TimeoutSeconds >= 0 {
				catalog.defaults.TimeoutSeconds = *defaults.TimeoutSeconds
			}
		}
	}
	for name, raw := range wire.Globals {
		if connection, ok := parseConnection(raw); ok {
			catalog.globals[name] = connection
		}
	}
	for project, raw := range wire.Projects {
		var projectConfig projectWire
		if json.Unmarshal(raw, &projectConfig) != nil || projectConfig.Databases == nil {
			continue
		}
		databases := make(map[string]connectionWire, len(projectConfig.Databases))
		for name, databaseRaw := range projectConfig.Databases {
			if connection, ok := parseConnection(databaseRaw); ok {
				databases[name] = connection
			}
		}
		catalog.projects[project] = databases
	}
	return catalog, nil
}

func parseConnection(raw json.RawMessage) (connectionWire, bool) {
	var wire connectionWire
	if json.Unmarshal(raw, &wire) != nil {
		return connectionWire{}, false
	}
	if wire.ConnectionString != nil {
		wire.InlineSecret = contract.NewSecretValue(*wire.ConnectionString)
		wire.ConnectionString = nil
	}
	document, err := wirejson.Parse(raw)
	if err != nil {
		return connectionWire{}, false
	}
	members, object := document.Members()
	if !object {
		return connectionWire{}, false
	}
	for _, member := range members {
		switch member.Name {
		case "provider", "connectionString", "connectionStringEnvironmentVariable", "credentialKey", "readonly", "maxRows", "timeoutSeconds":
			continue
		default:
			wire.Options = append(wire.Options, wirejson.Member{Name: member.Name, Value: member.Value.Clone()})
		}
	}
	return wire, true
}

// Resolve preserves project-over-global shadowing and projects legacy database configuration into
// the provider-neutral connection contract. Provider-specific interpretation remains in providers.
func (catalog Catalog) Resolve(project, database string) (data.Connection, error) {
	configured, found := catalog.lookup(project, database)
	if !found {
		return data.Connection{}, localized("db.error.database_not_found", l10n.A("project", project), l10n.A("database", database))
	}
	provider := registryProviderName(configured.Provider)
	options := []wirejson.Member{
		{Name: "defaultReadonly", Value: wirejson.BoolValue(catalog.defaults.ReadOnly)},
		{Name: "defaultMaxRows", Value: wirejson.NumberValue(strconv.Itoa(catalog.defaults.MaxRows))},
		{Name: "defaultTimeoutSeconds", Value: wirejson.NumberValue(strconv.Itoa(catalog.defaults.TimeoutSeconds))},
	}
	options = append(options, configured.Options...)
	if configured.ConnectionStringEnvironmentVariable != nil && strings.TrimSpace(*configured.ConnectionStringEnvironmentVariable) != "" {
		options = append(options, wirejson.Member{Name: "connectionStringEnvironmentVariable", Value: wirejson.StringValue(*configured.ConnectionStringEnvironmentVariable)})
	}
	if configured.ReadOnly != nil {
		options = append(options, wirejson.Member{Name: "readonly", Value: wirejson.BoolValue(*configured.ReadOnly)})
	}
	if configured.MaxRows != nil {
		options = append(options, wirejson.Member{Name: "maxRows", Value: wirejson.NumberValue(strconv.Itoa(*configured.MaxRows))})
	}
	if configured.TimeoutSeconds != nil {
		options = append(options, wirejson.Member{Name: "timeoutSeconds", Value: wirejson.NumberValue(strconv.Itoa(*configured.TimeoutSeconds))})
	}
	source := data.Source{
		Key: data.SourceKey(project + "/" + database), Provider: provider,
		Project: contract.Some(contract.ProjectKey(project)), DisplayName: database,
		Options: wirejson.ObjectValue(options...),
	}
	connection := data.Connection{Source: source}
	if configured.CredentialKey != nil && strings.TrimSpace(*configured.CredentialKey) != "" {
		connection.CredentialKey = contract.Some(contract.SecretKey(*configured.CredentialKey))
	}
	if !configured.InlineSecret.Empty() {
		connection.Secret = contract.Some(configured.InlineSecret)
	}
	return connection, nil
}

func registryProviderName(configured string) data.ProviderName {
	if configured == "" || sqlserver.IsProviderName(configured) {
		return data.ProviderName(sqlserver.ProviderName)
	}
	return data.ProviderName(strings.TrimSpace(configured))
}

func (catalog Catalog) lookup(project, database string) (connectionWire, bool) {
	if databases, found := catalog.projects[project]; found {
		if connection, found := databases[database]; found {
			return connection, true
		}
	}
	connection, found := catalog.globals[database]
	return connection, found
}

type CatalogEntry struct {
	Project  *string `json:"project"`
	Database string  `json:"database"`
}

func (catalog Catalog) Entries() []CatalogEntry {
	entries := make([]CatalogEntry, 0, len(catalog.globals))
	for _, database := range sortedKeys(catalog.globals) {
		entries = append(entries, CatalogEntry{Database: database})
	}
	for _, project := range sortedKeys(catalog.projects) {
		for _, database := range sortedKeys(catalog.projects[project]) {
			projectName := project
			entries = append(entries, CatalogEntry{Project: &projectName, Database: database})
		}
	}
	return entries
}

func sortedKeys[V any](values map[string]V) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}
