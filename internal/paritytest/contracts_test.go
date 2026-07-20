package paritytest_test

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/buildinfo"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/dbcompat"
)

func TestVersionFlagAndVersionCommandUseDifferentContracts(t *testing.T) {
	originalVersion, originalCommit := buildinfo.Version, buildinfo.Commit
	t.Cleanup(func() {
		buildinfo.Version, buildinfo.Commit = originalVersion, originalCommit
	})

	buildinfo.Version = "2026.07.17.3"
	buildinfo.Commit = "26b737af29029bc821cd6ddb4661c1f2266e3766"
	if got, want := buildinfo.Informational(), "2026.07.17.3+26b737a"; got != want {
		t.Fatalf("informational version = %q, want %q", got, want)
	}
	if got, want := buildinfo.Version, "2026.07.17.3"; got != want {
		t.Fatalf("runtime/machine version = %q, want %q", got, want)
	}

	buildinfo.Commit = " short "
	if got, want := buildinfo.Informational(), "2026.07.17.3+short"; got != want {
		t.Fatalf("short informational version = %q, want %q", got, want)
	}
	buildinfo.Commit = "   "
	if got := buildinfo.Informational(); got != buildinfo.Version {
		t.Fatalf("blank commit informational version = %q, want %q", got, buildinfo.Version)
	}
}

func TestOrderedMapRetainsInsertionOrderAcrossMutation(t *testing.T) {
	ordered := contract.NewOrderedMap[string, int](0)
	ordered.Set("zeta", 1)
	ordered.Set("alpha", 2)
	ordered.Set("middle", 3)
	if replaced := ordered.Set("alpha", 20); !replaced {
		t.Fatal("updating alpha was not reported as a replacement")
	}
	if deleted := ordered.Delete("zeta"); !deleted {
		t.Fatal("deleting zeta failed")
	}
	ordered.Set("zeta", 10)

	if got, want := ordered.Keys(), []string{"alpha", "middle", "zeta"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("ordered keys = %#v, want %#v", got, want)
	}
	if got, want := ordered.Values(), []int{20, 3, 10}; !reflect.DeepEqual(got, want) {
		t.Fatalf("ordered values = %#v, want %#v", got, want)
	}
	clone := ordered.Clone()
	clone.Set("alpha", 99)
	if value, _ := ordered.Get("alpha"); value != 20 {
		t.Fatalf("mutating clone changed original value to %d", value)
	}
}

func TestSQLGuardMatchesRustCompatibilityCases(t *testing.T) {
	tests := []struct {
		name      string
		statement string
		allowed   bool
		reason    string
	}{
		{name: "empty", statement: " \t\n", reason: "The SQL query is empty."},
		{name: "select", statement: "SELECT id, name FROM dbo.Users", allowed: true},
		{name: "select into", statement: "SELECT * INTO dbo.Copy FROM dbo.Source", reason: "Forbidden SQL keyword in read-only mode: INTO."},
		{name: "sequence advancement", statement: "SELECT NEXT VALUE FOR dbo.InvoiceSequence", reason: "Forbidden SQL keyword in read-only mode: NEXT VALUE FOR."},
		{name: "next column", statement: "SELECT next FROM dbo.Items", allowed: true},
		{name: "linked server passthrough", statement: "SELECT * FROM OPENQUERY(RemoteServer, 'DELETE FROM dbo.Users')", reason: "Forbidden SQL keyword in read-only mode: OPENQUERY."},
		{name: "openquery substring identifier", statement: "SELECT openquery_status FROM dbo.Items", allowed: true},
		{name: "cte", statement: "WITH active AS (SELECT id FROM dbo.Users) SELECT id FROM active", allowed: true},
		{name: "stored procedure introspection", statement: "sp_help 'dbo.Users'", reason: "Only SELECT/WITH queries are allowed."},
		{name: "leading comments", statement: "-- read only\n/* inventory */ SELECT 1", allowed: true},
		{name: "wrong verb", statement: "PRINT 'hello'", reason: "Only SELECT/WITH queries are allowed."},
		{name: "insert", statement: "WITH row AS (SELECT 1 AS id) INSERT INTO log SELECT id FROM row", reason: "Only SELECT/WITH queries are allowed."},
		{name: "execute", statement: "SELECT 1; EXECUTE dbo.mutate", reason: "Only one read-only SQL statement is allowed."},
		{name: "keyword inside identifier", statement: "SELECT updated_at, created_by FROM audit", allowed: true},
		{name: "comment delimiters cannot hide mutation", statement: "SELECT '/*'; DELETE FROM dbo.Users; SELECT '*/'", reason: "Only one read-only SQL statement is allowed."},
		{name: "comment markers inside string-only select", statement: "SELECT '/* DELETE FROM dbo.Users */' AS sample", allowed: true},
		{name: "double-quoted delimiters cannot hide mutation", statement: `SELECT "/*"; DELETE FROM dbo.Users; SELECT "*/"`, reason: "Only one read-only SQL statement is allowed."},
		{name: "bracket-quoted delimiters cannot hide mutation", statement: "SELECT [/*]; DELETE FROM dbo.Users; SELECT [*/]", reason: "Only one read-only SQL statement is allowed."},
		{name: "comment markers inside double-quoted select", statement: `SELECT "/* DELETE FROM dbo.Users */"`, allowed: true},
		{name: "comment markers inside bracket-quoted select", statement: "SELECT [/* DELETE FROM dbo.Users */]", allowed: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			result := sqlserver.ValidateReadOnlySQL(test.statement)
			if result.IsAllowed != test.allowed {
				t.Fatalf("IsAllowed = %v, want %v (reason %v)", result.IsAllowed, test.allowed, result.Reason)
			}
			if test.reason == "" {
				if result.Reason != nil {
					t.Fatalf("allowed query has reason %q", *result.Reason)
				}
				return
			}
			if result.Reason == nil || *result.Reason != test.reason {
				t.Fatalf("reason = %v, want %q", result.Reason, test.reason)
			}
		})
	}
}

