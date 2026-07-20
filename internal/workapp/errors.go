package workapp

import (
	"fmt"

	"github.com/sachahjkl/dw/internal/l10n"
)

// Error is a stable provider-neutral user-facing problem. English preserves
// compatibility for non-localizing callers; Localized is the presentation
// gateway used by CLI and TUI.
type Error struct {
	Message l10n.Message
	English string
	Cause   error
}

func (e *Error) Error() string {
	if e == nil {
		return ""
	}
	if e.Cause != nil {
		return fmt.Sprintf(e.English, messageValues(e.Message)...) + ": " + e.Cause.Error()
	}
	return fmt.Sprintf(e.English, messageValues(e.Message)...)
}
func (e *Error) Unwrap() error {
	if e == nil {
		return nil
	}
	return e.Cause
}
func (e *Error) Localized() l10n.Message {
	if e == nil {
		return l10n.Message{}
	}
	return e.Message
}

func problem(id l10n.ID, english string, args ...l10n.Arg) error {
	return &Error{Message: l10n.M(id, args...), English: english}
}
func problemCause(id l10n.ID, english string, cause error, args ...l10n.Arg) error {
	return &Error{Message: l10n.M(id, args...), English: english, Cause: cause}
}
func messageValues(message l10n.Message) []any {
	values := make([]any, len(message.Args))
	for index, arg := range message.Args {
		values[index] = arg.Value
	}
	return values
}

func projectRequired(command string) error {
	return problem(msgCommandProjectRequired, "%s requires a configured project: a configured project is required", l10n.A("command", command))
}
func repositoriesRequired(command, detail string) error {
	return problem(msgCommandRepositoriesRequired, "%s %s: at least one work repository is required", l10n.A("command", command), l10n.A("detail", detail))
}
func capabilityUnavailable(name string) error {
	return problem(msgCapabilityUnavailable, "%s capability is unavailable", l10n.A("capability", name))
}
func itemNotFound(id string) error {
	return problem(msgItemNotFound, "work item not found: %s", l10n.A("id", id))
}
func prItemsNotFound(id int64, repositories string) error {
	return problem(msgPRItemsNotFound, "no work item linked to PR #%d in tested repositories: %s", l10n.A("id", id), l10n.A("repositories", repositories))
}
func invalidPullRequestID(value any) error {
	return problem(msgPullRequestIDInvalid, "invalid pull request ID %v: must be positive", l10n.A("id", value))
}
func invalidProviderPullRequestID(value string, cause error) error {
	return problemCause(msgProviderPullRequestIDInvalid, "invalid pull request ID %q returned by provider", cause, l10n.A("id", value))
}

const (
	msgProjectRequired              l10n.ID = "work.error.project-required"
	msgCommandProjectRequired       l10n.ID = "work.error.command-project-required"
	msgItemsRequired                l10n.ID = "work.error.items-required"
	msgRepositoriesRequired         l10n.ID = "work.error.repositories-required"
	msgCommandRepositoriesRequired  l10n.ID = "work.error.command-repositories-required"
	msgInvalidHandoff               l10n.ID = "work.error.invalid-handoff"
	msgCapabilityUnavailable        l10n.ID = "work.error.capability-unavailable"
	msgChangelogTableFormat         l10n.ID = "work.error.changelog-table-format"
	msgChangelogIDsTable            l10n.ID = "work.error.changelog-ids-table"
	msgChangelogSource              l10n.ID = "work.error.changelog-source"
	msgFinishPRProviderless         l10n.ID = "work.error.finish-pr-providerless"
	msgItemNotFound                 l10n.ID = "work.error.item-not-found"
	msgPRItemsNotFound              l10n.ID = "work.error.pr-items-not-found"
	msgOpenPRProject                l10n.ID = "work.error.open-pr-project"
	msgStartItemRequired            l10n.ID = "work.error.start-item-required"
	msgOpenPRRepository             l10n.ID = "work.error.open-pr-repository"
	msgWorkspaceParentMissing       l10n.ID = "work.error.workspace-parent-missing"
	msgChildUnsupported             l10n.ID = "work.error.child-unsupported"
	msgItemTypeMissing              l10n.ID = "work.error.item-type-missing"
	msgItemTypeUnsupported          l10n.ID = "work.error.item-type-unsupported"
	msgProviderStateResultMissing   l10n.ID = "work.error.provider-state-result-missing"
	msgProviderPullRequestIDInvalid l10n.ID = "work.error.provider-pull-request-id-invalid"
	msgPullRequestIDInvalid         l10n.ID = "work.error.pull-request-id-invalid"
)
