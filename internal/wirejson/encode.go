package wirejson

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"strings"
)

// Compact encodes value without insignificant whitespace. Object order and
// number lexemes are preserved and HTML characters are not escaped.
func Compact(value Value) ([]byte, error) {
	var buffer bytes.Buffer
	if err := writeValue(&buffer, value, "", 0); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

// Pretty encodes value with two-space indentation and no trailing newline.
func Pretty(value Value) ([]byte, error) { return PrettyIndent(value, "  ") }

// PrettyIndent encodes value with the supplied indentation unit.
func PrettyIndent(value Value, indent string) ([]byte, error) {
	if strings.ContainsAny(indent, "\r\n") {
		return nil, fmt.Errorf("wirejson.invalid-indent")
	}
	var buffer bytes.Buffer
	if err := writeValue(&buffer, value, indent, 0); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

// EncodePretty writes pretty JSON and a final newline for stream-oriented CLI
// output. Pretty itself remains newline-free for config round trips.
func EncodePretty(writer io.Writer, value Value) error {
	data, err := Pretty(value)
	if err != nil {
		return err
	}
	if _, err := writer.Write(data); err != nil {
		return err
	}
	_, err = io.WriteString(writer, "\n")
	return err
}

// MarshalJSON implements json.Marshaler using compact deterministic output.
func (v Value) MarshalJSON() ([]byte, error) { return Compact(v) }

func writeValue(writer *bytes.Buffer, value Value, indent string, depth int) error {
	switch value.kind {
	case Null:
		writer.WriteString("null")
	case Bool:
		if value.boolean {
			writer.WriteString("true")
		} else {
			writer.WriteString("false")
		}
	case Number:
		if !validNumber(value.text) {
			return fmt.Errorf("wirejson.invalid-number:%s", value.text)
		}
		writer.WriteString(value.text)
	case String:
		writer.WriteString(strconv.Quote(value.text))
	case Array:
		if len(value.array) == 0 {
			writer.WriteString("[]")
			return nil
		}
		writer.WriteByte('[')
		for i := range value.array {
			if i > 0 {
				writer.WriteByte(',')
			}
			writeBreak(writer, indent, depth+1)
			if err := writeValue(writer, value.array[i], indent, depth+1); err != nil {
				return err
			}
		}
		writeBreak(writer, indent, depth)
		writer.WriteByte(']')
	case Object:
		if len(value.object) == 0 {
			writer.WriteString("{}")
			return nil
		}
		writer.WriteByte('{')
		for i := range value.object {
			if i > 0 {
				writer.WriteByte(',')
			}
			writeBreak(writer, indent, depth+1)
			writer.WriteString(strconv.Quote(value.object[i].Name))
			writer.WriteByte(':')
			if indent != "" {
				writer.WriteByte(' ')
			}
			if err := writeValue(writer, value.object[i].Value, indent, depth+1); err != nil {
				return err
			}
		}
		writeBreak(writer, indent, depth)
		writer.WriteByte('}')
	case Invalid:
		return fmt.Errorf("wirejson.invalid-value")
	default:
		return fmt.Errorf("wirejson.unknown-kind:%d", value.kind)
	}
	return nil
}

func writeBreak(writer *bytes.Buffer, indent string, depth int) {
	if indent == "" {
		return
	}
	writer.WriteByte('\n')
	for range depth {
		writer.WriteString(indent)
	}
}

func validNumber(lexeme string) bool {
	if lexeme == "" {
		return false
	}
	first := lexeme[0]
	return (first == '-' || first >= '0' && first <= '9') && json.Valid([]byte(lexeme))
}
