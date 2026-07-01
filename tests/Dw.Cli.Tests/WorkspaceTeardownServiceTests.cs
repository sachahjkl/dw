namespace Dw.Cli.Tests;

public sealed class WorkspaceTeardownServiceTests
{
    [Fact]
    public void Plan_removes_each_repo_worktree_and_prunes_git_anchors()
    {
        var manifest = new WorkspaceManifest(1, "55222", null, "ha", "feat", "slug", "feat/55222-slug", DateTimeOffset.UtcNow, ["front"], "created");
        var projectConfig = new ProjectConfig(
            "HA",
            new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase)
            {
                ["front"] = new("https://example/front.git", "develop", AnchorName: "front.git", Folder: "front")
            });

        var root = Path.Combine(Path.GetTempPath(), "dw-root");
        var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-55222-slug");
        var steps = WorkspaceTeardownService.Plan(root, workspace, manifest, projectConfig).ToArray();

        Assert.Contains(steps, step => step.Repository == "front" && step.Action == "worktree remove" && step.Target == Path.Combine(workspace, "front") && step.GitDir == Path.Combine(root, "projects", "ha", "repositories", "front.git"));
        Assert.Contains(steps, step => step.Repository == "front" && step.Action == "worktree prune" && step.Target == Path.Combine(root, "projects", "ha", "repositories", "front.git") && step.GitDir == Path.Combine(root, "projects", "ha", "repositories", "front.git"));
        Assert.Contains(steps, step => step.Repository == "workspace" && step.Action == "delete directory" && step.Target == workspace);
    }

    [Fact]
    public void Teardown_dry_run_does_not_execute_git_or_delete_workspace()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-teardown-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-55222-slug");
            fs.WriteAllText(Path.Combine(root, "config", "projects.json"), """
{
  "schema": 1,
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": {
          "url": "https://example/front.git",
          "defaultBranch": "develop",
          "anchorName": "front.git",
          "folder": "front"
        }
      }
    }
  }
}
""");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "55222", null, "ha", "feat", "slug", "feat/55222-slug", DateTimeOffset.UtcNow, ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var processRunner = new CapturingProcessRunner();
            var context = new CommandContext(output, error, new FixedClock(), fs, processRunner);

            var exitCode = WorkspaceTeardownService.Teardown(context, new WorkspaceTeardownOptions(workspace, null, null, Continue: false, Execute: false, Yes: false), root);

            Assert.Equal(0, exitCode);
            Assert.Empty(processRunner.Calls);
            Assert.True(Directory.Exists(workspace));
            Assert.Contains("Dry-run uniquement", output.ToString());
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    [Fact]
    public void Teardown_execute_uses_anchor_git_dir_for_worktree_remove()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-teardown-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-55222-slug");
            var anchor = Path.Combine(root, "projects", "ha", "repositories", "front.git");
            fs.WriteAllText(Path.Combine(root, "config", "projects.json"), """
{
  "schema": 1,
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": {
          "url": "https://example/front.git",
          "defaultBranch": "develop",
          "anchorName": "front.git",
          "folder": "front"
        }
      }
    }
  }
}
""");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "55222", null, "ha", "feat", "slug", "feat/55222-slug", DateTimeOffset.UtcNow, ["front"], "created")));
            fs.CreateDirectory(Path.Combine(workspace, "front"));
            fs.CreateDirectory(anchor);
            using var output = new StringWriter();
            using var error = new StringWriter();
            var processRunner = new CapturingProcessRunner();
            var context = new CommandContext(output, error, new FixedClock(), fs, processRunner);

            var exitCode = WorkspaceTeardownService.Teardown(context, new WorkspaceTeardownOptions(workspace, null, null, Continue: false, Execute: true, Yes: true), root);

            Assert.Equal(0, exitCode);
            Assert.Contains(processRunner.Calls, call => call.SequenceEqual(["--git-dir", anchor, "worktree", "remove", "--force", Path.Combine(workspace, "front")]));
            Assert.Contains(processRunner.Calls, call => call.SequenceEqual(["--git-dir", anchor, "worktree", "prune"]));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 6, 22, 12, 0, 0, TimeSpan.Zero);
    }

    private sealed class CapturingProcessRunner : IProcessRunner
    {
        public List<IReadOnlyList<string>> Calls { get; } = [];

        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        {
            Calls.Add(arguments.ToArray());
            return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
        }

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
        {
            Calls.Add(arguments.ToArray());
            return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
        }

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => throw new NotSupportedException();
    }
}
