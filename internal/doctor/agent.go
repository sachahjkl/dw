package doctor

import (
	"context"

	"github.com/sachahjkl/dw/internal/contract"
)

var canonicalAgents = [...]contract.Agent{
	contract.AgentOpenCode,
	contract.AgentCursor,
	contract.AgentClaude,
	contract.AgentCodex,
	contract.AgentCodexCLI,
	contract.AgentCopilot,
}

type AgentReport struct {
	Checks []AgentCheck `json:"checks"`
}

type AgentCheck struct {
	Agent     contract.Agent `json:"agent"`
	Command   string         `json:"command"`
	Available bool           `json:"available"`
}

func (report AgentReport) AvailableCount() int {
	available := 0
	for _, check := range report.Checks {
		if check.Available {
			available++
		}
	}
	return available
}

func (report AgentReport) TotalCount() int { return len(report.Checks) }
func (report AgentReport) Passed() bool    { return report.AvailableCount() == report.TotalCount() }
func (report AgentReport) Status() Status {
	if report.Passed() {
		return StatusHealthy
	}
	return StatusNeedsFixes
}
func (report AgentReport) ExitCode() int {
	if report.Passed() {
		return 0
	}
	return 1
}

// RunAgents checks one requested agent or the six canonical agents in compatibility order.
func RunAgents(ctx context.Context, process Process, requested *contract.Agent) AgentReport {
	agents := canonicalAgents[:]
	if requested != nil {
		agents = []contract.Agent{*requested}
	}
	checks := make([]AgentCheck, 0, len(agents))
	for _, agent := range agents {
		command := agentExecutable(agent)
		output, err := process.Output(ctx, command, "--help")
		checks = append(checks, AgentCheck{
			Agent: agent, Command: command, Available: err == nil && output.ExitCode == 0,
		})
	}
	return AgentReport{Checks: checks}
}

func agentExecutable(agent contract.Agent) string {
	switch agent {
	case contract.AgentOpenCode:
		return "opencode"
	case contract.AgentCursor, contract.AgentCursorAgent, contract.AgentGeneric:
		return "agent"
	case contract.AgentClaude:
		return "claude"
	case contract.AgentCodex, contract.AgentCodexCLI:
		return "codex"
	case contract.AgentCopilot:
		return "copilot"
	default:
		return string(agent)
	}
}
