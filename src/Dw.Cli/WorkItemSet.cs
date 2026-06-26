namespace Dw.Cli.Workspaces;

internal sealed record WorkItemSet(IReadOnlyList<string> Ids)
{
    public string PrimaryId => Ids[0];

    public string DisplayText => string.Join(", ", Ids);

    public bool Contains(string id)
        => Ids.Contains(id, StringComparer.OrdinalIgnoreCase);

    public bool ContainsAll(IEnumerable<string> ids)
        => ids.All(Contains);

    public static WorkItemSet Parse(string raw)
    {
        var ids = raw
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

        if (ids.Length == 0)
        {
            throw new DwException("Au moins un work item est requis.", 2);
        }

        return new WorkItemSet(ids);
    }

    public static WorkItemSet? ParseOptional(string? raw)
        => string.IsNullOrWhiteSpace(raw) ? null : Parse(raw);

    public static bool SetEquals(WorkItemSet left, WorkItemSet right)
        => left.Ids.Count == right.Ids.Count &&
           left.Ids.All(right.Contains);
}
