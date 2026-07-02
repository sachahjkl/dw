using System.Text.Json;
using Dw.Cli.Contracts;

namespace Dw.Cli.Workspaces;

internal static class TaskHandoffValidateService
{
    private static readonly string[] AllowedStatuses = [WorkflowContracts.Handoff.StatusTodo, WorkflowContracts.Handoff.StatusInProgress, WorkflowContracts.Handoff.StatusDone, WorkflowContracts.Handoff.StatusBlocked];

    public static int Run(CommandContext context, WorkspaceOpenOptions options, bool json)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options);
        var report = BuildReport(context.FileSystem, workspace);

        if (json)
        {
            context.Out.WriteLine(JsonSerializer.Serialize(report, AppJsonContext.Default.TaskHandoffValidationReport));
        }
        else
        {
            PrintReport(context, report);
        }

        return report.IsValid ? 0 : 2;
    }

    internal static TaskHandoffValidationReport BuildReport(IFileSystem fileSystem, string workspace)
    {
        var manifest = WorkspaceManifestReader.Read(fileSystem, Path.Combine(workspace, "task.json"));
        var items = new List<TaskHandoffValidationItem>();

        foreach (var repository in manifest.Repositories.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            var path = Path.Combine(workspace, $"{WorkflowContracts.Workspace.HandoffPrefix}{repository}{WorkflowContracts.Workspace.MarkdownExtension}");
            if (!fileSystem.FileExists(path))
            {
                items.Add(new TaskHandoffValidationItem(repository, path, Status: WorkflowContracts.Handoff.StatusMissing, Valid: false, Message: "Fichier handoff manquant."));
                continue;
            }

            var text = fileSystem.ReadAllText(path);
            if (!WorkspaceHandoffService.TryParseSummary(text, repository, out var summary, out var error) || summary is null)
            {
                items.Add(new TaskHandoffValidationItem(repository, path, Status: WorkflowContracts.Handoff.StatusInvalid, Valid: false, Message: error ?? "Bloc structuré invalide."));
                continue;
            }

            if (!AllowedStatuses.Contains(summary.Status, StringComparer.OrdinalIgnoreCase))
            {
                items.Add(new TaskHandoffValidationItem(repository, path, Status: WorkflowContracts.Handoff.StatusInvalid, Valid: false, Message: $"Status handoff invalide: {summary.Status}. Attendus: {string.Join(", ", AllowedStatuses)}."));
                continue;
            }

            var status = string.Equals(summary.Status, WorkflowContracts.Handoff.StatusDone, StringComparison.OrdinalIgnoreCase)
                ? WorkflowContracts.Handoff.StatusValid
                : summary.Status.ToLowerInvariant();
            var valid = string.Equals(summary.Status, WorkflowContracts.Handoff.StatusDone, StringComparison.OrdinalIgnoreCase);
            items.Add(new TaskHandoffValidationItem(
                repository,
                path,
                Status: status,
                Valid: valid,
                Message: valid
                    ? "Handoff valide."
                    : $"Handoff parseable mais pas pret pour finish (status: {summary.Status}).",
                DoneCount: summary.Done.Count,
                DecisionCount: summary.Decisions.Count,
                RiskCount: summary.Risks.Count,
                BlockerCount: summary.Blockers.Count,
                FollowUpCount: summary.FollowUp.Count));
        }

        return new TaskHandoffValidationReport(
            SchemaVersion: WorkflowContracts.Schemas.TaskHandoffValidation,
            Workspace: workspace,
            Project: manifest.Project,
            Items: items,
            IsValid: items.All(item => item.Valid));
    }

    private static void PrintReport(CommandContext context, TaskHandoffValidationReport report)
    {
        context.Out.WriteLine($"Handoff validation: {report.Workspace}");
        context.Out.WriteLine($"Projet: {report.Project}");
        context.Out.WriteLine();

        foreach (var item in report.Items)
        {
            context.Out.WriteLine($"- [{item.Status}] {item.Repository}: {item.Message}");
            if (item.Valid)
            {
                context.Out.WriteLine($"  done={item.DoneCount} decisions={item.DecisionCount} risks={item.RiskCount} blockers={item.BlockerCount} follow_up={item.FollowUpCount}");
            }
        }

        if (!report.IsValid)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Validation handoff echouee: completer/corriger les handoffs avant task finish.");
        }
    }
}

internal sealed record TaskHandoffValidationReport(
    string SchemaVersion,
    string Workspace,
    string Project,
    IReadOnlyList<TaskHandoffValidationItem> Items,
    bool IsValid);

internal sealed record TaskHandoffValidationItem(
    string Repository,
    string Path,
    string Status,
    bool Valid,
    string Message,
    int DoneCount = 0,
    int DecisionCount = 0,
    int RiskCount = 0,
    int BlockerCount = 0,
    int FollowUpCount = 0);
