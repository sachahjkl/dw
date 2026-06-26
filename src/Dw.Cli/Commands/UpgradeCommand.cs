namespace Dw.Cli.Commands;

using System.IO.Compression;

internal static class UpgradeCommand
{
    private const ushort WindowsPeSignature = 0x5A4D;
    private static readonly char[] SpinnerFrames = ['|', '/', '-', '\\'];

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
        context.Out.WriteLine(TerminalOutput.Bold(TerminalOutput.Cyan("Preparation de l'upgrade...", context.Out), context.Out));
        var release = client.GetLatestReleaseAsync(updates).GetAwaiter().GetResult();
        var manifest = client.DownloadManifestAsync(release, updates.AssetName).GetAwaiter().GetResult();
        var asset = manifest.Assets.FirstOrDefault(asset => string.Equals(asset.Rid, rid, StringComparison.OrdinalIgnoreCase))
                    ?? throw new DwException($"Aucun asset pour RID {rid}.");

        if (string.IsNullOrWhiteSpace(asset.Url))
        {
            throw new DwException("release.json doit contenir assets[].url pour telecharger un asset.");
        }

        var executablePath = Environment.ProcessPath ?? throw new DwException("Chemin du binaire courant indisponible.");
        var tempAsset = Path.Combine(Path.GetTempPath(), $"dw-upgrade-{Guid.NewGuid():N}{Path.GetExtension(asset.FileName)}");
        using (Step(context, $"Telechargement en cours ({asset.FileName})"))
        {
            using var response = http.GetAsync(asset.Url).GetAwaiter().GetResult();
            var body = response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
            if (!response.IsSuccessStatusCode)
            {
                throw new DwException($"Telechargement upgrade impossible HTTP {(int)response.StatusCode}.");
            }

            File.WriteAllBytes(tempAsset, body);
        }

        string hash;
        using (Step(context, "Verification SHA256"))
        {
            hash = Sha256.FileHashAsync(tempAsset).GetAwaiter().GetResult();
        }

        if (!string.Equals(hash, asset.Sha256, StringComparison.OrdinalIgnoreCase))
        {
            File.Delete(tempAsset);
            throw new DwException($"SHA256 invalide pour {tempAsset}. Attendu {asset.Sha256}, obtenu {hash}.");
        }

        string replacement;
        using (Step(context, "Desarchivage / preparation du binaire"))
        {
            replacement = PrepareReplacementExecutable(asset.FileName, tempAsset, rid);
        }

        using (Step(context, "Remplacement du binaire"))
        {
            ReplaceExecutable(context, executablePath, replacement);
        }

