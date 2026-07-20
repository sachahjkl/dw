package gitrepo

import (
	"context"
	"encoding/base64"
	"errors"
	"os"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
	dwprocess "github.com/sachahjkl/dw/internal/process"
)

func (client Client) run(ctx context.Context, operation Operation, repositoryPath RepositoryPath, credential *Credential, remoteURL *RemoteURL, arguments ...string) (dwprocess.Result, error) {
	return client.runCommand(ctx, operation, repositoryPath, credential, remoteURL, nil, arguments...)
}

func (client Client) runInput(ctx context.Context, operation Operation, repositoryPath RepositoryPath, input []byte, arguments ...string) (dwprocess.Result, error) {
	return client.runCommand(ctx, operation, repositoryPath, nil, nil, input, arguments...)
}

func (client Client) runCommand(ctx context.Context, operation Operation, repositoryPath RepositoryPath, credential *Credential, remoteURL *RemoteURL, input []byte, arguments ...string) (dwprocess.Result, error) {
	if remoteURL != nil && !supportedRemoteURL(*remoteURL) {
		cause := errors.New(l10n.Text("git.unsupported-transport"))
		return dwprocess.Result{ExitCode: -1}, client.operationError(operation, repositoryPath, dwprocess.Result{}, cause, false, nil, "")
	}
	resolvedCredential := resolveCredential(credential, remoteURL)
	commandArguments := make([]string, 0, len(arguments)+34)
	commandArguments = append(commandArguments,
		"-c", "core.hooksPath="+os.DevNull,
		"-c", "commit.gpgSign=false",
		"-c", "core.fsmonitor=false",
		"-c", "rebase.autoStash=false",
		"-c", "rebase.updateRefs=false",
		"-c", "log.order=default",
		"-c", "fetch.pruneTags=false",
		"-c", "fetch.recurseSubmodules=false",
		"-c", "push.gpgSign=false",
		"-c", "protocol.allow=never",
		"-c", "protocol.file.allow=always",
		"-c", "protocol.git.allow=always",
		"-c", "protocol.http.allow=always",
		"-c", "protocol.https.allow=always",
		"-c", "protocol.ssh.allow=always",
	)
	if resolvedCredential != nil && !resolvedCredential.empty() {
		commandArguments = append(commandArguments, "-c", "credential.helper=")
	}
	if repositoryPath != "" {
		commandArguments = append(commandArguments, "-C", string(repositoryPath))
	}
	commandArguments = append(commandArguments, arguments...)
	environment := make([]dwprocess.EnvironmentVariable, 0, len(client.Environment)+10)
	environment = append(environment, client.Environment...)
	environment = append(environment,
		dwprocess.EnvironmentVariable{Name: "GIT_TERMINAL_PROMPT", Value: "0"},
		dwprocess.EnvironmentVariable{Name: "GCM_INTERACTIVE", Value: "Never"},
		dwprocess.EnvironmentVariable{Name: "GIT_SSH_COMMAND", Value: "ssh -oBatchMode=yes"},
		dwprocess.EnvironmentVariable{Name: "SSH_ASKPASS_REQUIRE", Value: "never"},
	)
	if resolvedCredential != nil && !resolvedCredential.empty() {
		token := resolvedCredential.token.Reveal()
		authorization := base64.StdEncoding.EncodeToString([]byte("dw:" + token))
		environment = append(environment,
			dwprocess.EnvironmentVariable{Name: "GIT_CONFIG_COUNT", Value: "1"},
			dwprocess.EnvironmentVariable{Name: "GIT_CONFIG_KEY_0", Value: "http.extraHeader"},
			dwprocess.EnvironmentVariable{Name: "GIT_CONFIG_VALUE_0", Value: "Authorization: Basic " + authorization},
			dwprocess.EnvironmentVariable{Name: "GIT_TRACE", Value: "0"},
			dwprocess.EnvironmentVariable{Name: "GIT_TRACE_CURL", Value: "0"},
			dwprocess.EnvironmentVariable{Name: "GIT_CURL_VERBOSE", Value: "0"},
		)
	}
	result, err := dwprocess.Output(ctx, dwprocess.Command{
		FileName:    client.executable(),
		Arguments:   commandArguments,
		Environment: environment,
		Input:       input,
	})
	if err == nil {
		return result, nil
	}
	return result, client.operationError(operation, repositoryPath, result, err, resolvedCredential != nil, resolvedCredential, remoteString(remoteURL))
}

