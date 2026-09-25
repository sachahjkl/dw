package sqlite

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"
	"unicode"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
	_ "modernc.org/sqlite"
)

const ProviderName = "sqlite"

type Provider struct{}

func New() *Provider                      { return &Provider{} }
func (*Provider) Name() data.ProviderName { return data.ProviderName(ProviderName) }

func (*Provider) ValidateRead(_ context.Context, connection data.Connection, query data.NativeQuery) error {
	if _, err := databasePath(connection); err != nil {
		return err
	}
	statement := strings.TrimSpace(query.Statement)
	if statement == "" {
		return fmt.Errorf("sqlite.empty-query")
	}
	cleaned, multiple := lexStatement(statement)
	if multiple {
		return fmt.Errorf("sqlite.multiple-statements")
	}
	fields := strings.Fields(cleaned)
	if len(fields) == 0 {
		return fmt.Errorf("sqlite.empty-query")
	}
	verb := strings.ToLower(fields[0])
	switch verb {
	case "select", "with", "explain":
		return nil
	case "pragma":
		return validatePragma(cleaned)
	default:
		return fmt.Errorf("sqlite.read-only-query-required:%s", verb)
	}
}

var allowedPragmas = map[string]struct{}{
	"table_info": {}, "table_xinfo": {}, "index_list": {}, "index_info": {}, "index_xinfo": {},
	"foreign_key_list": {}, "database_list": {}, "table_list": {}, "collation_list": {},
}

func validatePragma(cleaned string) error {
	if strings.Contains(cleaned, "=") {
		return fmt.Errorf("sqlite.pragma-assignment")
	}
	rest := strings.TrimSpace(cleaned[len("pragma"):])
	end := strings.IndexFunc(rest, func(r rune) bool { return r != '_' && r != '.' && !unicode.IsLetter(r) && !unicode.IsDigit(r) })
	if end < 0 {
		end = len(rest)
	}
	name := strings.ToLower(rest[:end])
	if dot := strings.LastIndexByte(name, '.'); dot >= 0 {
		name = name[dot+1:]
	}
	if _, ok := allowedPragmas[name]; !ok {
		return fmt.Errorf("sqlite.pragma-not-allowed:%s", name)
	}
	return nil
}

// lexStatement blanks out string literals, quoted identifiers and comments, and reports whether a
// top-level ';' is followed by another statement.
func lexStatement(statement string) (string, bool) {
	var output strings.Builder
	output.Grow(len(statement))
	terminated := false
	for index := 0; index < len(statement); {
		current := statement[index]
		switch {
		case current == '\'' || current == '"' || current == '`' || current == '[':
			closing := current
			if current == '[' {
				closing = ']'
			}
			index++
			for index < len(statement) {
				if statement[index] == closing {
					if closing != ']' && index+1 < len(statement) && statement[index+1] == closing {
						index += 2
						continue
					}
					index++
					break
				}
				index++
			}
			output.WriteByte(' ')
			if terminated {
				return output.String(), true
			}
		case current == '-' && index+1 < len(statement) && statement[index+1] == '-':
			for index < len(statement) && statement[index] != '\n' {
				index++
			}
			output.WriteByte(' ')
		case current == '/' && index+1 < len(statement) && statement[index+1] == '*':
			end := strings.Index(statement[index+2:], "*/")
			if end < 0 {
				index = len(statement)
			} else {
				index += end + 4
			}
			output.WriteByte(' ')
		case current == ';':
			terminated = true
			output.WriteByte(' ')
			index++
		default:
			if terminated && !unicode.IsSpace(rune(current)) {
				return output.String(), true
			}
			output.WriteByte(current)
			index++
		}
	}
	return output.String(), false
}

