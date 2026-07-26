package dataapp

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/data"
)

type projectionProvider struct{}

func (projectionProvider) Name() data.ProviderName { return "projection" }
func (projectionProvider) Catalog(context.Context, data.Connection) ([]data.CatalogEntry, error) {
	return []data.CatalogEntry{{Kind: data.CatalogView, Catalog: "analytics", Schema: "public", Name: "people"}}, nil
}
func (projectionProvider) Describe(context.Context, data.Connection, data.ObjectRef) (data.Description, error) {
	return data.Description{Columns: []data.Column{{Name: "name", NativeType: "TEXT", Nullable: true}}}, nil
}

func TestCatalogAndDescribeUseProviderNeutralColumns(t *testing.T) {
	root := t.TempDir()
	configDirectory := filepath.Join(root, "config")
	if err := os.MkdirAll(configDirectory, 0o755); err != nil {
		t.Fatal(err)
	}
	configuration := `{"schema":1,"defaults":{"readonly":true,"maxRows":500,"timeoutSeconds":600},"globals":{"sample":{"provider":"projection","connectionString":"opaque"}},"projects":{}}`
	if err := os.WriteFile(filepath.Join(configDirectory, "databases.json"), []byte(configuration), 0o600); err != nil {
		t.Fatal(err)
	}
	registry := data.NewRegistry()
	if err := registry.Register(projectionProvider{}); err != nil {
		t.Fatal(err)
	}
	service := NewService(registry, nil)
	selection := Selection{Root: root, Source: "sample"}

	catalog, err := service.Catalog(context.Background(), selection)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(catalog.Columns, []string{"Namespace", "Resource", "Kind"}) {
		t.Fatalf("catalog columns = %#v", catalog.Columns)
	}
	if len(catalog.Rows) != 1 || catalog.Rows[0][0].Value != "analytics.public" || catalog.Rows[0][2].Value != "view" {
		t.Fatalf("catalog rows = %#v", catalog.Rows)
	}

	description, err := service.Describe(context.Background(), selection, "people")
	if err != nil {
		t.Fatal(err)
	}
	if description == nil || !reflect.DeepEqual(description.Columns, []string{"Field", "Type", "Nullable"}) {
		t.Fatalf("description = %#v", description)
	}
	if len(description.Rows) != 1 || description.Rows[0][2].Value != "Yes" {
		t.Fatalf("description rows = %#v", description.Rows)
	}
}

func TestMissingDataConfigurationIncludesNextStep(t *testing.T) {
	_, err := Inventory(t.TempDir())
	if err == nil || !strings.Contains(err.Error(), "Next: run dw init") {
		t.Fatalf("missing configuration error = %v", err)
	}
}