func (client Client) operationError(operation Operation, repositoryPath RepositoryPath, result dwprocess.Result, cause error, credentialAvailable bool, credential *Credential, remoteURL string) error {
	detail := strings.TrimSpace(string(result.Stderr))
	if detail == "" {
		detail = cause.Error()
	}
	if credential != nil && !credential.token.Empty() {
		token := credential.token.Reveal()
		detail = strings.ReplaceAll(detail, token, "***")
		detail = strings.ReplaceAll(detail, base64.StdEncoding.EncodeToString([]byte("dw:"+token)), "***")
	}
	detail = redactRemoteCredentials(detail, remoteURL)
	path := repositoryPath
	invocation := Invocation{Operation: operation}
	if path != "" {
		invocation.RepositoryPath = &path
	}
	if authKind, ok := classifyAuthFailure(detail, credentialAvailable); ok {
		return &Error{
			Kind:        ErrorAuthentication,
			Operation:   operation,
			Detail:      detail,
			AuthKind:    authKind,
			Remediation: remediationFor(authKind),
			Invocation:  invocation,
			cause:       cause,
		}
	}
	return &Error{
		Kind:       ErrorOperationFailed,
		Operation:  operation,
		Detail:     detail,
		Invocation: invocation,
		cause:      cause,
	}
}

func resolveCredential(explicit *Credential, remoteURL *RemoteURL) *Credential {
	if remoteURL == nil || !isHTTPSRemote(string(*remoteURL)) {
		return nil
	}
	if explicit != nil && !explicit.empty() {
		copy := *explicit
		return &copy
	}
	if !isAzureDevOpsURL(string(*remoteURL)) {
		return nil
	}
	for _, name := range []string{"DW_ADO_TOKEN", "AZURE_DEVOPS_EXT_PAT"} {
		if value := os.Getenv(name); strings.TrimSpace(value) != "" {
			credential := NewPersonalAccessToken(contract.NewSecretValue(value))
			return &credential
		}
	}
	return nil
}

func isHTTPSRemote(value string) bool {
	normalized := strings.ToLower(strings.TrimSpace(value))
	return strings.HasPrefix(normalized, "https://") || strings.HasPrefix(normalized, "http://")
}

func isAzureDevOpsURL(value string) bool {
	normalized := strings.ToLower(value)
	return strings.Contains(normalized, "dev.azure.com") || strings.Contains(normalized, "visualstudio.com")
}

func classifyAuthFailure(detail string, credentialAvailable bool) (AuthFailureKind, bool) {
	normalized := strings.ToLower(detail)
	if strings.Contains(normalized, "host key verification failed") || strings.Contains(normalized, "remote host identification has changed") {
		return AuthSSHHostKeyMissing, true
	}
	if strings.Contains(normalized, "permission denied (publickey)") || strings.Contains(normalized, "permission denied (publickey,password)") {
		return AuthSSHKeyUnavailable, true
	}
	for _, fragment := range []string{
		"terminal prompts disabled",
		"could not read username",
		"could not read password",
		"authentication failed",
		"missing git https credential",
		"authentication required",
		"requested url returned error: 401",
		"requested url returned error: 403",
	} {
		if strings.Contains(normalized, fragment) {
			if credentialAvailable {
				return AuthHTTPSCredentialRejected, true
			}
			return AuthHTTPSCredentialMissing, true
		}
	}
	return "", false
}

