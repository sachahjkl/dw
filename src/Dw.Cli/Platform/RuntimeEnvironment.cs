namespace Dw.Cli.Platform;

internal static class RuntimeEnvironment
{
    public static bool IsNixManagedInstall()
        => IsNixManagedExecutablePath(Environment.ProcessPath);

    internal static bool IsNixManagedExecutablePath(string? executablePath)
        => !string.IsNullOrWhiteSpace(executablePath)
           && executablePath.StartsWith("/nix/store/", StringComparison.Ordinal);
}
