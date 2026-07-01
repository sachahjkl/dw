namespace Dw.Cli.AzureDevOps;

internal static class AzureDevOpsDefaults
{
    public const string TenantId = "organizations";
    public const string ResourceId = "499b84ac-1321-427f-aa17-267ca6975798";
    public const string PublicClientId = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";

    public static readonly string[] Scopes = [$"{ResourceId}/.default"];
}
