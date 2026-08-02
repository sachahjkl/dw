package controller

import "github.com/sachahjkl/dw/internal/l10n"

func startRoute() Route {
	return Route{Key: "workspace.start", Machine: jsonMachine, Build: buildWorkspaceStart, Project: workspacePhaseProject}
}

const (
	promptStartCreate l10n.ID = "cli.prompt.start-create"
	promptStartOpen   l10n.ID = "cli.prompt.start-open"
)
