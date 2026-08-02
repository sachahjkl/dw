package controller

func workItemListRoute() Route {
	return Route{Key: "work.item.list", Machine: jsonMachine, Build: buildWorkItemList, Project: assignedProject}
}
