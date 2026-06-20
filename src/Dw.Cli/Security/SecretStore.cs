namespace Dw.Cli.Security;

internal interface ISecretStore
{
    void Set(string key, string secret);
    string? Get(string key);
    void Delete(string key);
}

internal static class SecretStoreFactory
{
    public static ISecretStore Create()
        => OperatingSystem.IsWindows()
            ? new WindowsCredentialManagerSecretStore()
            : throw new DwException("Secret store non supporte sur cette plateforme pour le moment.");
}
