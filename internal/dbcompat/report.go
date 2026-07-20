package dbcompat

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"os"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/l10n"
)

type QueryResult = sqlserver.QueryResult
type Cell = sqlserver.Cell
type SQLGuardResult = sqlserver.GuardResult

func Guard(statement string) SQLGuardResult { return sqlserver.ValidateReadOnlySQL(statement) }

// ProjectTable preserves the legacy dw db string-or-null machine contract over the provider-neutral
// table model. Binary data uses the historical uppercase 0xHEX representation.
func ProjectTable(table data.Table) QueryResult {
	result := QueryResult{Columns: make([]string, len(table.Columns)), Rows: make([][]Cell, len(table.Rows)), Truncated: table.Truncated}
	for index, column := range table.Columns {
		result.Columns[index] = column.Name
	}
	for rowIndex, row := range table.Rows {
		projected := make([]Cell, len(row))
		for columnIndex, value := range row {
			switch value.Kind() {
			case data.ValueNull:
				projected[columnIndex] = sqlserver.NullCell()
			case data.ValueBoolean:
				boolean, _ := value.Boolean()
				projected[columnIndex] = sqlserver.StringCell(strconv.FormatBool(boolean))
			case data.ValueBinary:
				binary, _ := value.Binary()
				encoded := make([]byte, hex.EncodedLen(len(binary)))
				hex.Encode(encoded, binary)
				projected[columnIndex] = sqlserver.StringCell("0x" + strings.ToUpper(string(encoded)))
			default:
				text, ok := value.Text()
				if ok {
					projected[columnIndex] = sqlserver.StringCell(text)
				}
			}
		}
		result.Rows[rowIndex] = projected
	}
	return result
}

func QueryTSV(result QueryResult) string {
	var output strings.Builder
	for index, column := range result.Columns {
		if index > 0 {
			output.WriteByte('\t')
		}
		output.WriteString(column)
	}
	output.WriteByte('\n')
	for _, row := range result.Rows {
		for index, cell := range row {
			if index > 0 {
				output.WriteByte('\t')
			}
			if cell.Valid {
				output.WriteString(cell.Value)
			} else {
				output.WriteString("NULL")
			}
		}
		output.WriteByte('\n')
	}
	output.WriteString("-- ")
	output.WriteString(strconv.Itoa(len(result.Rows)))
	if result.Truncated {
		output.WriteString(" rows (truncated)")
	} else {
		output.WriteString(" rows")
	}
	return output.String()
}

func PrettyJSON(value any) ([]byte, error) {
	return json.MarshalIndent(value, "", "  ")
}

type ConnectionSourceKind string

const (
	SourceCredential  ConnectionSourceKind = "credential"
	SourceEnvironment ConnectionSourceKind = "environment"
	SourceInline      ConnectionSourceKind = "inline"
	SourceMissing     ConnectionSourceKind = "missing"
	SourceMultiple    ConnectionSourceKind = "multiple"
)

type ConnectionSource struct {
	Kind        ConnectionSourceKind
	Key         string
	Variable    string
	ValueMasked bool
}

func (source ConnectionSource) String() string {
	switch source.Kind {
	case SourceCredential:
		return "credential:" + source.Key
	case SourceEnvironment:
		return "environment:" + source.Variable
	case SourceInline:
		return "inline:<hidden>"
	case SourceMultiple:
		return "multiple"
	default:
		return "missing"
	}
}

func (source ConnectionSource) MarshalJSON() ([]byte, error) {
	var projection any
	switch source.Kind {
	case SourceCredential:
		projection = struct {
			Kind ConnectionSourceKind `json:"kind"`
			Key  string               `json:"key"`
		}{source.Kind, source.Key}
	case SourceEnvironment:
		projection = struct {
			Kind     ConnectionSourceKind `json:"kind"`
			Variable string               `json:"variable"`
		}{source.Kind, source.Variable}
	case SourceInline:
		projection = struct {
			Kind        ConnectionSourceKind `json:"kind"`
			ValueMasked bool                 `json:"value_masked"`
		}{source.Kind, true}
	default:
		projection = struct {
			Kind ConnectionSourceKind `json:"kind"`
		}{source.Kind}
	}
	return json.Marshal(projection)
}

type DatabaseListEntry struct {
	Project        *string          `json:"project"`
	Database       string           `json:"database"`
	Provider       string           `json:"provider"`
	Source         ConnectionSource `json:"source"`
	ReadOnly       bool             `json:"readonly"`
	MaxRows        int              `json:"maxRows"`
	TimeoutSeconds int              `json:"timeoutSeconds"`
	Warnings       []string         `json:"warnings"`
}

type DatabaseListReport struct {
	Root     string              `json:"root"`
	Entries  []DatabaseListEntry `json:"entries"`
	Warnings []string            `json:"warnings"`
}

