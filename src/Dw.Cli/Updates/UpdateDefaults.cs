namespace Dw.Cli.Updates;

internal static class UpdateDefaults
{
    public const string Owner = "sachahjkl";
    public const string Repository = "dw";
    public const string ManifestAssetName = "release.json";

    public static UpdateOptions Options { get; } = new(
        Owner,
        Repository,
        IncludePrerelease: false,
        ManifestAssetName);
}
