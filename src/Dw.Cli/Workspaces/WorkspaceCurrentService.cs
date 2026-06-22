namespace Dw.Cli.Workspaces;

internal static class WorkspaceCurrentService
{
    public static string? FindWorkspacePath(IFileSystem fileSystem, string startPath)
    {
        var current = Path.GetFullPath(startPath);
        if (fileSystem.FileExists(Path.Combine(current, "task.json")))
        {
            return current;
        }

        var directory = new DirectoryInfo(current);
        while (directory is not null)
        {
            if (fileSystem.FileExists(Path.Combine(directory.FullName, "task.json")))
            {
                return directory.FullName;
            }

            directory = directory.Parent;
        }

        return null;
    }
}
