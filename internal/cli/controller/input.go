package controller

import (
	"bufio"
	tea "charm.land/bubbletea/v2"
	"context"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
	"golang.org/x/term"
)

// TerminalInput is the CLI action input adapter. Prompts are written to stderr
// so stdout remains a stable data stream.
type TerminalInput struct {
	streams   console.Streams
	localizer l10n.Localizer
	reader    *bufio.Reader
}

func NewTerminalInput(streams console.Streams, localizer l10n.Localizer) *TerminalInput {
	return &TerminalInput{streams: streams, localizer: localizer, reader: bufio.NewReader(streams.Stdin)}
}

func (input *TerminalInput) Request(ctx context.Context, prompt action.Prompt) (action.Response, error) {
	if err := ctx.Err(); err != nil {
		return action.Response{}, err
	}
	if !input.streams.StdinTTY {
		return action.Response{}, fmt.Errorf("cli.input-requires-terminal:%s", prompt.ID)
	}
	if err := prompt.Validate(); err != nil {
		return action.Response{}, err
	}
	label := input.localizer.Render(prompt.Label)
	switch prompt.Kind {
	case action.PromptConfirm:
		accepted, err := input.confirm(label, prompt.Default)
		return action.Response{Kind: prompt.Kind, Accepted: accepted}, err
	case action.PromptText:
		value, err := input.text(label, prompt.Default)
		return action.Response{Kind: prompt.Kind, Text: value}, err
	case action.PromptSecret:
		value, err := input.secret(label)
		return action.Response{Kind: prompt.Kind, Secret: contract.NewSecretValue(value)}, err
	case action.PromptSelectOne:
		value, err := input.selectOne(ctx, prompt, label)
		return action.Response{Kind: prompt.Kind, Value: value}, err
	case action.PromptSelectMany:
		values, err := input.selectMany(ctx, prompt, label)
		return action.Response{Kind: prompt.Kind, Values: values}, err
	default:
		return action.Response{}, fmt.Errorf("cli.unknown-prompt-kind:%s", prompt.Kind)
	}
}

func (input *TerminalInput) confirm(label string, defaultValue *action.ChoiceValue) (bool, error) {
	defaultAccepted := defaultValue != nil && strings.EqualFold(string(*defaultValue), "true")
	suffix := " [y/N]: "
	if defaultAccepted {
		suffix = " [Y/n]: "
	}
	value, err := input.readLine(label + suffix)
	if err != nil {
		return false, err
	}
	value = strings.ToLower(strings.TrimSpace(value))
	if value == "" {
		return defaultAccepted, nil
	}
	switch value {
	case "y", "yes":
		return true, nil
	case "n", "no":
		return false, nil
	default:
		return false, fmt.Errorf("cli.invalid-confirmation")
	}
}

func (input *TerminalInput) text(label string, defaultValue *action.ChoiceValue) (string, error) {
	suffix := ": "
	if defaultValue != nil {
		suffix = " [" + string(*defaultValue) + "]: "
	}
	value, err := input.readLine(label + suffix)
	if err != nil {
		return "", err
	}
	value = strings.TrimSpace(value)
	if value == "" && defaultValue != nil {
		value = string(*defaultValue)
	}
	return value, nil
}

func (input *TerminalInput) secret(label string) (string, error) {
	if _, err := io.WriteString(input.streams.Stderr, label+": "); err != nil {
		return "", err
	}
	file, ok := input.streams.Stdin.(*os.File)
	if !ok {
		return "", fmt.Errorf("cli.secret-input-requires-terminal-file")
	}
	value, err := term.ReadPassword(int(file.Fd()))
	if _, newlineErr := io.WriteString(input.streams.Stderr, "\n"); err == nil {
		err = newlineErr
	}
	if err != nil {
		return "", err
	}
	return string(value), nil
}

func (input *TerminalInput) selectOne(ctx context.Context, prompt action.Prompt, label string) (action.ChoiceValue, error) {
	model, err := input.runSelection(ctx, prompt, label, false)
	if err != nil {
		return "", err
	}
	return prompt.Choices[model.cursor].Value, nil
}

func (input *TerminalInput) selectMany(ctx context.Context, prompt action.Prompt, label string) ([]action.ChoiceValue, error) {
	model, err := input.runSelection(ctx, prompt, label, true)
	if err != nil {
		return nil, err
	}
	values := make([]action.ChoiceValue, 0, len(model.selected))
	for index, choice := range prompt.Choices {
		if model.selected[index] {
			values = append(values, choice.Value)
		}
	}
	return values, nil
}

