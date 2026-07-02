namespace Dw.Cli.Commands;

internal static class RefreshCommand
{
    public static int Run(CommandContext context, string? configuredRoot, string? profileName)
    {
        var root = configuredRoot
            ?? UserSettingsStore.Load(context.FileSystem).Root
            ?? AppPaths.DefaultRoot;
        root = Path.GetFullPath(Environment.ExpandEnvironmentVariables(root));

        if (!context.FileSystem.DirectoryExists(root))
        {
            throw new DwException($"Root DevWorkflow introuvable: {root}", 2);
        }

        var profile = string.IsNullOrWhiteSpace(profileName)
            ? InitProfile.Detect(context.FileSystem, root)
            : InitProfile.Resolve(profileName);

        EnsureRootDirectories(context.FileSystem, root);
        SchemaResourceWriter.Write(context.FileSystem, root, overwrite: true);
        WriteRootAgentFiles(context.FileSystem, root, profile);
        RefreshWorkspaceAgentFiles(context.FileSystem, root);
        EnsureBareRepoFetchRefspecs(context, root);

        context.Out.WriteLine($"Root rafraichi: {root}");
        context.Out.WriteLine($"Profil: {profile.Name}");
        context.Out.WriteLine("Schemas et contextes agents regeneres.");
        context.Out.WriteLine("Fichiers utilisateurs preserves: projects.json, workflow.json, databases.json, plan.md.");
        return 0;
    }

    private static void EnsureRootDirectories(IFileSystem fileSystem, string root)
    {
        fileSystem.CreateDirectory(root);
        fileSystem.CreateDirectory(Path.Combine(root, "config"));
        fileSystem.CreateDirectory(Path.Combine(root, "config", "opencode"));
        fileSystem.CreateDirectory(Path.Combine(root, "config", "claude"));
        fileSystem.CreateDirectory(Path.Combine(root, "config", "cursor"));
        fileSystem.CreateDirectory(Path.Combine(root, "config", "codex"));
        fileSystem.CreateDirectory(Path.Combine(root, "config", "copilot"));
        fileSystem.CreateDirectory(Path.Combine(root, "projects"));
        fileSystem.CreateDirectory(Path.Combine(root, "cache"));
    }

    private static void WriteRootAgentFiles(IFileSystem fileSystem, string root, InitProfile profile)
    {
        fileSystem.WriteAllText(Path.Combine(root, "config", "opencode", "AGENTS.md"), profile.AgentsMd);
        fileSystem.WriteAllText(Path.Combine(root, "config", "opencode", "opencode.jsonc"), profile.OpenCodeJsonc);
        fileSystem.WriteAllText(Path.Combine(root, "config", "claude", "CLAUDE.md"), profile.AgentsMd);
        fileSystem.WriteAllText(Path.Combine(root, "config", "cursor", "devworkflow.mdc"), profile.AgentsMd);
        fileSystem.WriteAllText(Path.Combine(root, "config", "codex", "AGENTS.md"), profile.AgentsMd);
        fileSystem.WriteAllText(Path.Combine(root, "config", "codex", "config.toml"), Templates.WorkspaceCodexConfig);
        fileSystem.WriteAllText(Path.Combine(root, "config", "copilot", "copilot-instructions.md"), profile.AgentsMd);
    }

    private static void RefreshWorkspaceAgentFiles(IFileSystem fileSystem, string root)
    {
        foreach (var workspace in WorkspaceDiscoveryService.FindWorkspaces(fileSystem, root))
        {
            foreach (var file in AgentAdapterRegistry.WorkspaceConfigFiles(new AgentWorkspaceConfigRequest(
                         workspace.Path,
                         workspace.Manifest.ParentWorkItems,
                         workspace.Manifest.Project)))
            {
                fileSystem.WriteAllText(Path.Combine(workspace.Path, file.RelativePath), file.Content);
            }

            WorkspaceHandoffService.WriteFiles(fileSystem, workspace.Path, workspace.Manifest);
        }
    }

    private static void EnsureBareRepoFetchRefspecs(CommandContext context, string root)
    {
        var projectsRoot = Path.Combine(root, "projects");
        if (!context.FileSystem.DirectoryExists(projectsRoot))
        {
            return;
        }

        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        foreach (var project in projects.Projects)
        {
            var projectRoot = Path.Combine(projectsRoot, project.Key);
            var reposRoot = Path.Combine(projectRoot, "repositories");
            if (!context.FileSystem.DirectoryExists(reposRoot))
            {
                continue;
            }

            foreach (var repo in project.Value.Repositories)
            {
                var anchorName = string.IsNullOrWhiteSpace(repo.Value.AnchorName)
                    ? $"{repo.Key}.git"
                    : repo.Value.AnchorName;
                var anchorPath = Path.Combine(reposRoot, anchorName);
                if (!context.FileSystem.DirectoryExists(anchorPath))
                {
                    continue;
                }

                var result = context.ProcessRunner.RunAsync(
                    "git",
                    ["--git-dir", anchorPath, "config", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"],
                    projectRoot).GetAwaiter().GetResult();

                if (result.ExitCode == 0)
                {
                    context.Out.WriteLine($"  fetch refspec corrige: {project.Key}/{anchorName}");
                }
            }
        }
    }
}
