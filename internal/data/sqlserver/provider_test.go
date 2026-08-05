package sqlserver

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/l10n"
)

func TestNewNativeQueryReportPreservesColumnsWithoutRows(t *testing.T) {
	columns := []string{"id", "display_name"}

	report := newNativeQueryReport(columns)

	if !reflect.DeepEqual(report.Columns, columns) {
		t.Fatalf("columns = %#v, want %#v", report.Columns, columns)
	}
	if report.Rows == nil || len(report.Rows) != 0 {
		t.Fatalf("rows = %#v, want a non-nil empty slice", report.Rows)
	}
	columns[0] = "mutated"
	if report.Columns[0] != "id" {
		t.Fatalf("report retained caller-owned column storage: %#v", report.Columns)
	}
}

func TestProviderErrorExposesLocalizedSQLDetail(t *testing.T) {
	problem := &ProviderError{Kind: ErrorSQL, Reason: "login failed"}
	if got := l10n.Render(problem.Localized()); got != "SQL Server error: login failed" {
		t.Fatalf("localized error = %q", got)
	}
}

func TestDecodePersistedConnectionString(t *testing.T) {
	plain := "Server=db;Database=app;User Id=user;Password=secret"
	littleEndian := make([]byte, 0, len(plain)*2)
	for _, current := range []byte(plain) {
		littleEndian = append(littleEndian, current, 0)
	}
	if got := decodePersistedConnectionString(string(littleEndian)); got != plain {
		t.Fatalf("decoded connection string = %q", got)
	}
	if got := decodePersistedConnectionString(plain); got != plain {
		t.Fatalf("UTF-8 connection string changed to %q", got)
	}
}
