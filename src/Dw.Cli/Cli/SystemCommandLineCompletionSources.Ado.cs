namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static IEnumerable<CompletionItem> ProjectCompletions(CommandContext context, CompletionContext? completion = null)
        => DynamicCompletions(completion, () => CompleteProjects(context));

    private static IEnumerable<CompletionItem> WorkspaceCompletions(CommandContext context, CompletionContext? completion = null)
        => DynamicCompletions(completion, () => WorkspaceCompletions(context, completion?.ParseResult));

    private static IEnumerable<CompletionItem> WorkspaceCompletions(CommandContext context, ParseResult? parseResult)
        => CompleteWorkspaces(context, Filters(parseResult));

    private static IEnumerable<CompletionItem> WorkItemCompletions(CommandContext context, CompletionContext? completion = null)
        => DynamicCompletions(completion, () => WorkItemCompletions(context, completion?.ParseResult, completion?.WordToComplete));

    private static IEnumerable<CompletionItem> WorkItemCompletions(CommandContext context, ParseResult? parseResult, string? token)
        => CompleteWorkItems(context, Filters(parseResult, token));

    private static IEnumerable<CompletionItem> RepositoryCompletions(CommandContext context, CompletionContext? completion = null)
        => DynamicCompletions(completion, () => RepositoryCompletions(context, completion?.ParseResult));

    private static IEnumerable<CompletionItem> RepositoryCompletions(CommandContext context, ParseResult? parseResult)
        => CompleteRepositories(context, Filters(parseResult, includeWorkItems: false));
}
