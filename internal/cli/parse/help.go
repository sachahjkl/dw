package parse

import (
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/spec"
)

func Help(root *spec.Command, path []string, version string) (string, *Error) {
	command, ok := spec.Lookup(root, path)
	if !ok {
		return "", parseError(root, path, UnknownCommand, spec.MsgErrUnknownCmd, strings.Join(path, " "), "")
	}
	var out strings.Builder
	out.WriteString(command.Text(command.Summary))
	if command == root && version != "" {
		out.WriteByte(' ')
		out.WriteString(version)
	}
	out.WriteString("\n\n")
	out.WriteString(command.Text(spec.MsgUsage))
	out.WriteString(": ")
	out.WriteString(usage(root, command, path))
	out.WriteString("\n")

	if children := command.VisibleChildren(); len(children) != 0 {
		out.WriteString("\n")
		out.WriteString(command.Text(spec.MsgCommands))
		out.WriteString(":\n")
		rows := make([]helpRow, 0, len(children))
		for _, child := range children {
			rows = append(rows, helpRow{child.Name, command.Text(child.Summary)})
		}
		writeRows(&out, rows)
	}
	if positionals := command.Positionals(); len(positionals) != 0 {
		out.WriteString("\n")
		out.WriteString(command.Text(spec.MsgArguments))
		out.WriteString(":\n")
		rows := make([]helpRow, 0, len(positionals))
		for _, arg := range positionals {
			label := arg.ValueName
			if label == "" {
				label = strings.ToUpper(arg.Name)
			}
			rows = append(rows, helpRow{label, command.Text(arg.Help)})
		}
		writeRows(&out, rows)
	}
	options := command.Options(true)
	if len(options) != 0 {
		out.WriteString("\n")
		out.WriteString(command.Text(spec.MsgOptions))
		out.WriteString(":\n")
		rows := make([]helpRow, 0, len(options))
		for _, arg := range options {
			text := command.Text(arg.Help)
			if arg.Default != nil {
				value := arg.Default.String
				if arg.Kind == spec.Int {
					value = fmt.Sprint(arg.Default.Int)
				}
				text += " [" + command.Text(spec.MsgDefault) + ": " + value + "]"
			}
			rows = append(rows, helpRow{optionLabel(arg), text})
		}

		writeRows(&out, rows)
	}
	return strings.TrimRight(out.String(), "\n") + "\n", nil
}

// Diagnostic renders a parse failure without appending the full command help.
func Diagnostic(root *spec.Command, problem *Error) string {
	if problem == nil {
		return ""
	}
	command, path, message := problem.Command, problem.Path, "error: "+problem.Error()
	if problem.Kind == UnknownCommand {
		message = fmt.Sprintf(command.Text(spec.MsgErrSubcommand), problem.Token)
	}
	return message + "\n\n" + command.Text(spec.MsgUsage) + ": " + usage(root, command, path) + "\n\n" + command.Text(spec.MsgHelpHint) + "\n"
}

func Version(name, version string) string {
	if name == "" {
		name = "dw"
	}
	if version == "" {
		return name + "\n"
	}
	return name + " " + version + "\n"
}

type helpRow struct{ label, description string }

func writeRows(out *strings.Builder, rows []helpRow) {
	width := 0
	for _, row := range rows {
		if len(row.label) > width {
			width = len(row.label)
		}
	}
	for _, row := range rows {
		out.WriteString("  ")
		out.WriteString(row.label)
		out.WriteString(strings.Repeat(" ", width-len(row.label)+2))
		out.WriteString(row.description)
		out.WriteByte('\n')
	}
}

func usage(root, command *spec.Command, path []string) string {
	var out strings.Builder
	out.WriteString(root.Name)
	if len(path) != 0 {
		out.WriteByte(' ')
		out.WriteString(strings.Join(path, " "))
	}
	if len(command.Options(true)) != 0 {
		out.WriteString(" [OPTIONS]")
	}
	if len(command.Children) != 0 {
		out.WriteString(" <COMMAND>")
	}
	for _, arg := range command.Positionals() {
		label := arg.ValueName
		if label == "" {
			label = strings.ToUpper(arg.Name)
		}
		if arg.Trailing || arg.Repeatable {
			label += "..."
		}
		if arg.Required {
			out.WriteByte(' ')
			out.WriteString("<" + label + ">")
		} else {
			out.WriteString(" [")
			out.WriteString(label)
			out.WriteByte(']')
		}
	}
	return out.String()
}

func optionLabel(arg spec.Argument) string {
	var label string
	if arg.Short != 0 && arg.Long != "" {
		label = "-" + string(arg.Short) + ", --" + arg.Long
	} else if arg.Long != "" {
		label = "--" + arg.Long
	} else {
		label = "-" + string(arg.Short)
	}
	if arg.Kind == spec.Count {
		label += "..."
	} else if arg.Kind != spec.Bool {
		valueName := arg.ValueName
		if valueName == "" {
			valueName = strings.ToUpper(arg.Name)
		}
		label += " <" + valueName + ">"
	}
	return label
}
