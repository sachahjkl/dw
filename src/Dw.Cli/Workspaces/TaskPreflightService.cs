using System.Text.Json;
using Dw.Cli.Contracts;

namespace Dw.Cli.Workspaces;

internal static class TaskPreflightService
{
    public static int Run(CommandContext context, WorkspaceOpenOptions options, bool json)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options);
        var report = BuildReport(context, root, workspace);

        if (json)
        {
            context.Out.WriteLine(JsonSerializer.Serialize(report, AppJsonContext.Default.TaskPreflightReport));
        }
        else
        {
            PrintReport(context, report);
        }

        return report.HasBlockingIssues ? 2 : 0;
    }

    internal static TaskPreflightReport BuildReport(CommandContext context, string root, string workspace)
    {
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var adoContext = TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: true)
            ?? throw new DwException("Contexte Azure DevOps indisponible.");

        var issues = new List<TaskPreflightIssue>();
        foreach (var workItem in manifest.ParentWorkItems)
        {
            using var document = adoContext.Client.GetWorkItemExpandedAsync(workItem.Id, adoContext.Token).GetAwaiter().GetResult();
            var aiContext = AdoCommand.MapAiContextItem(document.RootElement, adoContext.Options, summaryOnly: false);

            issues.AddRange(BuildPredecessorIssues(adoContext, aiContext));
            issues.AddRange(BuildChildTaskIssues(adoContext, aiContext, manifest));
            issues.AddRange(BuildStaleContextIssues(aiContext, manifest));
            issues.AddRange(BuildAttachmentIssues(aiContext));
        }

        return new TaskPreflightReport(
            SchemaVersion: WorkflowContracts.Schemas.TaskPreflight,
            Workspace: workspace,
            Project: manifest.Project,
            WorkItemIds: manifest.ParentWorkItems.Select(item => item.Id).ToArray(),
            Issues: issues,
            HasBlockingIssues: issues.Any(issue => issue.Severity == WorkflowContracts.Preflight.SeverityBlocking));
    }

    private static IReadOnlyList<TaskPreflightIssue> BuildPredecessorIssues(AdoContext adoContext, AdoAiContextItem aiContext)
    {
        if (aiContext.Links.PredecessorIds.Count == 0)
        {
            return [];
        }

        var blocking = adoContext.Client
            .GetWorkItemSnapshotsAsync(aiContext.Links.PredecessorIds, adoContext.Token)
            .GetAwaiter()
            .GetResult()
            .Where(snapshot => !TaskCommand.IsFinalState(snapshot.Type, snapshot.State))
            .ToArray();
        if (blocking.Length == 0)
        {
            return [];
        }

        return
        [
            new TaskPreflightIssue(
                Code: WorkflowContracts.Preflight.CodePredecessorsActive,
                Severity: WorkflowContracts.Preflight.SeverityBlocking,
                WorkItemId: aiContext.WorkItem.Id,
                Message: $"Le work item #{aiContext.WorkItem.Id} a des predecesseurs non termines.",
                Details: $"Predecesseurs actifs: {string.Join(", ", blocking.Select(snapshot => $"#{snapshot.Id} [{snapshot.State ?? "?"}]"))}",
                RelatedIds: blocking.Select(snapshot => snapshot.Id).ToArray())
        ];
    }

    private static IReadOnlyList<TaskPreflightIssue> BuildChildTaskIssues(AdoContext adoContext, AdoAiContextItem aiContext, WorkspaceManifest manifest)
    {
        if (aiContext.Links.ChildIds.Count == 0)
        {
            return [];
        }

        var activeChildren = adoContext.Client
            .GetWorkItemSnapshotsAsync(aiContext.Links.ChildIds, adoContext.Token)
            .GetAwaiter()
            .GetResult()
            .Where(snapshot => !TaskCommand.IsFinalState(snapshot.Type, snapshot.State))
            .ToArray();
        if (activeChildren.Length == 0)
        {
            return [];
        }

        var knownChildIds = manifest.NormalizedChildTasks.Select(task => task.Id)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        var known = activeChildren.Where(snapshot => knownChildIds.Contains(snapshot.Id)).Select(snapshot => snapshot.Id).ToArray();
        var details = $"Enfants actifs detectes: {string.Join(", ", activeChildren.Select(snapshot => $"#{snapshot.Id} [{snapshot.State ?? "?"}]"))}";
        if (known.Length > 0)
        {
            details += $". Deja relies au workspace: {string.Join(", ", known.Select(id => $"#{id}"))}";
        }

        return
        [
            new TaskPreflightIssue(
                Code: WorkflowContracts.Preflight.CodeChildrenActive,
                Severity: WorkflowContracts.Preflight.SeverityWarning,
                WorkItemId: aiContext.WorkItem.Id,
                Message: $"Le work item #{aiContext.WorkItem.Id} a deja des enfants ADO actifs.",
                Details: details,
                RelatedIds: activeChildren.Select(snapshot => snapshot.Id).ToArray())
        ];
    }

    private static IReadOnlyList<TaskPreflightIssue> BuildStaleContextIssues(AdoAiContextItem aiContext, WorkspaceManifest manifest)
    {
        var manifestItem = manifest.ParentWorkItems.FirstOrDefault(item => string.Equals(item.Id, aiContext.WorkItem.Id, StringComparison.OrdinalIgnoreCase));
        if (manifestItem is null)
        {
            return [];
        }

        var staleReasons = new List<string>();
        if (!string.Equals(manifestItem.Title, aiContext.WorkItem.Title, StringComparison.Ordinal))
        {
            staleReasons.Add("titre local different d'ADO");
        }

        if (!string.Equals(manifestItem.State, aiContext.WorkItem.State, StringComparison.Ordinal))
        {
            staleReasons.Add("etat local different d'ADO");
        }

        if (!string.Equals(manifestItem.Type, aiContext.WorkItem.Type, StringComparison.Ordinal))
        {
            staleReasons.Add("type local different d'ADO");
        }

        if (staleReasons.Count == 0)
        {
            return [];
        }

        return
        [
            new TaskPreflightIssue(
                Code: WorkflowContracts.Preflight.CodeContextStale,
                Severity: WorkflowContracts.Preflight.SeverityWarning,
                WorkItemId: aiContext.WorkItem.Id,
                Message: $"Le contexte ADO local du workspace semble stale pour #{aiContext.WorkItem.Id}.",
                Details: string.Join("; ", staleReasons),
                RelatedIds: [aiContext.WorkItem.Id])
        ];
    }

    private static IReadOnlyList<TaskPreflightIssue> BuildAttachmentIssues(AdoAiContextItem aiContext)
    {
        if (aiContext.Attachments.Items.Count == 0)
        {
            return [];
        }

        var names = aiContext.Attachments.Items
            .Select(item => item.Name)
            .Where(name => !string.IsNullOrWhiteSpace(name))
            .Cast<string>()
            .ToArray();

        return
        [
            new TaskPreflightIssue(
                Code: WorkflowContracts.Preflight.CodeAttachmentsPresent,
                Severity: WorkflowContracts.Preflight.SeverityWarning,
                WorkItemId: aiContext.WorkItem.Id,
                Message: $"Le work item #{aiContext.WorkItem.Id} a des pieces jointes a traiter comme source factuelle.",
                Details: names.Length == 0
                    ? $"Pieces jointes presentes. Dossier attendu: {aiContext.Attachments.DirectoryHint}"
                    : $"Pieces jointes presentes: {string.Join(", ", names)}. Dossier attendu: {aiContext.Attachments.DirectoryHint}",
                RelatedIds: [aiContext.WorkItem.Id])
        ];
    }

    private static void PrintReport(CommandContext context, TaskPreflightReport report)
    {
        context.Out.WriteLine($"Preflight workspace: {report.Workspace}");
        context.Out.WriteLine($"Projet: {report.Project}");
        context.Out.WriteLine($"Work items: {string.Join(", ", report.WorkItemIds.Select(id => $"#{id}"))}");
        context.Out.WriteLine();

        if (report.Issues.Count == 0)
        {
            context.Out.WriteLine("Aucun warning ni blocage detecte.");
            return;
        }

        foreach (var issue in report.Issues)
        {
            context.Out.WriteLine($"- [{issue.Severity}] {issue.Code}: {issue.Message}");
            if (!string.IsNullOrWhiteSpace(issue.Details))
            {
                context.Out.WriteLine($"  {issue.Details}");
            }
        }

        if (report.HasBlockingIssues)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Blocages detectes: demander confirmation utilisateur avant de forcer l'implementation.");
        }
    }
}

internal sealed record TaskPreflightReport(
    string SchemaVersion,
    string Workspace,
    string Project,
    IReadOnlyList<string> WorkItemIds,
    IReadOnlyList<TaskPreflightIssue> Issues,
    bool HasBlockingIssues);

internal sealed record TaskPreflightIssue(
    string Code,
    string Severity,
    string WorkItemId,
    string Message,
    string? Details,
    IReadOnlyList<string> RelatedIds);
