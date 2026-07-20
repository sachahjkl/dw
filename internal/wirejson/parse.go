package wirejson

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

const maximumDepth = 1024

// Parse parses exactly one JSON value while retaining object member order,
// duplicate names, explicit nulls, and number spellings.
func Parse(data []byte) (Value, error) {
	return Decode(bytes.NewReader(data))
}

// Decode reads exactly one JSON value from reader.
func Decode(reader io.Reader) (Value, error) {
	decoder := json.NewDecoder(reader)
	decoder.UseNumber()
	value, err := decodeValue(decoder, 0)
	if err != nil {
		return Value{}, err
	}
	if token, err := decoder.Token(); err != io.EOF {
		if err != nil {
			return Value{}, fmt.Errorf("wirejson.trailing-data: %w", err)
		}
		return Value{}, fmt.Errorf("wirejson.trailing-token:%v", token)
	}
	return value, nil
}

func decodeValue(decoder *json.Decoder, depth int) (Value, error) {
	if depth > maximumDepth {
		return Value{}, fmt.Errorf("wirejson.maximum-depth")
	}
	token, err := decoder.Token()
	if err != nil {
		return Value{}, fmt.Errorf("wirejson.decode: %w", err)
	}
	switch token := token.(type) {
	case nil:
		return NullValue(), nil
	case bool:
		return BoolValue(token), nil
	case string:
		return StringValue(token), nil
	case json.Number:
		return NumberValue(token.String()), nil
	case json.Delim:
		switch token {
		case '[':
			values := make([]Value, 0)
			for decoder.More() {
				value, err := decodeValue(decoder, depth+1)
				if err != nil {
					return Value{}, err
				}
				values = append(values, value)
			}
			closing, err := decoder.Token()
			if err != nil || closing != json.Delim(']') {
				return Value{}, fmt.Errorf("wirejson.unclosed-array")
			}
			return Value{kind: Array, array: values}, nil
		case '{':
			members := make([]Member, 0)
			for decoder.More() {
				nameToken, err := decoder.Token()
				if err != nil {
					return Value{}, fmt.Errorf("wirejson.object-name: %w", err)
				}
				name, ok := nameToken.(string)
				if !ok {
					return Value{}, fmt.Errorf("wirejson.object-name-not-string")
				}
				value, err := decodeValue(decoder, depth+1)
				if err != nil {
					return Value{}, err
				}
				members = append(members, Member{Name: name, Value: value})
			}
			closing, err := decoder.Token()
			if err != nil || closing != json.Delim('}') {
				return Value{}, fmt.Errorf("wirejson.unclosed-object")
			}
			return Value{kind: Object, object: members}, nil
		default:
			return Value{}, fmt.Errorf("wirejson.unexpected-delimiter:%c", token)
		}
	default:
		return Value{}, fmt.Errorf("wirejson.unexpected-token")
	}
}

// UnmarshalJSON replaces v with an independently owned ordered tree.
func (v *Value) UnmarshalJSON(data []byte) error {
	parsed, err := Parse(data)
	if err != nil {
		return err
	}
	*v = parsed
	return nil
}
