namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static string Root(CommandContext context)
        => UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;

    private static bool IsHelpRequest(CompletionContext? completion)
        => completion?.ParseResult?.Tokens.Any(token => token.Value is "--help" or "-h" or "-?") == true;

    private static CompletionItem Item(string label, string? detail = null)
        => new(label: label, kind: string.Empty, sortText: string.Empty, insertText: string.Empty, documentation: detail ?? string.Empty, detail: detail ?? string.Empty);

    private static IEnumerable<CompletionItem> DynamicCompletions(CompletionContext? completion, Func<IEnumerable<CompletionItem>> completions)
        => IsHelpRequest(completion)
            ? []
            : SafeCompletions(completions);

    private static IEnumerable<CompletionItem> SafeCompletions(Func<IEnumerable<CompletionItem>> completions)
    {
        try
        {
            return completions();
        }
        catch
        {
            return [];
        }
    }

    private static IEnumerable<CompletionItem> PrefixForMultiValue(IEnumerable<CompletionItem> completions, CompletionContext? completion)
    {
        return PrefixForMultiValue(completions, completion?.WordToComplete);
    }

    private static IEnumerable<CompletionItem> PrefixForMultiValue(IEnumerable<CompletionItem> completions, string? token)
    {
        if (string.IsNullOrWhiteSpace(token) || !token.Contains(',', StringComparison.Ordinal))
        {
            return completions;
        }

        var lastSeparator = token.LastIndexOf(',');
        if (lastSeparator < 0)
        {
            return completions;
        }

        var prefix = token[..(lastSeparator + 1)];
        return completions.Select(item => new CompletionItem(
            label: prefix + item.Label,
            kind: item.Kind ?? string.Empty,
            sortText: item.SortText,
            insertText: prefix + (string.IsNullOrWhiteSpace(item.InsertText) ? item.Label : item.InsertText),
            documentation: item.Documentation,
            detail: item.Detail));
    }
}
