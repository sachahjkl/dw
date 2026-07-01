namespace Dw.Cli.Agents;

internal sealed record AgentOpenRequest(
    string Root,
    string Workspace,
    bool Continue);

internal sealed record AgentLaunch(
    string FileName,
    IReadOnlyList<string> Arguments,
    IReadOnlyDictionary<string, string> Environment,
    string WorkingDirectory);

internal sealed record AgentWorkspaceConfigRequest(string Workspace, IReadOnlyList<WorkspaceWorkItem> WorkItems, string Project);

internal sealed record AgentWorkspaceConfigFile(string RelativePath, string Content);

internal interface IAgentAdapter
{
    string Name { get; }
    IReadOnlyList<string> Aliases => [];
    AgentLaunch BuildOpenLaunch(AgentOpenRequest request);
    IEnumerable<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request) => [];
}

internal static class AgentAdapterRegistry
{
    private static readonly IReadOnlyList<IAgentAdapter> RegisteredAdapters = new IAgentAdapter[]
    {
        new OpenCodeAgentAdapter(),
        new CursorAgentAdapter(),
        new ClaudeAgentAdapter(),
        new CodexCliAgentAdapter(),
        new CopilotAgentAdapter()
    };

    private static readonly IReadOnlyDictionary<string, IAgentAdapter> Adapters = RegisteredAdapters
        .SelectMany(adapter => new[] { adapter.Name }.Concat(adapter.Aliases)
            .Select(name => new KeyValuePair<string, IAgentAdapter>(name, adapter)))
        .ToDictionary(pair => pair.Key, pair => pair.Value, StringComparer.OrdinalIgnoreCase);

    public static IReadOnlyList<IAgentAdapter> All => RegisteredAdapters;

    public static IReadOnlyList<AgentWorkspaceConfigFile> WorkspaceConfigFiles(AgentWorkspaceConfigRequest request)
    {
        var files = new Dictionary<string, AgentWorkspaceConfigFile>(StringComparer.OrdinalIgnoreCase);
        foreach (var adapter in RegisteredAdapters)
        {
            foreach (var file in adapter.WorkspaceConfigFiles(request))
            {
                files.TryAdd(file.RelativePath, file);
            }
        }

        return files.Values.ToArray();
    }

    public static IAgentAdapter Resolve(string? agent)
    {
        var name = string.IsNullOrWhiteSpace(agent) ? AgentDefaults.DefaultAgent : agent;
        if (Adapters.TryGetValue(name, out var adapter))
        {
            return adapter;
        }

        throw new DwException($"Agent inconnu: {name}. Agents disponibles: {string.Join(", ", RegisteredAdapters.Select(adapter => adapter.Name))}", 2);
    }
}
