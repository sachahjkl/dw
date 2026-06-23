namespace Dw.Cli.Tests;

public sealed class WorkspaceOpenServiceTests
{
    [Fact]
    public void ResolveWorkspace_continue_uses_latest_matching_workspace()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var oldWorkspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-1-old");
            var newWorkspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-2-new");
            fs.WriteAllText(Path.Combine(oldWorkspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "1", null, "ha", "feat", "old", "feat/1-old", DateTimeOffset.Parse("2026-06-20T00:00:00Z"), ["front"], "created")));
            fs.WriteAllText(Path.Combine(newWorkspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "2", null, "ha", "feat", "new", "feat/2-new", DateTimeOffset.Parse("2026-06-21T00:00:00Z"), ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, new WorkspaceOpenOptions(null, "ha", null, Continue: true));

            Assert.Equal(newWorkspace, workspace);
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
    public void Open_sets_opencode_config_and_continue_flag()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-2-new");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "2", null, "ha", "feat", "new", "feat/2-new", DateTimeOffset.Parse("2026-06-21T00:00:00Z"), ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var processRunner = new CapturingProcessRunner();
            var context = new CommandContext(output, error, new FixedClock(), fs, processRunner);

            var exitCode = WorkspaceOpenService.Open(context, new WorkspaceOpenOptions(workspace, null, null, Continue: true), root);

            Assert.Equal(0, exitCode);
            Assert.Equal("opencode", processRunner.FileName);
            Assert.Equal(["-c", workspace], processRunner.Arguments);
            Assert.Equal(workspace, processRunner.WorkingDirectory);
            Assert.NotNull(processRunner.Environment);
            Assert.True(processRunner.Environment.TryGetValue("OPENCODE_CONFIG", out var configPath));
            Assert.Equal(Path.Combine(root, "config", "opencode", "opencode.jsonc"), configPath);
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
    public void ResolveWorkspace_uses_positional_work_item_id()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-new");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "11010", null, "ha", "feat", "new", "feat/11010-new", DateTimeOffset.Parse("2026-06-21T00:00:00Z"), ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var resolved = WorkspaceOpenService.ResolveWorkspace(context, root, new WorkspaceOpenOptions(null, "ha", null, Continue: false, PositionalWorkItemId: "11010"));

            Assert.Equal(workspace, resolved);
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
    public void ResolveWorkspace_rejects_conflicting_positional_and_option_work_item_ids()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), new RealFileSystem(), new NoopProcessRunner());

            var ex = Assert.Throws<DwException>(() => WorkspaceOpenService.ResolveWorkspace(context, root, new WorkspaceOpenOptions(null, "ha", "55206", Continue: false, PositionalWorkItemId: "11010")));

            Assert.Contains("work-item-id et --work-item", ex.Message);
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
    public void Open_uses_default_agent_from_workflow_config()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), """
{
  "schema": 1,
  "branchPrefixes": {},
  "agent": {
    "default": "claude"
  }
}
""");
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-2-new");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "2", null, "ha", "feat", "new", "feat/2-new", DateTimeOffset.Parse("2026-06-21T00:00:00Z"), ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var processRunner = new CapturingProcessRunner();
            var context = new CommandContext(output, error, new FixedClock(), fs, processRunner);

            var exitCode = WorkspaceOpenService.Open(context, new WorkspaceOpenOptions(workspace, null, null, Continue: true), root);

            Assert.Equal(0, exitCode);
            Assert.Equal("claude", processRunner.FileName);
            Assert.Equal(["--continue"], processRunner.Arguments);
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
    public void Open_prefers_project_default_agent_over_workflow_default()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-open-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), """
{
  "schema": 1,
  "branchPrefixes": {},
  "agent": {
    "default": "opencode"
  }
}
""");
            fs.WriteAllText(Path.Combine(root, "config", "projects.json"), """
{
  "schema": 1,
  "projects": {
    "ha": {
      "displayName": "HA",
      "agent": {
        "default": "claude"
      },
      "repositories": {}
    }
  }
}
""");
            var workspace = Path.Combine(root, "projects", "ha", "workspaces", "feat-2-new");
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(1, "2", null, "ha", "feat", "new", "feat/2-new", DateTimeOffset.Parse("2026-06-21T00:00:00Z"), ["front"], "created")));
            using var output = new StringWriter();
            using var error = new StringWriter();
            var processRunner = new CapturingProcessRunner();
            var context = new CommandContext(output, error, new FixedClock(), fs, processRunner);

            var exitCode = WorkspaceOpenService.Open(context, new WorkspaceOpenOptions(workspace, null, null, Continue: true), root);

            Assert.Equal(0, exitCode);
            Assert.Equal("claude", processRunner.FileName);
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
    public void ResolveOpenTarget_returns_repo_folder()
    {
        var manifest = new WorkspaceManifest(1, "2", null, "ha", "feat", "new", "feat/2-new", DateTimeOffset.UtcNow, ["front"], "created");
        var projectConfig = new ProjectConfig("HA", new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase)
        {
            ["front"] = new("", "develop", Folder: "custom-front")
        });

        var workspace = Path.Combine(Path.GetTempPath(), "workspace");
        var target = WorkspaceOpenService.ResolveOpenTarget(workspace, manifest, projectConfig, "front");

        Assert.Equal(Path.Combine(workspace, "custom-front"), target);
    }

    [Fact]
    public void AgentRegistry_rejects_unknown_agent()
    {
        var exception = Assert.Throws<DwException>(() => AgentAdapterRegistry.Resolve("unknown"));

        Assert.Contains("Agent inconnu", exception.Message);
    }

    [Theory]
    [InlineData("opencode", "opencode")]
    [InlineData("cursor", "agent")]
    [InlineData("claude", "claude")]
    [InlineData("codex-cli", "codex")]
    [InlineData("codex", "codex")]
    [InlineData("copilot", "copilot")]
    public void AgentRegistry_builds_launch_for_known_agents(string agent, string expectedFileName)
    {
        var adapter = AgentAdapterRegistry.Resolve(agent);

        var launch = adapter.BuildOpenLaunch(new AgentOpenRequest(@"S:\root", @"S:\workspace", Continue: false));

        Assert.Equal(expectedFileName, launch.FileName);
        Assert.Equal(@"S:\workspace", launch.WorkingDirectory);
    }

    [Fact]
    public void AgentRegistry_provides_workspace_config_files_from_adapters()
    {
        var files = AgentAdapterRegistry.WorkspaceConfigFiles(new AgentWorkspaceConfigRequest(@"S:\workspace", "55222", "ha"));

        Assert.Contains(files, file => file.RelativePath == "AGENTS.md");
        Assert.Contains(files, file => file.RelativePath == "CLAUDE.md");
        Assert.Contains(files, file => file.RelativePath == Path.Combine(".claude", "CLAUDE.md"));
        Assert.Contains(files, file => file.RelativePath == Path.Combine(".cursor", "rules", "devworkflow.mdc"));
        Assert.Contains(files, file => file.RelativePath == Path.Combine(".codex", "config.toml"));
        Assert.Contains(files, file => file.RelativePath == Path.Combine(".github", "copilot-instructions.md"));
    }

    [Fact]
    public void Codex_continue_uses_resume_last_with_cd()
    {
        var adapter = AgentAdapterRegistry.Resolve("codex");

        var launch = adapter.BuildOpenLaunch(new AgentOpenRequest(@"S:\root", @"S:\workspace", Continue: true));

        Assert.Equal(["resume", "--last", "--cd", @"S:\workspace"], launch.Arguments);
    }

    [Fact]
    public void Cursor_uses_workspace_flag()
    {
        var adapter = AgentAdapterRegistry.Resolve("cursor");

        var launch = adapter.BuildOpenLaunch(new AgentOpenRequest(@"S:\root", @"S:\workspace", Continue: true));

        Assert.Equal("agent", launch.FileName);
        Assert.Equal(["--workspace", @"S:\workspace", "--continue"], launch.Arguments);
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 6, 22, 12, 0, 0, TimeSpan.Zero);
    }

    private sealed class NoopProcessRunner : IProcessRunner
    {
        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(0);
    }

    private sealed class CapturingProcessRunner : IProcessRunner
    {
        public string? FileName { get; private set; }
        public IReadOnlyList<string>? Arguments { get; private set; }
        public string? WorkingDirectory { get; private set; }
        public IReadOnlyDictionary<string, string>? Environment { get; private set; }

        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => throw new NotSupportedException();

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
        {
            FileName = fileName;
            Arguments = arguments.ToArray();
            WorkingDirectory = workingDirectory;
            Environment = environment?.ToDictionary(StringComparer.OrdinalIgnoreCase);
            return Task.FromResult(0);
        }
    }
}
