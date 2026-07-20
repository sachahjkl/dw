package gitrepo

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"unicode"

	"github.com/sachahjkl/dw/internal/l10n"
	dwprocess "github.com/sachahjkl/dw/internal/process"
)

const (
	fallbackSSHRemote  = "dw-ssh"
	anchorFetchRefspec = "+refs/heads/*:refs/remotes/origin/*"
	zeroObjectID       = "0000000000000000000000000000000000000000"
)

// Client is an immutable native-Git service. Executable may name a controlled Git shim in callers;
// an empty value selects git. Environment is ordered and never included in errors.
type Client struct {
	Executable  string
	Environment []dwprocess.EnvironmentVariable
}

func NewClient() Client { return Client{Executable: "git"} }

func (client Client) executable() string {
	if client.Executable == "" {
		return "git"
	}
	return client.Executable
}

var defaultClient = NewClient()

func NormalizeSlug(value string) string {
	if strings.TrimSpace(value) == "" {
		return ""
	}
	var result strings.Builder
	result.Grow(min(len(value), 50))
	previousDash := false
	for _, original := range value {
		for _, character := range transliterate(original) {
			character = unicode.ToLower(character)
			if character <= unicode.MaxASCII && (character >= 'a' && character <= 'z' || character >= '0' && character <= '9') {
				if result.Len() < 50 {
					result.WriteRune(character)
				}
				previousDash = false
			} else if !previousDash {
				if result.Len() < 50 {
					result.WriteByte('-')
				}
				previousDash = true
			}
		}
	}
	return strings.Trim(result.String(), "-")
}

func SlugFromPhraseOrFallback(value *string, fallback string) TaskSlug {
	if value != nil {
		if normalized := NormalizeSlug(*value); normalized != "" {
			return TaskSlug(normalized)
		}
	}
	return TaskSlug(NormalizeSlug(fallback))
}

func BuildBranchName(typeName WorkItemTypeName, workItemIDs []WorkItemID, slug TaskSlug) BranchName {
	kind := strings.ToLower(strings.TrimSpace(string(typeName)))
	if kind == "" {
		kind = "feat"
	}
	return BranchName(kind + "/" + strings.Join(distinctIDs(workItemIDs), "-") + "-" + NormalizeSlug(string(slug)))
}

func BuildSubjectName(typeName WorkItemTypeName, workItemIDs []WorkItemID, slug TaskSlug) TaskSubjectName {
	kind := strings.ToLower(strings.TrimSpace(string(typeName)))
	if kind == "" {
		kind = "feat"
	}
	return TaskSubjectName(kind + "-" + strings.Join(distinctIDs(workItemIDs), "-") + "-" + NormalizeSlug(string(slug)))
}

func ResolveRemoteSourceBranch(defaultBranch BranchName) string {
	return "origin/" + string(defaultBranch)
}

func distinctIDs(values []WorkItemID) []string {
	result := make([]string, 0, len(values))
	for _, value := range values {
		candidate := string(value)
		if strings.TrimSpace(candidate) == "" {
			continue
		}
		found := false
		for _, existing := range result {
			if strings.EqualFold(existing, candidate) {
				found = true
				break
			}
		}
		if !found {
			result = append(result, candidate)
		}
	}
	return result
}

func UpdateRepository(repositoryPath RepositoryPath, defaultBranch BranchName, credential *Credential, sshURL *RemoteURL) error {
	return defaultClient.UpdateRepository(context.Background(), repositoryPath, defaultBranch, credential, sshURL)
}

