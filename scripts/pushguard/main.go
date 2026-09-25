// Command pushguard is a pre-push hook that refuses to push commits whose messages,
// file names or added lines match a local denylist. It only guards the remotes listed
// in `git config --get-all pushguard.remote`; the denylist (one case-insensitive
// regular expression per line, `#` comments) lives outside the repository, by default
// in `$GIT_COMMON_DIR/pushguard-denylist`, or at `git config pushguard.denylist`.
package main

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
)

const zeroObjectID = "0000000000000000000000000000000000000000"

type pushedRef struct {
	localRef, localSHA, remoteSHA string
}

func main() {
	remote, refs, err := pushContext(os.Args[1:], os.Stdin)
	if err == nil {
		err = run(remote, refs)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "pushguard:", err)
		os.Exit(1)
	}
}

func pushContext(arguments []string, stdin io.Reader) (string, []pushedRef, error) {
	if remote := os.Getenv("PRE_COMMIT_REMOTE_NAME"); remote != "" {
		to := os.Getenv("PRE_COMMIT_TO_REF")
		if to == "" {
			return "", nil, fmt.Errorf("PRE_COMMIT_TO_REF is empty")
		}
		return remote, []pushedRef{{localRef: to, localSHA: to, remoteSHA: os.Getenv("PRE_COMMIT_FROM_REF")}}, nil
	}
	if len(arguments) == 0 {
		return "", nil, fmt.Errorf("usage: pushguard <remote> [url] < refs")
	}
	var refs []pushedRef
	scanner := bufio.NewScanner(stdin)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 4 {
			refs = append(refs, pushedRef{localRef: fields[0], localSHA: fields[1], remoteSHA: fields[3]})
		}
	}
	return arguments[0], refs, scanner.Err()
}

func run(remote string, refs []pushedRef) error {
	guarded, _ := git("config", "--get-all", "pushguard.remote")
	if !slices.Contains(strings.Fields(guarded), remote) {
		return nil
	}
	patterns, err := loadDenylist()
	if err != nil {
		return err
	}
	var findings []string
	for _, ref := range refs {
		if ref.localSHA == "" || ref.localSHA == zeroObjectID {
			continue
		}
		commits, err := commitsToPush(remote, ref)
		if err != nil {
			return err
		}
		for _, commit := range commits {
			found, err := scanCommit(commit, patterns)
			if err != nil {
				return err
			}
			findings = append(findings, found...)
		}
	}
	if len(findings) != 0 {
		return fmt.Errorf("push to %q refused, denylisted content found:\n  %s", remote, strings.Join(findings, "\n  "))
	}
	return nil
}

func loadDenylist() ([]*regexp.Regexp, error) {
	path, _ := git("config", "--get", "pushguard.denylist")
	if path = strings.TrimSpace(path); path == "" {
		commonDir, err := git("rev-parse", "--path-format=absolute", "--git-common-dir")
		if err != nil {
			return nil, err
		}
		path = filepath.Join(strings.TrimSpace(commonDir), "pushguard-denylist")
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("guarded remote but denylist unreadable (%s): %w", path, err)
	}
	var patterns []*regexp.Regexp
	for _, line := range strings.Split(string(contents), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		pattern, err := regexp.Compile("(?i)" + line)
		if err != nil {
			return nil, fmt.Errorf("invalid denylist pattern %q: %w", line, err)
		}
		patterns = append(patterns, pattern)
	}
	if len(patterns) == 0 {
		return nil, fmt.Errorf("guarded remote but denylist %s is empty", path)
	}
	return patterns, nil
}

func commitsToPush(remote string, ref pushedRef) ([]string, error) {
	arguments := []string{"rev-list", ref.localSHA}
	if ref.remoteSHA != "" && ref.remoteSHA != zeroObjectID && objectExists(ref.remoteSHA) {
		arguments = append(arguments, "^"+ref.remoteSHA)
	}
	arguments = append(arguments, "--not", "--remotes="+remote)
	output, err := git(arguments...)
	if err != nil {
		return nil, err
	}
	return strings.Fields(output), nil
}

func scanCommit(commit string, patterns []*regexp.Regexp) ([]string, error) {
	message, err := git("log", "-1", "--format=%B", commit)
	if err != nil {
		return nil, err
	}
	diff, err := git("show", "--format=", "--unified=0", "--no-color", "--no-ext-diff", "--text", commit)
	if err != nil {
		return nil, err
	}
	short := commit[:min(len(commit), 10)]
	var findings []string
	report := func(where, text string) {
		for _, pattern := range patterns {
			if pattern.MatchString(text) {
				findings = append(findings, fmt.Sprintf("%s %s: %q matches /%s/", short, where, truncate(text), pattern.String()[4:]))
				return
			}
		}
	}
	report("message", message)
	file := ""
	for _, line := range strings.Split(diff, "\n") {
		switch {
		case strings.HasPrefix(line, "+++ "):
			file = strings.TrimPrefix(strings.TrimPrefix(line, "+++ "), "b/")
			report("path", file)
		case strings.HasPrefix(line, "+"):
			report(file, strings.TrimPrefix(line, "+"))
		}
	}
	return findings, nil
}

func truncate(text string) string {
	text = strings.TrimSpace(text)
	if len(text) > 120 {
		return text[:120] + "…"
	}
	return text
}

func objectExists(sha string) bool {
	_, err := git("cat-file", "-e", sha+"^{commit}")
	return err == nil
}

func git(arguments ...string) (string, error) {
	var stdout, stderr bytes.Buffer
	command := exec.Command("git", arguments...)
	command.Stdout = &stdout
	command.Stderr = &stderr
	if err := command.Run(); err != nil {
		return stdout.String(), fmt.Errorf("git %s: %w: %s", strings.Join(arguments, " "), err, strings.TrimSpace(stderr.String()))
	}
	return stdout.String(), nil
}
