// Package wirejson provides an ordered JSON tree for compatibility-sensitive
// configuration and wire contracts. Objects retain duplicate and unknown
// members, insertion order, explicit nulls, and original number lexemes.
package wirejson

import "fmt"

// Kind identifies a JSON value kind.
type Kind uint8

const (
	Invalid Kind = iota
	Null
	Bool
	Number
	String
	Array
	Object
)

// Member is one object member. Repeated names are retained.
type Member struct {
	Name  string
	Value Value
}

// Value is a JSON tree node. Its zero value is Invalid rather than Null so
// omitted initialization cannot silently introduce a wire null.
type Value struct {
	kind    Kind
	boolean bool
	text    string
	array   []Value
	object  []Member
}

func NullValue() Value                { return Value{kind: Null} }
func BoolValue(value bool) Value      { return Value{kind: Bool, boolean: value} }
func NumberValue(lexeme string) Value { return Value{kind: Number, text: lexeme} }
func StringValue(value string) Value  { return Value{kind: String, text: value} }
func ArrayValue(values ...Value) Value {
	return Value{kind: Array, array: append([]Value(nil), values...)}
}
func ObjectValue(members ...Member) Value {
	return Value{kind: Object, object: append([]Member(nil), members...)}
}

func (v Value) Kind() Kind               { return v.kind }
func (v Value) IsNull() bool             { return v.kind == Null }
func (v Value) IsInvalid() bool          { return v.kind == Invalid }
func (v Value) AsBool() (bool, bool)     { return v.boolean, v.kind == Bool }
func (v Value) AsNumber() (string, bool) { return v.text, v.kind == Number }
func (v Value) AsString() (string, bool) { return v.text, v.kind == String }

// ArrayValues returns the live ordered array. Mutations affect the Value.
func (v *Value) ArrayValues() ([]Value, bool) {
	if v == nil || v.kind != Array {
		return nil, false
	}
	return v.array, true
}

// Members returns the live ordered member sequence, including duplicates.
func (v *Value) Members() ([]Member, bool) {
	if v == nil || v.kind != Object {
		return nil, false
	}
	return v.object, true
}

// Lookup returns the last member with name, matching ordinary JSON decoder
// behavior while retaining earlier duplicates for lossless round trips.
func (v *Value) Lookup(name string) (*Value, bool) {
	if v == nil || v.kind != Object {
		return nil, false
	}
	for i := len(v.object) - 1; i >= 0; i-- {
		if v.object[i].Name == name {
			return &v.object[i].Value, true
		}
	}
	return nil, false
}

// ObjectAt traverses object members without discarding unknown siblings.
func (v *Value) ObjectAt(names ...string) (*Value, bool) {
	current := v
	for _, name := range names {
		next, ok := current.Lookup(name)
		if !ok || next.kind != Object {
			return nil, false
		}
		current = next
	}
	return current, true
}

// Set replaces the last member with name in place or appends a new member.
// Earlier duplicate members remain untouched by design.
func (v *Value) Set(name string, value Value) error {
	if v == nil || v.kind != Object {
		return fmt.Errorf("wirejson.not-object")
	}
	for i := len(v.object) - 1; i >= 0; i-- {
		if v.object[i].Name == name {
			v.object[i].Value = value
			return nil
		}
	}
	v.object = append(v.object, Member{Name: name, Value: value})
	return nil
}

// AppendMember always appends, including when the name already exists.
func (v *Value) AppendMember(name string, value Value) error {
	if v == nil || v.kind != Object {
		return fmt.Errorf("wirejson.not-object")
	}
	v.object = append(v.object, Member{Name: name, Value: value})
	return nil
}

// Delete removes every occurrence of name and reports whether any were found.
func (v *Value) Delete(name string) (bool, error) {
	if v == nil || v.kind != Object {
		return false, fmt.Errorf("wirejson.not-object")
	}
	kept := v.object[:0]
	found := false
	for _, member := range v.object {
		if member.Name == name {
			found = true
			continue
		}
		kept = append(kept, member)
	}
	for i := len(kept); i < len(v.object); i++ {
		v.object[i] = Member{}
	}
	v.object = kept
	return found, nil
}

// Clone performs a deep copy suitable for independent compatibility edits.
func (v Value) Clone() Value {
	clone := Value{kind: v.kind, boolean: v.boolean, text: v.text}
	if v.kind == Array {
		clone.array = make([]Value, len(v.array))
		for i := range v.array {
			clone.array[i] = v.array[i].Clone()
		}
	}
	if v.kind == Object {
		clone.object = make([]Member, len(v.object))
		for i := range v.object {
			clone.object[i] = Member{Name: v.object[i].Name, Value: v.object[i].Value.Clone()}
		}
	}
	return clone
}
