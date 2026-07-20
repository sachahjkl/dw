package spec

import (
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	MsgUsage            l10n.ID = "cli.heading.usage"
	MsgCommands         l10n.ID = "cli.heading.commands"
	MsgArguments        l10n.ID = "cli.heading.arguments"
	MsgOptions          l10n.ID = "cli.heading.options"
	MsgDefault          l10n.ID = "cli.annotation.default"
	MsgRequired         l10n.ID = "cli.annotation.required"
	MsgErrUnknownCmd    l10n.ID = "cli.error.unknown-command"
	MsgErrMissingCmd    l10n.ID = "cli.error.missing-command"
	MsgErrUnknownOpt    l10n.ID = "cli.error.unknown-option"
	MsgErrMissingValue  l10n.ID = "cli.error.missing-value"
	MsgErrInvalidValue  l10n.ID = "cli.error.invalid-value"
	MsgErrMissingArg    l10n.ID = "cli.error.missing-argument"
	MsgErrUnexpectedArg l10n.ID = "cli.error.unexpected-argument"
	MsgErrDuplicate     l10n.ID = "cli.error.duplicate-option"
	MsgErrConflict      l10n.ID = "cli.error.conflict"
	MsgErrRequires      l10n.ID = "cli.error.requires"
	MsgErrPositiveInt   l10n.ID = "cli.error.positive-integer"
	MsgCompletionTitle  l10n.ID = "cli.completion.title"
	MsgCompletionIntro  l10n.ID = "cli.completion.intro"
	MsgErrSubcommand    l10n.ID = "cli.error.unrecognized-subcommand"
	MsgHelpHint         l10n.ID = "cli.error.help-hint"
)

type builder struct {
	localizer l10n.Localizer
	english   map[l10n.ID]string
}

func newBuilder(localizer l10n.Localizer) *builder {
	b := &builder{localizer: localizer, english: make(map[l10n.ID]string)}
	b.english[MsgUsage] = "Usage"
	b.english[MsgCommands] = "Commands"
	b.english[MsgArguments] = "Arguments"
	b.english[MsgOptions] = "Options"
	b.english[MsgDefault] = "default"
	b.english[MsgRequired] = "required"
	b.english[MsgCompletionTitle] = "Shell completion"
	b.english[MsgCompletionIntro] = "Install the integration for your shell:"
	b.english[MsgErrUnknownCmd] = "unrecognized command %q"
	b.english[MsgErrSubcommand] = "error: unrecognized subcommand '%s'"
	b.english[MsgHelpHint] = "For more information, try '--help'."
	b.english[MsgErrMissingCmd] = "a command is required"
	b.english[MsgErrUnknownOpt] = "unexpected option %q"
	b.english[MsgErrMissingValue] = "option %q requires a value"
	b.english[MsgErrInvalidValue] = "invalid value %q for %s"
	b.english[MsgErrMissingArg] = "required argument %q was not provided"
	b.english[MsgErrUnexpectedArg] = "unexpected argument %q"
	b.english[MsgErrDuplicate] = "option %q cannot be used multiple times"
	b.english[MsgErrConflict] = "%s cannot be used with %s"
	b.english[MsgErrRequires] = "%s requires %s"
	b.english[MsgErrPositiveInt] = "%s must be a positive integer"
	return b
}

func (b *builder) msg(id, english string) l10n.ID {
	messageID := l10n.ID(id)
	b.english[messageID] = english
	return messageID
}

func (b *builder) command(name, key, summary string, args []Argument, children ...*Command) *Command {
	completionText := strings.TrimSuffix(summary, ".")
	if text, ok := commandCompletionText[key]; ok {
		completionText = text
	}
	return &Command{
		Name: name, Key: key,
		Summary:                b.msg("cli.command."+key+".summary", summary),
		Completion:             b.msg("cli.command."+key+".completion", completionText),
		CompletionAlphabetical: true, Arguments: args, Children: children,
	}
}

func (b *builder) option(key, name string, kind ValueKind, help string) Argument {
	valueName := strings.ToUpper(strings.ReplaceAll(name, "-", "_"))
	arg := Argument{Name: strings.ReplaceAll(name, "-", "_"), Long: name, ValueName: valueName, Kind: kind,
		Help: b.msg("cli.command."+key+".option."+name, help)}
	if text, ok := optionCompletionText[name]; ok {
		arg.Completion.OptionDescription = b.msg("cli.option."+name+".completion", text)
	}
	if text, ok := valueCompletionText[name]; ok {
		arg.Completion.Description = b.msg("cli.option."+name+".value-completion", text)
	}
	return arg
}

func (b *builder) positional(key, name, valueName string, kind ValueKind, required bool, help string) Argument {
	arg := Argument{Name: name, ValueName: valueName, Kind: kind, Required: required,
		Help: b.msg("cli.command."+key+".argument."+name, help)}
	if text, ok := positionalCompletionText[key]; ok {
		arg.Completion.Description = b.msg("cli.command."+key+".value-completion", text)
	}
	return arg
}

func conflict(arg Argument, names ...string) Argument { arg.Conflicts = names; return arg }
func require(arg Argument, names ...string) Argument  { arg.Requires = names; return arg }
func defaultString(arg Argument, value string) Argument {
	arg.Default = &Default{String: value}
	return arg
}
func defaultInt(arg Argument, value int64) Argument   { arg.Default = &Default{Int: value}; return arg }
func choices(arg Argument, values ...string) Argument { arg.Allowed = values; return arg }
func repeat(arg Argument) Argument                    { arg.Repeatable = true; return arg }
func trailing(arg Argument) Argument                  { arg.Trailing = true; return arg }
func completion(arg Argument, kind CompletionKind, values ...string) Argument {
	arg.Completion.Kind = kind
	arg.Completion.Values = values
	if arg.Completion.Description == "" {
		arg.Completion.Description = arg.Help
	}
	return arg
}
func valueDescriptions(b *builder, key string, arg Argument, values ...string) Argument {
	arg.Completion.ValueDescriptions = make([]l10n.ID, 0, len(values))
	for index, value := range values {
		arg.Completion.ValueDescriptions = append(arg.Completion.ValueDescriptions, b.msg("cli.command."+key+".completion-value."+strconv.Itoa(index), value))
	}
	return arg
}
func mandatory(arg Argument) Argument { arg.Required = true; return arg }
func positive(arg Argument) Argument  { arg.Validate = ValidatePositive; return arg }

func attach(root *Command, parent *Command, localizer l10n.Localizer, english map[l10n.ID]string) {
	root.parent = parent
	root.localizer = localizer
	if parent == nil {
		root.english = english
	}
	for _, child := range root.Children {
		attach(child, root, localizer, nil)
	}
}
