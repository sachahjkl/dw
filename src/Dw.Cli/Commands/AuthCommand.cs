namespace Dw.Cli.Commands;

internal static class AuthCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "login" => Login(context, args.Skip(1).ToArray()),
            "status" => Status(context, args.Skip(1).ToArray()),
            "logout" => Logout(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Help(CommandContext context)
    {
        context.Out.WriteLine("Usage: dw auth <login|status|logout> [--root <path>]");
        return 0;
    }

    private static int Login(CommandContext context, string[] args)
    {
        var root = CommandOptions.ResolveRoot(context, args);
        var workflow = WorkflowConfigLoader.Load(context.FileSystem, root);
        var provider = new AzureDevOpsTokenProvider(workflow.Auth);
        var token = provider.GetTokenInteractiveAsync().GetAwaiter().GetResult();

        context.Out.WriteLine($"Connecte via {token.Source}.");
        context.Out.WriteLine(token.ExpiresOn is null
            ? "Expiration inconnue."
            : $"Expire le {token.ExpiresOn:O}.");
        return 0;
    }

    private static int Status(CommandContext context, string[] args)
    {
        var root = CommandOptions.ResolveRoot(context, args);
        var workflow = WorkflowConfigLoader.Load(context.FileSystem, root);
        var provider = new AzureDevOpsTokenProvider(workflow.Auth);
        var token = provider.GetTokenSilentOrEnvironmentAsync().GetAwaiter().GetResult();

        if (token is null)
        {
            context.Out.WriteLine("Non connecte.");
            context.Out.WriteLine("Executer dw auth login ou definir DW_ADO_TOKEN.");
            return 1;
        }

        context.Out.WriteLine($"Connecte via {token.Source}.");
        context.Out.WriteLine(token.ExpiresOn is null
            ? "Expiration inconnue."
            : $"Expire le {token.ExpiresOn:O}.");
        return 0;
    }

    private static int Logout(CommandContext context, string[] args)
    {
        var root = CommandOptions.ResolveRoot(context, args);
        var workflow = WorkflowConfigLoader.Load(context.FileSystem, root);
        var provider = new AzureDevOpsTokenProvider(workflow.Auth);
        var removed = provider.LogoutAsync().GetAwaiter().GetResult();

        context.Out.WriteLine($"Sessions MSAL supprimees: {removed}.");
        context.Out.WriteLine("Les PAT definis via DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent geres par l'environnement.");
        return 0;
    }
}
