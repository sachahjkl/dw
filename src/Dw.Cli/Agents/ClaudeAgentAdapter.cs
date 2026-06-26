namespace Dw.Cli.Agents;

internal sealed class ClaudeAgentAdapter : IAgentAdapter
{
    public string Name => "claude";

    public AgentLaunch BuildOpenLaunch(AgentOpenRequest request)
        => new(
            "claude",
            request.Continue ? ["--continue"] : [],
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase),
            request.Workspace);

    public IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        yield return new AgentWorkspaceConfigFile("CLAUDE.md", Templates.WorkspaceClaudeMd(request.WorkItems, request.Project));
        yield return new AgentWorkspaceConfigFile(Path.Combine(".claude", "CLAUDE.md"), Templates.WorkspaceClaudeMd(request.WorkItems, request.Project));
    }
}
