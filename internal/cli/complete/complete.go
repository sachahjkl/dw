package complete

import (
	"os"
	"sort"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/l10n"
)

type Candidate struct {
	Label       string
	Description l10n.ID
}

type Context struct {
	Kind      spec.CompletionKind
	Path      []string
	Argument  string
	Root      string
	Project   string
	Workspace string
	WorkItem  string
	Prefix    string
}

type Resolver interface {
	ResolveCompletion(Context) ([]Candidate, error)
}

type ResolverFunc func(Context) ([]Candidate, error)

func (f ResolverFunc) ResolveCompletion(context Context) ([]Candidate, error) { return f(context) }

type Item struct {
	Label       string `json:"label"`
	Description string `json:"description"`
}

type analysis struct {
	command     *spec.Command
	path        []string
	selected    map[string]bool
	values      map[string]string
	positionals int
	waiting     *spec.Argument
}

// CompleteInstalled applies the COMP_LINE fallback used by the Bash integration.
func CompleteInstalled(root *spec.Command, words []string, resolver Resolver) ([]Item, error) {
	if len(words) == 0 {
		words = WordsFromBash(os.Getenv("COMP_LINE"))
	}
	return Complete(root, words, resolver)
}

func Complete(root *spec.Command, words []string, resolver Resolver) ([]Item, error) {
	if root == nil {
		return nil, nil
	}
	current := ""
	prior := words
	if len(words) != 0 {
		current = words[len(words)-1]
		prior = words[:len(words)-1]
	}
	base := analyze(root, prior)
	if base.waiting != nil {
		return completeArgument(root, base, *base.waiting, current, resolver)
	}
	if strings.HasPrefix(current, "--") {
		name := strings.TrimPrefix(strings.SplitN(current, "=", 2)[0], "--")
		if arg, ok := findOption(root, base.command, name); ok && arg.Kind != spec.Bool && arg.Kind != spec.Count && !strings.Contains(current, "=") {
			return completeArgument(root, base, arg, "", resolver)
		}
		if index := strings.IndexByte(current, '='); index >= 0 {
			if arg, ok := findOption(root, base.command, strings.TrimPrefix(current[:index], "--")); ok {
				return completeArgument(root, base, arg, current[index+1:], resolver)
			}
		}
		return completeOptions(root, base, current), nil
	}
	if strings.HasPrefix(current, "-") {
		return completeOptions(root, base, current), nil
	}

	full := analyze(root, words)
	if _, ok := base.command.Child(current); current != "" && ok {
		full = analyze(root, words)
		if len(full.command.Children) != 0 {
			return completeCommands(full.command, ""), nil
		}
		if arg, ok := positionalAt(full.command, 0); ok {
			return completeArgument(root, full, arg, "", resolver)
		}
		return nil, nil
	}
	if len(base.command.Children) != 0 {
		return completeCommands(base.command, current), nil
	}
	if arg, ok := positionalAt(base.command, base.positionals); ok {
		return completeArgument(root, base, arg, current, resolver)
	}
	return nil, nil
}

func completeCommands(command *spec.Command, prefix string) []Item {
	children := make([]*spec.Command, 0, len(command.Children))
	for _, child := range command.Children {
		if !child.Hidden && strings.HasPrefix(child.Name, prefix) {
			children = append(children, child)
		}
	}
	if command.CompletionAlphabetical {
		sort.SliceStable(children, func(i, j int) bool { return children[i].Name < children[j].Name })
	}
	items := make([]Item, 0, len(children))
	for _, child := range children {
		items = append(items, Item{Label: child.Name, Description: command.Text(child.Completion)})
	}
	return items
}

func completeOptions(root *spec.Command, state analysis, prefix string) []Item {
	items := make([]Item, 0, len(state.command.Arguments))
	for _, arg := range state.command.Arguments {
		if arg.Positional() || arg.Hidden || arg.Completion.Hidden || state.selected[arg.Name] {
			continue
		}
		label := arg.Token()
		if !strings.HasPrefix(label, prefix) || !allowedBySelection(state.command, arg, state.selected) {
			continue
		}
		description := arg.Completion.OptionDescription
		if description == "" {
			description = arg.Help
		}
		items = append(items, Item{Label: label, Description: state.command.Text(description)})
	}
	return items
}

