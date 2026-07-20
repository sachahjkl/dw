package sqlserver

import (
	"context"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"net/url"
	"strconv"
	"strings"
	"time"

	mssql "github.com/microsoft/go-mssqldb"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
)

const SchemaStatement = `select TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE
from INFORMATION_SCHEMA.TABLES
order by TABLE_SCHEMA, TABLE_NAME`

type Cell struct {
	Valid bool
	Value string
}

func NullCell() Cell { return Cell{} }

func StringCell(value string) Cell { return Cell{Valid: true, Value: value} }

func (cell Cell) MarshalJSON() ([]byte, error) {
	if !cell.Valid {
		return []byte("null"), nil
	}
	return strconv.AppendQuote(nil, cell.Value), nil
}

type QueryResult struct {
	Columns   []string `json:"columns"`
	Rows      [][]Cell `json:"rows"`
	Truncated bool     `json:"truncated"`
}

type Provider struct {
	secrets SecretStore
}

func New(secrets SecretStore) *Provider {
	return &Provider{secrets: secrets}
}

func (provider *Provider) Name() data.ProviderName { return data.ProviderName(ProviderName) }

func DescribeStatement(table string) string {
	schema, name := "dbo", table
	if before, after, found := strings.Cut(table, "."); found {
		schema, name = before, after
	}
	return "select COLUMN_NAME, DATA_TYPE, IS_NULLABLE, CHARACTER_MAXIMUM_LENGTH\n" +
		"from INFORMATION_SCHEMA.COLUMNS\n" +
		"where TABLE_SCHEMA = '" + escapeSQLLiteral(schema) + "'\n" +
		"  and TABLE_NAME = '" + escapeSQLLiteral(name) + "'\n" +
		"order by ORDINAL_POSITION"
}

func (provider *Provider) Schema(ctx context.Context, connection ResolvedConnection) (QueryResult, error) {
	unlimited := 0
	return provider.Query(ctx, connection, SchemaStatement, &unlimited)
}

func (provider *Provider) DescribeLegacy(ctx context.Context, connection ResolvedConnection, table string) (QueryResult, error) {
	unlimited := 0
	return provider.Query(ctx, connection, DescribeStatement(table), &unlimited)
}

// Query executes only guarded SQL, asks SQL Server for a read-only connection, consumes the first
// result set completely, and exposes every non-null database value as text.
func (provider *Provider) Query(ctx context.Context, connection ResolvedConnection, statement string, maxRowsOverride *int) (QueryResult, error) {
	if !IsProviderName(connection.Config.Provider) {
		return QueryResult{}, &ProviderError{Kind: ErrorUnsupportedProvider, Provider: strings.TrimSpace(connection.Config.Provider)}
	}
	if !connection.Defaults.ReadOnly || connection.Config.ReadOnly != nil && !*connection.Config.ReadOnly {
		return QueryResult{}, &ProviderError{Kind: ErrorReadOnlyRequired}
	}
	guard := ValidateReadOnlySQL(statement)
	if !guard.IsAllowed {
		reason := l10n.Text("db.error.unknown_reason")
		if guard.Reason != nil {
			reason = *guard.Reason
		}
		return QueryResult{}, &ProviderError{Kind: ErrorBlockedQuery, Reason: reason}
	}

	connectionString, err := ResolveConnectionString(ctx, connection.Config, provider.secrets)
	if err != nil {
		return QueryResult{}, err
	}
	maxRows := connection.Defaults.MaxRows
	if connection.Config.MaxRows != nil {
		maxRows = *connection.Config.MaxRows
	}
	if maxRowsOverride != nil {
		maxRows = *maxRowsOverride
	}
	timeoutSeconds := connection.Defaults.TimeoutSeconds
	if connection.Config.TimeoutSeconds != nil {
		timeoutSeconds = *connection.Config.TimeoutSeconds
	}
	if timeoutSeconds < 1 {
		timeoutSeconds = 1
	}

	timeoutDuration := time.Duration(timeoutSeconds) * time.Second
	if timeoutSeconds > int(time.Duration(1<<63-1)/time.Second) {
		timeoutDuration = time.Duration(1<<63 - 1)
	}
	queryContext, cancel := context.WithTimeout(ctx, timeoutDuration)
	defer cancel()

	plainConnectionString := connectionString.Reveal()
	database, err := sql.Open("sqlserver", EnforceReadOnlyConnectionString(plainConnectionString))
	if err != nil {
		return QueryResult{}, sqlProblem(err, plainConnectionString)
	}
	database.SetMaxOpenConns(1)
	database.SetMaxIdleConns(0)
	defer database.Close()

	rows, err := database.QueryContext(queryContext, statement)
	if err != nil {
		return QueryResult{}, queryProblem(queryContext, timeoutSeconds, err, plainConnectionString)
	}
	defer rows.Close()
	columnNames, err := rows.Columns()
	if err != nil {
		return QueryResult{}, sqlProblem(err, plainConnectionString)
	}
	columnTypes, err := rows.ColumnTypes()
	if err != nil {
		return QueryResult{}, sqlProblem(err, plainConnectionString)
	}

	result := QueryResult{Rows: make([][]Cell, 0)}
	for rows.Next() {
		values := make([]any, len(columnNames))
		destinations := make([]any, len(values))
		for index := range values {
			destinations[index] = &values[index]
		}
		if err := rows.Scan(destinations...); err != nil {
			return QueryResult{}, sqlProblem(err, plainConnectionString)
		}
		if result.Columns == nil {
			// Rust compatibility: a zero-row first result set reports no columns.
			result.Columns = append([]string(nil), columnNames...)
		}
		if maxRows > 0 && len(result.Rows) >= maxRows {
			result.Truncated = true
			continue
		}
		row := make([]Cell, len(values))
		for index, value := range values {
			row[index], err = cellFromDriverValue(value, columnTypes[index].DatabaseTypeName())
			if err != nil {
				return QueryResult{}, sqlProblem(err, plainConnectionString)
			}
		}
		result.Rows = append(result.Rows, row)
	}
	if err := rows.Err(); err != nil {
		return QueryResult{}, queryProblem(queryContext, timeoutSeconds, err, plainConnectionString)
	}
	if result.Columns == nil {
		result.Columns = []string{}
	}
	return result, nil
}

