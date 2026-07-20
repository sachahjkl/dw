package paritytest_test

import (
	"context"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/work/ado"
)

func requireLiveEnvironment(t *testing.T, names ...string) map[string]string {
	t.Helper()
	values := make(map[string]string, len(names))
	var missing []string
	for _, name := range names {
		value := strings.TrimSpace(os.Getenv(name))
		if value == "" {
			missing = append(missing, name)
		} else {
			values[name] = value
		}
	}
	if len(missing) != 0 {
		t.Skipf("live integration requires environment: %s", strings.Join(missing, ", "))
	}
	return values
}

func TestLiveSQLServerReadOnlyQueryRequiresEnvironment(t *testing.T) {
	environment := requireLiveEnvironment(t, "DW_LIVE_SQLSERVER_CONNECTION", "DW_LIVE_SQLSERVER_QUERY")
	readOnly := true
	maximumRows := 10
	timeoutSeconds := 30
	connection := sqlserver.ResolvedConnection{
		Config: sqlserver.ConnectionConfig{
			Provider:         sqlserver.ProviderName,
			ConnectionString: sqlserver.NewSecret(environment["DW_LIVE_SQLSERVER_CONNECTION"]),
			ReadOnly:         &readOnly,
			MaxRows:          &maximumRows,
			TimeoutSeconds:   &timeoutSeconds,
		},
		Defaults: sqlserver.DefaultSettings(),
	}
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	result, err := sqlserver.New(nil).Query(ctx, connection, environment["DW_LIVE_SQLSERVER_QUERY"], &maximumRows)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Rows) > maximumRows {
		t.Fatalf("live query returned %d rows above limit %d", len(result.Rows), maximumRows)
	}
}

func TestLiveADOReadRequiresEnvironment(t *testing.T) {
	environment := requireLiveEnvironment(t,
		"DW_ADO_TOKEN",
		"DW_LIVE_ADO_ORGANIZATION",
		"DW_LIVE_ADO_PROJECT",
		"DW_LIVE_ADO_WORK_ITEM_ID",
	)
	provider := ado.NewWithStore(ado.Options{
		Organization: environment["DW_LIVE_ADO_ORGANIZATION"],
		Project:      environment["DW_LIVE_ADO_PROJECT"],
	}, nil, nil)
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	items, err := provider.ReadItems(ctx, work.ProjectRef{
		Key:          contract.ProjectKey(environment["DW_LIVE_ADO_PROJECT"]),
		Organization: environment["DW_LIVE_ADO_ORGANIZATION"],
		Project:      environment["DW_LIVE_ADO_PROJECT"],
	}, []work.ItemID{work.ItemID(environment["DW_LIVE_ADO_WORK_ITEM_ID"])}, work.ReadOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 || items[0].ID.String() != environment["DW_LIVE_ADO_WORK_ITEM_ID"] {
		t.Fatalf("live ADO returned items %#v", items)
	}
}