func (provider *Provider) QueryNative(ctx context.Context, connection data.Connection, query data.NativeQuery) (data.Table, error) {
	if err := provider.ValidateRead(ctx, connection, query); err != nil {
		return data.Table{}, err
	}
	database, err := openReadOnly(connection)
	if err != nil {
		return data.Table{}, err
	}
	defer database.Close()
	queryContext := ctx
	if query.TimeoutSeconds > 0 {
		var cancel context.CancelFunc
		queryContext, cancel = context.WithTimeout(ctx, time.Duration(query.TimeoutSeconds)*time.Second)
		defer cancel()
	}
	rows, err := database.QueryContext(queryContext, query.Statement)
	if err != nil {
		return data.Table{}, fmt.Errorf("sqlite.query: %w", err)
	}
	defer rows.Close()
	return scanRows(rows, maximumRows(connection, query.MaximumRows))
}

func (*Provider) Catalog(ctx context.Context, connection data.Connection) ([]data.CatalogEntry, error) {
	database, err := openReadOnly(connection)
	if err != nil {
		return nil, err
	}
	defer database.Close()
	rows, err := database.QueryContext(ctx, `select name, type from sqlite_master where type in ('table', 'view') and name not like 'sqlite_%' order by name`)
	if err != nil {
		return nil, fmt.Errorf("sqlite.catalog: %w", err)
	}
	defer rows.Close()
	entries := make([]data.CatalogEntry, 0)
	for rows.Next() {
		var name, kind string
		if err := rows.Scan(&name, &kind); err != nil {
			return nil, fmt.Errorf("sqlite.catalog: %w", err)
		}
		entryKind := data.CatalogTable
		if kind == "view" {
			entryKind = data.CatalogView
		}
		entries = append(entries, data.CatalogEntry{Kind: entryKind, Schema: "main", Name: name})
	}
	return entries, rows.Err()
}

func (*Provider) Describe(ctx context.Context, connection data.Connection, object data.ObjectRef) (data.Description, error) {
	if strings.TrimSpace(object.Name) == "" {
		return data.Description{}, fmt.Errorf("sqlite.table-required")
	}
	database, err := openReadOnly(connection)
	if err != nil {
		return data.Description{}, err
	}
	defer database.Close()
	rows, err := database.QueryContext(ctx, "pragma table_info("+quoteIdentifier(object.Name)+")")
	if err != nil {
		return data.Description{}, fmt.Errorf("sqlite.describe: %w", err)
	}
	defer rows.Close()
	columns := make([]data.Column, 0)
	for rows.Next() {
		var cid int
		var name, nativeType string
		var notNull, primaryKey int
		var defaultValue any
		if err := rows.Scan(&cid, &name, &nativeType, &notNull, &defaultValue, &primaryKey); err != nil {
			return data.Description{}, fmt.Errorf("sqlite.describe: %w", err)
		}
		columns = append(columns, data.Column{Name: name, NativeType: nativeType, Nullable: notNull == 0 && primaryKey == 0, Ordinal: cid + 1})
	}
	if err := rows.Err(); err != nil {
		return data.Description{}, err
	}
	return data.Description{Object: data.ObjectRef{Catalog: object.Catalog, Schema: "main", Name: object.Name}, Columns: columns}, nil
}

func (provider *Provider) ReadTable(ctx context.Context, connection data.Connection, request data.TabularRead) (data.Table, error) {
	if strings.TrimSpace(request.Object.Name) == "" {
		return data.Table{}, fmt.Errorf("sqlite.table-required")
	}
	columns := "*"
	if len(request.Columns) > 0 {
		quoted := make([]string, len(request.Columns))
		for index, column := range request.Columns {
			quoted[index] = quoteIdentifier(column)
		}
		columns = strings.Join(quoted, ", ")
	}
	return provider.QueryNative(ctx, connection, data.NativeQuery{Statement: "select " + columns + " from " + quoteIdentifier(request.Object.Name), MaximumRows: request.MaximumRows})
}