func completeArgument(root *spec.Command, state analysis, arg spec.Argument, prefix string, resolver Resolver) ([]Item, error) {
	candidates := make([]Candidate, 0, len(arg.Completion.Values))
	for index, value := range arg.Completion.Values {
		description := arg.Completion.Description
		if index < len(arg.Completion.ValueDescriptions) {
			description = arg.Completion.ValueDescriptions[index]
		}
		candidates = append(candidates, Candidate{Label: value, Description: description})
	}
	if arg.Completion.Kind == spec.CompleteEnvVariable {
		dynamic, err := (EnvironmentResolver{}).ResolveCompletion(Context{Kind: arg.Completion.Kind, Prefix: prefix})
		if err != nil {
			return nil, err
		}
		candidates = append(candidates, dynamic...)
	}
	if resolver != nil && arg.Completion.Kind != spec.CompleteNone {
		dynamic, err := resolver.ResolveCompletion(Context{
			Kind: arg.Completion.Kind, Path: append([]string(nil), state.path...), Argument: arg.Name,
			Root: state.values["root"], Project: state.values["project"], Workspace: state.values["workspace"], WorkItem: state.values["work_item"], Prefix: prefix,
		})
		if err != nil {
			return nil, err
		}
		candidates = append(candidates, dynamic...)
	}
	seen := make(map[string]struct{}, len(candidates))
	items := make([]Item, 0, len(candidates))
	for _, candidate := range candidates {
		if !strings.HasPrefix(candidate.Label, prefix) {
			continue
		}
		if _, ok := seen[candidate.Label]; ok {
			continue
		}
		seen[candidate.Label] = struct{}{}
		description := candidate.Description
		if description == "" {
			description = arg.Completion.Description
		}
		items = append(items, Item{Label: candidate.Label, Description: state.command.Text(description)})
	}
	return items, nil
}

func analyze(root *spec.Command, words []string) analysis {
	state := analysis{command: root, selected: make(map[string]bool), values: make(map[string]string)}
	for index := 0; index < len(words); {
		token := words[index]
		if token == "" {
			break
		}
		if token == "--" {
			index++
			continue
		}
		if strings.HasPrefix(token, "--") {
			nameValue := strings.TrimPrefix(token, "--")
			name, inline, hasInline := nameValue, "", false
			if equal := strings.IndexByte(nameValue, '='); equal >= 0 {
				name, inline, hasInline = nameValue[:equal], nameValue[equal+1:], true
			}
			arg, ok := findOption(root, state.command, name)
			if !ok {
				index++
				continue
			}
			state.selected[arg.Name] = true
			if arg.Kind == spec.Bool || arg.Kind == spec.Count {
				index++
				continue
			}
			if hasInline {
				state.values[arg.Name] = inline
				index++
				continue
			}
			if index+1 >= len(words) {
				copy := arg
				state.waiting = &copy
				break
			}
			state.values[arg.Name] = words[index+1]
			index += 2
			continue
		}
		if strings.HasPrefix(token, "-") {
			index++
			continue
		}
		if len(state.command.Children) != 0 {
			child, ok := state.command.Child(token)
			if !ok {
				break
			}
			state.command = child
			state.path = append(state.path, child.Name)
			state.positionals = 0
			index++
			continue
		}
		state.positionals++
		index++
	}
	return state
}

func findOption(root, command *spec.Command, long string) (spec.Argument, bool) {
	if arg, ok := command.ArgumentByLong(long); ok {
		return *arg, true
	}
	for _, arg := range root.Arguments {
		if arg.Global && arg.Long == long {
			return arg, true
		}
	}
	return spec.Argument{}, false
}

func positionalAt(command *spec.Command, index int) (spec.Argument, bool) {
	positionals := command.Positionals()
	if len(positionals) == 0 {
		return spec.Argument{}, false
	}
	if index < len(positionals) {
		return positionals[index], true
	}
	last := positionals[len(positionals)-1]
	if last.Repeatable || last.Trailing {
		return last, true
	}
	return spec.Argument{}, false
}

func allowedBySelection(command *spec.Command, candidate spec.Argument, selected map[string]bool) bool {
	for _, conflict := range candidate.Conflicts {
		if selected[conflict] {
			return false
		}
	}
	for _, required := range candidate.Requires {
		if !selected[required] {
			return false
		}
	}
	for _, existing := range command.Arguments {
		if !selected[existing.Name] {
			continue
		}
		for _, conflict := range existing.Conflicts {
			if conflict == candidate.Name {
				return false
			}
		}
	}
	return true
}

// EnvironmentResolver supplies environment-variable names without importing a provider package.
type EnvironmentResolver struct{}

func (EnvironmentResolver) ResolveCompletion(context Context) ([]Candidate, error) {
	if context.Kind != spec.CompleteEnvVariable {
		return nil, nil
	}
	items := make([]Candidate, 0, len(os.Environ()))
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		if name != "" {
			items = append(items, Candidate{Label: name})
		}
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].Label < items[j].Label })
	return items, nil
}
