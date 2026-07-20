package controller

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"strconv"
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
		value, err := input.selectOne(prompt, label)
		return action.Response{Kind: prompt.Kind, Value: value}, err
	case action.PromptSelectMany:
		values, err := input.selectMany(prompt, label)
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

func (input *TerminalInput) selectOne(prompt action.Prompt, label string) (action.ChoiceValue, error) {
	if err := input.writeChoices(prompt, label); err != nil {
		return "", err
	}
	value, err := input.readLine("> ")
	if err != nil {
		return "", err
	}
	index, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || index < 1 || index > len(prompt.Choices) {
		return "", fmt.Errorf("cli.invalid-selection:%s", prompt.ID)
	}
	return prompt.Choices[index-1].Value, nil
}

func (input *TerminalInput) selectMany(prompt action.Prompt, label string) ([]action.ChoiceValue, error) {
	if err := input.writeChoices(prompt, label); err != nil {
		return nil, err
	}
	value, err := input.readLine("> ")
	if err != nil {
		return nil, err
	}
	value = strings.TrimSpace(value)
	if value == "" {
		return []action.ChoiceValue{}, nil
	}
	parts := strings.Split(value, ",")
	values := make([]action.ChoiceValue, 0, len(parts))
	seen := make(map[int]struct{}, len(parts))
	for _, part := range parts {
		index, conversionErr := strconv.Atoi(strings.TrimSpace(part))
		if conversionErr != nil || index < 1 || index > len(prompt.Choices) {
			return nil, fmt.Errorf("cli.invalid-selection:%s", prompt.ID)
		}
		if _, exists := seen[index]; exists {
			continue
		}
		seen[index] = struct{}{}
		values = append(values, prompt.Choices[index-1].Value)
	}
	return values, nil
}

func (input *TerminalInput) writeChoices(prompt action.Prompt, label string) error {
	if _, err := fmt.Fprintln(input.streams.Stderr, label); err != nil {
		return err
	}
	if prompt.Help != nil {
		if _, err := fmt.Fprintln(input.streams.Stderr, input.localizer.Render(*prompt.Help)); err != nil {
			return err
		}
	}
	for index, choice := range prompt.Choices {
		if _, err := fmt.Fprintf(input.streams.Stderr, "  %d) %s\n", index+1, input.localizer.Render(choice.Label)); err != nil {
			return err
		}
	}
	return nil
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
