namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static string Root(CommandContext context)
        => UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;

    private static CompletionItem Item(string label, string? detail = null)
        => new(label: label, kind: string.Empty, sortText: string.Empty, insertText: string.Empty, documentation: detail ?? string.Empty, detail: detail ?? string.Empty);

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
        var token = completion?.WordToComplete;
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
