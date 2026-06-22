namespace Dw.Cli.Commands;

internal static class UpdateCommand
{
    internal static int Check(CommandContext context)
    {
        EnsureSupportedHost();
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var updates = ResolveUpdates(workflow);

        using var http = new HttpClient();
        var client = new GitHubReleaseClient(http);
        var release = client.GetLatestReleaseAsync(updates).GetAwaiter().GetResult();
        var manifest = client.DownloadManifestAsync(release, updates.AssetName).GetAwaiter().GetResult();

        context.Out.WriteLine($"Latest release: {release.TagName}");
        context.Out.WriteLine($"Manifest version: {manifest.Version}+{manifest.Commit}");
        foreach (var asset in manifest.Assets)
        {
            context.Out.WriteLine($"- {asset.Rid}: {asset.FileName} {asset.Sha256}");
        }

        return 0;
    }

    internal static int Download(CommandContext context, string? rid, string? output)
    {
        EnsureSupportedHost();
        rid ??= "win-x64";
        output ??= Path.Combine(AppPaths.UserConfigDirectory, "updates");
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var updates = ResolveUpdates(workflow);

        using var http = new HttpClient();
        var client = new GitHubReleaseClient(http);
        var release = client.GetLatestReleaseAsync(updates).GetAwaiter().GetResult();
        var manifest = client.DownloadManifestAsync(release, updates.AssetName).GetAwaiter().GetResult();
        var asset = manifest.Assets.FirstOrDefault(asset => string.Equals(asset.Rid, rid, StringComparison.OrdinalIgnoreCase))
                    ?? throw new DwException($"Aucun asset pour RID {rid}.");

        if (string.IsNullOrWhiteSpace(asset.Url))
        {
            throw new DwException("release.json doit contenir assets[].url pour telecharger un asset.");
        }

        Directory.CreateDirectory(output);
        var destination = Path.Combine(output, asset.FileName);
        using (var response = http.GetAsync(asset.Url).GetAwaiter().GetResult())
        {
            var body = response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
            if (!response.IsSuccessStatusCode)
            {
                throw new DwException($"Telechargement update impossible HTTP {(int)response.StatusCode}.");
            }

            File.WriteAllBytes(destination, body);
        }

        var hash = Sha256.FileHashAsync(destination).GetAwaiter().GetResult();
        if (!string.Equals(hash, asset.Sha256, StringComparison.OrdinalIgnoreCase))
        {
            File.Delete(destination);
            throw new DwException($"SHA256 invalide pour {destination}. Attendu {asset.Sha256}, obtenu {hash}.");
        }

        context.Out.WriteLine($"Asset telecharge et verifie: {destination}");
        return 0;
    }

    internal static UpdateOptions ResolveUpdates(WorkflowConfig workflow)
        => workflow.Updates ?? UpdateDefaults.Options;

    internal static void EnsureSupportedHost(string? executablePath = null)
    {
        if (RuntimeEnvironment.IsNixManagedExecutablePath(executablePath ?? Environment.ProcessPath))
        {
            throw new DwException("Auto-update indisponible pour une installation Nix. Utiliser `nix run --refresh github:sachahjkl/dw` ou `nix profile upgrade`.");
        }
    }
}
