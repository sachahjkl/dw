namespace Dw.Cli.Git;

internal sealed class GitRepositoryStatusService(IProcessRunner processRunner, IFileSystem fileSystem)
{
    public async Task<IReadOnlyList<RepositoryStatus>> GetStatusesAsync(string workspacePath)
        => await GetStatusesAsync(workspacePath, projectConfig: null);

    public async Task<IReadOnlyList<RepositoryStatus>> GetStatusesAsync(string workspacePath, ProjectConfig? projectConfig)
    {
        var manifest = WorkspaceManifestReader.Read(fileSystem, Path.Combine(workspacePath, "task.json"));
        var statuses = new List<RepositoryStatus>();

        foreach (var repository in manifest.Repositories)
        {
            var repoPath = Path.Combine(workspacePath, RepositoryFolder(projectConfig, repository));
            if (!fileSystem.DirectoryExists(repoPath))
            {
                statuses.Add(new RepositoryStatus(repository, repoPath, false, false, false, "Dossier absent."));
                continue;
            }

            var result = await processRunner.RunAsync("git", ["status", "--short"], repoPath);
            if (result.ExitCode != 0)
            {
                statuses.Add(new RepositoryStatus(repository, repoPath, false, false, false, result.StandardError.Trim()));
                continue;
            }

            var output = result.StandardOutput.Trim();
            var hasChanges = output.Length > 0;
            var hasUnpushed = false;
            var detail = output;

            if (!hasChanges)
            {
                var upstreamResult = await processRunner.RunAsync("git", ["rev-list", "--count", "@{u}..HEAD"], repoPath);
                if (upstreamResult.ExitCode == 0 && int.TryParse(upstreamResult.StandardOutput.Trim(), NumberStyles.None, CultureInfo.InvariantCulture, out var ahead) && ahead > 0)
                {
                    hasUnpushed = true;
                    detail = $"{ahead} commit(s) non pousse(s).";
                }
            }

            statuses.Add(new RepositoryStatus(repository, repoPath, true, hasChanges, hasUnpushed, detail));
        }

        return statuses;
    }

    private static string RepositoryFolder(ProjectConfig? projectConfig, string repository)
    {
        var config = projectConfig?.Repositories.GetValueOrDefault(repository);
        return string.IsNullOrWhiteSpace(config?.Folder) ? repository : config.Folder;
    }
}

internal sealed record RepositoryStatus(
    string Repository,
    string Path,
    bool IsGitRepository,
    bool HasChanges,
    bool HasUnpushed,
    string Detail);
