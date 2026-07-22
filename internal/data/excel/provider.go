package excel

import (
	"context"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/xuri/excelize/v2"
)

const ProviderName = "excel"

type Provider struct{}

func New() *Provider                      { return &Provider{} }
func (*Provider) Name() data.ProviderName { return data.ProviderName(ProviderName) }

func (*Provider) Catalog(ctx context.Context, connection data.Connection) ([]data.CatalogEntry, error) {
	path, err := workbookPath(connection, "")
	if err != nil {
		return nil, err
	}
	book, err := excelize.OpenFile(path)
	if err != nil {
		return nil, fmt.Errorf("excel.open: %w", err)
	}
	defer book.Close()
	entries := make([]data.CatalogEntry, 0, len(book.GetSheetList()))
	for _, sheet := range book.GetSheetList() {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		entries = append(entries, data.CatalogEntry{Kind: data.CatalogTable, Catalog: filepath.Base(path), Name: sheet})
	}
	return entries, nil
}

func (provider *Provider) Describe(ctx context.Context, connection data.Connection, object data.ObjectRef) (data.Description, error) {
	table, err := provider.ReadWorkbook(ctx, connection, data.WorkbookRead{Worksheet: object.Name})
	if err != nil {
		return data.Description{}, err
	}
	return data.Description{Object: data.ObjectRef{Catalog: object.Catalog, Name: object.Name}, Columns: table.Columns}, nil
}

func (*Provider) ReadWorkbook(ctx context.Context, connection data.Connection, request data.WorkbookRead) (data.Table, error) {
	path, err := workbookPath(connection, request.Path)
	if err != nil {
		return data.Table{}, err
	}
	book, err := excelize.OpenFile(path)
	if err != nil {
		return data.Table{}, fmt.Errorf("excel.open: %w", err)
	}
	defer book.Close()
	sheet := strings.TrimSpace(request.Worksheet)
	if sheet == "" {
		sheets := book.GetSheetList()
		if len(sheets) == 0 {
			return data.Table{Columns: []data.Column{}, Rows: [][]data.Value{}}, nil
		}
		sheet = sheets[0]
	}
	if index, err := book.GetSheetIndex(sheet); err != nil || index < 0 {
		return data.Table{}, fmt.Errorf("excel.sheet-not-found:%s", sheet)
	}
	rows, err := book.GetRows(sheet)
	if err != nil {
		return data.Table{}, fmt.Errorf("excel.read: %w", err)
	}
	if strings.TrimSpace(request.Range) != "" {
		rows, err = selectRange(rows, request.Range)
		if err != nil {
			return data.Table{}, err
		}
	}
	if len(rows) == 0 {
		return data.Table{Columns: []data.Column{}, Rows: [][]data.Value{}}, nil
	}
	hasHeader := optionBool(connection.Source.Options, "header", true)
	width := 0
	for _, row := range rows {
		if len(row) > width {
			width = len(row)
		}
	}
	names := make([]string, width)
	start := 0
	if hasHeader {
		copy(names, rows[0])
		start = 1
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
	table := data.Table{Columns: make([]data.Column, len(selected)), Rows: make([][]data.Value, 0, len(rows)-start)}
	for index, sourceIndex := range selected {
		table.Columns[index] = data.Column{Name: names[sourceIndex], NativeType: "TEXT", Nullable: true, Ordinal: index + 1}
	}
	maximum := request.MaximumRows
	if maximum == 0 {
		maximum = optionInt(connection.Source.Options, "maxRows", optionInt(connection.Source.Options, "defaultMaxRows", 500))
	}
	for _, source := range rows[start:] {
		if err := ctx.Err(); err != nil {
			return data.Table{}, err
		}
		if maximum > 0 && len(table.Rows) >= maximum {
			table.Truncated = true
			break
		}
		row := make([]data.Value, len(selected))
		for index, sourceIndex := range selected {
			if sourceIndex < len(source) {
				row[index] = data.StringValue(source[sourceIndex])
			} else {
				row[index] = data.NullValue()
			}
		}
		table.Rows = append(table.Rows, row)
	}
	return table, nil
}

func workbookPath(connection data.Connection, requested string) (string, error) {
	if strings.TrimSpace(requested) != "" {
		return strings.TrimSpace(requested), nil
	}
	value, found := connection.Source.Options.Lookup("path")
	if !found {
		return "", fmt.Errorf("excel.path-required")
	}
	path, ok := value.AsString()
	if !ok || strings.TrimSpace(path) == "" {
		return "", fmt.Errorf("excel.path-required")
	}
	return strings.TrimSpace(path), nil
}

func selectRange(rows [][]string, reference string) ([][]string, error) {
	parts := strings.Split(strings.TrimSpace(reference), ":")
	if len(parts) > 2 || len(parts) == 0 {
		return nil, fmt.Errorf("excel.invalid-range:%s", reference)
	}
	startColumn, startRow, err := excelize.CellNameToCoordinates(parts[0])
	if err != nil {
		return nil, fmt.Errorf("excel.invalid-range:%s", reference)
	}
	endColumn, endRow := startColumn, startRow
	if len(parts) == 2 {
		endColumn, endRow, err = excelize.CellNameToCoordinates(parts[1])
		if err != nil {
			return nil, fmt.Errorf("excel.invalid-range:%s", reference)
		}
	}
	if endColumn < startColumn || endRow < startRow {
		return nil, fmt.Errorf("excel.invalid-range:%s", reference)
	}
	if startRow > len(rows) {
		return [][]string{}, nil
	}
	endRow = min(endRow, len(rows))
	selected := make([][]string, 0, endRow-startRow+1)
	for rowIndex := startRow - 1; rowIndex < endRow; rowIndex++ {
		row := make([]string, endColumn-startColumn+1)
		if rowIndex < len(rows) {
			for columnIndex := startColumn - 1; columnIndex < endColumn; columnIndex++ {
				if columnIndex < len(rows[rowIndex]) {
					row[columnIndex-startColumn+1] = rows[rowIndex][columnIndex]
				}
			}
		}
		selected = append(selected, row)
	}
	return selected, nil
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
			return nil, fmt.Errorf("excel.column-not-found:%s", requestedName)
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
	_ data.Cataloger      = (*Provider)(nil)
	_ data.Describer      = (*Provider)(nil)
	_ data.WorkbookReader = (*Provider)(nil)
)
