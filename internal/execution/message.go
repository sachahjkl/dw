package execution

import (
	"fmt"
	"reflect"
	"strconv"

	"github.com/sachahjkl/dw/internal/l10n"
)

const MessageSchemaV1 SchemaVersion = 1

type MessageArgKind string

const (
	MessageArgString  MessageArgKind = "string"
	MessageArgInteger MessageArgKind = "integer"
	MessageArgBoolean MessageArgKind = "boolean"
	MessageArgDecimal MessageArgKind = "decimal"
)

type MessageArgV1 struct {
	Name  string         `json:"name"`
	Kind  MessageArgKind `json:"kind"`
	Value string         `json:"value"`
}

type MessageV1 struct {
	Schema SchemaVersion  `json:"schema"`
	ID     l10n.ID        `json:"id"`
	Args   []MessageArgV1 `json:"args"`
}

func EncodeMessage(message l10n.Message) (MessageV1, error) {
	if message.ID == "" {
		return MessageV1{}, fmt.Errorf("execution.invalid-message-id")
	}
	encoded := MessageV1{Schema: MessageSchemaV1, ID: message.ID, Args: make([]MessageArgV1, len(message.Args))}
	for index, arg := range message.Args {
		value, err := encodeMessageArg(arg)
		if err != nil {
			return MessageV1{}, err
		}
		encoded.Args[index] = value
	}
	return encoded, nil
}

func DecodeMessage(message MessageV1) (l10n.Message, error) {
	if message.Schema != MessageSchemaV1 || message.ID == "" {
		return l10n.Message{}, fmt.Errorf("execution.invalid-message")
	}
	args := make([]l10n.Arg, len(message.Args))
	for index, arg := range message.Args {
		value, err := decodeMessageArg(arg)
		if err != nil {
			return l10n.Message{}, err
		}
		args[index] = value
	}
	return l10n.M(message.ID, args...), nil
}

type codedError interface {
	ErrorCode() ErrorCode
}

type localizedError interface {
	Localized() l10n.Message
}

type FailureError struct {
	failure   Failure
	localized l10n.Message
}

func NewFailureError(failure Failure) error {
	message, err := DecodeMessage(failure.Message)
	if err != nil {
		message = l10n.M(l10n.ID(failure.Code))
	}
	return &FailureError{failure: failure, localized: message}
}

func (err *FailureError) Error() string {
	return string(err.failure.Code)
}

func (err *FailureError) ErrorCode() ErrorCode {
	return err.failure.Code
}

func (err *FailureError) Localized() l10n.Message {
	return err.localized
}

func FailureFromError(err error) Failure {
	if err == nil {
		return Failure{}
	}
	code := ErrorCode("execution.unclassified-error")
	message := MessageV1{Schema: MessageSchemaV1, ID: "execution.unclassified-error", Args: []MessageArgV1{}}
	if coded, ok := err.(codedError); ok && coded.ErrorCode() != "" {
		code = coded.ErrorCode()
	}
	if localized, ok := err.(localizedError); ok {
		encoded, encodeErr := EncodeMessage(localized.Localized())
		if encodeErr == nil {
			message = encoded
		} else {
			message = MessageV1{Schema: MessageSchemaV1, ID: l10n.ID(code), Args: []MessageArgV1{}}
		}
	}
	return Failure{Code: code, Message: message}
}

func encodeMessageArg(arg l10n.Arg) (MessageArgV1, error) {
	if arg.Name == "" || arg.Value == nil {
		return MessageArgV1{}, fmt.Errorf("execution.invalid-message-argument:%s", arg.Name)
	}
	value := reflect.ValueOf(arg.Value)
	switch value.Kind() {
	case reflect.String:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgString, Value: value.String()}, nil
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgInteger, Value: strconv.FormatInt(value.Int(), 10)}, nil
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgInteger, Value: strconv.FormatUint(value.Uint(), 10)}, nil
	case reflect.Bool:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgBoolean, Value: strconv.FormatBool(value.Bool())}, nil
	case reflect.Float32:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgDecimal, Value: strconv.FormatFloat(value.Float(), 'f', -1, 32)}, nil
	case reflect.Float64:
		return MessageArgV1{Name: arg.Name, Kind: MessageArgDecimal, Value: strconv.FormatFloat(value.Float(), 'f', -1, 64)}, nil
	default:
		return MessageArgV1{}, fmt.Errorf("execution.unsupported-message-argument:%s:%T", arg.Name, arg.Value)
	}
}

func decodeMessageArg(arg MessageArgV1) (l10n.Arg, error) {
	if arg.Name == "" {
		return l10n.Arg{}, fmt.Errorf("execution.invalid-message-argument")
	}
	switch arg.Kind {
	case MessageArgString:
		return l10n.A(arg.Name, arg.Value), nil
	case MessageArgInteger:
		if value, err := strconv.ParseInt(arg.Value, 10, 64); err == nil {
			return l10n.A(arg.Name, value), nil
		}
		if value, err := strconv.ParseUint(arg.Value, 10, 64); err == nil {
			return l10n.A(arg.Name, value), nil
		}
		return l10n.Arg{}, fmt.Errorf("execution.invalid-message-integer:%s", arg.Name)
	case MessageArgBoolean:
		if arg.Value == "true" {
			return l10n.A(arg.Name, true), nil
		}
		if arg.Value == "false" {
			return l10n.A(arg.Name, false), nil
		}
		return l10n.Arg{}, fmt.Errorf("execution.invalid-message-boolean:%s", arg.Name)
	case MessageArgDecimal:
		value, err := strconv.ParseFloat(arg.Value, 64)
		if err != nil {
			return l10n.Arg{}, fmt.Errorf("execution.invalid-message-decimal:%s", arg.Name)
		}
		return l10n.A(arg.Name, value), nil
	default:
		return l10n.Arg{}, fmt.Errorf("execution.invalid-message-argument-kind:%s", arg.Kind)
	}
}