func cellFromDriverValue(value any, nativeType string) (Cell, error) {
	switch typed := value.(type) {
	case nil:
		return NullCell(), nil
	case string:
		return StringCell(typed), nil
	case []byte:
		if strings.EqualFold(nativeType, "UNIQUEIDENTIFIER") {
			var identifier mssql.UniqueIdentifier
			if err := identifier.Scan(typed); err != nil {
				return Cell{}, err
			}
			return StringCell(strings.ToLower(identifier.String())), nil
		}
		if !isBinaryNativeType(nativeType) {
			return StringCell(string(typed)), nil
		}
		encoded := make([]byte, hex.EncodedLen(len(typed)))
		hex.Encode(encoded, typed)
		return StringCell("0x" + strings.ToUpper(string(encoded))), nil
	case bool:
		return StringCell(strconv.FormatBool(typed)), nil
	case int64:
		return StringCell(strconv.FormatInt(typed, 10)), nil
	case int32:
		return StringCell(strconv.FormatInt(int64(typed), 10)), nil
	case int16:
		return StringCell(strconv.FormatInt(int64(typed), 10)), nil
	case int8:
		return StringCell(strconv.FormatInt(int64(typed), 10)), nil
	case uint64:
		return StringCell(strconv.FormatUint(typed, 10)), nil
	case float64:
		return StringCell(strconv.FormatFloat(typed, 'g', -1, 64)), nil
	case float32:
		return StringCell(strconv.FormatFloat(float64(typed), 'g', -1, 32)), nil
	case time.Time:
		return StringCell(typed.String()), nil
	case fmt.Stringer:
		return StringCell(typed.String()), nil
	default:
		return StringCell(fmt.Sprint(typed)), nil
	}
}
func isBinaryNativeType(nativeType string) bool {
	switch strings.ToUpper(nativeType) {
	case "BINARY", "VARBINARY", "IMAGE", "TIMESTAMP", "ROWVERSION", "UDT":
		return true
	default:
		return false
	}
}

// EnforceReadOnlyConnectionString returns a driver DSN whose safety options override every
// case-insensitive duplicate supplied by configuration.
func EnforceReadOnlyConnectionString(connectionString string) string {
	trimmed := strings.TrimSpace(connectionString)
	if parsed, err := url.Parse(trimmed); err == nil && strings.EqualFold(parsed.Scheme, "sqlserver") {
		query := parsed.Query()
		for key := range query {
			if isSafetyOption(key) {
				query.Del(key)
			}
		}
		query.Set("ApplicationIntent", "ReadOnly")
		query.Set("TrustServerCertificate", "true")
		parsed.RawQuery = query.Encode()
		return parsed.String()
	}

	segments := splitConnectionString(trimmed)
	kept := segments[:0]
	for _, segment := range segments {
		key, _, found := strings.Cut(segment, "=")
		if found && isSafetyOption(key) {
			continue
		}
		if strings.TrimSpace(segment) != "" {
			kept = append(kept, segment)
		}
	}
	kept = append(kept, "ApplicationIntent=ReadOnly", "TrustServerCertificate=true")
	return strings.Join(kept, ";")
}

