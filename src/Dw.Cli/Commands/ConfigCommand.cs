using System.Text.Json;
using Json.Schema;

namespace Dw.Cli.Commands;

internal static class ConfigCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "doctor" => Doctor(context, args.Skip(1).ToArray()),
            "show" => Show(context),
            "set-root" => SetRoot(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Help(CommandContext context)
    {
        context.Out.WriteLine("Usage: dw config <show|set-root|doctor> [--root <path>]");
        return 0;
    }

    private static int Show(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        context.Out.WriteLine($"Root: {settings.Root ?? AppPaths.DefaultRoot}");
        return 0;
    }

    private static int SetRoot(CommandContext context, string[] args)
    {
        var root = args.FirstOrDefault(arg => !arg.StartsWith("--", StringComparison.Ordinal))
            ?? throw new DwException("Usage: dw config set-root <path>", 2);
        root = Path.GetFullPath(Environment.ExpandEnvironmentVariables(root));
        UserSettingsStore.Save(context.FileSystem, new UserSettings(root));
        context.Out.WriteLine($"Root: {root}");
        return 0;
    }

    private static int Doctor(CommandContext context, string[] args)
    {
        var root = CommandOptions.ResolveRoot(context, args);
        var checks = new[]
        {
            CheckJsonSchema(context, Path.Combine(root, "config", "projects.json"), Path.Combine(root, "schemas", "projects.schema.json")),
            CheckJsonSchema(context, Path.Combine(root, "config", "workflow.json"), Path.Combine(root, "schemas", "workflow.schema.json")),
            CheckJsonSchema(context, Path.Combine(root, "config", "databases.json"), Path.Combine(root, "schemas", "databases.schema.json")),
            CheckJson(context, Path.Combine(root, "config", "opencode", "opencode.jsonc")),
            CheckExists(context, Path.Combine(root, "schemas", "projects.schema.json")),
            CheckExists(context, Path.Combine(root, "schemas", "workflow.schema.json")),
            CheckExists(context, Path.Combine(root, "schemas", "databases.schema.json"))
        };

        foreach (var check in checks)
        {
            context.Out.WriteLine($"{(check.Passed ? "[OK]  " : "[WARN]")} {check.Path}");
            if (!string.IsNullOrWhiteSpace(check.Message))
            {
                context.Out.WriteLine($"      {check.Message}");
            }
        }

        return checks.All(check => check.Passed) ? 0 : 1;
    }

    private static ConfigCheck CheckJsonSchema(CommandContext context, string path, string schemaPath)
    {
        var jsonCheck = CheckJson(context, path);
        if (!jsonCheck.Passed)
        {
            return jsonCheck;
        }

        if (!context.FileSystem.FileExists(schemaPath))
        {
            return new ConfigCheck(path, false, $"schema manquant: {schemaPath}");
        }

        try
        {
            var schema = JsonSchema.FromText(context.FileSystem.ReadAllText(schemaPath));
            using var document = JsonDocument.Parse(context.FileSystem.ReadAllText(path), new JsonDocumentOptions { AllowTrailingCommas = true, CommentHandling = JsonCommentHandling.Skip });
            var result = schema.Evaluate(document.RootElement, new EvaluationOptions { OutputFormat = OutputFormat.List });
            if (result.IsValid)
            {
                return new ConfigCheck(path, true, null);
            }

            return new ConfigCheck(path, false, "schema invalide");
        }
        catch (Exception ex) when (ex is JsonException or InvalidOperationException)
        {
            return new ConfigCheck(path, false, ex.Message);
        }
    }

    private static ConfigCheck CheckJson(CommandContext context, string path)
    {
        if (!context.FileSystem.FileExists(path))
        {
            return new ConfigCheck(path, false, "fichier manquant");
        }

        try
        {
            JsonDocument.Parse(context.FileSystem.ReadAllText(path), new JsonDocumentOptions { AllowTrailingCommas = true, CommentHandling = JsonCommentHandling.Skip }).Dispose();
            return new ConfigCheck(path, true, null);
        }
        catch (JsonException ex)
        {
            return new ConfigCheck(path, false, ex.Message);
        }
    }

    private static ConfigCheck CheckExists(CommandContext context, string path)
        => context.FileSystem.FileExists(path)
            ? new ConfigCheck(path, true, null)
            : new ConfigCheck(path, false, "fichier manquant");

    private sealed record ConfigCheck(string Path, bool Passed, string? Message);
}
