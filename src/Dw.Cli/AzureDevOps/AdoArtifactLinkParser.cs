namespace Dw.Cli.AzureDevOps;

internal sealed record AdoArtifactLink(string Display)
{
    public static AdoArtifactLink? TryParse(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }

        var decoded = Uri.UnescapeDataString(url);
        if (TryParsePullRequest(decoded, out var pullRequest))
        {
            return new AdoArtifactLink(pullRequest);
        }

        if (TryParseCommit(decoded, out var commit))
        {
            return new AdoArtifactLink(commit);
        }

        if (TryParseBuild(decoded, out var build))
        {
            return new AdoArtifactLink(build);
        }

        if (TryParseRelease(decoded, out var release))
        {
            return new AdoArtifactLink(release);
        }

        return null;
    }

    private static bool TryParsePullRequest(string value, out string display)
    {
        var match = AdoRegexes.PullRequestIdArtifact().Match(value);
        if (match.Success)
        {
            display = $"PR #{match.Groups["id"].Value}";
            return true;
        }

        match = AdoRegexes.PullRequestArtifact().Match(value);
        if (match.Success)
        {
            display = $"PR #{match.Groups["id"].Value}";
            return true;
        }

        match = AdoRegexes.PullRequestQuery().Match(value);
        if (match.Success)
        {
            display = $"PR #{match.Groups["id"].Value}";
            return true;
        }

        display = string.Empty;
        return false;
    }

    private static bool TryParseCommit(string value, out string display)
    {
        var match = AdoRegexes.CommitArtifact().Match(value);
        if (match.Success)
        {
            display = $"commit {match.Groups["hash"].Value}";
            return true;
        }

        match = AdoRegexes.CommitPath().Match(value);
        if (match.Success)
        {
            display = $"commit {match.Groups["hash"].Value}";
            return true;
        }

        display = string.Empty;
        return false;
    }

    private static bool TryParseBuild(string value, out string display)
    {
        var match = AdoRegexes.BuildArtifact().Match(value);
        if (match.Success)
        {
            display = $"build #{match.Groups[1].Value}";
            return true;
        }

        display = string.Empty;
        return false;
    }

    private static bool TryParseRelease(string value, out string display)
    {
        var match = AdoRegexes.ReleaseArtifact().Match(value);
        if (match.Success)
        {
            display = string.IsNullOrWhiteSpace(match.Groups[2].Value)
                ? $"release #{match.Groups[1].Value}"
                : $"release #{match.Groups[1].Value} environment #{match.Groups[2].Value}";
            return true;
        }

        display = string.Empty;
        return false;
    }
}
