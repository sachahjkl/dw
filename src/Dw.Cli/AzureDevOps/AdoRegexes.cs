using System.Text.RegularExpressions;

namespace Dw.Cli.AzureDevOps;

internal static partial class AdoRegexes
{
    [GeneratedRegex(@"/workItems/(\d+)$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex WorkItemRelationUrl();

    [GeneratedRegex("<li[^>]*>", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex HtmlListItem();

    [GeneratedRegex("<[^>]+>", RegexOptions.CultureInvariant)]
    internal static partial Regex HtmlTag();

    [GeneratedRegex(@"[ \t]+\r?\n", RegexOptions.CultureInvariant)]
    internal static partial Regex TrailingWhitespaceBeforeNewLine();

    [GeneratedRegex(@"(\r?\n){3,}", RegexOptions.CultureInvariant)]
    internal static partial Regex ExcessBlankLines();

    [GeneratedRegex(@"PullRequestId/(?:[^/]+/)*(?<id>\d+)$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex PullRequestIdArtifact();

    [GeneratedRegex(@"PullRequest/(?:[^/]+/)*(?<id>\d+)$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex PullRequestArtifact();

    [GeneratedRegex(@"[?&]pullRequestId=(?<id>\d+)", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex PullRequestQuery();

    [GeneratedRegex(@"Commit/(?:[^/]+/)*(?<hash>[0-9a-f]{7,40})$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex CommitArtifact();

    [GeneratedRegex(@"commits?/(?<hash>[0-9a-f]{7,40})", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex CommitPath();

    [GeneratedRegex(@"^(feat|fix|bug|chore|refactor|test)\(#[0-9]+(?: #[0-9]+)*\)\s?:\s\S.+$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex CommitMessage();

    [GeneratedRegex(@"Build/Build/(\d+)$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex BuildArtifact();

    [GeneratedRegex(@"Release/(?:Environment|Release)/(\d+)(?:/(\d+))?", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    internal static partial Regex ReleaseArtifact();
}