func Inventory(root string) (DatabaseListReport, error) {
	path := databasesPath(root)
	content, err := os.ReadFile(path)
	if err != nil {
		return DatabaseListReport{}, localized("db.error.config_read", l10n.A("path", path), l10n.A("error", err))
	}
	var wire catalogWire
	if err := json.Unmarshal(content, &wire); err != nil {
		return DatabaseListReport{}, localized("db.error.config_parse", l10n.A("path", path), l10n.A("error", err))
	}
	defaults := sqlserver.DefaultSettings()
	if len(wire.Defaults) > 0 {
		var values struct {
			ReadOnly       *bool `json:"readonly"`
			MaxRows        *int  `json:"maxRows"`
			TimeoutSeconds *int  `json:"timeoutSeconds"`
		}
		if json.Unmarshal(wire.Defaults, &values) == nil {
			if values.ReadOnly != nil {
				defaults.ReadOnly = *values.ReadOnly
			}
			if values.MaxRows != nil && *values.MaxRows >= 0 {
				defaults.MaxRows = *values.MaxRows
			}
			if values.TimeoutSeconds != nil && *values.TimeoutSeconds >= 0 {
				defaults.TimeoutSeconds = *values.TimeoutSeconds
			}
		}
	}
	report := DatabaseListReport{Root: root, Entries: []DatabaseListEntry{}, Warnings: []string{}}
	for _, database := range sortedKeys(wire.Globals) {
		entry, ok := inventoryEntry(nil, database, wire.Globals[database], defaults)
		if ok {
			report.Entries = append(report.Entries, entry)
		} else {
			report.Warnings = append(report.Warnings, l10n.Render(l10n.M("db.inventory.invalid_global", l10n.A("database", database))))
		}
	}
	for _, project := range sortedKeys(wire.Projects) {
		var projectConfig projectWire
		if json.Unmarshal(wire.Projects[project], &projectConfig) != nil || projectConfig.Databases == nil {
			report.Warnings = append(report.Warnings, l10n.Render(l10n.M("db.inventory.invalid_project", l10n.A("project", project))))
			continue
		}
		for _, database := range sortedKeys(projectConfig.Databases) {
			projectName := project
			entry, ok := inventoryEntry(&projectName, database, projectConfig.Databases[database], defaults)
			if ok {
				report.Entries = append(report.Entries, entry)
			} else {
				report.Warnings = append(report.Warnings, l10n.Render(l10n.M("db.inventory.invalid_entry", l10n.A("project", project), l10n.A("database", database))))
			}
		}
	}
	return report, nil
}

func inventoryEntry(project *string, database string, raw json.RawMessage, defaults sqlserver.Defaults) (DatabaseListEntry, bool) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var object map[string]any
	if decoder.Decode(&object) != nil {
		return DatabaseListEntry{}, false
	}
	provider, ok := object["provider"].(string)
	if !ok {
		return DatabaseListEntry{}, false
	}
	inline := nonblankValue(object["connectionString"])
	environment := nonblankValue(object["connectionStringEnvironmentVariable"])
	credential := nonblankValue(object["credentialKey"])
	count := boolInt(inline != "") + boolInt(environment != "") + boolInt(credential != "")
	source := ConnectionSource{Kind: SourceMissing}
	switch {
	case count > 1:
		source.Kind = SourceMultiple
	case inline != "":
		source.Kind, source.ValueMasked = SourceInline, true
	case environment != "":
		source.Kind, source.Variable = SourceEnvironment, environment
	case credential != "":
		source.Kind, source.Key = SourceCredential, credential
	}
	readonly := defaults.ReadOnly
	if configured, ok := object["readonly"].(bool); ok {
		readonly = configured
	}
	maxRows := integerValue(object["maxRows"], defaults.MaxRows)
	timeout := integerValue(object["timeoutSeconds"], defaults.TimeoutSeconds)
	warnings := []string{}
	if !sqlserver.IsProviderName(provider) {
		warnings = append(warnings, l10n.Text("db.warning.unsupported"))
	}
	if count == 0 {
		warnings = append(warnings, l10n.Text("db.warning.missing_source"))
	} else if count > 1 {
		warnings = append(warnings, l10n.Text("db.warning.multiple_sources"))
	}
	if !readonly {
		warnings = append(warnings, l10n.Text("db.warning.readonly_false"))
	}
	return DatabaseListEntry{Project: project, Database: database, Provider: provider, Source: source, ReadOnly: readonly, MaxRows: maxRows, TimeoutSeconds: timeout, Warnings: warnings}, true
}

func nonblankValue(value any) string {
	text, _ := value.(string)
	if strings.TrimSpace(text) == "" {
		return ""
	}
	return text
}

func integerValue(value any, fallback int) int {
	number, ok := value.(json.Number)
	if !ok {
		return fallback
	}
	parsed, err := strconv.ParseInt(string(number), 10, 64)
	if err != nil || parsed < 0 || int64(int(parsed)) != parsed {
		return fallback
	}
	return int(parsed)
}

func boolInt(value bool) int {
	if value {
		return 1
	}
	return 0
}
