package contract

type RelationID string

type ActionArgument struct {
	Name  string
	Value string
}

// ActionLink describes a semantic transition. Each frontend decides how to
// present or execute it without leaking another frontend's syntax.
type ActionLink struct {
	Relation  RelationID
	Arguments []ActionArgument
}