        context.Out.WriteLine($"{TerminalOutput.Bold(TerminalOutput.Green("Done", context.Out), context.Out)}: {manifest.Version}+{manifest.Commit}");
        return 0;
    }

    internal static string PrepareReplacementExecutable(string assetFileName, string assetPath, string rid)
    {
        if (assetFileName.EndsWith(".zip", StringComparison.OrdinalIgnoreCase))
        {
            return ExtractWindowsExecutable(assetPath);
        }

        if (assetFileName.EndsWith(".tar.gz", StringComparison.OrdinalIgnoreCase)
            || assetFileName.EndsWith(".tgz", StringComparison.OrdinalIgnoreCase))
        {
            File.Delete(assetPath);
            throw new DwException($"Asset archive non supporte pour l'upgrade automatique: {assetFileName} ({rid}).");
        }

        if (assetFileName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
        {
            EnsureWindowsExecutable(assetPath, assetFileName);
        }

        return assetPath;
    }

    private static string ExtractWindowsExecutable(string archivePath)
    {
        var destination = Path.Combine(Path.GetTempPath(), $"dw-upgrade-{Guid.NewGuid():N}.exe");
        try
        {
            using var archive = ZipFile.OpenRead(archivePath);
            var entry = archive.Entries.FirstOrDefault(entry => string.Equals(Path.GetFileName(entry.FullName), "dw.exe", StringComparison.OrdinalIgnoreCase))
                        ?? throw new DwException("Archive upgrade invalide: dw.exe introuvable.");

            entry.ExtractToFile(destination, overwrite: true);
            EnsureWindowsExecutable(destination, entry.FullName);
            return destination;
        }
        catch
        {
            if (File.Exists(destination))
            {
                File.Delete(destination);
            }

            throw;
        }
        finally
        {
            File.Delete(archivePath);
        }
    }

    private static void EnsureWindowsExecutable(string path, string displayName)
    {
        Span<byte> signature = stackalloc byte[2];
        bool isWindowsExecutable;
        using (var stream = File.OpenRead(path))
        {
            isWindowsExecutable = stream.Read(signature) == signature.Length && BitConverter.ToUInt16(signature) == WindowsPeSignature;
        }

        if (!isWindowsExecutable)
        {
            File.Delete(path);
            throw new DwException($"Asset upgrade invalide: {displayName} n'est pas un executable Windows.");
        }
    }

    private static void ReplaceExecutable(CommandContext context, string executablePath, string temp)
    {
        if (OperatingSystem.IsWindows())
        {
            var script = Path.Combine(Path.GetTempPath(), $"dw-upgrade-{Guid.NewGuid():N}.cmd");
            var backup = $"{executablePath}.bak";
            File.WriteAllText(script, WindowsReplacementScript(temp, executablePath, backup, Environment.ProcessId));
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("cmd.exe", $"/c \"{script}\"") { CreateNoWindow = true, UseShellExecute = false });
            context.Out.WriteLine($"{TerminalOutput.Yellow("Remplacement programme au prochain relachement du binaire", context.Out)}: {executablePath}");
            return;
        }

        File.Copy(temp, executablePath, overwrite: true);
        File.Delete(temp);
        _ = System.Diagnostics.Process.Start("chmod", ["+x", executablePath]);
        context.Out.WriteLine($"{TerminalOutput.Green("Binaire remplace", context.Out)}: {executablePath}");
    }

    internal static string WindowsReplacementScript(string temp, string executablePath, string backup, int pid)
        => $$"""
@echo off
setlocal
set "NEW={{temp}}"
set "TARGET={{executablePath}}"
set "BACKUP={{backup}}"
set "PID={{pid}}"

:wait
tasklist /FI "PID eq %PID%" 2>nul | find "%PID%" >nul
if not errorlevel 1 (
  timeout /t 1 /nobreak >nul
  goto wait
)

if not exist "%NEW%" exit /b 1
if exist "%BACKUP%" del /f /q "%BACKUP%" >nul 2>nul
if exist "%TARGET%" move /Y "%TARGET%" "%BACKUP%" >nul
copy /Y "%NEW%" "%TARGET%" >nul
if errorlevel 1 (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul
  exit /b 1
)
if not exist "%TARGET%" (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul
  exit /b 1
)
del /f /q "%NEW%" >nul 2>nul
del /f /q "%BACKUP%" >nul 2>nul
del /f /q "%~f0" >nul 2>nul
""";

    internal static UpdateOptions ResolveUpdates(WorkflowConfig workflow)
        => workflow.Updates ?? UpdateDefaults.Options;

    internal static void EnsureSupportedHost(string? executablePath = null)
    {
        if (RuntimeEnvironment.IsNixManagedExecutablePath(executablePath ?? Environment.ProcessPath))
        {
            throw new DwException("Auto-update indisponible pour une installation Nix. Utiliser `nix run --refresh github:sachahjkl/dw` ou `nix profile upgrade`.");
        }
    }

    private static IDisposable Step(CommandContext context, string message)
        => new UpgradeStep(context.Out, message);

    internal sealed class UpgradeStep : IDisposable
    {
        private readonly TextWriter writer;
        private readonly string message;
        private readonly CancellationTokenSource cancellation = new();
        private readonly Task? spinnerTask;
        private readonly bool animate;

        public UpgradeStep(TextWriter writer, string message)
        {
            this.writer = writer;
            this.message = message;
            animate = TerminalOutput.SupportsAnsi(writer);

            if (animate)
            {
                spinnerTask = Task.Run(Spin);
            }
            else
            {
                writer.WriteLine($"{TerminalOutput.Cyan(message, writer)}...");
            }
        }

        public void Dispose()
        {
            cancellation.Cancel();
            if (spinnerTask is not null)
            {
                try
                {
                    spinnerTask.GetAwaiter().GetResult();
                }
                catch (OperationCanceledException)
                {
                }

                writer.Write("\r");
                writer.Write(new string(' ', message.Length + 10));
                writer.Write("\r");
            }

            writer.WriteLine($"{TerminalOutput.Cyan(message, writer)}: {TerminalOutput.Bold(TerminalOutput.Green("Done", writer), writer)}");
        }

        private async Task Spin()
        {
            var index = 0;
            while (!cancellation.Token.IsCancellationRequested)
            {
                writer.Write($"\r{TerminalOutput.Cyan(message, writer)}... {TerminalOutput.Bold(SpinnerFrames[index++ % SpinnerFrames.Length].ToString(), writer)}");
                await Task.Delay(100, cancellation.Token);
            }
        }
    }
}
