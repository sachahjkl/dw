package bootstrap

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/providerapp"
)

func TestProviderCompletionValuesMatchCommandDomain(t *testing.T) {
	providers := []providerapp.Summary{
		{Name: "azure-devops", Kinds: []providerapp.Kind{providerapp.KindWork}, Capabilities: []string{"authenticator"}},
		{Name: "github", Kinds: []providerapp.Kind{providerapp.KindWork}, Capabilities: []string{"authenticator"}},
		{Name: "sqlite", Kinds: []providerapp.Kind{providerapp.KindData}, Capabilities: []string{"tabular-reader"}},
	}

	tests := []struct {
		name string
		path []string
		want []string
	}{
		{name: "auth", path: []string{"provider", "auth", "login"}, want: []string{"azure-devops", "github"}},
		{name: "work", path: []string{"work", "item", "list"}, want: []string{"azure-devops", "github"}},
		{name: "data", path: []string{"data", "read"}, want: []string{"sqlite"}},
		{name: "inspection", path: []string{"provider", "show"}, want: []string{"azure-devops", "github", "sqlite"}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := providerCompletionValues(providers, test.path); !reflect.DeepEqual(got, test.want) {
				t.Fatalf("providers = %#v, want %#v", got, test.want)
			}
		})
	}
}
