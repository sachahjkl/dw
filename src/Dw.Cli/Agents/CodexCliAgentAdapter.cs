namespace Dw.Cli.Agents;

internal sealed class CodexCliAgentAdapter : IAgentAdapter
{
    public string Name => "codex-cli";

    public IReadOnlyList<string> Aliases => ["codex"];

    public AgentLaunch BuildOpenLaunch(AgentOpenRequest request)
        => new(
            "codex",
            request.Continue ? ["resume", "--last", "--cd", request.Workspace] : ["--cd", request.Workspace],
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase),
            request.Workspace);

    public IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        yield return new AgentWorkspaceConfigFile("AGENTS.md", Templates.WorkspaceAgentsMd(request.WorkItemId, request.Project));
        yield return new AgentWorkspaceConfigFile(Path.Combine(".codex", "config.toml"), Templates.WorkspaceCodexConfig);
    }
}
