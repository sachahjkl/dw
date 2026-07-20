//go:build !windows

package process

import (
	"context"
	"os/exec"
)

func appendPlatformCandidates(candidates []ResolvedCommand, _ string, _ []string) []ResolvedCommand {
	return candidates
}

func prepareCandidate(candidate ResolvedCommand) (ResolvedCommand, error) { return candidate, nil }

func executableCommand(ctx context.Context, candidate ResolvedCommand) *exec.Cmd {
	return exec.CommandContext(ctx, candidate.FileName, candidate.Arguments...)
}

func environmentNameEqual(left, right string) bool { return left == right }