func remediationFor(kind AuthFailureKind) AuthRemediation {
	switch kind {
	case AuthHTTPSCredentialMissing:
		return RemediationConfigureHTTPSCredential
	case AuthHTTPSCredentialRejected:
		return RemediationVerifyHTTPSCredential
	case AuthSSHHostKeyMissing:
		return RemediationTrustSSHHostKey
	default:
		return RemediationConfigureSSHKey
	}
}

func shouldTrySSHFallback(err error) bool {
	var problem *Error
	if !errors.As(err, &problem) || problem.Kind != ErrorAuthentication {
		return false
	}
	return problem.AuthKind == AuthHTTPSCredentialMissing || problem.AuthKind == AuthHTTPSCredentialRejected
}

func (client Client) ensureRepository(ctx context.Context, repositoryPath RepositoryPath) error {
	_, err := client.run(ctx, OperationOpenRepository, repositoryPath, nil, nil, "rev-parse", "--git-dir")
	return err
}

func (client Client) ensureBareRepository(ctx context.Context, repositoryPath RepositoryPath) error {
	result, err := client.run(ctx, OperationOpenRepository, repositoryPath, nil, nil, "rev-parse", "--is-bare-repository")
	if err != nil {
		return err
	}
	if strings.TrimSpace(string(result.Stdout)) != "true" {
		return client.operationError(OperationOpenRepository, repositoryPath, result, errors.New(l10n.Text("git.not-bare")), false, nil, "")
	}
	return nil
}

func (client Client) repositoryHasChanges(ctx context.Context, repositoryPath RepositoryPath) (bool, error) {
	result, err := client.run(ctx, OperationStatus, repositoryPath, nil, nil, "status", "--porcelain=v1", "-z", "--untracked-files=all")
	return len(result.Stdout) != 0, err
}

func (client Client) cloneBare(ctx context.Context, httpURL RemoteURL, sshURL *RemoteURL, anchorPath RepositoryPath, credential *Credential) error {
	_, err := client.run(ctx, OperationCloneBare, "", credential, &httpURL,
		"clone", "--bare", "--", string(httpURL), string(anchorPath))
	if err == nil {
		return nil
	}
	if !shouldTrySSHFallback(err) || sshURL == nil || strings.TrimSpace(string(*sshURL)) == "" {
		return err
	}
	if removeErr := os.RemoveAll(string(anchorPath)); removeErr != nil {
		return client.operationError(OperationCloneBare, anchorPath, dwprocess.Result{}, removeErr, false, nil, "")
	}
	normalizedSSH := RemoteURL(NormalizeRemoteURL(*sshURL))
	if _, err = client.run(ctx, OperationCloneBare, "", nil, &normalizedSSH,
		"clone", "--bare", "--", string(normalizedSSH), string(anchorPath)); err != nil {
		return err
	}
	return client.configureRemotes(ctx, anchorPath, &httpURL, &normalizedSSH)
}

func (client Client) configureRemotes(ctx context.Context, repositoryPath RepositoryPath, originURL *RemoteURL, sshURL *RemoteURL) error {
	if err := client.ensureRepository(ctx, repositoryPath); err != nil {
		return err
	}
	if originURL != nil {
		normalized := RemoteURL(NormalizeRemoteURL(*originURL))
		if err := client.setRemoteURL(ctx, repositoryPath, "origin", normalized); err != nil {
			return err
		}
	}
	if sshURL != nil && strings.TrimSpace(string(*sshURL)) != "" {
		normalized := RemoteURL(NormalizeRemoteURL(*sshURL))
		if err := client.setRemoteURL(ctx, repositoryPath, fallbackSSHRemote, normalized); err != nil {
			return err
		}
	}
	return nil
}

