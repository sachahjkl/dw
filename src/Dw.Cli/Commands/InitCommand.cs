namespace Dw.Cli.Commands;

internal static class InitCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var root = OptionValue(args, "--root") ?? FirstPositional(args) ?? AppPaths.DefaultRoot;
        var profile = InitProfile.Resolve(OptionValue(args, "--profile"));
        var noSave = args.Any(arg => string.Equals(arg, "--no-save", StringComparison.OrdinalIgnoreCase));
        var dryRun = args.Any(arg => string.Equals(arg, "--dry-run", StringComparison.OrdinalIgnoreCase));
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

    private static string? FirstPositional(string[] args)
        => args.FirstOrDefault(arg => !arg.StartsWith("-", StringComparison.Ordinal));

    private static string? OptionValue(string[] args, string name)
    {
        for (var i = 0; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
            {
                return args[i + 1];
            }
        }

        return null;
    }
}
