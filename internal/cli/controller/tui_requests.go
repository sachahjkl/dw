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
	case "ado.assigned":
		return buildADOAssigned(invocation)
	case "ado.state.set":
		return buildADOStateSet(invocation)
	case "work.item.doing":
		root := resolvedRoot(invocation.Values)
		states, _, _ := taskStartSettings(root)
		return workapp.DoingRequest{Root: root, Project: invocation.Values.String("project"), IDs: split(invocation.Values.String("id")), States: states}, nil
	case "work.start":
		return buildWorkStart(invocation)
	case "work.finish":
		return buildWorkFinish(invocation)
	case "work.teardown":
		return buildWorkTeardown(invocation)
	case "work.prune":
		return buildWorkPrune(invocation)
	default:
		return nil, fmt.Errorf("cli.tui-direct-route-unavailable:%s", invocation.Command.Key)
	}
}
