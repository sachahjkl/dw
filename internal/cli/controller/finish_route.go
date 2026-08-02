package controller

import "github.com/sachahjkl/dw/internal/l10n"

func finishRoute() Route {
	return Route{Key: "workspace.finish", Machine: jsonMachine, Build: buildWorkspaceFinish, Project: workspacePhaseProject}
}

const (
	promptFinishMode  l10n.ID = "cli.prompt.finish-mode"
	promptFinishPush  l10n.ID = "cli.prompt.finish-push"
	promptFinishDraft l10n.ID = "cli.prompt.finish-draft"
	promptFinishReady l10n.ID = "cli.prompt.finish-ready"
	promptFinishKeep  l10n.ID = "cli.prompt.finish-keep"
)
