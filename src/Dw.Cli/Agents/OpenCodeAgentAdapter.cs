namespace Dw.Cli.Agents;

internal sealed class OpenCodeAgentAdapter : IAgentAdapter
{
    public string Name => AgentDefaults.DefaultAgent;

    public AgentLaunch BuildOpenLaunch(AgentOpenRequest request)
    {
        var opencodeConfig = Path.Combine(request.Root, "config", "opencode", "opencode.jsonc");
        var arguments = request.Continue
            ? new[] { "-c", request.Workspace }
            : [request.Workspace];

        return new AgentLaunch(
            "opencode",
            arguments,
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                ["OPENCODE_CONFIG"] = opencodeConfig
            },
            request.Workspace);
    }

    public IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        yield return new AgentWorkspaceConfigFile("AGENTS.md", Templates.WorkspaceAgentsMd(request.WorkItemId, request.Project));
    }
}
