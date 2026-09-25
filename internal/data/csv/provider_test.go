package csv

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func TestProviderReadsQuotedCSVAndSelectedColumns(t *testing.T) {
	path := filepath.Join(t.TempDir(), "people.csv")
	if err := os.WriteFile(path, []byte("name,notes,age\nAda,\"math, engines\",36\nGrace,compilers,85\n"), 0o600); err != nil {
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
	if len(catalog) != 1 || catalog[0].Name != "people" {
		t.Fatalf("catalog = %#v", catalog)
	}
	table, err := provider.ReadTable(context.Background(), connection, data.TabularRead{Columns: []string{"notes", "name"}, MaximumRows: 1})
	if err != nil {
		t.Fatal(err)
	}
	if len(table.Rows) != 1 || !table.Truncated || table.Columns[0].Name != "notes" {
		t.Fatalf("table = %#v", table)
	}
	if value, ok := table.Rows[0][0].Text(); !ok || value != "math, engines" {
		t.Fatalf("quoted field = %q, text=%v", value, ok)
	}
}

func TestProviderInfersTabDelimiterAndSyntheticHeaders(t *testing.T) {
	path := filepath.Join(t.TempDir(), "values.tsv")
	if err := os.WriteFile(path, []byte("alpha\tbeta\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	connection := data.Connection{Source: data.Source{Options: wirejson.ObjectValue(
		wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
		wirejson.Member{Name: "header", Value: wirejson.BoolValue(false)},
	)}}
	table, err := New().ReadTable(context.Background(), connection, data.TabularRead{})
	if err != nil {
		t.Fatal(err)
	}
	if len(table.Columns) != 2 || table.Columns[0].Name != "column_1" || len(table.Rows) != 1 {
		t.Fatalf("table = %#v", table)
	}
}

func TestProviderDecodesWindows1252AndLatin1(t *testing.T) {
	path := filepath.Join(t.TempDir(), "legacy.csv")
	if err := os.WriteFile(path, []byte("nom,prix\nCaf\xe9,\x805\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct{ encoding, name, price string }{
		{"windows-1252", "Café", "€5"},
		{"latin1", "Café", "\u00805"},
	} {
		connection := data.Connection{Source: data.Source{Options: wirejson.ObjectValue(
			wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
			wirejson.Member{Name: "encoding", Value: wirejson.StringValue(test.encoding)},
		)}}
		table, err := New().ReadTable(context.Background(), connection, data.TabularRead{})
		if err != nil {
			t.Fatal(err)
		}
		name, _ := table.Rows[0][0].Text()
		price, _ := table.Rows[0][1].Text()
		if name != test.name || price != test.price {
			t.Fatalf("%s: row = %q, %q", test.encoding, name, price)
		}
	}
	connection := data.Connection{Source: data.Source{Options: wirejson.ObjectValue(
		wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
		wirejson.Member{Name: "encoding", Value: wirejson.StringValue("ebcdic")},
	)}}
	if _, err := New().ReadTable(context.Background(), connection, data.TabularRead{}); err == nil {
		t.Fatal("unsupported encoding was accepted")
	}
}
