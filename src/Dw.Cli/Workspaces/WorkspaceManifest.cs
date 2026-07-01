using System.Text.Json;

namespace Dw.Cli.Workspaces;

internal sealed record WorkspaceManifest(
    int Schema,
    string WorkItemId,
    string? TaskId,
    string Project,
    string Type,
    string Slug,
    string BranchName,
    DateTimeOffset CreatedAt,
    IReadOnlyList<string> Repositories,
    string Status,
    string? WorkItemType = null,
    string? WorkItemTitle = null,
    string? WorkItemState = null,
    IReadOnlyDictionary<string, string>? ChildTaskIds = null,
    IReadOnlyList<WorkspaceChildTask>? ChildTasks = null,
    IReadOnlyList<WorkspaceWorkItem>? WorkItems = null)
{
    public IReadOnlyList<WorkspaceWorkItem> ParentWorkItems
        => NormalizeWorkItems(WorkItemId, WorkItemType, WorkItemTitle, WorkItemState, WorkItems);

    public string PrimaryWorkItemId => ParentWorkItems[0].Id;

    public string DisplayWorkItemIds => string.Join(", ", ParentWorkItems.Select(item => item.Id));

    public string DisplayWorkItems => string.Join(", ", ParentWorkItems.Select(FormatWorkItem));

    public IReadOnlyList<WorkspaceChildTask> NormalizedChildTasks
        => NormalizeChildTasks(ChildTaskIds, ChildTasks);

    public IReadOnlyDictionary<string, string> LegacyChildTaskIds
        => NormalizedChildTasks
            .GroupBy(task => task.Repository, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.First().Id, StringComparer.OrdinalIgnoreCase);

    public IReadOnlyList<string> BranchWorkItemIds
        => ParentWorkItems.Select(item => item.Id)
            .Concat(string.IsNullOrWhiteSpace(TaskId) ? [] : [TaskId])
            .Concat(NormalizedChildTasks.Select(task => task.Id).Where(id => !string.IsNullOrWhiteSpace(id)))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

    public string DisplayBranchWorkItemIds => string.Join(", ", BranchWorkItemIds);

    public IReadOnlyList<string> AllKnownWorkItemIds => BranchWorkItemIds;

    public bool MatchesWorkItem(string workItemId)
        => AllKnownWorkItemIds.Contains(workItemId, StringComparer.OrdinalIgnoreCase);

    private static IReadOnlyList<WorkspaceWorkItem> NormalizeWorkItems(
        string workItemId,
        string? workItemType,
        string? workItemTitle,
        string? workItemState,
        IReadOnlyList<WorkspaceWorkItem>? workItems)
    {
        var normalized = (workItems ?? [])
            .Where(item => !string.IsNullOrWhiteSpace(item.Id))
            .GroupBy(item => item.Id, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First())
            .ToList();

        if (normalized.Count == 0)
        {
            normalized.Add(new WorkspaceWorkItem(workItemId, workItemType, workItemTitle, workItemState));
            return normalized;
        }

        if (!normalized.Any(item => string.Equals(item.Id, workItemId, StringComparison.OrdinalIgnoreCase)))
        {
            normalized.Insert(0, new WorkspaceWorkItem(workItemId, workItemType, workItemTitle, workItemState));
            return normalized;
        }

        normalized.Sort((left, right) => string.Equals(left.Id, workItemId, StringComparison.OrdinalIgnoreCase)
            ? -1
            : string.Equals(right.Id, workItemId, StringComparison.OrdinalIgnoreCase)
                ? 1
                : 0);

        return normalized;
    }

    public static string FormatWorkItem(WorkspaceWorkItem item)
    {
        var title = string.IsNullOrWhiteSpace(item.Title) ? "(sans titre)" : item.Title;
        return $"#{item.Id} {title}";
    }

    private static IReadOnlyList<WorkspaceChildTask> NormalizeChildTasks(
        IReadOnlyDictionary<string, string>? childTaskIds,
        IReadOnlyList<WorkspaceChildTask>? childTasks)
    {
        var normalized = (childTasks ?? [])
            .Where(task => !string.IsNullOrWhiteSpace(task.Id) && !string.IsNullOrWhiteSpace(task.Repository))
            .Select((task, index) => new { Task = task, Index = index })
            .GroupBy(entry => entry.Task.Id, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.OrderBy(entry => entry.Index).First().Task)
            .ToList();

        if (childTaskIds is null)
        {
            return normalized;
        }

        foreach (var pair in childTaskIds)
        {
            if (string.IsNullOrWhiteSpace(pair.Key) || string.IsNullOrWhiteSpace(pair.Value))
            {
                continue;
            }

            if (normalized.Any(task => string.Equals(task.Id, pair.Value, StringComparison.OrdinalIgnoreCase)))
            {
                continue;
            }

            normalized.Add(new WorkspaceChildTask(pair.Key, pair.Value));
        }

        return normalized;
    }
}

internal sealed record WorkspaceWorkItem(
    string Id,
    string? Type = null,
    string? Title = null,
    string? State = null);

internal sealed record WorkspaceChildTask(
    string Repository,
    string Id,
    string? Title = null);

internal static class WorkspaceManifestWriter
{
    public static string Serialize(WorkspaceManifest manifest)
        => JsonSerializer.Serialize(manifest, AppJsonContext.Default.WorkspaceManifest);
}

internal static class WorkspaceManifestReader
{
    public static WorkspaceManifest Read(IFileSystem fileSystem, string path)
    {
        if (!fileSystem.FileExists(path))
        {
            throw new DwException($"Manifest task introuvable: {path}");
        }

        return JsonSerializer.Deserialize(fileSystem.ReadAllText(path), AppJsonContext.Default.WorkspaceManifest)
               ?? throw new DwException($"Manifest task invalide: {path}");
    }
}
