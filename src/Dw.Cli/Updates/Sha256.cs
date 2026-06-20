using System.Security.Cryptography;

namespace Dw.Cli.Updates;

internal static class Sha256
{
    public static async Task<string> FileHashAsync(string path, CancellationToken cancellationToken = default)
    {
        await using var stream = File.OpenRead(path);
        var hash = await SHA256.HashDataAsync(stream, cancellationToken);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }
}
