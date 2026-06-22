namespace Dw.Cli.Cli;

internal sealed record CliCommandSpec(
    string Name,
    string Usage,
    string Description,
    Func<CommandContext, string[], Task<int>> Handler,
    string[] Completions,
    IReadOnlyDictionary<string, string> CompletionDescriptions,
    string[] Aliases,
    string[] PotentialNext);

internal static class CliCatalog
{
    private static readonly Lazy<IReadOnlyList<CliCommandSpec>> CommandsLazy = new(() =>
    [
        Command("version", "version", "Affiche la version du CLI", VersionCommand.Run),
        Command("guide", "guide", "Explique le parcours de demarrage", GuideCommand.Run, aliases: ["get-started"], potentialNext: ["dw init --profile ogf --root C:\\Dev\\dw"]),
        Command("doctor", "doctor [--fix]", "Diagnostique l'environnement local", DoctorCommand.RunAsync, completions: ["--fix"], potentialNext: ["dw config show", "dw config set-root C:\\Dev\\dw", "dw auth status"]),
        Command("init", "init [--root <path>]", "Initialise un root DevWorkflow (--dry-run pour simuler)", InitCommand.Run, ["--profile", "--root", "--dry-run", "--no-save"], potentialNext: ["dw config doctor", "dw doctor", "dw auth login"]),
        Command("agent", "agent <context|open|config|doctor>", "Affiche le contexte ou ouvre un agent", AgentCommand.Run, ["context", "open", "config", "doctor", "show", "set-default", "--workspace", "--project", "--work-item", "--continue", "--agent", "opencode", "cursor", "claude", "codex-cli", "codex", "copilot"], completionDescriptions: AgentCompletionDescriptions, potentialNext: ["dw agent doctor", "dw ado assigned --project ha", "dw task open --continue"]),
        Command("ado", "ado <assigned|work-item|context>", "Lit Azure DevOps sans modifier", AdoCommand.Run, ["assigned", "work-item", "context", "--project", "--root", "--summary", "--comments", "--top"], completionDescriptions: AdoCompletionDescriptions, potentialNext: ["dw ado assigned --project ha", "dw task start <id> --project ha"]),
        Command("auth", "auth <login|status|logout>", "Gere la connexion Azure DevOps", AuthCommand.Run, ["login", "status", "logout", "--root"], potentialNext: ["dw ado assigned --project ha"]),
        Command("completion", "completion <shell>", "Genere l'autocompletion shell", CompletionCommand.Run, ["powershell", "bash", "zsh", "fish", "nushell", "nu"]),
        Command("task", "task <start|status|list|current|sync|prune|rename|open|teardown|add-repo|finish>", "Gere les workspaces, worktrees, commits, push et PR", TaskCommand.Run,
        [
            "start",
            "status",
            "list",
            "current",
            "sync",
            "prune",
            "rename",
            "open",
            "teardown",
            "add-repo",
            "finish",
            "--project",
            "--task",
            "--slug",
            "--type",
            "--only",
            "--workspace",
            "--work-item",
            "--repo",
            "--continue",
            "--yes",
            "--no-sync",
            "--json",
            "--agent",
            "opencode",
            "cursor",
            "claude",
            "codex-cli",
            "codex",
            "copilot",
            "--execute",
            "--message",
            "--create-pr",
            "--ready",
            "--skip-ado",
            "--skip-verify",
            "--create-child-tasks"
        ], completionDescriptions: TaskCompletionDescriptions, potentialNext: ["dw task start <id> --project ha", "dw task open --continue", "dw task finish --workspace <path>", "dw task prune"]),
        Command("config", "config <show|set-root|doctor>", "Valide et modifie la configuration", ConfigCommand.Run, ["show", "set-root", "doctor", "--root"], completionDescriptions: ConfigCompletionDescriptions, potentialNext: ["dw config set-root C:\\Dev\\dw", "dw config doctor", "dw doctor"]),
        Command("db", "db <schema|describe|query>", "Explore SQL Server en lecture seule", DbCommand.Run, ["schema", "describe", "query", "--project", "--database", "--env"]),
        Command("secret", "secret <set|get|delete>", "Stocke des secrets locaux via Windows Credential Manager", SecretCommand.Run, ["set", "get", "delete", "--value", "--from-env"], potentialNext: ["dw db schema --project ha --database dev"]),
        Command("update", "update <check|download>", "Verifie ou telecharge une release configuree", UpdateCommand.Run, ["check", "download", "--output", "--rid"], potentialNext: ["dw update check", "dw update download"])
    ]);

