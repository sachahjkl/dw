package l10n

// Error carries a presentation message and an optional machine-inspectable cause.
type Error struct {
	Message Message
	Cause   error
}

func (problem *Error) Error() string {
	if problem == nil {
		return ""
	}
	return Render(problem.Message)
}

func (problem *Error) Localized() Message {
	if problem == nil {
		return Message{}
	}
	return problem.Message
}

func (problem *Error) Unwrap() error {
	if problem == nil {
		return nil
	}
	return problem.Cause
}

func NewError(id ID, args ...Arg) error {
	return &Error{Message: M(id, args...)}
}

func WrapError(cause error, id ID, args ...Arg) error {
	return &Error{Message: M(id, args...), Cause: cause}
}
