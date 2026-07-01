namespace Dw.Cli.Commands;

internal static class AuthCommand
{
    internal static int Login(CommandContext context, string? configuredRoot)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var provider = new AzureDevOpsTokenProvider(workflow.Auth);
        var token = provider.GetTokenInteractiveAsync().GetAwaiter().GetResult();

        context.Out.WriteLine($"Connecte via {token.Source}.");
        context.Out.WriteLine(token.ExpiresOn is null
            ? "Expiration inconnue."
            : $"Expire le {token.ExpiresOn:O}.");
        return 0;
    }

    internal static int Status(CommandContext context, string? configuredRoot)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
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

    internal static int Logout(CommandContext context, string? configuredRoot)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var provider = new AzureDevOpsTokenProvider(workflow.Auth);
        var removed = provider.LogoutAsync().GetAwaiter().GetResult();

        context.Out.WriteLine($"Sessions MSAL supprimees: {removed}.");
        context.Out.WriteLine("Les PAT definis via DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent geres par l'environnement.");
        return 0;
    }
}
