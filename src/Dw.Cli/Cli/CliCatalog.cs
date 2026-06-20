namespace Dw.Cli.Cli;

internal sealed record CliCommandSpec(
    string Name,
    string Usage,
    string Description,
    Func<CommandContext, string[], Task<int>> Handler,
    string[] Completions,
    string[] Aliases);

internal static class CliCatalog
{
    public static IReadOnlyList<CliCommandSpec> Commands { get; } =
    [
        Command("version", "version", "Affiche la version du CLI", VersionCommand.Run),
        Command("guide", "guide", "Explique le parcours de demarrage", GuideCommand.Run, aliases: ["get-started"]),
        Command("doctor", "doctor [--fix]", "Diagnostique l'environnement local", DoctorCommand.RunAsync, completions: ["--fix"]),
        Command("init", "init [--root <path>]", "Initialise un root DevWorkflow (--dry-run pour simuler)", InitCommand.Run, ["--profile", "--root", "--dry-run", "--no-save"]),
        Command("agent", "agent context", "Affiche le contexte court pour agents IA", AgentCommand.Run, ["context"]),
        Command("ado", "ado <work-item|context>", "Lit Azure DevOps sans modifier", AdoCommand.Run, ["work-item", "context", "--project", "--root", "--summary", "--comments"]),
        Command("auth", "auth <login|status|logout>", "Gere la connexion Azure DevOps", AuthCommand.Run, ["login", "status", "logout", "--root"]),
        Command("completion", "completion <shell>", "Genere l'autocompletion shell", CompletionCommand.Run, ["powershell", "bash", "zsh", "fish", "nushell", "nu"]),
        Command("task", "task <start|status|add-repo|finish>", "Gere les workspaces, worktrees, commits, push et PR", TaskCommand.Run,
        [
            "start",
            "status",
            "add-repo",
            "finish",
            "--project",
            "--task",
            "--slug",
            "--type",
            "--only",
            "--workspace",
            "--execute",
            "--message",
            "--create-pr",
            "--ready",
            "--skip-ado",
            "--skip-verify",
            "--create-child-tasks"
        ]),
        Command("db", "db <schema|describe|query>", "Explore SQL Server en lecture seule", DbCommand.Run, ["schema", "describe", "query", "--project", "--database", "--env"]),
        Command("secret", "secret <set|get|delete>", "Stocke des secrets locaux via Windows Credential Manager", SecretCommand.Run, ["set", "get", "delete", "--value", "--from-env"]),
        Command("update", "update <check|download>", "Verifie ou telecharge une release configuree", UpdateCommand.Run, ["check", "download", "--output", "--rid"])
    ];

    public static IReadOnlyDictionary<string, CliCommandSpec> Dispatch { get; } =
        Commands
            .SelectMany(command => new[] { command.Name }.Concat(command.Aliases)
                .Select(name => new KeyValuePair<string, CliCommandSpec>(name, command)))
            .ToDictionary(pair => pair.Key, pair => pair.Value, StringComparer.OrdinalIgnoreCase);

    public static IReadOnlyDictionary<string, string[]> CompletionMap { get; } =
        new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase)
        {
            [""] = Commands.SelectMany(command => new[] { command.Name }.Concat(command.Aliases)).OrderBy(name => name).ToArray()
        }
        .Concat(Commands.Select(command => new KeyValuePair<string, string[]>(command.Name, command.Completions)))
        .ToDictionary(pair => pair.Key, pair => pair.Value, StringComparer.OrdinalIgnoreCase);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, string[], int> handler,
        string[]? completions = null,
        string[]? aliases = null)
        => new(name, usage, description, (context, args) => Task.FromResult(handler(context, args)), completions ?? [], aliases ?? []);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, int> handler,
        string[]? completions = null,
        string[]? aliases = null)
        => new(name, usage, description, (context, _) => Task.FromResult(handler(context)), completions ?? [], aliases ?? []);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, string[], Task<int>> handler,
        string[]? completions = null,
        string[]? aliases = null)
        => new(name, usage, description, handler, completions ?? [], aliases ?? []);
}
