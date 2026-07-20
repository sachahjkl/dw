package spec

import (
	"sort"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

type ValueKind uint8

const (
	Bool ValueKind = iota
	String
	Int
	Strings
	Count
)

type CompletionKind string

const (
	CompleteNone        CompletionKind = ""
	CompleteProfile     CompletionKind = "profile"
	CompleteProject     CompletionKind = "project"
	CompleteRepository  CompletionKind = "repository"
	CompleteWorkspace   CompletionKind = "workspace"
	CompleteWorkItem    CompletionKind = "work-item"
	CompleteAgent       CompletionKind = "agent"
	CompleteADOState    CompletionKind = "ado-state"
	CompleteDatabase    CompletionKind = "database"
	CompleteEnvironment CompletionKind = "database-environment"
	CompleteEnvVariable CompletionKind = "environment-variable"
	CompleteSecret      CompletionKind = "secret"
	CompleteShell       CompletionKind = "shell"
	CompleteColor       CompletionKind = "color"
	CompleteFormat      CompletionKind = "format"
	CompleteWorkType    CompletionKind = "work-type"
	CompleteRowLimit    CompletionKind = "row-limit"
)

type Completion struct {
	Kind              CompletionKind
	Values            []string
	Description       l10n.ID
	ValueDescriptions []l10n.ID
	OptionDescription l10n.ID
	Hidden            bool
}

type Default struct {
	String string
	Int    int64
}

type Validation uint8

const (
	ValidateNone Validation = iota
	ValidatePositive
)

type SpecialAction uint8

const (
	SpecialNone SpecialAction = iota
	SpecialHelp
	SpecialVersion
)

type Argument struct {
	Name       string
	Long       string
	Short      rune
	ValueName  string
	Kind       ValueKind
	Required   bool
	Repeatable bool
	Trailing   bool
	Hidden     bool
	Global     bool
	Default    *Default
	Validate   Validation
	Special    SpecialAction
	HelpBefore string
	Allowed    []string
	Conflicts  []string
	Requires   []string
	Help       l10n.ID
	Completion Completion
}

func (a Argument) Positional() bool { return a.Long == "" && a.Short == 0 }
func (a Argument) Token() string {
	if a.Long != "" {
		return "--" + a.Long
	}
	if a.Short != 0 {
		return "-" + string(a.Short)
	}
	return a.Name
}

type Command struct {
	Name                   string
	Key                    string
	Aliases                []string
	Hidden                 bool
	Summary                l10n.ID
	Completion             l10n.ID
	CompletionAlphabetical bool
	Arguments              []Argument
	Children               []*Command
	RejectedPaths          [][]string
	parent                 *Command
	localizer              l10n.Localizer
	english                map[l10n.ID]string
}

func (c *Command) Parent() *Command { return c.parent }

func (c *Command) Text(id l10n.ID) string {
	if id == "" {
		return ""
	}
	if c != nil && c.localizer != nil {
		if text := c.localizer.Text(id); text != "" && text != string(id) && !strings.HasPrefix(text, "[missing:") {
			return text
		}
	}
	for root := c; root != nil; root = root.parent {
		if root.english != nil {
			if text, ok := root.english[id]; ok {
				return text
			}
		}
	}
	return string(id)
}

func (c *Command) Child(name string) (*Command, bool) {
	for _, child := range c.Children {
		if child.Name == name {
			return child, true
		}
		for _, alias := range child.Aliases {
			if alias == name {
				return child, true
			}
		}
	}
	return nil, false
}

func (c *Command) ArgumentByLong(name string) (*Argument, bool) {
	for i := range c.Arguments {
		if c.Arguments[i].Long == name {
			return &c.Arguments[i], true
		}
	}
	return nil, false
}

func (c *Command) ArgumentByShort(name rune) (*Argument, bool) {
	for i := range c.Arguments {
		if c.Arguments[i].Short == name {
			return &c.Arguments[i], true
		}
	}
	return nil, false
}

func (c *Command) Positionals() []Argument {
	out := make([]Argument, 0, len(c.Arguments))
	for _, arg := range c.Arguments {
		if arg.Positional() {
			out = append(out, arg)
		}
	}
	return out
}

func (c *Command) VisibleChildren() []*Command {
	out := make([]*Command, 0, len(c.Children))
	for _, child := range c.Children {
		if !child.Hidden {
			out = append(out, child)
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out
}

func (c *Command) Options(inherited bool) []Argument {
	out := make([]Argument, 0, len(c.Arguments)+3)
	for _, arg := range c.Arguments {
		if !arg.Positional() && !arg.Hidden {
			out = append(out, arg)
		}
	}
	if inherited && c.parent != nil {
		root := c
		for root.parent != nil {
			root = root.parent
		}
		for _, arg := range root.Arguments {
			if arg.Global {
				out = append(out, arg)
			}
		}
	}
	sort.SliceStable(out, func(i, j int) bool {
		left, right := out[i].Long, out[j].Long
		if left == "" {
			left = string(out[i].Short)
		}
		if right == "" {
			right = string(out[j].Short)
		}
		return left < right
	})
	if c.parent != nil {
		for index := 0; index < len(out); index++ {
			before := out[index].HelpBefore
			if before == "" {
				continue
			}
			target := -1
			for candidate := range out {
				if out[candidate].Name == before {
					target = candidate
					break
				}
			}
			if target < 0 || index == target-1 {
				continue
			}
			arg := out[index]
			out = append(out[:index], out[index+1:]...)
			if index < target {
				target--
			}
			out = append(out, Argument{})
			copy(out[target+1:], out[target:])
			out[target] = arg
		}
	}
	return out
}

func Lookup(root *Command, path []string) (*Command, bool) {
	current := root
	for _, name := range path {
		child, ok := current.Child(name)
		if !ok {
			return nil, false
		}
		current = child
	}
	return current, true
}