func openReadOnly(connection data.Connection) (*sql.DB, error) {
	path, err := databasePath(connection)
	if err != nil {
		return nil, err
	}
	absolute, err := filepath.Abs(path)
	if err != nil {
		return nil, fmt.Errorf("sqlite.path: %w", err)
	}
	uriPath := filepath.ToSlash(absolute)
	if runtime.GOOS == "windows" {
		uriPath = "/" + uriPath
	}
	dsn := (&url.URL{Scheme: "file", Path: uriPath, RawQuery: "mode=ro&_pragma=query_only(1)"}).String()
	database, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("sqlite.open: %w", err)
	}
	database.SetMaxOpenConns(1)
	return database, nil
}

func databasePath(connection data.Connection) (string, error) {
	if value, ok := optionString(connection.Source.Options, "path"); ok {
		return value, nil
	}
	if secret, ok := connection.Secret.Get(); ok && strings.TrimSpace(secret.Reveal()) != "" {
		return secret.Reveal(), nil
	}
	return "", fmt.Errorf("sqlite.path-required")
}

func maximumRows(connection data.Connection, override int) int {
	if override != 0 {
		return override
	}
	if value, ok := optionInt(connection.Source.Options, "maxRows"); ok {
		return value
	}
	if value, ok := optionInt(connection.Source.Options, "defaultMaxRows"); ok {
		return value
	}
	return 500
}

func scanRows(rows *sql.Rows, maximum int) (data.Table, error) {
	names, err := rows.Columns()
	if err != nil {
		return data.Table{}, err
	}
	types, err := rows.ColumnTypes()
	if err != nil {
		return data.Table{}, err
	}
	table := data.Table{Columns: make([]data.Column, len(names)), Rows: make([][]data.Value, 0)}
	for index, name := range names {
		table.Columns[index] = data.Column{Name: name, NativeType: types[index].DatabaseTypeName(), Ordinal: index + 1}
		if nullable, ok := types[index].Nullable(); ok {
			table.Columns[index].Nullable = nullable
		}
	}
	for rows.Next() {
		values := make([]any, len(names))
		destinations := make([]any, len(names))
		for index := range values {
			destinations[index] = &values[index]
		}
		if err := rows.Scan(destinations...); err != nil {
			return data.Table{}, err
		}
		if maximum > 0 && len(table.Rows) >= maximum {
			table.Truncated = true
			break
		}
		row := make([]data.Value, len(values))
		for index, value := range values {
			row[index] = driverValue(value)
		}
		table.Rows = append(table.Rows, row)
	}
	return table, rows.Err()
}

func driverValue(value any) data.Value {
	switch typed := value.(type) {
	case nil:
		return data.NullValue()
	case int64:
		return data.IntegerValue(strconv.FormatInt(typed, 10))
	case float64:
		return data.DecimalValue(strconv.FormatFloat(typed, 'g', -1, 64))
	case bool:
		return data.BooleanValue(typed)
	case []byte:
		return data.BinaryValue(typed)
	case time.Time:
		return data.TimeValue(typed.Format(time.RFC3339Nano))
	default:
		return data.StringValue(fmt.Sprint(typed))
	}
}

func optionString(options wirejson.Value, name string) (string, bool) {
	value, found := options.Lookup(name)
	if !found {
		return "", false
	}
	text, ok := value.AsString()
	return strings.TrimSpace(text), ok && strings.TrimSpace(text) != ""
}

func optionInt(options wirejson.Value, name string) (int, bool) {
	value, found := options.Lookup(name)
	if !found {
		return 0, false
	}
	text, ok := value.AsNumber()
	if !ok {
		return 0, false
	}
	parsed, err := strconv.Atoi(text)
	return parsed, err == nil && parsed >= 0
}

func quoteIdentifier(value string) string { return `"` + strings.ReplaceAll(value, `"`, `""`) + `"` }

var (
	_ data.Cataloger     = (*Provider)(nil)
	_ data.Describer     = (*Provider)(nil)
	_ data.NativeQuerier = (*Provider)(nil)
	_ data.TabularReader = (*Provider)(nil)
	_ data.ReadPolicy    = (*Provider)(nil)
)
