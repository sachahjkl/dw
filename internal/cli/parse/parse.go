package parse

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/l10n"
)

type ExitClass uint8

const (
	ExitOK      ExitClass = 0
	ExitFailure ExitClass = 1
	ExitUsage   ExitClass = 2
)

type ErrorKind string

const (
	UnknownCommand     ErrorKind = "unknown-command"
	MissingCommand     ErrorKind = "missing-command"
	UnknownOption      ErrorKind = "unknown-option"
	MissingValue       ErrorKind = "missing-value"
	InvalidValue       ErrorKind = "invalid-value"
	MissingArgument    ErrorKind = "missing-argument"
	UnexpectedArgument ErrorKind = "unexpected-argument"
	DuplicateOption    ErrorKind = "duplicate-option"
	Conflict           ErrorKind = "conflict"
	Requires           ErrorKind = "requires"
)

type Error struct {
	Kind      ErrorKind
	MessageID l10n.ID
	Token     string
	Other     string
	Command   *spec.Command
	Path      []string
	Exit      ExitClass
}

func (e *Error) Error() string {
	if e == nil {
		return ""
	}
	template := e.Command.Text(e.MessageID)
	switch e.MessageID {
	case spec.MsgErrMissingCmd:
		return template
	case spec.MsgErrConflict, spec.MsgErrRequires:
		return fmt.Sprintf(template, e.Token, e.Other)
	case spec.MsgErrInvalidValue:
		return fmt.Sprintf(template, e.Token, e.Other)
	default:
		return fmt.Sprintf(template, e.Token)
	}
}

func (e *Error) ExitClass() ExitClass {
	if e == nil {
		return ExitOK
	}
	return e.Exit
}

type Intent uint8

const (
	IntentRun Intent = iota
	IntentHelp
	IntentVersion
)

type value struct {
	name    string
	kind    spec.ValueKind
	present bool
	boolean bool
	text    string
	texts   []string
	integer int64
}

type Values struct{ entries []value }

func (v Values) lookup(name string) (value, bool) {
	for _, entry := range v.entries {
		if entry.name == name {
			return entry, true
		}
	}
	return value{}, false
}
func (v Values) Has(name string) bool      { entry, ok := v.lookup(name); return ok && entry.present }
func (v Values) Bool(name string) bool     { entry, _ := v.lookup(name); return entry.boolean }
func (v Values) String(name string) string { entry, _ := v.lookup(name); return entry.text }
func (v Values) Strings(name string) []string {
	entry, _ := v.lookup(name)
	return append([]string(nil), entry.texts...)
}
func (v Values) Int(name string) int64 { entry, _ := v.lookup(name); return entry.integer }

type Result struct {
	Command   *spec.Command
	Path      []string
	Values    Values
	Verbosity uint8
	Intent    Intent
}

func Parse(root *spec.Command, args []string) (*Result, *Error) {
	if root == nil {
		panic("parse: nil command grammar")
	}
	current := root
	path := make([]string, 0, 5)
	verbosity := uint8(0)
	index := 0
	for len(current.Children) != 0 {
		if index >= len(args) {
			return nil, parseError(current, path, MissingCommand, spec.MsgErrMissingCmd, "", "")
		}
		token := args[index]
		if intent, consumed, nextVerbosity, ok, err := global(root, current, token, verbosity); ok {
			if err != nil {
				err.Path = append([]string(nil), path...)
				return nil, err
			}
			verbosity = nextVerbosity
			index += consumed
			if intent != IntentRun {
				return &Result{Command: current, Path: path, Verbosity: verbosity, Intent: intent}, nil
			}
			continue
		}
		if strings.HasPrefix(token, "-") {
			return nil, parseError(current, path, UnknownOption, spec.MsgErrUnknownOpt, token, "")
		}
		child, ok := current.Child(token)
		if !ok {
			return nil, parseError(current, path, UnknownCommand, spec.MsgErrUnknownCmd, token, "")
		}
		current = child
		path = append(path, child.Name)
		index++
	}
	result := &Result{Command: current, Path: path, Verbosity: verbosity, Intent: IntentRun}
	values, intent, verbosity, err := parseLeaf(root, current, path, args[index:], verbosity)
	if err != nil {
		return nil, err
	}
	result.Values, result.Intent, result.Verbosity = values, intent, verbosity
	return result, nil
}

