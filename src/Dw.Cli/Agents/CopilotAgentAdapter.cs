namespace Dw.Cli.Agents;

internal sealed class CopilotAgentAdapter : IAgentAdapter
{
    public string Name => "copilot";

    public AgentLaunch BuildOpenLaunch(AgentOpenRequest request)
        => new(
            "copilot",
            request.Continue ? ["--continue"] : [],
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase),
            request.Workspace);

    public IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        yield return new AgentWorkspaceConfigFile(Path.Combine(".github", "copilot-instructions.md"), Templates.WorkspaceCopilotInstructions(request.WorkItems, request.Project));
    }
}
