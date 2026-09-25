package sqlserver

import (
	"reflect"
	"testing"
	"time"

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

func TestReadOnlyConnectionStringTrustServerCertificate(t *testing.T) {
	enabled, disabled := true, false
	tests := []struct {
		input string
		trust *bool
		want  string
	}{
		{"Server=db;Database=app", nil, "Server=db;Database=app;ApplicationIntent=ReadOnly;TrustServerCertificate=true"},
		{"Server=db;TrustServerCertificate=false;ApplicationIntent=ReadWrite", nil, "Server=db;ApplicationIntent=ReadOnly;TrustServerCertificate=false"},
		{"Server=db;Trust Server Certificate=true;TRUST-SERVER-CERTIFICATE=false", nil, "Server=db;ApplicationIntent=ReadOnly;TrustServerCertificate=false"},
		{"Server=db;TrustServerCertificate=true", &disabled, "Server=db;ApplicationIntent=ReadOnly;TrustServerCertificate=false"},
		{"Server=db", &enabled, "Server=db;ApplicationIntent=ReadOnly;TrustServerCertificate=true"},
		{"sqlserver://db/app?TrustServerCertificate=false", nil, "sqlserver://db/app?ApplicationIntent=ReadOnly&TrustServerCertificate=false"},
		{"sqlserver://db/app?trust_server_certificate=true", &disabled, "sqlserver://db/app?ApplicationIntent=ReadOnly&TrustServerCertificate=false"},
	}
	for _, test := range tests {
		if got := ReadOnlyConnectionString(test.input, test.trust); got != test.want {
			t.Errorf("ReadOnlyConnectionString(%q) = %q, want %q", test.input, got, test.want)
		}
	}
}

func TestTimeCellUsesRFC3339Nano(t *testing.T) {
	cell, err := cellFromDriverValue(time.Date(2026, 1, 2, 3, 4, 5, 6, time.UTC), "DATETIME2")
	if err != nil || cell.Value != "2026-01-02T03:04:05.000000006Z" {
		t.Fatalf("cell = %#v, err = %v", cell, err)
	}
}

func TestProviderErrorExposesLocalizedSQLDetail(t *testing.T) {
	problem := &ProviderError{Kind: ErrorSQL, Reason: "login failed"}
	if got := l10n.Render(problem.Localized()); got != "SQL Server error: login failed" {
		t.Fatalf("localized error = %q", got)
	}
}