func parseLeaf(root, command *spec.Command, path []string, args []string, verbosity uint8) (Values, Intent, uint8, *Error) {
	values := initialValues(command)
	positionals := command.Positionals()
	position := 0
	endOptions := false
	for index := 0; index < len(args); {
		token := args[index]
		if !endOptions {
			if intent, consumed, nextVerbosity, ok, err := global(root, command, token, verbosity); ok {
				if err != nil {
					err.Path = append([]string(nil), path...)
					return Values{}, IntentRun, verbosity, err
				}
				verbosity = nextVerbosity
				index += consumed
				if intent != IntentRun {
					return values, intent, verbosity, nil
				}
				continue
			}
			if token == "--" {
				endOptions = true
				index++
				continue
			}
			if strings.HasPrefix(token, "--") {
				name, inline, hasInline := splitLong(token)
				arg, ok := command.ArgumentByLong(name)
				if !ok {
					if position < len(positionals) && positionals[position].Trailing {
						endOptions = true
					} else {
						return Values{}, IntentRun, verbosity, parseError(command, path, UnknownOption, spec.MsgErrUnknownOpt, token, "")
					}
				} else {
					if arg.Kind == spec.Bool {
						if hasInline {
							return Values{}, IntentRun, verbosity, parseError(command, path, InvalidValue, spec.MsgErrInvalidValue, inline, arg.Token())
						}
						if err := setValue(command, path, &values, *arg, "true"); err != nil {
							return Values{}, IntentRun, verbosity, err
						}
						index++
						continue
					}
					var raw string
					if hasInline {
						raw = inline
						index++
					} else {
						if index+1 >= len(args) || args[index+1] == "--" || (strings.HasPrefix(args[index+1], "-") && arg.Kind != spec.Int) {
							return Values{}, IntentRun, verbosity, parseError(command, path, MissingValue, spec.MsgErrMissingValue, arg.Token(), "")
						}
						raw = args[index+1]
						index += 2
					}
					if err := setValue(command, path, &values, *arg, raw); err != nil {
						return Values{}, IntentRun, verbosity, err
					}
					continue
				}
			}
			if strings.HasPrefix(token, "-") {
				if position < len(positionals) && positionals[position].Trailing {
					endOptions = true
				} else {
					return Values{}, IntentRun, verbosity, parseError(command, path, UnknownOption, spec.MsgErrUnknownOpt, token, "")
				}
			}
		}
		if position >= len(positionals) {
			return Values{}, IntentRun, verbosity, parseError(command, path, UnexpectedArgument, spec.MsgErrUnexpectedArg, token, "")
		}
		arg := positionals[position]
		if arg.Trailing {
			for ; index < len(args); index++ {
				if err := setValue(command, path, &values, arg, args[index]); err != nil {
					return Values{}, IntentRun, verbosity, err
				}
			}
			position++
			break
		}
		if err := setValue(command, path, &values, arg, token); err != nil {
			return Values{}, IntentRun, verbosity, err
		}
		index++
		if !arg.Repeatable {
			position++
		}
	}
	for _, arg := range command.Arguments {
		entry, _ := values.lookup(arg.Name)
		if arg.Required && !entry.present {
			return Values{}, IntentRun, verbosity, parseError(command, path, MissingArgument, spec.MsgErrMissingArg, arg.Token(), "")
		}
		if !entry.present {
			continue
		}
		for _, name := range arg.Conflicts {
			if values.Has(name) {
				return Values{}, IntentRun, verbosity, parseError(command, path, Conflict, spec.MsgErrConflict, arg.Token(), argumentToken(command, name))
			}
		}
		for _, name := range arg.Requires {
			if !values.Has(name) {
				return Values{}, IntentRun, verbosity, parseError(command, path, Requires, spec.MsgErrRequires, arg.Token(), argumentToken(command, name))
			}
		}
	}
	return values, IntentRun, verbosity, nil
}

func initialValues(command *spec.Command) Values {
	values := Values{entries: make([]value, 0, len(command.Arguments))}
	for _, arg := range command.Arguments {
		entry := value{name: arg.Name, kind: arg.Kind}
		if arg.Default != nil {
			entry.text, entry.integer = arg.Default.String, arg.Default.Int
		}
		values.entries = append(values.entries, entry)
	}
	return values
}

