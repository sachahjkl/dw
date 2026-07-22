package controller

import (
	"reflect"
	"testing"
)

func TestSelectRepositoryPairsMapsExplicitLocalNames(t *testing.T) {
	configuredLocal := []string{"front", "api"}
	configuredProvider := []string{"platform/front", "api-service"}

	local, provider := selectRepositoryPairs(configuredLocal, configuredProvider, []string{"front", "unconfigured"})

	if want := []string{"front", "unconfigured"}; !reflect.DeepEqual(local, want) {
		t.Fatalf("local repositories = %#v, want %#v", local, want)
	}
	if want := []string{"platform/front", "unconfigured"}; !reflect.DeepEqual(provider, want) {
		t.Fatalf("provider repositories = %#v, want %#v", provider, want)
	}
}

func TestSelectRepositoryPairsKeepsConfiguredDefaults(t *testing.T) {
	configuredLocal := []string{"front"}
	configuredProvider := []string{"platform/front"}

	local, provider := selectRepositoryPairs(configuredLocal, configuredProvider, nil)

	if !reflect.DeepEqual(local, configuredLocal) || !reflect.DeepEqual(provider, configuredProvider) {
		t.Fatalf("default pairs = %#v/%#v, want %#v/%#v", local, provider, configuredLocal, configuredProvider)
	}
}
