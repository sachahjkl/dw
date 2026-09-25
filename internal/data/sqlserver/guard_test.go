package sqlserver

import "testing"

func TestGuardRejectsSameLineStatementBypasses(t *testing.T) {
	for _, statement := range []string{
		"SELECT 1 SHUTDOWN WITH NOWAIT",
		"SELECT 1 KILL 52",
		"SELECT 1 DBCC FREEPROCCACHE",
		"SELECT 1 BACKUP DATABASE db TO DISK = 'x'",
		"SELECT 1 RESTORE DATABASE db FROM DISK = 'x'",
		"SELECT 1 USE master",
		"SELECT 1 SET NOCOUNT ON",
		"SELECT 1 DECLARE @x int",
		"SELECT 1 DENY SELECT ON t TO u",
		"SELECT 1 RECONFIGURE",
		"SELECT 1 WAITFOR DELAY '00:10'",
		"SELECT 1 BULK INSERT t FROM 'x'",
		"SELECT 1 RECEIVE * FROM q",
		"SELECT 1 SEND ON CONVERSATION @h",
		"SELECT 1 SELECT 2",
		"SELECT 1 /* c */ SHUTDOWN",
		"SELECT 1 BEGIN TRAN",
	} {
		if result := ValidateReadOnlySQL(statement); result.IsAllowed {
			t.Errorf("bypass allowed: %q", statement)
		}
	}
}

func TestGuardAllowsIdentifiersContainingKeywords(t *testing.T) {
	for _, statement := range []string{
		"SELECT settings, offset_value, username, backups FROM dbo.Config",
		"SELECT id FROM dbo.Items ORDER BY id OFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY",
		"SELECT TOP 5 WITH TIES id FROM dbo.Items ORDER BY id",
		"SELECT id FROM dbo.Items WITH (NOLOCK)",
		"SELECT 'SHUTDOWN' AS word, [set] FROM dbo.Items",
		"SELECT 1 UNION ALL SELECT 2 UNION SELECT 3",
	} {
		if result := ValidateReadOnlySQL(statement); !result.IsAllowed {
			t.Errorf("valid query blocked: %q (%v)", statement, *result.Reason)
		}
	}
}

func TestIsBoundary(t *testing.T) {
	tests := []struct {
		value string
		index int
		want  bool
	}{
		{"set", 0, true},
		{"set", 3, true},
		{"a set", 2, true},
		{"settings", 3, false},
		{"offset", 3, false},
		{"a_set", 2, false},
		{"a.set", 2, true},
		{"set.x", 3, true},
		{"ab", 1, false},
		{"a b", 2, true},
	}
	for _, test := range tests {
		if got := isBoundary(test.value, test.index); got != test.want {
			t.Errorf("isBoundary(%q, %d) = %v, want %v", test.value, test.index, got, test.want)
		}
	}
}
