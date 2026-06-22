using System.Net.Http.Headers;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Dw.Cli.Updates;

internal sealed class GitHubReleaseClient(HttpClient httpClient)
{
    public async Task<GitHubRelease> GetLatestReleaseAsync(UpdateOptions options, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(options.Owner) || string.IsNullOrWhiteSpace(options.Repository))
        {
            throw new DwException("Configuration updates.owner / updates.repository manquante dans workflow.json.");
        }

        httpClient.DefaultRequestHeaders.UserAgent.Clear();
        httpClient.DefaultRequestHeaders.UserAgent.Add(new ProductInfoHeaderValue("dw", "1.0"));

        var uri = options.IncludePrerelease
            ? $"https://api.github.com/repos/{options.Owner}/{options.Repository}/releases"
            : $"https://api.github.com/repos/{options.Owner}/{options.Repository}/releases/latest";

        using var response = await httpClient.GetAsync(uri, cancellationToken);
        var body = await response.Content.ReadAsStringAsync(cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            throw new DwException($"GitHub Releases HTTP {(int)response.StatusCode}: {body}");
        }

        if (!options.IncludePrerelease)
        {
            return JsonSerializer.Deserialize(body, AppJsonContext.Default.GitHubRelease)
                   ?? throw new DwException("Reponse GitHub release invalide.");
        }

        var releases = JsonSerializer.Deserialize(body, AppJsonContext.Default.ListGitHubRelease)
                       ?? throw new DwException("Reponse GitHub releases invalide.");
        return releases.FirstOrDefault()
               ?? throw new DwException("Aucune release GitHub trouvee.");
    }

    public async Task<ReleaseManifest> DownloadManifestAsync(GitHubRelease release, string assetName, CancellationToken cancellationToken = default)
    {
        var asset = release.Assets.FirstOrDefault(asset => string.Equals(asset.Name, assetName, StringComparison.OrdinalIgnoreCase))
                    ?? throw new DwException($"Asset release introuvable: {assetName}");

        using var response = await httpClient.GetAsync(asset.BrowserDownloadUrl, cancellationToken);
        var body = await response.Content.ReadAsStringAsync(cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            throw new DwException($"Telechargement release.json impossible HTTP {(int)response.StatusCode}: {body}");
        }

        return JsonSerializer.Deserialize(body, AppJsonContext.Default.ReleaseManifest)
               ?? throw new DwException("release.json invalide.");
    }
}

internal sealed record GitHubRelease(
    [property: JsonPropertyName("tag_name")]
    string TagName,
    bool Prerelease,
    IReadOnlyList<GitHubReleaseAsset> Assets);

internal sealed record GitHubReleaseAsset(
    string Name,
    [property: JsonPropertyName("browser_download_url")]
    string BrowserDownloadUrl);
