// Package gitrepo provides typed repository operations backed exclusively by the native Git executable.
package gitrepo

import (
	"encoding/json"
	"errors"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

type RepositoryPath string
type BranchName string
type AnchorName string
type RemoteURL string
type Revision string
type CommitMessage string
type TaskSlug string
type TaskSubjectName string
type WorkItemID string
type WorkItemTypeName string
type WorkspaceRepositoryName string
type ProjectRootPath string

type RewriteNote struct {
	Strategy string `json:"strategy"`
	Status   string `json:"status"`
}

func CurrentStrategy() RewriteNote { return RewriteNote{Strategy: "native-git", Status: "active"} }

type StatusDetailKind string

const (
	StatusMissingDirectory StatusDetailKind = "missing-directory"
	StatusOpenFailed       StatusDetailKind = "open-failed"
	StatusStatusFailed     StatusDetailKind = "status-failed"
	StatusChanged          StatusDetailKind = "changed"
	StatusUnpushed         StatusDetailKind = "unpushed"
	StatusClean            StatusDetailKind = "clean"
)

type RepositoryStatus struct {
	Path            RepositoryPath         `json:"path"`
	IsGitRepository bool                   `json:"isGitRepository"`
	HasChanges      bool                   `json:"hasChanges"`
	HasUnpushed     bool                   `json:"hasUnpushed"`
	Detail          RepositoryStatusDetail `json:"detail"`
}

type RepositoryStatusDetail struct {
	Kind   StatusDetailKind `json:"kind"`
	Detail string           `json:"detail,omitempty"`
	Paths  []string         `json:"paths,omitempty"`
	Ahead  int              `json:"ahead,omitempty"`
}

type RevisionRange struct {
	From Revision `json:"from"`
	To   Revision `json:"to"`
}

type CommitMessages string

func (messages CommitMessages) String() string { return string(messages) }

type WorktreePrepareRequest struct {
	ProjectRoot   ProjectRootPath         `json:"projectRoot"`
	Repository    WorkspaceRepositoryName `json:"repository"`
	HTTPURL       RemoteURL               `json:"httpUrl"`
	SSHURL        *RemoteURL              `json:"sshUrl,omitempty"`
	DefaultBranch BranchName              `json:"defaultBranch"`
	AnchorName    AnchorName              `json:"anchorName"`
	BranchName    BranchName              `json:"branchName"`
	WorktreePath  RepositoryPath          `json:"worktreePath"`
	Credential    *Credential             `json:"-"`
}

type WorktreePrepareStatus string

const (
	WorktreePlaceholder WorktreePrepareStatus = "placeholder"
	WorktreePrepared    WorktreePrepareStatus = "prepared"
)

type WorktreePrepareDetailKind string

const (
	WorktreeMissingRemoteURL         WorktreePrepareDetailKind = "missing-remote-url"
	WorktreeAlreadyPresent           WorktreePrepareDetailKind = "already-present"
	WorktreeCreatedExistingBranch    WorktreePrepareDetailKind = "created-from-existing-branch"
	WorktreeCreatedFromBaseReference WorktreePrepareDetailKind = "created-from-base-reference"
)

type WorktreePrepareResult struct {
	Repository WorkspaceRepositoryName `json:"repository"`
	Status     WorktreePrepareStatus   `json:"status"`
	Detail     WorktreePrepareDetail   `json:"detail"`
}

type WorktreePrepareDetail struct {
	Kind      WorktreePrepareDetailKind `json:"kind"`
	Branch    BranchName                `json:"branch,omitempty"`
	Reference string                    `json:"reference,omitempty"`
}

// Credential is safe to format and serialize. Its plaintext can only be consumed inside this package.
type Credential struct{ token contract.SecretValue }

func NewPersonalAccessToken(token contract.SecretValue) Credential { return Credential{token: token} }
func (Credential) String() string                                  { return "<hidden>" }
func (Credential) GoString() string                                { return "gitrepo.Credential(<hidden>)" }
func (Credential) MarshalJSON() ([]byte, error)                    { return json.Marshal("<hidden>") }
func (credential Credential) empty() bool                          { return credential.token.Empty() }

type Operation string

const (
	OperationOpenRepository  Operation = "open-repository"
	OperationStatus          Operation = "status"
	OperationLog             Operation = "log"
	OperationFetch           Operation = "fetch"
	OperationRebase          Operation = "rebase"
	OperationCommit          Operation = "commit"
	OperationPush            Operation = "push"
	OperationCloneBare       Operation = "clone-bare"
	OperationConfigureRemote Operation = "configure-remote"
	OperationWorktreeAdd     Operation = "worktree-add"
	OperationWorktreeRemove  Operation = "worktree-remove"
	OperationWorktreePrune   Operation = "worktree-prune"
)

type Invocation struct {
	Operation      Operation       `json:"operation"`
	RepositoryPath *RepositoryPath `json:"repositoryPath,omitempty"`
}

type ErrorKind string

const (
	ErrorOperationFailed ErrorKind = "operation-failed"
	ErrorAuthentication  ErrorKind = "authentication"
)

type AuthFailureKind string

const (
	AuthHTTPSCredentialMissing  AuthFailureKind = "https-credential-missing"
	AuthHTTPSCredentialRejected AuthFailureKind = "https-credential-rejected"
	AuthSSHHostKeyMissing       AuthFailureKind = "ssh-host-key-missing"
	AuthSSHKeyUnavailable       AuthFailureKind = "ssh-key-unavailable"
)

type AuthRemediation string

const (
	RemediationConfigureHTTPSCredential AuthRemediation = "configure-https-credential"
	RemediationVerifyHTTPSCredential    AuthRemediation = "verify-https-credential"
	RemediationTrustSSHHostKey          AuthRemediation = "trust-ssh-host-key"
	RemediationConfigureSSHKey          AuthRemediation = "configure-ssh-key"
)

type Error struct {
	Kind        ErrorKind       `json:"kind"`
	Operation   Operation       `json:"operation"`
	Detail      string          `json:"detail"`
	AuthKind    AuthFailureKind `json:"authKind,omitempty"`
	Remediation AuthRemediation `json:"remediation,omitempty"`
	Invocation  Invocation      `json:"invocation"`
	cause       error
}

func (problem *Error) Message() l10n.Message {
	if problem.Kind == ErrorAuthentication {
		return l10n.M("git.authentication-failed",
			l10n.A("kind", authKindMessage(problem.AuthKind)),
			l10n.A("remediation", remediationMessage(problem.Remediation)),
			l10n.A("detail", problem.Detail),
		)
	}
	return l10n.M("git.operation-failed",
		l10n.A("operation", problem.Operation),
		l10n.A("detail", problem.Detail),
	)
}

func (problem *Error) Localized() l10n.Message { return problem.Message() }
func (problem *Error) Error() string           { return l10n.Render(problem.Message()) }

func (problem *Error) Unwrap() error { return problem.cause }

func IsErrorKind(err error, kind ErrorKind) bool {
	var problem *Error
	return errors.As(err, &problem) && problem.Kind == kind
}

func authKindMessage(kind AuthFailureKind) any {
	switch kind {
	case AuthHTTPSCredentialMissing:
		return l10n.M("git.auth.https-missing")
	case AuthHTTPSCredentialRejected:
		return l10n.M("git.auth.https-rejected")
	case AuthSSHHostKeyMissing:
		return l10n.M("git.auth.ssh-host-key")
	case AuthSSHKeyUnavailable:
		return l10n.M("git.auth.ssh-key")
	default:
		return string(kind)
	}
}

func remediationMessage(remediation AuthRemediation) any {
	switch remediation {
	case RemediationConfigureHTTPSCredential:
		return l10n.M("git.remediation.configure-https")
	case RemediationVerifyHTTPSCredential:
		return l10n.M("git.remediation.verify-https")
	case RemediationTrustSSHHostKey:
		return l10n.M("git.remediation.trust-host-key")
	case RemediationConfigureSSHKey:
		return l10n.M("git.remediation.configure-ssh")
	default:
		return string(remediation)
	}
}
