namespace Dw.Cli.Commands;

internal static class UpgradeCommand
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

    internal static int Run(CommandContext context, string? rid)
    {
        EnsureSupportedHost();
        rid ??= "win-x64";
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

        var executablePath = Environment.ProcessPath ?? throw new DwException("Chemin du binaire courant indisponible.");
        var temp = Path.Combine(Path.GetTempPath(), $"dw-upgrade-{Guid.NewGuid():N}{Path.GetExtension(asset.FileName)}");
        using (var response = http.GetAsync(asset.Url).GetAwaiter().GetResult())
        {
            var body = response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
            if (!response.IsSuccessStatusCode)
            {
                throw new DwException($"Telechargement upgrade impossible HTTP {(int)response.StatusCode}.");
            }

            File.WriteAllBytes(temp, body);
        }

        var hash = Sha256.FileHashAsync(temp).GetAwaiter().GetResult();
        if (!string.Equals(hash, asset.Sha256, StringComparison.OrdinalIgnoreCase))
        {
            File.Delete(temp);
            throw new DwException($"SHA256 invalide pour {temp}. Attendu {asset.Sha256}, obtenu {hash}.");
        }

        ReplaceExecutable(context, executablePath, temp);
        context.Out.WriteLine($"Upgrade prepare: {manifest.Version}+{manifest.Commit}");
        return 0;
    }

    private static void ReplaceExecutable(CommandContext context, string executablePath, string temp)
    {
        if (OperatingSystem.IsWindows())
        {
            var script = Path.Combine(Path.GetTempPath(), $"dw-upgrade-{Guid.NewGuid():N}.cmd");
            File.WriteAllText(script, $$"""
@echo off
ping 127.0.0.1 -n 2 > nul
move /Y "{{temp}}" "{{executablePath}}" > nul
del "%~f0" > nul
""");
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("cmd.exe", $"/c \"{script}\"") { CreateNoWindow = true, UseShellExecute = false });
            context.Out.WriteLine($"Remplacement programme au prochain relachement du binaire: {executablePath}");
            return;
        }

        File.Copy(temp, executablePath, overwrite: true);
        File.Delete(temp);
        if (!OperatingSystem.IsWindows())
        {
            _ = System.Diagnostics.Process.Start("chmod", ["+x", executablePath]);
        }

        context.Out.WriteLine($"Binaire remplace: {executablePath}");
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
