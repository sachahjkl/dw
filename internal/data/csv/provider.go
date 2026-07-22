package csv

import (
	"context"
	"encoding/csv"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
)

const ProviderName = "csv"

type Provider struct{}

func New() *Provider                      { return &Provider{} }
func (*Provider) Name() data.ProviderName { return data.ProviderName(ProviderName) }

func (*Provider) Catalog(_ context.Context, connection data.Connection) ([]data.CatalogEntry, error) {
	path, err := sourcePath(connection)
	if err != nil {
		return nil, err
	}
	return []data.CatalogEntry{{Kind: data.CatalogTable, Name: tableName(path)}}, nil
}

func (provider *Provider) Describe(ctx context.Context, connection data.Connection, object data.ObjectRef) (data.Description, error) {
	table, err := provider.ReadTable(ctx, connection, data.TabularRead{Object: object, MaximumRows: 1})
	if err != nil {
		return data.Description{}, err
	}
	return data.Description{Object: data.ObjectRef{Name: objectName(connection, object)}, Columns: table.Columns}, nil
}

func (*Provider) ReadTable(ctx context.Context, connection data.Connection, request data.TabularRead) (data.Table, error) {
	path, err := sourcePath(connection)
	if err != nil {
		return data.Table{}, err
	}
	file, err := os.Open(path)
	if err != nil {
		return data.Table{}, fmt.Errorf("csv.open: %w", err)
	}
	defer file.Close()
	reader := csv.NewReader(file)
	reader.FieldsPerRecord = -1
	reader.ReuseRecord = true
	reader.TrimLeadingSpace = optionBool(connection.Source.Options, "trimSpace", false)
	delimiter, err := configuredDelimiter(connection.Source.Options, filepath.Ext(path))
	if err != nil {
		return data.Table{}, err
	}
	reader.Comma = delimiter
	first, err := reader.Read()
	if err == io.EOF {
		return data.Table{Columns: []data.Column{}, Rows: [][]data.Value{}}, nil
	}
	if err != nil {
		return data.Table{}, fmt.Errorf("csv.read: %w", err)
	}
	first = append([]string(nil), first...)
	if len(first) > 0 {
		first[0] = strings.TrimPrefix(first[0], "\ufeff")
	}
	hasHeader := optionBool(connection.Source.Options, "header", true)
	names := first
	if !hasHeader {
		names = make([]string, len(first))
		for index := range names {
			names[index] = "column_" + strconv.Itoa(index+1)
		}
	}
	for index := range names {
		if strings.TrimSpace(names[index]) == "" {
			names[index] = "column_" + strconv.Itoa(index+1)
		}
	}
	selected, err := selectedColumns(names, request.Columns)
	if err != nil {
		return data.Table{}, err
	}
	table := data.Table{Columns: make([]data.Column, len(selected)), Rows: make([][]data.Value, 0)}
	for index, sourceIndex := range selected {
		table.Columns[index] = data.Column{Name: names[sourceIndex], NativeType: "TEXT", Nullable: true, Ordinal: index + 1}
	}
	maximum := request.MaximumRows
	if maximum == 0 {
		maximum = optionInt(connection.Source.Options, "maxRows", optionInt(connection.Source.Options, "defaultMaxRows", 500))
	}
	appendRecord := func(record []string) {
		if maximum > 0 && len(table.Rows) >= maximum {
			table.Truncated = true
			return
		}
		row := make([]data.Value, len(selected))
		for index, sourceIndex := range selected {
			if sourceIndex < len(record) {
				row[index] = data.StringValue(record[sourceIndex])
			} else {
				row[index] = data.NullValue()
			}
		}
		table.Rows = append(table.Rows, row)
	}
	if !hasHeader {
		appendRecord(first)
	}
	for {
		if err := ctx.Err(); err != nil {
			return data.Table{}, err
		}
		record, readErr := reader.Read()
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			return data.Table{}, fmt.Errorf("csv.read: %w", readErr)
		}
		appendRecord(record)
		if table.Truncated {
			break
		}
	}
	return table, nil
}

func sourcePath(connection data.Connection) (string, error) {
	value, found := connection.Source.Options.Lookup("path")
	if !found {
		return "", fmt.Errorf("csv.path-required")
	}
	path, ok := value.AsString()
	if !ok || strings.TrimSpace(path) == "" {
		return "", fmt.Errorf("csv.path-required")
	}
	return strings.TrimSpace(path), nil
}

func tableName(path string) string {
	base := filepath.Base(path)
	return strings.TrimSuffix(base, filepath.Ext(base))
}

func objectName(connection data.Connection, object data.ObjectRef) string {
	if strings.TrimSpace(object.Name) != "" {
		return object.Name
	}
	path, _ := sourcePath(connection)
	return tableName(path)
}

func configuredDelimiter(options wirejson.Value, extension string) (rune, error) {
	if value, found := options.Lookup("delimiter"); found {
		text, ok := value.AsString()
		if !ok {
			return 0, fmt.Errorf("csv.invalid-delimiter")
		}
		switch strings.ToLower(text) {
		case "tab", "\\t":
			return '\t', nil
		case "comma":
			return ',', nil
		case "semicolon":
			return ';', nil
		case "pipe":
			return '|', nil
		}
		delimiter, size := utf8.DecodeRuneInString(text)
		if delimiter == utf8.RuneError || size != len(text) || delimiter == '\r' || delimiter == '\n' || delimiter == 0 {
			return 0, fmt.Errorf("csv.invalid-delimiter")
		}
		return delimiter, nil
	}
	if strings.EqualFold(extension, ".tsv") {
		return '\t', nil
	}
	return ',', nil
}

func selectedColumns(names, requested []string) ([]int, error) {
	if len(requested) == 0 {
		result := make([]int, len(names))
		for index := range result {
			result[index] = index
		}
		return result, nil
	}
	result := make([]int, len(requested))
	for requestedIndex, requestedName := range requested {
		found := false
		for sourceIndex, name := range names {
			if name == requestedName {
				result[requestedIndex] = sourceIndex
				found = true
				break
			}
		}
		if !found {
			return nil, fmt.Errorf("csv.column-not-found:%s", requestedName)
		}
	}
	return result, nil
}

func optionBool(options wirejson.Value, name string, fallback bool) bool {
	value, found := options.Lookup(name)
	if !found {
		return fallback
	}
	result, ok := value.AsBool()
	if !ok {
		return fallback
	}
	return result
}

func optionInt(options wirejson.Value, name string, fallback int) int {
	value, found := options.Lookup(name)
	if !found {
		return fallback
	}
	text, ok := value.AsNumber()
	if !ok {
		return fallback
	}
	result, err := strconv.Atoi(text)
	if err != nil || result < 0 {
		return fallback
	}
	return result
}

var (
	_ data.Cataloger     = (*Provider)(nil)
	_ data.Describer     = (*Provider)(nil)
	_ data.TabularReader = (*Provider)(nil)
)
