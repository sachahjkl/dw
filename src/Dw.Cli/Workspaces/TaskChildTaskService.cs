namespace Dw.Cli.Workspaces;

internal sealed record TaskChildTaskCreateOptions(string Repository, string Title, WorkspaceOpenOptions OpenOptions);

internal static class TaskChildTaskService
{
    public static int Create(CommandContext context, TaskChildTaskCreateOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options.OpenOptions);
        var manifestPath = Path.Combine(workspace, "task.json");
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, manifestPath);
        var parent = manifest.ParentWorkItems[0];
        if (!RequiresChildTasks(parent.Type))
        {
            throw new DwException("Cette commande est reservee aux User Story et Anomalie.", 2);
        }

        var repository = options.Repository.Trim();
        if (string.IsNullOrWhiteSpace(repository))
        {
            throw new DwException("Le repo de la sous-tache est obligatoire.", 2);
        }

        var existingChildTasks = manifest.NormalizedChildTasks.ToList();

        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var adoContext = TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: true)
            ?? throw new DwException("Contexte Azure DevOps indisponible.");

        var childTasks = TaskCommand.CreateChildTasks(
            context,
            adoContext,
            new WorkItemSnapshot(parent.Id, parent.Type, parent.State, parent.Title, null),
            [repository],
            options.Title,
            "dw task create-child-task");

        existingChildTasks.AddRange(childTasks);

        var created = childTasks[0];
        var updated = manifest with { ChildTasks = existingChildTasks };
        context.FileSystem.WriteAllText(manifestPath, WorkspaceManifestWriter.Serialize(updated));
        context.Out.WriteLine($"Sous-tache enregistree dans le workspace: {repository} -> #{created.Id}");
        return 0;
    }

    private static bool RequiresChildTasks(string? workItemType)
    {
        var normalized = (workItemType ?? string.Empty).Trim().ToLowerInvariant();
        return normalized is "user story" or "anomalie";
    }
}
