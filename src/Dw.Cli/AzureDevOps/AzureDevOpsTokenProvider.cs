using Microsoft.Identity.Client;
using Microsoft.Identity.Client.Extensions.Msal;

namespace Dw.Cli.AzureDevOps;

internal sealed class AzureDevOpsTokenProvider(AuthOptions? authOptions)
{
    public async Task<TokenResult> GetTokenInteractiveAsync(CancellationToken cancellationToken = default)
    {
        var configured = RequireConfiguredAuth();
        var app = BuildApplication(configured);
        var scopes = configured.Scopes.Length > 0 ? configured.Scopes : AzureDevOpsDefaults.Scopes;

        var result = await app.AcquireTokenInteractive(scopes)
            .WithUseEmbeddedWebView(false)
            .ExecuteAsync(cancellationToken);

        return new TokenResult(result.AccessToken, result.ExpiresOn, "MSAL interactive", AzureDevOpsAuthenticationScheme.Bearer);
    }

    public async Task<TokenResult?> GetTokenSilentOrEnvironmentAsync(CancellationToken cancellationToken = default)
    {
        var env = Environment.GetEnvironmentVariable("DW_ADO_TOKEN")
                  ?? Environment.GetEnvironmentVariable("AZURE_DEVOPS_EXT_PAT");

        if (!string.IsNullOrWhiteSpace(env))
        {
            return new TokenResult(env, null, "environment PAT", AzureDevOpsAuthenticationScheme.Basic);
        }

        if (authOptions is null)
        {
            return null;
        }

        var app = BuildApplication(authOptions);
        var account = (await app.GetAccountsAsync()).FirstOrDefault();
        if (account is null)
        {
            return null;
        }

        try
        {
            var scopes = authOptions.Scopes.Length > 0 ? authOptions.Scopes : AzureDevOpsDefaults.Scopes;
            var result = await app.AcquireTokenSilent(scopes, account).ExecuteAsync(cancellationToken);
            return new TokenResult(result.AccessToken, result.ExpiresOn, "MSAL cache", AzureDevOpsAuthenticationScheme.Bearer);
        }
        catch (MsalUiRequiredException)
        {
            return null;
        }
    }

    public async Task<int> LogoutAsync(CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(authOptions?.ClientId))
        {
            return 0;
        }

        var app = BuildApplication(authOptions);
        var accounts = (await app.GetAccountsAsync()).ToArray();
        foreach (var account in accounts)
        {
            cancellationToken.ThrowIfCancellationRequested();
            await app.RemoveAsync(account);
        }

        return accounts.Length;
    }

    private AuthOptions RequireConfiguredAuth()
    {
        if (authOptions is null)
        {
            throw new DwException("Auth ADO non configuree. Renseigner auth dans workflow.json ou utiliser DW_ADO_TOKEN.");
        }

        return authOptions;
    }

    private static IPublicClientApplication BuildApplication(AuthOptions options)
    {
        var tenant = string.IsNullOrWhiteSpace(options.TenantId) ? AzureDevOpsDefaults.TenantId : options.TenantId;
        var clientId = string.IsNullOrWhiteSpace(options.ClientId) ? AzureDevOpsDefaults.PublicClientId : options.ClientId;
        var app = PublicClientApplicationBuilder
            .Create(clientId)
            .WithAuthority(AzureCloudInstance.AzurePublic, tenant)
            .WithDefaultRedirectUri()
            .Build();

        Directory.CreateDirectory(AppPaths.UserConfigDirectory);
        var storageProperties = new StorageCreationPropertiesBuilder(
                "dw.msalcache.bin",
                AppPaths.UserConfigDirectory)
            .Build();
        var cacheHelper = MsalCacheHelper.CreateAsync(storageProperties).GetAwaiter().GetResult();
        cacheHelper.RegisterCache(app.UserTokenCache);
        return app;
    }
}

internal enum AzureDevOpsAuthenticationScheme
{
    Bearer,
    Basic
}

internal sealed record TokenResult(
    string AccessToken,
    DateTimeOffset? ExpiresOn,
    string Source,
    AzureDevOpsAuthenticationScheme Scheme);