func (input *TerminalInput) runSelection(ctx context.Context, prompt action.Prompt, label string, multi bool) (*selectionModel, error) {
	choices := make([]terminalChoice, len(prompt.Choices))
	model := &selectionModel{
		label:    label,
		multi:    multi,
		selected: make(map[int]bool),
		choices:  choices,
	}
	if prompt.Help != nil {
		model.help = input.localizer.Render(*prompt.Help)
	}
	for index, choice := range prompt.Choices {
		model.choices[index] = terminalChoice{label: input.localizer.Render(choice.Label)}
		if choice.Description != nil {
			model.choices[index].description = input.localizer.Render(*choice.Description)
		}
		if prompt.Default != nil && choice.Value == *prompt.Default {
			model.cursor = index
			if multi {
				model.selected[index] = true
			}
		}
	}
	_, err := tea.NewProgram(
		model,
		tea.WithContext(ctx),
		tea.WithInput(input.streams.Stdin),
		tea.WithOutput(input.streams.Stderr),
	).Run()
	if err != nil {
		return nil, err
	}
	if model.canceled {
		return nil, fmt.Errorf("selection canceled")
	}
	return model, nil
}

type terminalChoice struct {
	label       string
	description string
}

type selectionModel struct {
	label    string
	help     string
	choices  []terminalChoice
	cursor   int
	multi    bool
	selected map[int]bool
	canceled bool
	done     bool
}

func (*selectionModel) Init() tea.Cmd { return nil }

func (model *selectionModel) Update(message tea.Msg) (tea.Model, tea.Cmd) {
	key, ok := message.(tea.KeyPressMsg)
	if !ok {
		return model, nil
	}
	return model, model.handleKey(key.String())
}

func (model *selectionModel) handleKey(key string) tea.Cmd {
	switch key {
	case "ctrl+c", "esc":
		model.canceled = true
		model.done = true
		return tea.Quit
	case "up", "k":
		model.cursor = (model.cursor - 1 + len(model.choices)) % len(model.choices)
	case "down", "j":
		model.cursor = (model.cursor + 1) % len(model.choices)
	case "home", "g":
		model.cursor = 0
	case "end", "G":
		model.cursor = len(model.choices) - 1
	case " ", "space":
		if model.multi {
			model.selected[model.cursor] = !model.selected[model.cursor]
		}
	case "enter":
		model.done = true
		return tea.Quit
	}
	return nil
}

func (model *selectionModel) View() tea.View {
	if model.done {
		if model.canceled {
			return tea.NewView("")
		}
		values := make([]string, 0, len(model.selected))
		if model.multi {
			for index, choice := range model.choices {
				if model.selected[index] {
					values = append(values, choice.label)
				}
			}
		} else {
			values = append(values, model.choices[model.cursor].label)
		}
		return tea.NewView(model.label + ": " + strings.Join(values, ", ") + "\n")
	}
	var out strings.Builder
	out.WriteString(model.label)
	out.WriteByte('\n')
	if model.help != "" {
		out.WriteString(model.help)
		out.WriteByte('\n')
	}
	for index, choice := range model.choices {
		cursor := "  "
		if index == model.cursor {
			cursor = "› "
		}
		out.WriteString(cursor)
		if model.multi {
			marker := "[ ] "
			if model.selected[index] {
				marker = "[x] "
			}
			out.WriteString(marker)
		}
		out.WriteString(choice.label)
		out.WriteByte('\n')
		if choice.description != "" {
			out.WriteString("    ")
			out.WriteString(choice.description)
			out.WriteByte('\n')
		}
	}
	out.WriteString("\n↑/↓ move")
	if model.multi {
		out.WriteString(" • space toggle • enter confirm")
	} else {
		out.WriteString(" • enter select")
	}
	out.WriteString(" • esc cancel\n")
	return tea.NewView(out.String())
}

func (input *TerminalInput) readLine(prompt string) (string, error) {
	if _, err := io.WriteString(input.streams.Stderr, prompt); err != nil {
		return "", err
	}
	value, err := input.reader.ReadString('\n')
	if err != nil && err != io.EOF {
		return "", err
	}
	if err == io.EOF && value == "" {
		return "", err
	}
	return strings.TrimRight(value, "\r\n"), nil
}
