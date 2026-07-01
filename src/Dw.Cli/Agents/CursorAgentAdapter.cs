using Dw.Cli.Cli;

namespace Dw.Cli.Agents;

internal sealed class CursorAgentAdapter : IAgentAdapter
{
    public string Name => "cursor";

    public IReadOnlyList<string> Aliases => ["cursor-agent", "agent"];

    public AgentLaunch BuildOpenLaunch(AgentOpenRequest request)
        => new(
            "agent",
            request.Continue ? [OptionNames.Workspace, request.Workspace, OptionNames.Continue] : [OptionNames.Workspace, request.Workspace],
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase),
            request.Workspace);

    public IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        yield return new AgentWorkspaceConfigFile(Path.Combine(".cursor", "rules", "devworkflow.mdc"), Templates.WorkspaceCursorRule(request.WorkItems, request.Project));
    }
}
