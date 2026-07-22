package excel

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/xuri/excelize/v2"
)

func TestProviderCatalogsSheetsAndReadsRanges(t *testing.T) {
	path := filepath.Join(t.TempDir(), "people.xlsx")
	book := excelize.NewFile()
	if err := book.SetSheetName("Sheet1", "People"); err != nil {
		t.Fatal(err)
	}
	for cell, value := range map[string]any{
		"A1": "name", "B1": "age", "A2": "Ada", "B2": 36,
		"A3": "Grace", "B3": 85,
	} {
		if err := book.SetCellValue("People", cell, value); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := book.NewSheet("Empty"); err != nil {
		t.Fatal(err)
	}
	if err := book.SaveAs(path); err != nil {
		t.Fatal(err)
	}
	if err := book.Close(); err != nil {
		t.Fatal(err)
	}
	connection := data.Connection{Source: data.Source{Provider: ProviderName, Options: wirejson.ObjectValue(
		wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
	)}}
	provider := New()

	catalog, err := provider.Catalog(context.Background(), connection)
	if err != nil {
		t.Fatal(err)
	}
	if len(catalog) != 2 || catalog[0].Name != "People" || catalog[1].Name != "Empty" {
		t.Fatalf("catalog = %#v", catalog)
	}
	table, err := provider.ReadWorkbook(context.Background(), connection, data.WorkbookRead{Worksheet: "People", Range: "A1:B5"})
	if err != nil {
		t.Fatal(err)
	}
	if len(table.Columns) != 2 || table.Columns[0].Name != "name" || len(table.Rows) != 2 {
		t.Fatalf("table = %#v", table)
	}
	if value, ok := table.Rows[0][1].Text(); !ok || value != "36" {
		t.Fatalf("age = %q, text=%v", value, ok)
	}
	if _, err := provider.ReadWorkbook(context.Background(), connection, data.WorkbookRead{Worksheet: "Missing"}); err == nil {
		t.Fatal("missing sheet did not fail")
	}
}