func (client Client) UpdateRepository(ctx context.Context, repositoryPath RepositoryPath, defaultBranch BranchName, credential *Credential, sshURL *RemoteURL) error {
	if err := client.ensureRepository(ctx, repositoryPath); err != nil {
		return err
	}
	if err := client.configureRemotes(ctx, repositoryPath, nil, sshURL); err != nil {
		return err
	}
	changed, err := client.repositoryHasChanges(ctx, repositoryPath)
	if err != nil {
		return err
	}
	stashed := false
	if changed {
		_, err = client.run(ctx, OperationCommit, repositoryPath, nil, nil, "stash", "push", "--include-untracked", "--message", l10n.Text("git.autostash-message"))
		if err != nil {
			return err
		}
		stashed = true
	}
	if err = client.fetchWithFallback(ctx, repositoryPath, credential, sshURL); err != nil {
		return err
	}
	source := ResolveRemoteSourceBranch(defaultBranch)
	_, err = client.run(ctx, OperationRebase, repositoryPath, nil, nil, "rebase", "--quiet", source)
	if err != nil {
		_, _ = client.run(ctx, OperationRebase, repositoryPath, nil, nil, "rebase", "--abort")
		return fmt.Errorf("%s: %w", l10n.Render(l10n.M("git.rebase-conflict",
			l10n.A("repository", repositoryPath),
			l10n.A("source", source),
			l10n.A("cause", err),
		)), err)
	}
	if stashed {
		if _, err = client.run(ctx, OperationRebase, repositoryPath, nil, nil, "stash", "pop"); err != nil {
			return err
		}
	}
	return nil
}

func InspectRepositoryStatus(repositoryPath RepositoryPath) RepositoryStatus {
	return defaultClient.RepositoryStatus(context.Background(), repositoryPath)
}

func (client Client) RepositoryStatus(ctx context.Context, repositoryPath RepositoryPath) RepositoryStatus {
	status := RepositoryStatus{Path: repositoryPath}
	information, err := os.Stat(string(repositoryPath))
	if err != nil || !information.IsDir() {
		status.Detail.Kind = StatusMissingDirectory
		return status
	}
	if err = client.ensureRepository(ctx, repositoryPath); err != nil {
		status.Detail = RepositoryStatusDetail{Kind: StatusOpenFailed, Detail: errorDetail(err)}
		return status
	}
	result, err := client.run(ctx, OperationStatus, repositoryPath, nil, nil, "status", "--porcelain=v1", "-z", "--untracked-files=all")
	if err != nil {
		status.Detail = RepositoryStatusDetail{Kind: StatusStatusFailed, Detail: errorDetail(err)}
		return status
	}
	status.IsGitRepository = true
	paths := parsePorcelainPaths(result.Stdout)
	if len(paths) != 0 {
		status.HasChanges = true
		status.Detail = RepositoryStatusDetail{Kind: StatusChanged, Paths: paths}
		return status
	}
	ahead := 0
	result, err = client.run(ctx, OperationStatus, repositoryPath, nil, nil, "rev-list", "--count", "--end-of-options", "@{u}..HEAD")
	if err == nil {
		ahead, _ = strconv.Atoi(strings.TrimSpace(string(result.Stdout)))
	}
	status.HasUnpushed = ahead > 0
	if ahead > 0 {
		status.Detail = RepositoryStatusDetail{Kind: StatusUnpushed, Ahead: ahead}
	} else {
		status.Detail.Kind = StatusClean
	}
	return status
}

func HasCommitsAheadOf(repositoryPath RepositoryPath, base Revision) (bool, error) {
	return defaultClient.HasCommitsAheadOf(context.Background(), repositoryPath, base)
}

func (client Client) HasCommitsAheadOf(ctx context.Context, repositoryPath RepositoryPath, base Revision) (bool, error) {
	result, err := client.run(ctx, OperationLog, repositoryPath, nil, nil, "rev-list", "--count", "--end-of-options", string(base)+"..HEAD")
	if err != nil {
		return false, err
	}
	count, err := strconv.Atoi(strings.TrimSpace(string(result.Stdout)))
	if err != nil {
		return false, client.operationError(OperationLog, repositoryPath, result, err, false, nil, "")
	}
	return count > 0, nil
}

func CommitMessagesInRange(revisionRange RevisionRange) (CommitMessages, error) {
	return CommitMessagesInRangeAt(".", revisionRange)
}

func CommitMessagesInRangeAt(repositoryPath RepositoryPath, revisionRange RevisionRange) (CommitMessages, error) {
	return defaultClient.CommitMessagesInRangeAt(context.Background(), repositoryPath, revisionRange)
}

