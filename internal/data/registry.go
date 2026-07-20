package data

import (
	"fmt"
	"sync"

	"github.com/sachahjkl/dw/internal/l10n"
)

type DuplicateProviderError struct{ Provider ProviderName }

func (e *DuplicateProviderError) Error() string {
	return "data.duplicate-provider:" + string(e.Provider)
}
func (e *DuplicateProviderError) Localized() l10n.Message {
	return l10n.M("error.duplicate-data-provider", l10n.A("name", e.Provider))
}

type ProviderNotFoundError struct{ Provider ProviderName }

func (e *ProviderNotFoundError) Error() string {
	return "data.provider-not-found:" + string(e.Provider)
}
func (e *ProviderNotFoundError) Localized() l10n.Message {
	return l10n.M("error.provider-not-found", l10n.A("provider", e.Provider))
}

type UnsupportedCapabilityError struct {
	Provider   ProviderName
	Capability Capability
}

func (e *UnsupportedCapabilityError) Error() string {
	return "data.unsupported-capability:" + string(e.Provider) + ":" + string(e.Capability)
}
func (e *UnsupportedCapabilityError) Localized() l10n.Message {
	return l10n.M("error.unsupported-capability", l10n.A("provider", e.Provider), l10n.A("capability", e.Capability))
}

// Require performs a typed optional-capability lookup.
func Require[T any](provider Provider, capability Capability) (T, error) {
	if implementation, ok := any(provider).(T); ok {
		return implementation, nil
	}
	var zero T
	name := ProviderName("")
	if provider != nil {
		name = provider.Name()
	}
	return zero, &UnsupportedCapabilityError{Provider: name, Capability: capability}
}

type Registry struct {
	mu        sync.RWMutex
	providers map[ProviderName]Provider
	order     []ProviderName
}

func NewRegistry() *Registry { return &Registry{providers: make(map[ProviderName]Provider)} }

func (r *Registry) Register(provider Provider) error {
	if provider == nil {
		return fmt.Errorf("data.nil-provider")
	}
	name := provider.Name()
	if name == "" {
		return fmt.Errorf("data.empty-provider-name")
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.providers[name]; exists {
		return &DuplicateProviderError{Provider: name}
	}
	r.providers[name] = provider
	r.order = append(r.order, name)
	return nil
}

func (r *Registry) Get(name ProviderName) (Provider, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	provider, ok := r.providers[name]
	if !ok {
		return nil, &ProviderNotFoundError{Provider: name}
	}
	return provider, nil
}

func (r *Registry) Providers() []Provider {
	r.mu.RLock()
	defer r.mu.RUnlock()
	providers := make([]Provider, len(r.order))
	for i, name := range r.order {
		providers[i] = r.providers[name]
	}
	return providers
}
