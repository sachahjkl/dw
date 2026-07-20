package paritytest_test

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/workspace"
)

func TestLegacyTaskManifestNormalizesWithoutLosingExtensions(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "contract", "legacy-task.json")
	manifest, err := workspace.ReadManifest(fixture)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := manifest.Repositories, []string{"front", "back"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("repositories = %#v, want %#v", got, want)
	}
	if got := manifest.PrimaryWorkItemID(); got != "123" {
		t.Fatalf("primary work item = %q, want 123", got)
	}
	extension, exists := manifest.Unknown["legacyExtension"]
	if !exists {
		t.Fatal("legacy extension was discarded")
	}
	var decoded struct {
		Enabled bool     `json:"enabled"`
		Order   []string `json:"order"`
	}
	if err := json.Unmarshal(extension, &decoded); err != nil {
		t.Fatal(err)
	}
	if !decoded.Enabled || !reflect.DeepEqual(decoded.Order, []string{"z", "a"}) {
		t.Fatalf("legacy extension changed: %#v", decoded)
	}
}

func TestManifestJSONIsByteStableAndFieldOrdered(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "contract", "legacy-task.json")
	manifest, err := workspace.ReadManifest(fixture)
	if err != nil {
		t.Fatal(err)
	}
	first, err := json.Marshal(manifest)
	if err != nil {
		t.Fatal(err)
	}
	for iteration := 0; iteration < 100; iteration++ {
		got, err := json.Marshal(manifest)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, first) {
			t.Fatalf("marshal %d is nondeterministic:\nfirst: %s\n got: %s", iteration, first, got)
		}
	}
	orderedKeys := [][]byte{
		[]byte(`"schema"`),
		[]byte(`"workItemId"`),
		[]byte(`"taskId"`),
		[]byte(`"project"`),
		[]byte(`"type"`),
		[]byte(`"slug"`),
		[]byte(`"branchName"`),
		[]byte(`"createdAt"`),
		[]byte(`"repositories"`),
		[]byte(`"status"`),
		[]byte(`"legacyExtension"`),
	}
	position := -1
	for _, key := range orderedKeys {
		next := bytes.Index(first, key)
		if next <= position {
			t.Fatalf("key %s is not in contract order in %s", key, first)
		}
		position = next
	}
}

func TestWriteManifestUsesAtomicRealFilesystemRoundTrip(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "contract", "legacy-task.json")
	manifest, err := workspace.ReadManifest(fixture)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "nested", workspace.ManifestFile)
	if err := workspace.WriteManifest(path, manifest); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasSuffix(contents, []byte("\n")) || bytes.HasSuffix(contents, []byte("\n\n")) {
		t.Fatalf("manifest must have exactly one trailing newline: %q", contents)
	}
	roundTrip, err := workspace.ReadManifest(path)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(roundTrip.Repositories, manifest.Repositories) || roundTrip.PrimaryWorkItemID() != manifest.PrimaryWorkItemID() {
		t.Fatalf("round trip changed manifest: %#v", roundTrip)
	}
}
