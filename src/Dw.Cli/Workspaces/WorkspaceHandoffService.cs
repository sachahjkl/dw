using Dw.Cli.Contracts;

namespace Dw.Cli.Workspaces;

internal static class WorkspaceHandoffService
{
    public static void WriteFiles(IFileSystem fileSystem, string workspace, WorkspaceManifest manifest)
    {
        foreach (var repository in manifest.Repositories.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            var path = Path.Combine(workspace, $"{WorkflowContracts.Workspace.HandoffPrefix}{repository}{WorkflowContracts.Workspace.MarkdownExtension}");
            fileSystem.WriteAllText(path, Templates.HandoffMd(manifest, repository));
        }
    }

    public static WorkspaceHandoffSummary ReadRequiredSummary(IFileSystem fileSystem, string workspace, string repository)
    {
        var path = Path.Combine(workspace, $"{WorkflowContracts.Workspace.HandoffPrefix}{repository}{WorkflowContracts.Workspace.MarkdownExtension}");
        if (!fileSystem.FileExists(path))
        {
            throw new DwException($"Handoff manquant pour {repository}: {path}. Generer ou restaurer les handoffs avant task finish.", 2);
        }

        var text = fileSystem.ReadAllText(path);
        if (!TryParseSummary(text, repository, out var summary, out var error) || summary is null)
        {
            throw new DwException($"Handoff invalide pour {repository}: {error ?? "bloc de synthese introuvable"}. Fichier: {path}", 2);
        }

        return summary;
    }

    internal static bool TryParseSummary(string text, string expectedRepository, out WorkspaceHandoffSummary? summary, out string? error)
    {
        summary = null;
        error = null;
        var lines = (text ?? string.Empty).Replace("\r\n", "\n", StringComparison.Ordinal).Split('\n');
        var start = Array.FindIndex(lines, line => string.Equals(line.Trim(), WorkflowContracts.Handoff.YamlFenceStart, StringComparison.OrdinalIgnoreCase));
        if (start < 0)
        {
            error = "bloc ```yaml absent";
            return false;
        }

        var end = Array.FindIndex(lines, start + 1, line => string.Equals(line.Trim(), WorkflowContracts.Handoff.FenceEnd, StringComparison.Ordinal));
        if (end < 0)
        {
            error = "fin du bloc yaml absente";
            return false;
        }

        var status = string.Empty;
        var repository = string.Empty;
        var sections = new Dictionary<string, Dictionary<string, List<string>>>(StringComparer.OrdinalIgnoreCase)
        {
            [WorkflowContracts.Handoff.SectionSummary] = CreateSectionMap([WorkflowContracts.Handoff.Done, WorkflowContracts.Handoff.Decisions, WorkflowContracts.Handoff.Risks, WorkflowContracts.Handoff.Blockers, WorkflowContracts.Handoff.FollowUp]),
            [WorkflowContracts.Handoff.SectionVerification] = CreateSectionMap([WorkflowContracts.Handoff.Commands, WorkflowContracts.Handoff.ManualChecks]),
            [WorkflowContracts.Handoff.SectionArtifacts] = CreateSectionMap([WorkflowContracts.Handoff.Files, WorkflowContracts.Handoff.Screenshots, WorkflowContracts.Handoff.Attachments])
        };

        string? currentSection = null;
        string? currentKey = null;
        for (var i = start + 1; i < end; i++)
        {
            var raw = lines[i].TrimEnd();
            if (string.IsNullOrWhiteSpace(raw))
            {
                continue;
            }

            var indent = CountIndentation(lines[i]);
            var trimmed = raw.Trim();
            if (indent == 0)
            {
                currentKey = null;
                if (trimmed.Equals($"{WorkflowContracts.Handoff.SectionSummary}:", StringComparison.OrdinalIgnoreCase)
                    || trimmed.Equals($"{WorkflowContracts.Handoff.SectionVerification}:", StringComparison.OrdinalIgnoreCase)
                    || trimmed.Equals($"{WorkflowContracts.Handoff.SectionArtifacts}:", StringComparison.OrdinalIgnoreCase))
                {
                    currentSection = trimmed[..^1];
                    continue;
                }

                var split = SplitKeyValue(trimmed);
                if (split is null)
                {
                    continue;
                }

                if (split.Value.Key.Equals(WorkflowContracts.Handoff.Status, StringComparison.OrdinalIgnoreCase))
                {
                    status = split.Value.Value;
                }
                else if (split.Value.Key.Equals(WorkflowContracts.Handoff.Repository, StringComparison.OrdinalIgnoreCase))
                {
                    repository = split.Value.Value;
                }

                continue;
            }

            if (indent == 2)
            {
                if (currentSection is null || !sections.TryGetValue(currentSection, out var bucket))
                {
                    error = $"section inconnue autour de '{trimmed}'";
                    return false;
                }

                var split = SplitKeyValue(trimmed);
                if (split is null || !bucket.ContainsKey(split.Value.Key))
                {
                    error = $"cle inconnue dans {currentSection}: '{trimmed}'";
                    return false;
                }

                currentKey = split.Value.Key;
                if (!string.Equals(split.Value.Value, "[]", StringComparison.Ordinal))
                {
                    var inlineValue = TrimScalar(split.Value.Value);
                    if (!string.IsNullOrWhiteSpace(inlineValue))
                    {
                        bucket[currentKey].Add(inlineValue);
                    }
                }

                continue;
            }

            if (indent >= 4 && trimmed.StartsWith("- ", StringComparison.Ordinal))
            {
                if (currentSection is null || currentKey is null)
                {
                    error = $"element de liste hors section: '{trimmed}'";
                    return false;
                }

                sections[currentSection][currentKey].Add(TrimScalar(trimmed[2..]));
            }
        }

        if (string.IsNullOrWhiteSpace(status))
        {
            error = "status absent";
            return false;
        }

        if (string.IsNullOrWhiteSpace(repository))
        {
            error = "repository absent";
            return false;
        }

        if (!string.Equals(repository, expectedRepository, StringComparison.OrdinalIgnoreCase))
        {
            error = $"repository attendu '{expectedRepository}', trouve '{repository}'";
            return false;
        }

        summary = new WorkspaceHandoffSummary(
            Repository: repository,
            Status: status,
            Done: sections[WorkflowContracts.Handoff.SectionSummary][WorkflowContracts.Handoff.Done],
            Decisions: sections[WorkflowContracts.Handoff.SectionSummary][WorkflowContracts.Handoff.Decisions],
            Risks: sections[WorkflowContracts.Handoff.SectionSummary][WorkflowContracts.Handoff.Risks],
            Blockers: sections[WorkflowContracts.Handoff.SectionSummary][WorkflowContracts.Handoff.Blockers],
            FollowUp: sections[WorkflowContracts.Handoff.SectionSummary][WorkflowContracts.Handoff.FollowUp],
            VerificationCommands: sections[WorkflowContracts.Handoff.SectionVerification][WorkflowContracts.Handoff.Commands],
            ManualChecks: sections[WorkflowContracts.Handoff.SectionVerification][WorkflowContracts.Handoff.ManualChecks],
            Files: sections[WorkflowContracts.Handoff.SectionArtifacts][WorkflowContracts.Handoff.Files],
            Screenshots: sections[WorkflowContracts.Handoff.SectionArtifacts][WorkflowContracts.Handoff.Screenshots],
            Attachments: sections[WorkflowContracts.Handoff.SectionArtifacts][WorkflowContracts.Handoff.Attachments]);
        return true;
    }

