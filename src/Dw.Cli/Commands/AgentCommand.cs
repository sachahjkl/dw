using Dw.Cli.Agents;

namespace Dw.Cli.Commands;

internal static class AgentCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        if (args.Length == 0 || args[0] is "-h" or "--help")
        {
            context.Out.WriteLine("Usage: dw agent <context|open>");
            return 0;
        }

        return args[0].ToLowerInvariant() switch
        {
            "context" => WriteContext(context),
            "open" => Open(context, args.Skip(1).ToArray()),
            "config" => Config(context, args.Skip(1).ToArray()),
            "doctor" => Doctor(context, args.Skip(1).ToArray()),
            _ => throw new DwException($"Sous-commande agent inconnue: {args[0]}", 2)
        };
    }

    private static int WriteContext(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        context.Out.WriteLine(Templates.AgentContext(root));
        return 0;
    }

    private static int Open(CommandContext context, string[] args)
        => WorkspaceOpenService.Open(context, OpenOptions(args));

    private static int Config(CommandContext context, string[] args)
    {
        var root = CommandOptions.ResolveRoot(context, args);
        var sub = args.FirstOrDefault(arg => !arg.StartsWith("--", StringComparison.OrdinalIgnoreCase))?.ToLowerInvariant();
        if (sub is null or "show")
        {
            var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
            context.Out.WriteLine($"Agent par defaut: {workflow.Agent?.Default ?? AgentDefaults.DefaultAgent}");
            return 0;
        }

        if (sub == "set-default")
        {
            var agent = args.SkipWhile(arg => !string.Equals(arg, "set-default", StringComparison.OrdinalIgnoreCase)).Skip(1).FirstOrDefault();
            if (string.IsNullOrWhiteSpace(agent))
            {
                throw new DwException("Usage: dw agent config set-default <agent>", 2);
            }

            WorkflowConfigStore.SetDefaultAgent(context.FileSystem, root, agent);
            context.Out.WriteLine($"Agent par defaut: {agent}");
            return 0;
        }

        throw new DwException("Usage: dw agent config [show|set-default <agent>]", 2);
    }

    private static int Doctor(CommandContext context, string[] args)
    {
        var requested = CommandOptions.OptionValue(args, "--agent");
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

    internal static WorkspaceOpenOptions OpenOptions(string[] args)
        => new(
            Workspace: CommandOptions.OptionValue(args, "--workspace"),
            Project: CommandOptions.OptionValue(args, "--project"),
            WorkItemId: CommandOptions.OptionValue(args, "--work-item"),
            Continue: CommandOptions.HasFlag(args, "--continue"),
            Agent: CommandOptions.OptionValue(args, "--agent"),
            Repository: CommandOptions.OptionValue(args, "--repo"));
}
