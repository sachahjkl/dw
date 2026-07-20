package paritytest_test

import (
	"bytes"
	"testing"

	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/data"
)

func TestConsoleQueryMachineFormatsPreserveOrderAndTypes(t *testing.T) {
	table := data.Table{
		Columns:   []data.Column{{Name: "Id"}, {Name: "Name"}, {Name: "Enabled"}},
		Rows:      [][]data.Value{{data.IntegerValue("001"), data.NullValue(), data.BooleanValue(true)}},
		Truncated: true,
	}
	if got, want := console.RenderQueryTSV(table), "Id\tName\tEnabled\n001\tNULL\ttrue\n-- 1 rows (truncated)"; got != want {
		t.Fatalf("console TSV = %q, want %q", got, want)
	}
	projection := console.QueryJSONProjection(table)
	compact, err := console.RenderCompactJSON(projection)
	if err != nil {
		t.Fatal(err)
	}
	want := []byte(`{"columns":["Id","Name","Enabled"],"rows":[["001",null,true]],"truncated":true}`)
	if !bytes.Equal(compact, want) {
		t.Fatalf("compact query JSON = %s, want %s", compact, want)
	}
	for iteration := 0; iteration < 100; iteration++ {
		got, err := console.RenderCompactJSON(projection)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, want) {
			t.Fatalf("query JSON render %d = %s, want %s", iteration, got, want)
		}
	}
	pretty, err := console.RenderJSON(projection)
	if err != nil {
		t.Fatal(err)
	}
	if pretty.Format != console.FormatJSON || len(pretty.Body) == 0 || pretty.Body[len(pretty.Body)-1] == '\n' {
		t.Fatalf("pretty machine output contract = %#v", pretty)
	}
}
