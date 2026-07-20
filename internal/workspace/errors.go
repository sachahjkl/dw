package workspace

import "github.com/sachahjkl/dw/internal/l10n"

// Error carries a stable localization message while retaining an optional
// machine-inspectable cause for errors.Is/errors.As.
type Error struct {
	Message l10n.Message
	Cause   error
}

func (err *Error) Error() string {
	if err == nil {
		return ""
	}
	return l10n.Render(err.Message)
}
func (err *Error) Localized() l10n.Message {
	if err == nil {
		return l10n.Message{}
	}
	return err.Message
}
func (err *Error) Unwrap() error {
	if err == nil {
		return nil
	}
	return err.Cause
}

func localized(id l10n.ID, args ...l10n.Arg) error {
	return &Error{Message: l10n.M(id, args...)}
}

func localizedCause(id l10n.ID, cause error, args ...l10n.Arg) error {
	return &Error{Message: l10n.M(id, args...), Cause: cause}
}

func localizedDetail(id l10n.ID, cause error, args ...l10n.Arg) error {
	if cause != nil {
		args = append(args, l10n.A("detail", cause.Error()))
	}
	return localizedCause(id, cause, args...)
}

func localizedOperation(operation string, cause error) error {
	if cause == nil {
		return nil
	}
	if _, ok := cause.(interface{ Localized() l10n.Message }); ok {
		return cause
	}
	return localizedDetail("workspace.error.operation", cause, l10n.A("operation", operation))
}

var (
	ErrNoWorkspace              = &Error{Message: l10n.M("workspace.error.no-workspace")}
	ErrNoCurrentWorkspace       = &Error{Message: l10n.M("workspace.error.no-current-workspace")}
	ErrInvalidManifest          = &Error{Message: l10n.M("workspace.error.invalid-manifest", l10n.A("path", ManifestFile))}
	ErrWorkspaceConflict        = &Error{Message: l10n.M("workspace.error.workspace-conflict", l10n.A("detail", "workspace"))}
	ErrMissingRepository        = &Error{Message: l10n.M("workspace.error.missing-repository", l10n.A("repository", "repository"))}
	ErrEmptyWorkItemSet         = &Error{Message: l10n.M("workspace.error.empty-work-item-set")}
	ErrApprovalRequired         = &Error{Message: l10n.M("workspace.error.approval-required")}
	ErrInvalidHandoff           = &Error{Message: l10n.M("workspace.error.invalid-handoff")}
	ErrVerificationFailed       = &Error{Message: l10n.M("workspace.error.verification-failed")}
	ErrWorkCapabilityRequired   = &Error{Message: l10n.M("workspace.error.work-capability-required")}
	ErrGitCapabilityRequired    = &Error{Message: l10n.M("workspace.error.git-capability-required")}
	ErrSecretCapabilityRequired = &Error{Message: l10n.M("workspace.error.secret-capability-required")}
)
