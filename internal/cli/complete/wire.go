package complete

import (
	"fmt"
	"io"
	"strings"

	"github.com/sachahjkl/dw/internal/wirejson"
)

type Format string

const (
	FormatBash       Format = "bash"
	FormatFish       Format = "fish"
	FormatJSON       Format = "json"
	FormatZsh        Format = "zsh"
	FormatPowerShell Format = "powershell"
	FormatElvish     Format = "elvish"
)

func ParseFormat(value string) (Format, error) {
	format := Format(strings.ToLower(value))
	switch format {
	case FormatBash, FormatFish, FormatJSON, FormatZsh, FormatPowerShell, FormatElvish:
		return format, nil
	default:
		return "", fmt.Errorf("cli.completion.invalid-format:%s", value)
	}
}

func Render(format Format, items []Item) ([]byte, error) {
	if format == FormatJSON || format == FormatPowerShell {
		values := make([]wirejson.Value, 0, len(items))
		for _, item := range items {
			values = append(values, wirejson.ObjectValue(
				wirejson.Member{Name: "label", Value: wirejson.StringValue(item.Label)},
				wirejson.Member{Name: "description", Value: wirejson.StringValue(item.Description)},
			))
		}
		encoded, err := wirejson.Compact(wirejson.ArrayValue(values...))
		if err != nil {
			return nil, err
		}
		return append(encoded, '\n'), nil
	}
	var out strings.Builder
	for _, item := range items {
		out.WriteString(item.Label)
		if (format == FormatFish || format == FormatZsh || format == FormatElvish) && strings.TrimSpace(item.Description) != "" {
			out.WriteByte('\t')
			out.WriteString(item.Description)
		}
		out.WriteByte('\n')
	}
	return []byte(out.String()), nil
}

func Write(writer io.Writer, format Format, items []Item) error {
	encoded, err := Render(format, items)
	if err != nil {
		return err
	}
	_, err = writer.Write(encoded)
	return err
}

// WordsFromBash extracts command words from COMP_LINE using the Rust-compatible whitespace contract.
func WordsFromBash(line string) []string {
	fields := strings.Fields(line)
	for index, field := range fields {
		if field == "dw" {
			return append([]string(nil), fields[index+1:]...)
		}
	}
	return nil
}
