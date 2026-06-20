using System.Text.Json.Serialization;

namespace Dw.Cli.Updates;

internal sealed record ReleaseManifest(
    int Schema,
    string Version,
    string Commit,
    string Channel,
    IReadOnlyList<ReleaseAsset> Assets);

internal sealed record ReleaseAsset(
    string Rid,
    string FileName,
    string Sha256,
    [property: JsonPropertyName("url")] string? Url = null);