func setValue(command *spec.Command, path []string, values *Values, arg spec.Argument, raw string) *Error {
	for index := range values.entries {
		entry := &values.entries[index]
		if entry.name != arg.Name {
			continue
		}
		if entry.present && !arg.Repeatable {
			return parseError(command, path, DuplicateOption, spec.MsgErrDuplicate, arg.Token(), "")
		}
		for _, allowed := range arg.Allowed {
			if raw == allowed {
				return assign(command, path, entry, arg, raw)
			}
		}
		if len(arg.Allowed) != 0 {
			return parseError(command, path, InvalidValue, spec.MsgErrInvalidValue, raw, arg.Token())
		}
		return assign(command, path, entry, arg, raw)
	}
	return parseError(command, path, InvalidValue, spec.MsgErrInvalidValue, raw, arg.Token())
}

func assign(command *spec.Command, path []string, entry *value, arg spec.Argument, raw string) *Error {
	entry.present = true
	switch arg.Kind {
	case spec.Bool:
		entry.boolean = true
	case spec.Int:
		bits := 32
		if arg.Validate == spec.ValidatePositive {
			bits = 64
		}
		parsed, err := strconv.ParseInt(raw, 10, bits)
		if err != nil {
			return parseError(command, path, InvalidValue, spec.MsgErrInvalidValue, raw, arg.Token())
		}
		if arg.Validate == spec.ValidatePositive && parsed <= 0 {
			return parseError(command, path, InvalidValue, spec.MsgErrPositiveInt, arg.Token(), "")
		}
		entry.integer = parsed
	case spec.Strings:
		entry.texts = append(entry.texts, raw)
	case spec.String:
		if arg.Repeatable {
			entry.texts = append(entry.texts, raw)
		} else {
			entry.text = raw
		}
	}
	return nil
}

func global(root, command *spec.Command, token string, verbosity uint8) (Intent, int, uint8, bool, *Error) {
	var matched *spec.Argument
	if strings.HasPrefix(token, "--") {
		name := strings.TrimPrefix(token, "--")
		if arg, ok := root.ArgumentByLong(name); ok && arg.Global {
			matched = arg
		}
	} else if len(token) == 2 && token[0] == '-' {
		if arg, ok := root.ArgumentByShort(rune(token[1])); ok && arg.Global {
			matched = arg
		}
	}
	if matched != nil {
		switch matched.Special {
		case spec.SpecialHelp:
			return IntentHelp, 1, verbosity, true, nil
		case spec.SpecialVersion:
			return IntentVersion, 1, verbosity, true, nil
		}
		if matched.Kind == spec.Count {
			if verbosity == 255 {
				return IntentRun, 1, verbosity, true, parseError(command, nil, InvalidValue, spec.MsgErrInvalidValue, token, matched.Name)
			}
			return IntentRun, 1, verbosity + 1, true, nil
		}
	}
	if len(token) > 2 && token[0] == '-' && token[1] != '-' {
		next := verbosity
		intent := IntentRun
		for _, short := range token[1:] {
			arg, ok := root.ArgumentByShort(short)
			if !ok || !arg.Global {
				return IntentRun, 0, verbosity, false, nil
			}
			if arg.Kind == spec.Count {
				if next == 255 {
					return IntentRun, 1, verbosity, true, parseError(command, nil, InvalidValue, spec.MsgErrInvalidValue, token, arg.Name)
				}
				next++
			}
			if arg.Special == spec.SpecialHelp {
				intent = IntentHelp
			}
			if arg.Special == spec.SpecialVersion {
				intent = IntentVersion
			}
		}
		return intent, 1, next, true, nil
	}
	return IntentRun, 0, verbosity, false, nil
}

func splitLong(token string) (string, string, bool) {
	value := strings.TrimPrefix(token, "--")
	if index := strings.IndexByte(value, '='); index >= 0 {
		return value[:index], value[index+1:], true
	}
	return value, "", false
}

func parseError(command *spec.Command, path []string, kind ErrorKind, id l10n.ID, token, other string) *Error {
	return &Error{Kind: kind, MessageID: id, Token: token, Other: other, Command: command, Path: append([]string(nil), path...), Exit: ExitUsage}
}

func argumentToken(command *spec.Command, name string) string {
	for _, arg := range command.Arguments {
		if arg.Name == name {
			return arg.Token()
		}
	}
	return name
}
