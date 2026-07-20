package secret

import "github.com/sachahjkl/dw/internal/l10n"

// LocalizedError separates a stable machine-safe error code from presentation text.
type LocalizedError struct {
	code    string
	message l10n.Message
	cause   error
}

func newLocalizedError(code string, message l10n.Message, cause error) *LocalizedError {
	return &LocalizedError{code: code, message: message, cause: cause}
}

func (problem *LocalizedError) Error() string           { return problem.code }
func (problem *LocalizedError) Localized() l10n.Message { return problem.message }
func (problem *LocalizedError) Unwrap() error           { return problem.cause }
