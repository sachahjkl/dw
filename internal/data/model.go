// Package data defines provider-neutral, read-oriented data-source contracts.
// Concrete SQL Server implementations belong in a child package.
package data

import (
	"encoding/base64"
	"encoding/json"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
)

type ProviderName string
type SourceKey string

type Source struct {
	Key         SourceKey
	Provider    ProviderName
	Project     contract.Optional[contract.ProjectKey]
	DisplayName string
	Options     wirejson.Value
}

// Connection contains references and opaque provider options. Secret material
// uses SecretValue and therefore cannot be serialized accidentally.
type Connection struct {
	Source        Source
	CredentialKey contract.Optional[contract.SecretKey]
	Secret        contract.Optional[contract.SecretValue]
}

type CatalogEntryKind string

const (
	CatalogDatabase CatalogEntryKind = "database"
	CatalogSchema   CatalogEntryKind = "schema"
	CatalogTable    CatalogEntryKind = "table"
	CatalogView     CatalogEntryKind = "view"
	CatalogWorkbook CatalogEntryKind = "workbook"
	CatalogDocument CatalogEntryKind = "document"
)

type CatalogEntry struct {
	Kind        CatalogEntryKind
	Catalog     string
	Schema      string
	Name        string
	Description string
}

type ObjectRef struct {
	Catalog string
	Schema  string
	Name    string
}

type Column struct {
	Name       string
	NativeType string
	Nullable   bool
	Ordinal    int
}

type Description struct {
	Object  ObjectRef
	Columns []Column
}

type ValueKind string

const (
	ValueNull    ValueKind = "null"
	ValueString  ValueKind = "string"
	ValueBoolean ValueKind = "boolean"
	ValueInteger ValueKind = "integer"
	ValueDecimal ValueKind = "decimal"
	ValueTime    ValueKind = "time"
	ValueBinary  ValueKind = "binary"
)

// Value retains database precision by storing integer and decimal lexemes.
// Binary values are copied on construction and access.
type Value struct {
	kind    ValueKind
	text    string
	boolean bool
	binary  []byte
}

func NullValue() Value                 { return Value{kind: ValueNull} }
func StringValue(value string) Value   { return Value{kind: ValueString, text: value} }
func BooleanValue(value bool) Value    { return Value{kind: ValueBoolean, boolean: value} }
func IntegerValue(lexeme string) Value { return Value{kind: ValueInteger, text: lexeme} }
func DecimalValue(lexeme string) Value { return Value{kind: ValueDecimal, text: lexeme} }
func TimeValue(value string) Value     { return Value{kind: ValueTime, text: value} }
func BinaryValue(value []byte) Value {
	return Value{kind: ValueBinary, binary: append([]byte(nil), value...)}
}

func (v Value) Kind() ValueKind { return v.kind }
func (v Value) Text() (string, bool) {
	if v.kind == ValueString || v.kind == ValueInteger || v.kind == ValueDecimal || v.kind == ValueTime {
		return v.text, true
	}
	return "", false
}
func (v Value) Boolean() (bool, bool) { return v.boolean, v.kind == ValueBoolean }
func (v Value) Binary() ([]byte, bool) {
	if v.kind != ValueBinary {
		return nil, false
	}
	return append([]byte(nil), v.binary...), true
}

// JSONValue produces the deterministic machine representation used in query
// reports. Invalid numeric lexemes become strings rather than invalid JSON.
func (v Value) JSONValue() wirejson.Value {
	switch v.kind {
	case ValueNull:
		return wirejson.NullValue()
	case ValueBoolean:
		return wirejson.BoolValue(v.boolean)
	case ValueInteger, ValueDecimal:
		if json.Valid([]byte(v.text)) && v.text != "true" && v.text != "false" && v.text != "null" {
			return wirejson.NumberValue(v.text)
		}
		return wirejson.StringValue(v.text)
	case ValueBinary:
		return wirejson.StringValue(base64.StdEncoding.EncodeToString(v.binary))
	default:
		return wirejson.StringValue(v.text)
	}
}

// MarshalJSON prevents the unexported representation from becoming an empty
// object when a table is encoded by standard JSON machinery.
func (v Value) MarshalJSON() ([]byte, error) { return v.JSONValue().MarshalJSON() }

type Table struct {
	Columns   []Column
	Rows      [][]Value
	Truncated bool
}

type NativeQuery struct {
	Statement      string
	MaximumRows    int
	TimeoutSeconds int
}

type TabularRead struct {
	Object      ObjectRef
	Columns     []string
	MaximumRows int
}

type WorkbookRead struct {
	Path      string
	Worksheet string
	Range     string
}

type DocumentRead struct {
	Path string
}

type Document struct {
	MediaType string
	Text      string
	Bytes     []byte
}

type DiscoveryRepository struct {
	Name string
	Root string
}

type DiscoveryWorkspace struct {
	Path         string
	Project      contract.Optional[contract.ProjectKey]
	Repositories []DiscoveryRepository
}

type DiscoveryRequest struct {
	Root       string
	Workspaces []DiscoveryWorkspace
}

type DiscoveredSource struct {
	Source        Source
	Repository    string
	Application   string
	Environment   string
	Name          string
	CredentialKey contract.SecretKey
	Secret        contract.SecretValue
	Eligible      bool
	Detail        string
	SourcePaths   []string
}

type DiscoveryReport struct {
	ScannedFiles int
	Sources      []DiscoveredSource
	Warnings     []string
}
