using System.Text.RegularExpressions;

namespace Dw.Cli.Database;

internal static partial class SqlReadOnlyGuard
{
    private static readonly string[] ForbiddenTokens =
    [
        "insert",
        "update",
        "delete",
        "merge",
        "drop",
        "alter",
        "truncate",
        "exec",
        "execute",
        "create",
        "grant",
        "revoke"
    ];

    public static SqlGuardResult Validate(string sql)
    {
        if (string.IsNullOrWhiteSpace(sql))
        {
            return SqlGuardResult.Blocked("La requete SQL est vide.");
        }

        var cleaned = StripComments(sql).Trim();
        if (!StartsWithReadOnlyVerb(cleaned))
        {
            return SqlGuardResult.Blocked("Seules les requetes SELECT/WITH et l'introspection read-only sont autorisees.");
        }

        foreach (var token in ForbiddenTokens)
        {
            if (Regex.IsMatch(cleaned, $@"\b{Regex.Escape(token)}\b", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant))
            {
                return SqlGuardResult.Blocked($"Mot-cle SQL interdit en mode read-only: {token.ToUpperInvariant()}.");
            }
        }

        return SqlGuardResult.Allowed();
    }

    private static bool StartsWithReadOnlyVerb(string sql)
        => sql.StartsWith("select", StringComparison.OrdinalIgnoreCase)
           || sql.StartsWith("with", StringComparison.OrdinalIgnoreCase)
           || sql.StartsWith("sp_help", StringComparison.OrdinalIgnoreCase);

    private static string StripComments(string sql)
    {
        var withoutBlockComments = BlockCommentRegex().Replace(sql, " ");
        return LineCommentRegex().Replace(withoutBlockComments, " ");
    }

    [GeneratedRegex(@"/\*.*?\*/", RegexOptions.Singleline)]
    private static partial Regex BlockCommentRegex();

    [GeneratedRegex(@"--.*?$", RegexOptions.Multiline)]
    private static partial Regex LineCommentRegex();
}

internal sealed record SqlGuardResult(bool IsAllowed, string? Reason)
{
    public static SqlGuardResult Allowed() => new(true, null);

    public static SqlGuardResult Blocked(string reason) => new(false, reason);
}
