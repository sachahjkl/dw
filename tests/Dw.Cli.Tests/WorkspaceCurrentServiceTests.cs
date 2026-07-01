namespace Dw.Cli.Tests;

public sealed class WorkspaceCurrentServiceTests
{
    [Fact]
    public void FindWorkspacePath_finds_parent_task_json()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-current-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var workspace = Path.Combine(root, "workspace");
            var repo = Path.Combine(workspace, "front", "src");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), "{}");
            fs.CreateDirectory(repo);

            var found = WorkspaceCurrentService.FindWorkspacePath(fs, repo);

            Assert.Equal(workspace, found);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }
}