func TestSQLGuardRejectsMultipleTopLevelStatements(t *testing.T) {
	for _, statement := range []string{
		"SELECT 1; SELECT 2",
		"SELECT 1; DBCC CHECKIDENT ('dbo.Users', RESEED, 0)",
		"SELECT 1\nSELECT 2",
		"SELECT 1\nDBCC CHECKIDENT ('dbo.Users', RESEED, 0)",
	} {
		result := sqlserver.ValidateReadOnlySQL(statement)
		if result.IsAllowed || result.Reason == nil {
			t.Errorf("multiple top-level statements were allowed: %q (%#v)", statement, result)
		}
	}
	for _, statement := range []string{
		"SELECT 1",
		"SELECT 1;",
		"SELECT ';' AS separator;",
		"SELECT 1 /* ; ignored */;",
		"SELECT 1 -- ; ignored\n;",
		"SELECT 1\nUNION ALL\nSELECT 2;",
		"SELECT (SELECT 1) AS nested;",
	} {
		result := sqlserver.ValidateReadOnlySQL(statement)
		if !result.IsAllowed {
			t.Errorf("single SELECT was blocked: %q (%#v)", statement, result)
		}
	}
}

func TestDatabaseTSVPreservesNullAndTruncationContract(t *testing.T) {
	result := dbcompat.QueryResult{
		Columns:   []string{"Id", "Name"},
		Rows:      [][]sqlserver.Cell{{sqlserver.StringCell("1"), sqlserver.NullCell()}},
		Truncated: true,
	}
	if got, want := dbcompat.QueryTSV(result), "Id\tName\n1\tNULL\n-- 1 rows (truncated)"; got != want {
		t.Fatalf("TSV = %q, want %q", got, want)
	}
	result.Rows = nil
	result.Truncated = false
	if got, want := dbcompat.QueryTSV(result), "Id\tName\n-- 0 rows"; got != want {
		t.Fatalf("empty TSV = %q, want %q", got, want)
	}
}

func TestSQLServerSafetyOptionsOverrideDuplicates(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "URL query duplicates",
			input: "sqlserver://user:pass@localhost/app?application+intent=ReadWrite&ApplicationIntent=ReadWrite&trust_server_certificate=false&TrustServerCertificate=false&encrypt=true",
			want:  "sqlserver://user:pass@localhost/app?ApplicationIntent=ReadOnly&TrustServerCertificate=true&encrypt=true",
		},
		{
			name:  "semicolon duplicates with protected delimiters",
			input: `Server=localhost;Password="a;b";Application Intent=ReadWrite;application_intent=ReadWrite;Trust Server Certificate=false;TRUST-SERVER-CERTIFICATE=false;Extra={x;y};Database=app`,
			want:  `Server=localhost;Password="a;b";Extra={x;y};Database=app;ApplicationIntent=ReadOnly;TrustServerCertificate=true`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := sqlserver.EnforceReadOnlyConnectionString(test.input); got != test.want {
				t.Fatalf("normalized DSN = %q, want %q", got, test.want)
			}
		})
	}
}