    public static IReadOnlyList<CliCommandSpec> Commands => CommandsLazy.Value;

    private static readonly IReadOnlyDictionary<string, string> AgentCompletionDescriptions = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
    {
        ["context"] = "Affiche le contexte court pour agents IA",
        ["open"] = "Ouvre un workspace dans l'agent configure",
        ["config"] = "Lit ou modifie la configuration agent",
        ["doctor"] = "Verifie les agents disponibles",
        ["show"] = "Affiche la configuration courante",
        ["set-default"] = "Definit l'agent par defaut",
        ["--workspace"] = "Chemin explicite du workspace",
        ["--project"] = "Filtre projet dw",
        ["--work-item"] = "Filtre work item ADO",
        ["--continue"] = "Reprend la derniere session/workspace",
        ["--agent"] = "Force l'agent a utiliser"
    };

    private static readonly IReadOnlyDictionary<string, string> AdoCompletionDescriptions = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
    {
        ["assigned"] = "Liste les work items assignes a @Me",
        ["work-item"] = "Affiche un resume de work item",
        ["context"] = "Affiche le contexte complet d'un work item",
        ["--project"] = "Projet dw pour resoudre Azure DevOps",
        ["--root"] = "Root DevWorkflow a utiliser",
        ["--summary"] = "Limite la sortie au resume",
        ["--comments"] = "Nombre de commentaires a charger",
        ["--top"] = "Nombre maximum d'items"
    };

    private static readonly IReadOnlyDictionary<string, string> TaskCompletionDescriptions = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
    {
        ["start"] = "Cree un workspace et des worktrees",
        ["status"] = "Liste les chemins des workspaces",
        ["list"] = "Liste les workspaces avec metadonnees",
        ["current"] = "Affiche le workspace courant",
        ["sync"] = "Synchronise task.json depuis ADO",
        ["prune"] = "Nettoie les workspaces en etat final",
        ["rename"] = "Renomme slug, branche et dossier workspace",
        ["open"] = "Ouvre le workspace dans un agent",
        ["teardown"] = "Supprime les worktrees et le workspace",
        ["add-repo"] = "Ajoute un repo au workspace existant",
        ["finish"] = "Dry-run ou commit/push/PR",
        ["--project"] = "Projet dw",
        ["--task"] = "ID de tache ADO concrete",
        ["--slug"] = "Texte source du slug",
        ["--type"] = "Type de branche feat/fix/bug/chore",
        ["--only"] = "Repos a creer, separes par virgule",
        ["--workspace"] = "Chemin explicite du workspace",
        ["--work-item"] = "Filtre work item ADO",
        ["--repo"] = "Repo cible dans le workspace",
        ["--continue"] = "Utilise le workspace le plus recent",
        ["--yes"] = "Confirme sans prompt",
        ["--no-sync"] = "Desactive le sync ADO automatique",
        ["--json"] = "Sortie JSON",
        ["--agent"] = "Agent a utiliser pour open",
        ["--execute"] = "Execute vraiment l'action",
        ["--message"] = "Message de commit",
        ["--create-pr"] = "Ouvre une PR apres push",
        ["--ready"] = "Cree une PR non draft",
        ["--skip-ado"] = "Ignore Azure DevOps",
        ["--skip-verify"] = "Ignore les verifications configurees",
        ["--create-child-tasks"] = "Cree les taches ADO enfant"
    };

    private static readonly IReadOnlyDictionary<string, string> ConfigCompletionDescriptions = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
    {
        ["show"] = "Affiche le root configure",
        ["set-root"] = "Definit le root DevWorkflow",
        ["doctor"] = "Valide les fichiers config",
        ["--root"] = "Root a utiliser pour cette commande"
    };

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

