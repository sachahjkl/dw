package controller

import (
	"context"
	"fmt"
	"sort"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
)

type Builder func(*parse.Result) (action.Request, error)
type Projector func(action.ResultEnvelope, *parse.Result) (console.OutputFormat, *console.JSONProjection, error)
type Direct func(context.Context, Execution, *parse.Result) (Outcome, error)
type MachineMode func(parse.Values) bool
type Status func(action.ResultEnvelope) console.ExitCode

type Route struct {
	Key     string
	Build   Builder
	Project Projector
	Direct  Direct
	Machine MachineMode
	Grant   SafetyGrant
	Status  Status
}

type Outcome struct {
	Output console.Output
	Code   console.ExitCode
}

type Registry struct {
	routes map[string]Route
	order  []string
}

func NewRegistry() *Registry { return &Registry{routes: make(map[string]Route)} }

func (registry *Registry) Register(route Route) error {
	if route.Key == "" {
		return fmt.Errorf("cli.empty-route-key")
	}
	if (route.Direct == nil) == (route.Build == nil) {
		return fmt.Errorf("cli.invalid-route:%s", route.Key)
	}
	if route.Build != nil && route.Project == nil {
		return fmt.Errorf("cli.missing-projector:%s", route.Key)
	}
	if _, exists := registry.routes[route.Key]; exists {
		return fmt.Errorf("cli.duplicate-route:%s", route.Key)
	}
	registry.routes[route.Key] = route
	registry.order = append(registry.order, route.Key)
	return nil
}

func (registry *Registry) Route(key string) (Route, bool) {
	route, ok := registry.routes[key]
	return route, ok
}

func (registry *Registry) Keys() []string { return append([]string(nil), registry.order...) }

// ValidateComplete makes grammar/controller drift a bootstrap error rather than
// a runtime "not implemented" path.
func (registry *Registry) ValidateComplete(root *spec.Command) error {
	if root == nil {
		return fmt.Errorf("cli.nil-grammar")
	}
	required := leafKeys(root)
	missing := make([]string, 0)
	for _, key := range required {
		if _, exists := registry.routes[key]; !exists {
			missing = append(missing, key)
		}
	}
	extra := make([]string, 0)
	for key := range registry.routes {
		if !contains(required, key) {
			extra = append(extra, key)
		}
	}
	if len(missing) == 0 && len(extra) == 0 {
		return nil
	}
	sort.Strings(missing)
	sort.Strings(extra)
	return fmt.Errorf("cli.route-coverage:missing=%v:extra=%v", missing, extra)
}

func leafKeys(root *spec.Command) []string {
	keys := make([]string, 0)
	var visit func(*spec.Command)
	visit = func(command *spec.Command) {
		if len(command.Children) == 0 {
			keys = append(keys, command.Key)
			return
		}
		for _, child := range command.Children {
			visit(child)
		}
	}
	for _, child := range root.Children {
		visit(child)
	}
	return keys
}

func contains(values []string, candidate string) bool {
	for _, value := range values {
		if value == candidate {
			return true
		}
	}
	return false
}
