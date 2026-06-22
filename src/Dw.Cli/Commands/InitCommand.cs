namespace Dw.Cli.Commands;

internal static class InitCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var root = CommandOptions.OptionValue(args, "--root") ?? CommandOptions.FirstPositional(args) ?? AppPaths.DefaultRoot;
        var profile = InitProfile.Resolve(CommandOptions.OptionValue(args, "--profile"));
        var noSave = CommandOptions.HasFlag(args, "--no-save");
        var dryRun = CommandOptions.HasFlag(args, "--dry-run");
        root = Path.GetFullPath(Environment.ExpandEnvironmentVariables(root));

        if (dryRun)
        {
            context.Out.WriteLine($"Dry-run init DevWorkflow: {root}");
            context.Out.WriteLine($"Profil: {profile.Name}");
            foreach (var path in PlannedPaths(root))
            {
                context.Out.WriteLine($"  would create/write: {path}");
            }

            if (!noSave)
            {
                context.Out.WriteLine($"  would save user root: {root}");
            }
            else
            {
                context.Out.WriteLine("  would not modify user settings (--no-save).");
            }

            return 0;
        }

        var fs = context.FileSystem;
        fs.CreateDirectory(root);
        fs.CreateDirectory(Path.Combine(root, "config"));
        fs.CreateDirectory(Path.Combine(root, "config", "opencode"));
        fs.CreateDirectory(Path.Combine(root, "projects"));
        fs.CreateDirectory(Path.Combine(root, "cache"));
        SchemaResourceWriter.WriteIfMissing(fs, root);

        InitFileWriter.WriteIfMissing(fs, Path.Combine(root, "config", "projects.json"), profile.ProjectsJson);
        InitFileWriter.WriteIfMissing(fs, Path.Combine(root, "config", "workflow.json"), profile.WorkflowJson);
        InitFileWriter.WriteIfMissing(fs, Path.Combine(root, "config", "databases.json"), profile.DatabasesJson);
        InitFileWriter.WriteIfMissing(fs, Path.Combine(root, "config", "opencode", "AGENTS.md"), profile.AgentsMd);
        InitFileWriter.WriteIfMissing(fs, Path.Combine(root, "config", "opencode", "opencode.jsonc"), profile.OpenCodeJsonc);

        if (!noSave)
        {
            UserSettingsStore.Save(fs, new UserSettings(root));
        }

        context.Out.WriteLine($"Root DevWorkflow initialise: {root}");
        context.Out.WriteLine($"Profil: {profile.Name}");
        if (noSave)
        {
            context.Out.WriteLine("Settings utilisateur non modifies (--no-save).");
        }

        context.Out.WriteLine("Prochaine etape conseillee: dw doctor");
        return 0;
    }

    private static IEnumerable<string> PlannedPaths(string root)
    {
        yield return root;
        yield return Path.Combine(root, "config");
        yield return Path.Combine(root, "config", "projects.json");
        yield return Path.Combine(root, "config", "workflow.json");
        yield return Path.Combine(root, "config", "databases.json");
        yield return Path.Combine(root, "config", "opencode", "AGENTS.md");
        yield return Path.Combine(root, "config", "opencode", "opencode.jsonc");
        yield return Path.Combine(root, "projects");
        yield return Path.Combine(root, "cache");
        yield return Path.Combine(root, "schemas");
        yield return Path.Combine(root, "schemas", "projects.schema.json");
        yield return Path.Combine(root, "schemas", "workflow.schema.json");
        yield return Path.Combine(root, "schemas", "databases.schema.json");
        yield return Path.Combine(root, "schemas", "release.schema.json");
    }

}
