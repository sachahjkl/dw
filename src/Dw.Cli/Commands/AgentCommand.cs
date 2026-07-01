using Dw.Cli.Agents;

namespace Dw.Cli.Commands;

internal static class AgentCommand
{
    internal static int WriteContext(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        context.Out.WriteLine(Templates.AgentContext(root));
        return 0;
    }

    internal static int ShowConfig(CommandContext context, string? configuredRoot)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        context.Out.WriteLine($"Agent par defaut: {workflow.Agent?.Default ?? AgentDefaults.DefaultAgent}");
        return 0;
    }

    internal static int SetDefaultAgent(CommandContext context, string? configuredRoot, string agent)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        WorkflowConfigStore.SetDefaultAgent(context.FileSystem, root, agent);
        context.Out.WriteLine($"Agent par defaut: {agent}");
        return 0;
    }

    internal static int Doctor(CommandContext context, string? requested)
    {
        var agents = string.IsNullOrWhiteSpace(requested)
            ? AgentAdapterRegistry.All
            : [AgentAdapterRegistry.Resolve(requested)];
        context.Out.WriteLine("Agent      Command    Status");
        foreach (var agent in agents)
        {
            var launch = agent.BuildOpenLaunch(new AgentOpenRequest(AppPaths.DefaultRoot, Environment.CurrentDirectory, Continue: false));
            var status = IsAvailable(context, launch.FileName) ? "OK" : "missing";
            context.Out.WriteLine($"{agent.Name,-10} {launch.FileName,-10} {status}");
        }

        return 0;
    }

    private static bool IsAvailable(CommandContext context, string fileName)
    {
        try
        {
            var result = context.ProcessRunner.RunAsync(fileName, ["--help"], Environment.CurrentDirectory).GetAwaiter().GetResult();
            return result.ExitCode == 0;
        }
        catch
        {
            return false;
        }
    }
}
