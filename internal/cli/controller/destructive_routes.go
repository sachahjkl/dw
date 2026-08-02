package controller

func teardownRoute() Route {
	return Route{Key: "workspace.teardown", Machine: jsonMachine, Build: buildWorkspaceTeardown, Project: workspacePhaseProject}
}

func pruneRoute() Route {
	return Route{Key: "workspace.prune", Machine: jsonMachine, Build: buildWorkspacePrune, Project: workspacePhaseProject}
}