    private static Dictionary<string, List<string>> CreateSectionMap(IEnumerable<string> keys)
        => keys.ToDictionary(key => key, _ => new List<string>(), StringComparer.OrdinalIgnoreCase);

    private static int CountIndentation(string value)
    {
        var count = 0;
        while (count < value.Length && value[count] == ' ')
        {
            count++;
        }

        return count;
    }

    private static KeyValuePair<string, string>? SplitKeyValue(string value)
    {
        var separator = value.IndexOf(':');
        if (separator < 0)
        {
            return null;
        }

        return new KeyValuePair<string, string>(value[..separator].Trim(), value[(separator + 1)..].Trim());
    }

    private static string TrimScalar(string value)
    {
        var trimmed = value.Trim();
        if (trimmed.Length >= 2 && ((trimmed[0] == '"' && trimmed[^1] == '"') || (trimmed[0] == '\'' && trimmed[^1] == '\'')))
        {
            return trimmed[1..^1];
        }

        return trimmed;
    }
}

internal sealed record WorkspaceHandoffSummary(
    string Repository,
    string Status,
    IReadOnlyList<string> Done,
    IReadOnlyList<string> Decisions,
    IReadOnlyList<string> Risks,
    IReadOnlyList<string> Blockers,
    IReadOnlyList<string> FollowUp,
    IReadOnlyList<string> VerificationCommands,
    IReadOnlyList<string> ManualChecks,
    IReadOnlyList<string> Files,
    IReadOnlyList<string> Screenshots,
    IReadOnlyList<string> Attachments);