func (client Client) CommitMessagesInRangeAt(ctx context.Context, repositoryPath RepositoryPath, revisionRange RevisionRange) (CommitMessages, error) {
	result, err := client.run(ctx, OperationLog, repositoryPath, nil, nil,
		"log", "--format=%B%x1e", "--end-of-options", string(revisionRange.From)+".."+string(revisionRange.To))
	if err != nil {
		return "", err
	}
	return CommitMessages(result.Stdout), nil
}

func CommitRepository(repositoryPath RepositoryPath, message CommitMessage) error {
	return defaultClient.CommitRepository(context.Background(), repositoryPath, message)
}

func (client Client) CommitRepository(ctx context.Context, repositoryPath RepositoryPath, message CommitMessage) error {
	if _, err := client.run(ctx, OperationCommit, repositoryPath, nil, nil, "add", "--all", "--", "."); err != nil {
		return err
	}
	_, err := client.runInput(ctx, OperationCommit, repositoryPath, []byte(message), "commit", "--file=-", "--cleanup=verbatim", "--allow-empty", "--allow-empty-message", "--no-verify", "--no-gpg-sign")
	return err
}

func PushRepository(repositoryPath RepositoryPath, branchName BranchName) error {
	return defaultClient.PushRepository(context.Background(), repositoryPath, branchName, false)
}

func PushRepositoryForceWithLease(repositoryPath RepositoryPath, branchName BranchName) error {
	return defaultClient.PushRepository(context.Background(), repositoryPath, branchName, true)
}

func (client Client) PushRepository(ctx context.Context, repositoryPath RepositoryPath, branchName BranchName, forceWithLease bool) error {
	sshURL, err := client.configuredRemoteURL(ctx, repositoryPath, fallbackSSHRemote)
	if err != nil {
		return err
	}
	originURL, err := client.configuredRemoteURL(ctx, repositoryPath, "origin")
	if err != nil {
		return err
	}
	destination := "refs/heads/" + string(branchName)
	refspec := "refs/heads/" + string(branchName) + ":" + destination
	arguments := []string{"push"}
	if forceWithLease {
		expected, resolveErr := client.referenceObjectID(ctx, repositoryPath, "refs/remotes/origin/"+string(branchName))
		if resolveErr != nil {
			return resolveErr
		}
		arguments = append(arguments, "--force-with-lease="+destination+":"+expected)
	}
	arguments = append(arguments, "origin", refspec)
	_, err = client.run(ctx, OperationPush, repositoryPath, nil, originURL, arguments...)
	if err == nil {
		return nil
	}
	if !shouldTrySSHFallback(err) || sshURL == nil {
		return err
	}
	arguments[len(arguments)-2] = fallbackSSHRemote
	_, err = client.run(ctx, OperationPush, repositoryPath, nil, sshURL, arguments...)
	return err
}

func PrepareWorktree(request WorktreePrepareRequest) (WorktreePrepareResult, error) {
	return defaultClient.PrepareWorktree(context.Background(), request)
}