func isSafetyOption(key string) bool {
	normalized := strings.NewReplacer(" ", "", "-", "", "_", "").Replace(strings.ToLower(strings.TrimSpace(key)))
	return normalized == "applicationintent" || normalized == "trustservercertificate"
}

func splitConnectionString(value string) []string {
	segments := make([]string, 0, 8)
	start := 0
	quote := byte(0)
	braceDepth := 0
	valueStart := false
	seenEquals := false
	for index := 0; index < len(value); index++ {
		current := value[index]
		if quote != 0 {
			if current == quote {
				if index+1 < len(value) && value[index+1] == quote {
					index++
					continue
				}
				quote = 0
			}
			continue
		}
		if braceDepth > 0 {
			if current == '}' {
				if index+1 < len(value) && value[index+1] == '}' {
					index++
					continue
				}
				braceDepth--
			}
			continue
		}
		if current == ';' {
			segments = append(segments, value[start:index])
			start = index + 1
			valueStart = false
			seenEquals = false
			continue
		}
		if current == '=' && !seenEquals {
			valueStart = true
			seenEquals = true
			continue
		}
		if valueStart {
			if current == ' ' || current == '\t' {
				continue
			}
			switch current {
			case '\'', '"':
				quote = current
			case '{':
				braceDepth = 1
			}
			valueStart = false
		}
	}
	segments = append(segments, value[start:])
	return segments
}

func queryProblem(ctx context.Context, seconds int, err error, sensitive string) error {
	if errors.Is(ctx.Err(), context.DeadlineExceeded) || errors.Is(err, context.DeadlineExceeded) {
		return &ProviderError{Kind: ErrorTimeout, Seconds: seconds}
	}
	return sqlProblem(err, sensitive)
}

func sqlProblem(err error, sensitive string) error {
	return &ProviderError{Kind: ErrorSQL, Reason: redactDriverMessage(err.Error(), sensitive)}
}

func redactDriverMessage(message, sensitive string) string {
	if sensitive != "" {
		message = strings.ReplaceAll(message, sensitive, "<hidden>")
		if parsed, err := url.Parse(sensitive); err == nil && parsed.User != nil {
			if password, found := parsed.User.Password(); found && password != "" {
				message = strings.ReplaceAll(message, password, "<hidden>")
			}
		}
		for _, segment := range splitConnectionString(sensitive) {
			key, value, found := strings.Cut(segment, "=")
			value = strings.TrimSpace(value)
			if !found || value == "" {
				continue
			}
			normalized := strings.ToLower(strings.TrimSpace(key))
			if normalized != "password" && normalized != "pwd" && normalized != "access token" {
				continue
			}
			message = strings.ReplaceAll(message, value, "<hidden>")
			if unquoted := unquoteConnectionValue(value); unquoted != value && unquoted != "" {
				message = strings.ReplaceAll(message, unquoted, "<hidden>")
			}
		}
	}

	lowered := strings.ToLower(message)
	for _, marker := range []string{"password=", "pwd=", "access token="} {
		for {
			start := strings.Index(lowered, marker)
			if start < 0 {
				break
			}
			valueStart := start + len(marker)
			valueEnd := strings.IndexByte(message[valueStart:], ';')
			if valueEnd < 0 {
				valueEnd = len(message)
			} else {
				valueEnd += valueStart
			}
			message = message[:valueStart] + "<hidden>" + message[valueEnd:]
			lowered = strings.ToLower(message)
		}
	}
	return message
}

func unquoteConnectionValue(value string) string {
	if len(value) < 2 {
		return value
	}
	first, last := value[0], value[len(value)-1]
	if (first == '\'' || first == '"') && last == first {
		return strings.ReplaceAll(value[1:len(value)-1], string([]byte{first, first}), string(first))
	}
	if first == '{' && last == '}' {
		return strings.ReplaceAll(value[1:len(value)-1], "}}", "}")
	}
	return value
}

func escapeSQLLiteral(value string) string { return strings.ReplaceAll(value, "'", "''") }
