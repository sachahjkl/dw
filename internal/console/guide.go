package console

import (
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

type GuideResult struct{ Version string }

type GuideStep struct {
	Title    MessageID
	Commands []string
	Detail   MessageID
}

var guideSteps = []GuideStep{
	{"guide.step.installation", []string{"dw version", "dw doctor"}, "guide.step.installation.detail"},
	{"guide.step.initialize", []string{"dw init", "dw config show", "dw init --root ~/dev/dw"}, "guide.step.initialize.detail"},
	{"guide.step.ado", []string{"dw provider auth login azure-devops", "dw provider auth status azure-devops", "dw work item list --project <project>"}, "guide.step.ado.detail"},
	{"guide.step.workspace", []string{"dw workspace start <work-item-id>", "dw workspace start <work-item-id> --execute", "dw workspace open --continue"}, "guide.step.workspace.detail"},
	{"guide.step.daily", []string{"dw workspace status", "dw workspace list", "dw workspace current", "dw workspace preflight --continue", "dw workspace sync --continue"}, "guide.step.daily.detail"},
	{"guide.step.contents", []string{"dw workspace item add --continue", "dw workspace item remove --continue", "dw workspace repo add <repo>", "dw workspace repo latest --continue"}, "guide.step.contents.detail"},
	{"guide.step.complete", []string{"dw workspace handoff validate --continue", "dw workspace commit --continue", "dw workspace finish --continue", "dw workspace finish --continue --execute"}, "guide.step.complete.detail"},
	{"guide.step.cleanup", []string{"dw workspace teardown --continue", "dw workspace prune"}, "guide.step.cleanup.detail"},
	{"guide.step.tools", []string{"dw work item show <id>", "dw work context show <id>", "dw work changelog <ids>", "dw data catalog", "dw data describe <resource>", "dw data query --query \"select top 20 * from ...\"", "dw agent context"}, "guide.step.tools.detail"},
	{"guide.step.completion", []string{"dw completion show", "dw completion install fish", "dw completion install powershell"}, "guide.step.completion.detail"},
}

func RenderVersion(version string, localizer Localizer, theme Theme) Output {
	localizer = WithConsoleMessages(localizer)
	return TextOutput(FormatHuman, theme.Title(localize(localizer, "build.product")+" "+version))
}

func RenderGuide(result GuideResult, localizer Localizer, theme Theme) string {
	localizer = WithConsoleMessages(localizer)
	blocks := []string{
		theme.Title(localize(localizer, "guide.title", l10n.A("version", result.Version))),
		localize(localizer, "guide.subtitle"),
	}
	for index, step := range guideSteps {
		lines := []string{theme.Label(localize(localizer, "guide.step.numbered", l10n.A("number", index+1), l10n.A("title", localize(localizer, step.Title))))}
		for _, command := range step.Commands {
			lines = append(lines, "  "+theme.Command(command))
		}
		lines = append(lines, "  "+localize(localizer, step.Detail))
		blocks = append(blocks, strings.Join(lines, "\n"))
	}
	blocks = append(blocks, theme.Panel(strings.Join([]string{
		theme.Label(localize(localizer, "guide.diagnostics")),
		"  " + theme.Command("dw doctor --fix"),
		"  " + theme.Command("dw config doctor"),
		"  " + theme.Command("dw refresh"),
		"  " + localize(localizer, "guide.diagnostics.detail"),
	}, "\n")))
	return strings.Join(blocks, "\n\n")
}