func (client Client) PrepareWorktree(ctx context.Context, request WorktreePrepareRequest) (WorktreePrepareResult, error) {
	result := WorktreePrepareResult{Repository: request.Repository}
	if strings.TrimSpace(string(request.HTTPURL)) == "" {
		if err := os.MkdirAll(string(request.WorktreePath), 0o755); err != nil {
			return result, client.operationError(OperationWorktreeAdd, request.WorktreePath, dwprocess.Result{}, err, false, nil, "")
		}
		result.Status = WorktreePlaceholder
		result.Detail.Kind = WorktreeMissingRemoteURL
		return result, nil
	}
	repositoriesRoot := filepath.Join(string(request.ProjectRoot), "repositories")
	anchorPath := RepositoryPath(filepath.Join(repositoriesRoot, string(request.AnchorName)))
	if err := os.MkdirAll(repositoriesRoot, 0o755); err != nil {
		return result, client.operationError(OperationCloneBare, anchorPath, dwprocess.Result{}, err, false, nil, "")
	}
	httpURL := RemoteURL(NormalizeRemoteURL(request.HTTPURL))
	if information, err := os.Stat(string(anchorPath)); err != nil || !information.IsDir() {
		if err = client.cloneBare(ctx, httpURL, request.SSHURL, anchorPath, request.Credential); err != nil {
			return result, err
		}
	} else if err = client.ensureBareRepository(ctx, anchorPath); err != nil {
		return result, err
	}
	if err := client.configureRemotes(ctx, anchorPath, &httpURL, request.SSHURL); err != nil {
		return result, err
	}
	if _, err := client.run(ctx, OperationConfigureRemote, anchorPath, nil, nil,
		"config", "--replace-all", "remote.origin.fetch", anchorFetchRefspec); err != nil {
		return result, err
	}
	if err := client.fetchWithFallback(ctx, anchorPath, request.Credential, request.SSHURL); err != nil {
		return result, err
	}
	if information, err := os.Stat(string(request.WorktreePath)); err == nil && information.IsDir() {
		result.Status = WorktreePrepared
		result.Detail.Kind = WorktreeAlreadyPresent
		return result, nil
	}
	baseRef := ""
	for _, candidate := range []string{"origin/" + string(request.DefaultBranch), "refs/heads/" + string(request.DefaultBranch)} {
		if client.referenceExists(ctx, anchorPath, candidate+"^{commit}") {
			baseRef = candidate
			break
		}
	}
	if baseRef == "" {
		detail := errors.New(l10n.Render(l10n.M("git.base-not-found", l10n.A("branch", request.DefaultBranch))))
		return result, client.operationError(OperationWorktreeAdd, anchorPath, dwprocess.Result{}, detail, false, nil, "")
	}
	branchRef := "refs/heads/" + string(request.BranchName)
	branchExists := client.referenceExists(ctx, anchorPath, branchRef)
	if !branchExists {
		if _, err := client.run(ctx, OperationWorktreeAdd, anchorPath, nil, nil,
			"-c", "branch.autoSetupMerge=false", "branch", "--", string(request.BranchName), baseRef); err != nil {
			return result, err
		}
	}
	if _, err := client.run(ctx, OperationWorktreeAdd, anchorPath, nil, nil,
		"worktree", "add", "--", string(request.WorktreePath), string(request.BranchName)); err != nil {
		return result, err
	}
	result.Status = WorktreePrepared
	if branchExists {
		result.Detail = WorktreePrepareDetail{Kind: WorktreeCreatedExistingBranch, Branch: request.BranchName}
	} else {
		result.Detail = WorktreePrepareDetail{Kind: WorktreeCreatedFromBaseReference, Reference: baseRef}
	}
	return result, nil
}

func WorktreeRemove(gitDirectory, worktreePath RepositoryPath) error {
	return defaultClient.WorktreeRemove(context.Background(), gitDirectory, worktreePath)
}

func (client Client) WorktreeRemove(ctx context.Context, gitDirectory, worktreePath RepositoryPath) error {
	if err := client.ensureBareRepository(ctx, gitDirectory); err != nil {
		return err
	}
	_, removalErr := client.run(ctx, OperationWorktreeRemove, gitDirectory, nil, nil,
		"worktree", "remove", "--force", "--", string(worktreePath))
	if removalErr == nil {
		return nil
	}
	if _, statErr := os.Stat(string(worktreePath)); statErr == nil {
		if removeErr := os.RemoveAll(string(worktreePath)); removeErr != nil {
			return client.operationError(OperationWorktreeRemove, worktreePath, dwprocess.Result{}, removeErr, false, nil, "")
		}
	}
	if _, err := client.run(ctx, OperationWorktreePrune, gitDirectory, nil, nil, "worktree", "prune", "--expire", "now"); err != nil {
		return err
	}
	registered, err := client.worktreeRegistered(ctx, gitDirectory, worktreePath)
	if err != nil {
		return err
	}
	if registered {
		return removalErr
	}
	return nil
}