    public static IReadOnlyDictionary<string, IReadOnlyDictionary<string, string>> CompletionDescriptionMap { get; } =
        new Dictionary<string, IReadOnlyDictionary<string, string>>(StringComparer.OrdinalIgnoreCase)
        {
            [""] = Commands.SelectMany(command => new[] { command.Name }.Concat(command.Aliases)
                    .Select(name => new KeyValuePair<string, string>(name, command.Description)))
                .ToDictionary(pair => pair.Key, pair => pair.Value, StringComparer.OrdinalIgnoreCase)
        }
        .Concat(Commands.Select(command => new KeyValuePair<string, IReadOnlyDictionary<string, string>>(
            command.Name,
            command.CompletionDescriptions)))
        .ToDictionary(pair => pair.Key, pair => pair.Value, StringComparer.OrdinalIgnoreCase);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, string[], int> handler,
        string[]? completions = null,
        IReadOnlyDictionary<string, string>? completionDescriptions = null,
        string[]? aliases = null,
        string[]? potentialNext = null)
        => new(name, usage, description, (context, args) => Task.FromResult(handler(context, args)), completions ?? [], CompletionDescriptions(completions, completionDescriptions), aliases ?? [], potentialNext ?? []);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, int> handler,
        string[]? completions = null,
        IReadOnlyDictionary<string, string>? completionDescriptions = null,
        string[]? aliases = null,
        string[]? potentialNext = null)
        => new(name, usage, description, (context, _) => Task.FromResult(handler(context)), completions ?? [], CompletionDescriptions(completions, completionDescriptions), aliases ?? [], potentialNext ?? []);

    private static CliCommandSpec Command(
        string name,
        string usage,
        string description,
        Func<CommandContext, string[], Task<int>> handler,
        string[]? completions = null,
        IReadOnlyDictionary<string, string>? completionDescriptions = null,
        string[]? aliases = null,
        string[]? potentialNext = null)
        => new(name, usage, description, handler, completions ?? [], CompletionDescriptions(completions, completionDescriptions), aliases ?? [], potentialNext ?? []);

    private static IReadOnlyDictionary<string, string> CompletionDescriptions(string[]? completions, IReadOnlyDictionary<string, string>? explicitDescriptions = null)
        => (completions ?? [])
            .ToDictionary(value => value, value => explicitDescriptions?.GetValueOrDefault(value) ?? CompletionDescription(value), StringComparer.OrdinalIgnoreCase);

    private static string CompletionDescription(string value)
        => value switch
        {
            "start" => "Cree un workspace de travail",
            "open" => "Ouvre le workspace dans un agent",
            "list" => "Liste les workspaces",
            "current" => "Affiche le workspace courant",
            "sync" => "Synchronise task.json avec ADO",
            "prune" => "Nettoie les workspaces finis",
            "rename" => "Renomme slug/branche/workspace",
            "teardown" => "Supprime un workspace",
            "finish" => "Commit/push/PR",
            "assigned" => "Liste les items ADO assignes",
            "context" => "Affiche le contexte detaille",
            "doctor" => "Diagnostique",
            "config" => "Configure",
            "--project" => "Projet dw",
            "--workspace" => "Chemin workspace",
            "--work-item" => "ID work item ADO",
            "--continue" => "Reprendre le dernier workspace/session",
            "--agent" => "Agent a lancer",
            "--json" => "Sortie JSON",
            "--execute" => "Execute vraiment",
            "--yes" => "Confirme sans prompt",
            _ => value.StartsWith("--", StringComparison.Ordinal) ? "Option" : "Sous-commande ou valeur"
        };

    public static void WriteCommandHelp(TextWriter writer, string commandName)
    {
        var command = Dispatch.GetValueOrDefault(commandName)
            ?? throw new DwException($"Commande inconnue: {commandName}", 2);
        writer.WriteLine($"Usage: dw {command.Usage}");
        writer.WriteLine(command.Description);
        if (command.PotentialNext.Length > 0)
        {
            writer.WriteLine();
            writer.WriteLine("Potential next:");
            foreach (var next in command.PotentialNext)
            {
                writer.WriteLine($"  {next}");
            }
        }
    }
}
