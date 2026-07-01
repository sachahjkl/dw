namespace Dw.Cli.AzureDevOps;

internal sealed record AdoClientInputs(WorkflowConfig Workflow, AzureDevOpsOptions AzureDevOps, TokenResult Token);

internal static class AdoClientFactory
{
    public static AdoClientInputs CreateInputs(CommandContext context, string? configuredRoot, string? projectName)
    {
        var root = RootResolver.Resolve(context, configuredRoot);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = string.IsNullOrWhiteSpace(projectName)
            ? null
            : DevWorkflowConfigLoader.ResolveProject(projects, projectName);
        var azureDevOps = ResolveAzureDevOpsOptions(workflow, projectConfig);
        if (azureDevOps is null)
        {
            throw new DwException("Configuration azureDevOps manquante dans workflow.json.");
        }

        var token = new AzureDevOpsTokenProvider(workflow.Auth)
            .GetTokenSilentOrEnvironmentAsync()
            .GetAwaiter()
            .GetResult()
            ?? throw new DwException("Non connecte a Azure DevOps. Executer dw auth login ou definir DW_ADO_TOKEN.");

        return new AdoClientInputs(workflow, azureDevOps, token);
    }

    private static AzureDevOpsOptions? ResolveAzureDevOpsOptions(WorkflowConfig workflow, ProjectConfig? projectConfig)
    {
        if (projectConfig?.IncludedProjects is { Length: > 0 } && projectConfig.AzureDevOps is null)
        {
            return null;
        }

        if (projectConfig?.AzureDevOps is null)
        {
            return workflow.AzureDevOps;
        }

        if (workflow.AzureDevOps is null)
        {
            return projectConfig.AzureDevOps;
        }

        return new AzureDevOpsOptions(
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.OrganizationUrl)
                ? workflow.AzureDevOps.OrganizationUrl
                : projectConfig.AzureDevOps.OrganizationUrl,
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.Project)
                ? workflow.AzureDevOps.Project
                : projectConfig.AzureDevOps.Project,
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.ApiVersion)
                ? workflow.AzureDevOps.ApiVersion
                : projectConfig.AzureDevOps.ApiVersion);
    }
}
