package sqlserver

import (
	"reflect"
	"testing"
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