func (client Client) worktreeRegistered(ctx context.Context, gitDirectory, worktreePath RepositoryPath) (bool, error) {
	result, err := client.run(ctx, OperationWorktreeRemove, gitDirectory, nil, nil, "worktree", "list", "--porcelain", "-z")
	if err != nil {
		return false, err
	}
	target := filepath.Clean(string(worktreePath))
	for _, field := range strings.Split(string(result.Stdout), "\x00") {
		if path, found := strings.CutPrefix(field, "worktree "); found && filepath.Clean(path) == target {
			return true, nil
		}
	}
	return false, nil
}

func WorktreePrune(gitDirectory RepositoryPath) error {
	return defaultClient.WorktreePrune(context.Background(), gitDirectory)
}

func (client Client) WorktreePrune(ctx context.Context, gitDirectory RepositoryPath) error {
	if err := client.ensureBareRepository(ctx, gitDirectory); err != nil {
		return err
	}
	_, err := client.run(ctx, OperationWorktreePrune, gitDirectory, nil, nil, "worktree", "prune", "--expire", "now")
	return err
}

// ConfigureRemotes is the compatibility-facing concrete service requested by config refresh.
func ConfigureRemotes(repositoryPath string, originURL string, sshURL *string) error {
	var ssh *RemoteURL
	if sshURL != nil {
		value := RemoteURL(*sshURL)
		ssh = &value
	}
	var origin *RemoteURL
	if strings.TrimSpace(originURL) != "" {
		value := RemoteURL(originURL)
		origin = &value
	}
	return defaultClient.configureRemotes(context.Background(), RepositoryPath(repositoryPath), origin, ssh)
}

func (client Client) ConfigureRemotes(ctx context.Context, repositoryPath RepositoryPath, originURL RemoteURL, sshURL *RemoteURL) error {
	return client.configureRemotes(ctx, repositoryPath, &originURL, sshURL)
}

func NormalizeRemoteURL(remoteURL RemoteURL) string {
	raw := strings.TrimSpace(string(remoteURL))
	if strings.Contains(raw, "://") {
		parsed, err := url.Parse(raw)
		if err == nil && parsed.User != nil {
			if strings.EqualFold(parsed.Scheme, "http") || strings.EqualFold(parsed.Scheme, "https") {
				parsed.User = nil
				return parsed.String()
			}
			if _, hasPassword := parsed.User.Password(); hasPassword {
				parsed.User = url.User(parsed.User.Username())
				return parsed.String()
			}
		}
		return raw
	}
	if strings.HasPrefix(raw, "/") || strings.HasPrefix(raw, "./") || strings.HasPrefix(raw, "../") {
		return raw
	}
	authority, path, found := strings.Cut(raw, ":")
	if !found || len(authority) == 1 || strings.ContainsAny(authority, `/\\`) || strings.TrimSpace(path) == "" {
		return raw
	}
	return "ssh://" + authority + "/" + path
}

func supportedRemoteURL(remoteURL RemoteURL) bool {
	raw := strings.TrimSpace(string(remoteURL))
	if raw == "" || strings.Contains(raw, "::") {
		return false
	}
	normalized := NormalizeRemoteURL(remoteURL)
	if !strings.Contains(normalized, "://") {
		return true
	}
	parsed, err := url.Parse(normalized)
	if err != nil {
		return false
	}
	switch strings.ToLower(parsed.Scheme) {
	case "file", "git", "http", "https", "ssh":
		return true
	default:
		return false
	}
}

func WorktreeName(request WorktreePrepareRequest) string {
	raw := string(request.Repository) + "-" + string(request.BranchName)
	var name strings.Builder
	previousDash := false
	for _, character := range raw {
		if character <= unicode.MaxASCII && (character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9') {
			name.WriteRune(unicode.ToLower(character))
			previousDash = false
		} else if !previousDash {
			name.WriteByte('-')
			previousDash = true
		}
	}
	return strings.Trim(name.String(), "-")
}
