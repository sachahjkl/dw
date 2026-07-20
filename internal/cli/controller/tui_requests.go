package controller

import (
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/workapp"
)

// BuildDirectRequest converts a parsed TUI form targeting a CLI-direct phased
// route into the concrete action request. The TUI owns confirmation and result
// sequencing for these requests.
func BuildDirectRequest(invocation *parse.Result) (action.Request, error) {
	if invocation == nil || invocation.Command == nil {
		return nil, fmt.Errorf("cli.invalid-tui-invocation")
	}
	switch invocation.Command.Key {
	case "work.item.list":
		return buildWorkItemList(invocation)
	case "work.item.state.set":
		return buildWorkItemStateSet(invocation)
	case "work.item.doing":
		root := resolvedRoot(invocation.Values)
		states, _, _ := taskStartSettings(root)
		project := invocation.Values.String("project")
		return workapp.DoingRequest{Provider: selectedWorkProvider(invocation.Values, root, project), Root: root, Project: project, IDs: split(invocation.Values.String("id")), States: states}, nil
	case "workspace.start":
		return buildWorkspaceStart(invocation)
	case "workspace.finish":
		return buildWorkspaceFinish(invocation)
	case "workspace.teardown":
		return buildWorkspaceTeardown(invocation)
	case "workspace.prune":
		return buildWorkspacePrune(invocation)
	default:
		return nil, fmt.Errorf("cli.tui-direct-route-unavailable:%s", invocation.Command.Key)
	}
}