func (client Client) setRemoteURL(ctx context.Context, repositoryPath RepositoryPath, name string, remoteURL RemoteURL) error {
	if !supportedRemoteURL(remoteURL) {
		cause := errors.New(l10n.Text("git.unsupported-transport"))
		return client.operationError(OperationConfigureRemote, repositoryPath, dwprocess.Result{}, cause, false, nil, "")
	}
	current, err := client.configuredRemoteURL(ctx, repositoryPath, name)
	if err != nil {
		return err
	}
	if current != nil && string(*current) == string(remoteURL) {
		return nil
	}
	if current == nil {
		_, err = client.run(ctx, OperationConfigureRemote, repositoryPath, nil, nil, "remote", "add", "--", name, string(remoteURL))
	} else {
		_, err = client.run(ctx, OperationConfigureRemote, repositoryPath, nil, nil, "remote", "set-url", "--", name, string(remoteURL))
	}
	return err
}

func (client Client) configuredRemoteURL(ctx context.Context, repositoryPath RepositoryPath, name string) (*RemoteURL, error) {
	result, err := client.run(ctx, OperationConfigureRemote, repositoryPath, nil, nil, "remote", "get-url", name)
	if err != nil {
		var operationError *Error
		if errors.As(err, &operationError) {
			var exitError *dwprocess.ExitError
			if errors.As(operationError, &exitError) && exitError.Code == 2 {
				return nil, nil
			}
		}
		return nil, err
	}
	value := strings.TrimSpace(string(result.Stdout))
	if value == "" {
		return nil, nil
	}
	remoteURL := RemoteURL(value)
	return &remoteURL, nil
}

func (client Client) fetchWithFallback(ctx context.Context, repositoryPath RepositoryPath, credential *Credential, sshURL *RemoteURL) error {
	if sshURL != nil {
		if err := client.configureRemotes(ctx, repositoryPath, nil, sshURL); err != nil {
			return err
		}
	}
	originURL, err := client.configuredRemoteURL(ctx, repositoryPath, "origin")
	if err != nil {
		return err
	}
	_, err = client.run(ctx, OperationFetch, repositoryPath, credential, originURL,
		"fetch", "--prune", "origin", anchorFetchRefspec)
	if err == nil {
		return nil
	}
	if !shouldTrySSHFallback(err) || sshURL == nil || strings.TrimSpace(string(*sshURL)) == "" {
		return err
	}
	normalized := RemoteURL(NormalizeRemoteURL(*sshURL))
	_, err = client.run(ctx, OperationFetch, repositoryPath, nil, &normalized,
		"fetch", "--prune", fallbackSSHRemote, anchorFetchRefspec)
	return err
}

func (client Client) referenceExists(ctx context.Context, repositoryPath RepositoryPath, reference string) bool {
	_, err := client.run(ctx, OperationOpenRepository, repositoryPath, nil, nil, "rev-parse", "--verify", "--quiet", "--end-of-options", reference)
	return err == nil
}

func (client Client) referenceObjectID(ctx context.Context, repositoryPath RepositoryPath, reference string) (string, error) {
	result, err := client.run(ctx, OperationOpenRepository, repositoryPath, nil, nil, "show-ref", "--verify", "--hash", reference)
	if err != nil {
		var operationError *Error
		var exitError *dwprocess.ExitError
		if errors.As(err, &operationError) && errors.As(operationError, &exitError) && exitError.Code == 1 {
			return zeroObjectID, nil
		}
		return "", err
	}
	oid := strings.TrimSpace(string(result.Stdout))
	if len(oid) != len(zeroObjectID) {
		return "", client.operationError(OperationOpenRepository, repositoryPath, result, errors.New(l10n.Text("git.invalid-object-id")), false, nil, "")
	}
	return oid, nil
}

func parsePorcelainPaths(output []byte) []string {
	records := strings.Split(string(output), "\x00")
	paths := make([]string, 0, len(records))
	for index := 0; index < len(records); index++ {
		record := records[index]
		if len(record) < 3 {
			continue
		}
		paths = append(paths, record[3:])
		if record[0] == 'R' || record[0] == 'C' || record[1] == 'R' || record[1] == 'C' {
			index++
		}
	}
	if len(paths) == 0 {
		return nil
	}
	return paths
}

func errorDetail(err error) string {
	var problem *Error
	if errors.As(err, &problem) {
		return problem.Detail
	}
	return err.Error()
}

