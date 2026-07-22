package sqlite

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func TestProviderReadsSQLiteCatalogSchemaAndRows(t *testing.T) {
	path := filepath.Join(t.TempDir(), "sample.sqlite")
	database, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(`create table people (id integer primary key, name text not null); insert into people values (1, 'Ada'), (2, 'Grace')`); err != nil {
		t.Fatal(err)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	connection := data.Connection{Source: data.Source{Provider: ProviderName, Options: wirejson.ObjectValue(
		wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
		wirejson.Member{Name: "maxRows", Value: wirejson.NumberValue("1")},
	)}}
	provider := New()

	catalog, err := provider.Catalog(context.Background(), connection)
	if err != nil {
		t.Fatal(err)
	}
	if len(catalog) != 1 || catalog[0].Name != "people" || catalog[0].Schema != "main" {
		t.Fatalf("catalog = %#v", catalog)
	}
	description, err := provider.Describe(context.Background(), connection, data.ObjectRef{Name: "people"})
	if err != nil {
		t.Fatal(err)
	}
	if len(description.Columns) != 2 || description.Columns[0].Name != "id" || description.Columns[1].Name != "name" {
		t.Fatalf("description = %#v", description)
	}
	table, err := provider.QueryNative(context.Background(), connection, data.NativeQuery{Statement: "select id, name from people order by id"})
	if err != nil {
		t.Fatal(err)
	}
	if len(table.Columns) != 2 || len(table.Rows) != 1 || !table.Truncated {
		t.Fatalf("table = %#v", table)
	}
	if value, ok := table.Rows[0][0].Text(); !ok || value != "1" {
		t.Fatalf("first id = %q, text=%v", value, ok)
	}
	if err := provider.ValidateRead(context.Background(), connection, data.NativeQuery{Statement: "delete from people"}); err == nil {
		t.Fatal("write statement passed the read-only guard")
	}
}

func TestProviderReadsCommittedWALData(t *testing.T) {
	path := filepath.Join(t.TempDir(), "live.sqlite")
	database, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	if _, err := database.Exec(`pragma journal_mode = wal; pragma wal_autocheckpoint = 0; create table entries (value text); insert into entries values ('visible')`); err != nil {
		t.Fatal(err)
	}
	connection := data.Connection{Source: data.Source{Provider: ProviderName, Options: wirejson.ObjectValue(
		wirejson.Member{Name: "path", Value: wirejson.StringValue(path)},
	)}}

	table, err := New().QueryNative(context.Background(), connection, data.NativeQuery{Statement: "select value from entries"})
	if err != nil {
		t.Fatal(err)
	}
	if len(table.Rows) != 1 {
		t.Fatalf("rows = %#v", table.Rows)
	}
	if value, ok := table.Rows[0][0].Text(); !ok || value != "visible" {
		t.Fatalf("value = %q, text=%v", value, ok)
	}
}