func remoteString(remoteURL *RemoteURL) string {
	if remoteURL == nil {
		return ""
	}
	return string(*remoteURL)
}

func redactRemoteCredentials(detail, remoteURL string) string {
	if remoteURL == "" {
		return detail
	}
	at := strings.LastIndex(remoteURL, "@")
	scheme := strings.Index(remoteURL, "://")
	if at < 0 || scheme < 0 || at < scheme+3 {
		return detail
	}
	userinfo := remoteURL[scheme+3 : at]
	if userinfo == "" {
		return detail
	}
	return strings.ReplaceAll(detail, userinfo, "***")
}

func transliterate(character rune) string {
	switch character {
	case 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å', 'Ā', 'Ă', 'Ą', 'Ǎ', 'à', 'á', 'â', 'ã', 'ä', 'å', 'ā', 'ă', 'ą', 'ǎ':
		return "a"
	case 'Æ', 'æ':
		return "ae"
	case 'Ç', 'Ć', 'Ĉ', 'Ċ', 'Č', 'ç', 'ć', 'ĉ', 'ċ', 'č':
		return "c"
	case 'Ð', 'Ď', 'ð', 'ď':
		return "d"
	case 'È', 'É', 'Ê', 'Ë', 'Ē', 'Ĕ', 'Ė', 'Ę', 'Ě', 'è', 'é', 'ê', 'ë', 'ē', 'ĕ', 'ė', 'ę', 'ě':
		return "e"
	case 'Ĝ', 'Ğ', 'Ġ', 'Ģ', 'ĝ', 'ğ', 'ġ', 'ģ':
		return "g"
	case 'Ĥ', 'Ħ', 'ĥ', 'ħ':
		return "h"
	case 'Ì', 'Í', 'Î', 'Ï', 'Ĩ', 'Ī', 'Ĭ', 'Į', 'İ', 'Ǐ', 'ì', 'í', 'î', 'ï', 'ĩ', 'ī', 'ĭ', 'į', 'ı', 'ǐ':
		return "i"
	case 'Ĵ', 'ĵ':
		return "j"
	case 'Ķ', 'ķ':
		return "k"
	case 'Ĺ', 'Ļ', 'Ľ', 'Ł', 'ĺ', 'ļ', 'ľ', 'ł':
		return "l"
	case 'Ñ', 'Ń', 'Ņ', 'Ň', 'ñ', 'ń', 'ņ', 'ň':
		return "n"
	case 'Ò', 'Ó', 'Ô', 'Õ', 'Ö', 'Ø', 'Ō', 'Ŏ', 'Ő', 'Ǒ', 'ò', 'ó', 'ô', 'õ', 'ö', 'ø', 'ō', 'ŏ', 'ő', 'ǒ':
		return "o"
	case 'Œ', 'œ':
		return "oe"
	case 'Ŕ', 'Ŗ', 'Ř', 'ŕ', 'ŗ', 'ř':
		return "r"
	case 'Ś', 'Ŝ', 'Ş', 'Š', 'Ș', 'ś', 'ŝ', 'ş', 'š', 'ș':
		return "s"
	case 'ß':
		return "ss"
	case 'Ť', 'Ţ', 'Ț', 'ť', 'ţ', 'ț':
		return "t"
	case 'Þ', 'þ':
		return "th"
	case 'Ù', 'Ú', 'Û', 'Ü', 'Ũ', 'Ū', 'Ŭ', 'Ů', 'Ű', 'Ų', 'Ǔ', 'ù', 'ú', 'û', 'ü', 'ũ', 'ū', 'ŭ', 'ů', 'ű', 'ų', 'ǔ':
		return "u"
	case 'Ŵ', 'ŵ':
		return "w"
	case 'Ý', 'Ŷ', 'Ÿ', 'ý', 'ŷ', 'ÿ':
		return "y"
	case 'Ź', 'Ż', 'Ž', 'ź', 'ż', 'ž':
		return "z"
	default:
		return string(character)
	}
}
