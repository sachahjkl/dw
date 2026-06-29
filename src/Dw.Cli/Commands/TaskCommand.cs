using System.Diagnostics;
using Dw.Cli.Agents;

namespace Dw.Cli.Commands;

internal static class TaskCommand
{
    internal static int AddRepo(CommandContext context, TaskAddRepoOptions options)
    {
        var repositoryKey = options.Repository;
        var workspace = options.Workspace ?? Environment.CurrentDirectory;
        workspace = Path.GetFullPath(workspace);
        var manifestPath = Path.Combine(workspace, "task.json");
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, manifestPath);
        if (manifest.Repositories.Contains(repositoryKey, StringComparer.OrdinalIgnoreCase))
        {
            context.Out.WriteLine($"Repo deja present dans le workspace: {repositoryKey}");
            return 0;
        }

        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = projects.Projects.GetValueOrDefault(manifest.Project);
        var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
            ?? throw new DwException($"Repo inconnu dans projects.json pour {manifest.Project}: {repositoryKey}", 2);
        var projectRoot = Path.Combine(root, "projects", manifest.Project);
        var folder = string.IsNullOrWhiteSpace(repository.Folder) ? repositoryKey : repository.Folder;
        var result = new GitWorktreeService(context.ProcessRunner, context.FileSystem)
            .PrepareAsync(projectRoot, repositoryKey, repository, manifest.BranchName, Path.Combine(workspace, folder))
            .GetAwaiter()
            .GetResult();

        if (result.Status == GitWorktreeStatus.Failed)
        {
            throw new DwException($"Creation worktree impossible pour {repositoryKey}: {result.Message}");
        }

        var updated = manifest with
        {
            Repositories = manifest.Repositories.Concat([repositoryKey]).Distinct(StringComparer.OrdinalIgnoreCase).ToArray()
        };
        context.FileSystem.WriteAllText(manifestPath, WorkspaceManifestWriter.Serialize(updated));
        context.Out.WriteLine($"Repo ajoute: {repositoryKey} - {result.Status} - {result.Message}");
        return 0;
    }

    internal static int Finish(CommandContext context, TaskFinishRequest request)
    {
        var workspace = ResolveWorkspacePath(context, request.Workspace, request.Continue);
        var draft = !request.Ready;

        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projectConfig = ResolveProjectConfig(context, manifest.Project);
        var statuses = new GitRepositoryStatusService(context.ProcessRunner, context.FileSystem)
            .GetStatusesAsync(workspace, projectConfig)
            .GetAwaiter()
            .GetResult();

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");
        PrintStatuses(context, statuses);

        var changed = statuses.Where(status => status.IsGitRepository && status.HasChanges).ToList();
        var unpushed = statuses.Where(status => status.IsGitRepository && status.HasUnpushed).ToList();
        var actionable = changed.Count > 0 ? changed : unpushed;
        var stageCommit = changed.Count > 0;

        if (actionable.Count == 0)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Rien a terminer.");
            return 0;
        }

        if (!request.Execute)
        {
            context.Out.WriteLine();
            context.Out.WriteLine(stageCommit
                ? "Dry-run uniquement. Relancer avec --execute pour committer/pousser."
                : "Dry-run uniquement. Relancer avec --execute pour pousser/creer PR.");
            return 0;
        }

        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var verificationResults = Array.Empty<VerificationResult>();

        if (!request.SkipVerify && (workflow.TaskFinish?.RunVerification ?? true))
        {
            verificationResults = RunVerification(context, workflow, actionable).ToArray();
            var failed = verificationResults.Where(result => result.ExitCode != 0).ToArray();
            if (failed.Length > 0)
            {
                foreach (var result in failed)
                {
                    context.Error.WriteLine($"Verification echouee [{result.Repository}]: {result.Command}");
                    if (!string.IsNullOrWhiteSpace(result.StandardError))
                    {
                        context.Error.WriteLine(result.StandardError.Trim());
                    }
                }

                throw new DwException("task finish bloque: verification echouee.");
            }
        }

        if (stageCommit)
        {
            var commitMessage = CommitMessage.Build(manifest, request.Message);
            foreach (var status in changed)
            {
                RunGitOrThrow(context, status.Path, "add", ".");
                RunGitOrThrow(context, status.Path, "commit", "-m", commitMessage);
                RunGitOrThrow(context, status.Path, "push", "-u", "origin", manifest.BranchName);
            }
        }
        else
        {
            foreach (var status in unpushed)
            {
                RunGitOrThrow(context, status.Path, "push", "-u", "origin", manifest.BranchName);
            }
        }

        context.Out.WriteLine(stageCommit ? "Commits/push termines." : "Push termine.");

        if (!request.CreatePr)
        {
            context.Out.WriteLine("PR non creee. Relancer avec --create-pr pour ouvrir les PR ADO.");
            return 0;
        }

        if (request.SkipAdo)
        {
            throw new DwException("--create-pr ne peut pas etre combine avec --skip-ado.", 2);
        }

        var adoContext = request.SkipAdo ? null : TryCreateAdoContext(context, workflow, projectConfig, required: true);
        if (adoContext is null)
        {
            throw new DwException("Contexte Azure DevOps indisponible.");
        }

        CreatePullRequests(context, adoContext, workflow, projectConfig, manifest, actionable, draft, verificationResults);
        return 0;
    }

    internal static int Commit(CommandContext context, TaskCommitRequest request)
    {
        var workspace = ResolveWorkspacePath(context, request.Workspace, request.Continue);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projectConfig = ResolveProjectConfig(context, manifest.Project);
        var statuses = new GitRepositoryStatusService(context.ProcessRunner, context.FileSystem)
            .GetStatusesAsync(workspace, projectConfig)
            .GetAwaiter()
            .GetResult();

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");
        PrintStatuses(context, statuses);

        var changed = statuses.Where(status => status.IsGitRepository && status.HasChanges).ToList();
        if (changed.Count == 0)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Rien a committer.");
            return 0;
        }

        var commitMessage = CommitMessage.Build(manifest, request.Message);
        context.Out.WriteLine();
        context.Out.WriteLine($"Message: {commitMessage}");

        if (!request.Execute)
        {
            context.Out.WriteLine("Dry-run uniquement. Relancer avec --execute pour committer.");
            return 0;
        }

        foreach (var status in changed)
        {
            RunGitOrThrow(context, status.Path, "add", ".");
            RunGitOrThrow(context, status.Path, "commit", "-m", commitMessage);
        }

        context.Out.WriteLine("Commits termines. Aucun push ni PR creee.");
        return 0;
    }

    private static string ResolveWorkspacePath(CommandContext context, string? workspace, bool useLatestWorkspace)
    {
        if (!string.IsNullOrWhiteSpace(workspace))
        {
            return Path.GetFullPath(Environment.ExpandEnvironmentVariables(workspace));
        }

        if (useLatestWorkspace)
        {
            var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
            return WorkspaceOpenService.ResolveWorkspace(context, root, new WorkspaceOpenOptions(null, null, null, Continue: true));
        }

        return WorkspaceCurrentService.FindWorkspacePath(context.FileSystem, Environment.CurrentDirectory)
               ?? throw new DwException("Aucun workspace task trouve depuis le dossier courant. Utiliser --workspace ou --continue.", 2);
    }

    private static ProjectConfig? ResolveProjectConfig(CommandContext context, string project)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        return DevWorkflowConfigLoader.ResolveProject(projects, project);
    }

    private static void PrintStatuses(CommandContext context, IReadOnlyList<RepositoryStatus> statuses)
    {
        foreach (var status in statuses)
        {
            context.Out.WriteLine();
            context.Out.WriteLine($"[{status.Repository}] {status.Path}");
            if (!status.IsGitRepository)
            {
                context.Out.WriteLine("Pas un repo Git utilisable.");
            }
            else if (status.HasChanges)
            {
                context.Out.WriteLine("Changements detectes:");
            }
            else if (status.HasUnpushed)
            {
                context.Out.WriteLine("Commits non pousses.");
            }
            else
            {
                context.Out.WriteLine("Aucun changement.");
            }

            if (!string.IsNullOrWhiteSpace(status.Detail))
            {
                context.Out.WriteLine(status.Detail);
            }
        }
    }

    private static void CreatePullRequests(
        CommandContext context,
        AdoContext adoContext,
        WorkflowConfig workflow,
        ProjectConfig? projectConfig,
        WorkspaceManifest manifest,
        IReadOnlyList<RepositoryStatus> changed,
        bool draft,
        IReadOnlyList<VerificationResult> verificationResults)
    {
        foreach (var status in changed)
        {
            var repo = projectConfig?.Repositories.GetValueOrDefault(status.Repository);
            var adoRepo = repo?.AzureDevOpsRepository;
            if (string.IsNullOrWhiteSpace(adoRepo))
            {
                context.Out.WriteLine($"PR ignoree pour {status.Repository}: azureDevOpsRepository manquant.");
                continue;
            }

            var target = repo?.PullRequestTargetBranch ?? repo?.DefaultBranch ?? "main";
            var request = new CreatePullRequestRequest(
                SourceRefName: $"refs/heads/{manifest.BranchName}",
                TargetRefName: $"refs/heads/{target}",
                Title: PullRequestText.Title(manifest),
                Description: PullRequestText.Description(manifest, status, ReadPlan(context, status.Path), verificationResults),
                IsDraft: draft,
                WorkItemRefs: WorkItemRefsFor(manifest));

            using var response = adoContext.Client.CreatePullRequestAsync(adoRepo, request, adoContext.Token).GetAwaiter().GetResult();
            var url = TryGetString(response.RootElement, "url") ?? "(url non retournee)";
            var pullRequestId = TryGetInt(response.RootElement, "pullRequestId");
            if (pullRequestId is not null)
            {
                foreach (var id in WorkItemIdsFor(manifest))
                {
                    try
                    {
                        adoContext.Client.LinkWorkItemToPullRequestAsync(adoRepo, pullRequestId.Value, id, adoContext.Token)
                            .GetAwaiter()
                            .GetResult()
                            .Dispose();
                    }
                    catch (DwException ex)
                    {
                        context.Out.WriteLine($"Lien PR/work item deja demande a la creation, lien explicite ignore pour #{id}: {ex.Message}");
                    }
                }
            }

            context.Out.WriteLine($"PR creee pour {status.Repository}: {url}");
        }

        if (workflow.TaskFinish?.UpdateWorkItemState ?? true)
        {
            UpdateFinishStates(context, adoContext, workflow, manifest);
        }
    }

    internal static void RunGitOrThrow(CommandContext context, string workingDirectory, params string[] args)
    {
        context.Debug($"git ({workingDirectory}): {string.Join(' ', args)}");
        var result = context.ProcessRunner.RunAsync("git", args, workingDirectory).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException(result.StandardError.Trim());
        }
    }

    internal static AdoContext? TryCreateAdoContext(CommandContext context, WorkflowConfig workflow, ProjectConfig? projectConfig, bool required)
    {
        var options = ResolveAzureDevOpsOptions(workflow, projectConfig);
        if (options is null)
        {
            if (required)
            {
                throw new DwException("Configuration azureDevOps manquante dans workflow.json.");
            }

            return null;
        }

        var tokenProvider = new AzureDevOpsTokenProvider(workflow.Auth);
        var token = tokenProvider.GetTokenSilentOrEnvironmentAsync().GetAwaiter().GetResult();
        if (token is null)
        {
            if (required)
            {
                throw new DwException("Non connecte a Azure DevOps. Executer dw auth login ou definir DW_ADO_TOKEN.");
            }

            return null;
        }

        context.Debug($"ADO token resolu via {token.Scheme}");
        return new AdoContext(new AzureDevOpsClient(new HttpClient(), options), token, options);
    }

    private static AzureDevOpsOptions? ResolveAzureDevOpsOptions(WorkflowConfig workflow, ProjectConfig? projectConfig)
    {
        if (projectConfig?.IncludedProjects is { Length: > 0 } && projectConfig.AzureDevOps is null)
        {
            return null;
        }

        if (projectConfig?.AzureDevOps is null)
        {
            return workflow.AzureDevOps;
        }

        if (workflow.AzureDevOps is null)
        {
            return projectConfig.AzureDevOps;
        }

        return new AzureDevOpsOptions(
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.OrganizationUrl)
                ? workflow.AzureDevOps.OrganizationUrl
                : projectConfig.AzureDevOps.OrganizationUrl,
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.Project)
                ? workflow.AzureDevOps.Project
                : projectConfig.AzureDevOps.Project,
            string.IsNullOrWhiteSpace(projectConfig.AzureDevOps.ApiVersion)
                ? workflow.AzureDevOps.ApiVersion
                : projectConfig.AzureDevOps.ApiVersion);
    }

    internal static IReadOnlyDictionary<string, string> CreateChildTasks(
        CommandContext context,
        AdoContext adoContext,
        WorkItemSnapshot parent,
        IReadOnlyList<string> repositories)
    {
        var created = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var repository in repositories)
        {
            var title = AdoTaskNaming.ChildTaskTitle(repository, parent.Title ?? $"Work item {parent.Id}");
            using var document = adoContext.Client.CreateWorkItemAsync("Task",
                [
                    new JsonPatchOperation("add", "/fields/System.Title", title),
                    new JsonPatchOperation("add", "/fields/System.History", DevWorkflowTraceComment(parent.Id, repository)),
                    new JsonPatchOperation("add", "/relations/-", new WorkItemRelationRef(
                        "System.LinkTypes.Hierarchy-Reverse",
                        AzureDevOpsUris.WorkItemApiUrl(adoContext.Options, parent.Id).AbsoluteUri,
                        new WorkItemRelationAttributes("creation dw task start")))
                ],
                adoContext.Token).GetAwaiter().GetResult();
            var id = TryGetString(document.RootElement, "id") ?? throw new DwException("ADO n'a pas retourne l'id de la tache creee.");
            created[repository] = id;
            context.Out.WriteLine($"ADO task creee [{repository}]: #{id} {title}");
        }

        return created;
    }

    private static string DevWorkflowTraceComment(string parentId, string repository)
        => $"Créé automatiquement par Dev Workflow {AppVersion.InformationalVersion()} via dw task start. Parent #{parentId}. Repository: {repository}.";

    internal static void UpdateWorkItemState(AzureDevOpsClient client, TokenResult token, string workItemId, string state, string history)
    {
        client.UpdateWorkItemAsync(workItemId,
            [
                new JsonPatchOperation("add", "/fields/System.History", history),
                new JsonPatchOperation("add", "/fields/System.State", state)
            ],
            token).GetAwaiter().GetResult().Dispose();
    }

    private static void UpdateFinishStates(CommandContext context, AdoContext adoContext, WorkflowConfig workflow, WorkspaceManifest manifest)
    {
        foreach (var id in WorkItemIdsFor(manifest))
        {
            var item = adoContext.Client.GetWorkItemSnapshotAsync(id, adoContext.Token).GetAwaiter().GetResult();
            var state = AdoWorkflowStates.FinishState(item.Type ?? manifest.WorkItemType, workflow.TaskFinish);
            if (string.IsNullOrWhiteSpace(state))
            {
                context.Out.WriteLine($"ADO item {id}: etat inchange ({item.Type}).");
                continue;
            }

            if (string.Equals(item.State, state, StringComparison.OrdinalIgnoreCase))
            {
                context.Out.WriteLine($"ADO item {id}: deja en etat {state}.");
                continue;
            }

            UpdateWorkItemState(adoContext.Client, adoContext.Token, id, state, "dw task finish: PR ouverte");
            context.Out.WriteLine($"ADO item {id}: etat -> {state}");
        }
    }

    private static IEnumerable<VerificationResult> RunVerification(
        CommandContext context,
        WorkflowConfig workflow,
        IReadOnlyList<RepositoryStatus> changed)
    {
        var configured = workflow.TaskFinish?.VerificationCommands;
        if (configured is null || configured.Count == 0)
        {
            context.Out.WriteLine("Verification: aucune commande configuree.");
            yield break;
        }

        foreach (var status in changed)
        {
            if (!configured.TryGetValue(status.Repository, out var commands))
            {
                continue;
            }

            foreach (var command in commands)
            {
                var resolvedCommand = ResolveNodePackageManagerCommand(context, command);
                context.Out.WriteLine($"Verification [{status.Repository}]: {resolvedCommand}");
                var result = RunShell(context, status.Path, resolvedCommand);
                yield return new VerificationResult(status.Repository, resolvedCommand, result.ExitCode, result.StandardOutput, result.StandardError);
            }
        }
    }

    internal static string ResolveNodePackageManagerCommand(CommandContext context, string command)
    {
        var trimmed = command.TrimStart();
        if (!trimmed.StartsWith("npm ", StringComparison.OrdinalIgnoreCase))
        {
            return command;
        }

        if (!IsCommandAvailable(context, "pnpm"))
        {
            return command;
        }

        var leadingWhitespaceLength = command.Length - trimmed.Length;
        return string.Concat(command.AsSpan(0, leadingWhitespaceLength), "pnpm", trimmed.AsSpan("npm".Length));
    }

    private static bool IsCommandAvailable(CommandContext context, string command)
    {
        try
        {
            var result = OperatingSystem.IsWindows()
                ? context.ProcessRunner.RunAsync("cmd", $"/c {command} --version").GetAwaiter().GetResult()
                : context.ProcessRunner.RunAsync(command, "--version").GetAwaiter().GetResult();
            return result.ExitCode == 0;
        }
        catch
        {
            return false;
        }
    }

    private static ProcessResult RunShell(CommandContext context, string workingDirectory, string command)
    {
        context.Debug($"shell ({workingDirectory}): {command}");
        var shell = OperatingSystem.IsWindows() ? "powershell" : "sh";
        var args = OperatingSystem.IsWindows()
            ? new[] { "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command }
            : ["-lc", command];
        return context.ProcessRunner.RunAsync(shell, args, workingDirectory).GetAwaiter().GetResult();
    }

    private static string ReadPlan(CommandContext context, string repositoryPath)
    {
        var workspace = Directory.GetParent(repositoryPath)?.FullName ?? repositoryPath;
        var planPath = Path.Combine(workspace, "plan.md");
        return context.FileSystem.FileExists(planPath) ? context.FileSystem.ReadAllText(planPath) : string.Empty;
    }

    private static IReadOnlyList<ResourceRef> WorkItemRefsFor(WorkspaceManifest manifest)
        => WorkItemIdsFor(manifest).Select(id => new ResourceRef(id)).ToArray();

    private static IReadOnlyList<string> WorkItemIdsFor(WorkspaceManifest manifest)
    {
        var ids = manifest.ParentWorkItems
            .Select(item => item.Id)
            .Where(id => !string.IsNullOrWhiteSpace(id))
            .ToList();

        if (!string.IsNullOrWhiteSpace(manifest.TaskId))
        {
            ids.Add(manifest.TaskId);
        }

        if (manifest.ChildTaskIds is not null)
        {
            ids.AddRange(manifest.ChildTaskIds.Values.Where(id => !string.IsNullOrWhiteSpace(id)));
        }

        return ids.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
    }

    private static string? TryGetString(System.Text.Json.JsonElement element, string property)
    {
        if (!element.TryGetProperty(property, out var value))
        {
            return null;
        }

        return value.ValueKind == System.Text.Json.JsonValueKind.String ? value.GetString() : value.GetRawText().Trim('"');
    }

    private static int? TryGetInt(System.Text.Json.JsonElement element, string property)
        => element.TryGetProperty(property, out var value) && value.TryGetInt32(out var id) ? id : null;

    private static ProjectConfig? ResolveProjectConfig(DevWorkflowConfig config, string project)
        => DevWorkflowConfigLoader.ResolveProject(config, project);

    internal static string ResolveSlug(string? requestedSlug, string workItemId, WorkItemSnapshot? workItem)
    {
        if (!string.IsNullOrWhiteSpace(requestedSlug))
        {
            return Slug.FromPhraseOrFallback(requestedSlug, $"work-item-{workItemId}");
        }

        var fallbackSource = workItem?.Title;
        if (!string.IsNullOrWhiteSpace(fallbackSource))
        {
            fallbackSource = StripDecorations(fallbackSource);
        }

        return Slug.FromPhraseOrFallback(fallbackSource, $"work-item-{workItemId}");
    }

    private static string StripDecorations(string value)
    {
        Debug.Assert(!string.IsNullOrWhiteSpace(value), "Work item title should not be empty when stripping decorations.");
        return string.Join(' ', value
            .Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(part => !part.StartsWith("[", StringComparison.Ordinal) && !part.EndsWith("]", StringComparison.Ordinal)));
    }

    internal static bool IsFinalState(string? workItemType, string? state)
    {
        var normalizedState = (state ?? string.Empty).Trim().ToLowerInvariant();
        if (string.IsNullOrWhiteSpace(normalizedState))
        {
            return false;
        }

        var normalizedType = (workItemType ?? string.Empty).Trim().ToLowerInvariant();
        return normalizedType switch
        {
            "user story" or "anomalie" => normalizedState is "validé" or "valide" or "cloturé" or "clôturé" or "abandonné" or "abandonne",
            "bug" or "activité" or "activite" => normalizedState is "cloturé" or "clôturé" or "abandonné" or "abandonne",
            _ => normalizedState is "validé" or "valide" or "cloturé" or "clôturé" or "abandonné" or "abandonne"
        };
    }

    internal static IReadOnlyList<string> ResolveRepositories(ProjectConfig? projectConfig, string? only)
    {
        if (!string.IsNullOrWhiteSpace(only))
        {
            return only.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        }

        if (projectConfig is not null && projectConfig.Repositories.Count > 0)
        {
            return projectConfig.Repositories.Keys.ToArray();
        }

        return ["front", "back"];
    }
}

internal sealed record AdoContext(AzureDevOpsClient Client, TokenResult Token, AzureDevOpsOptions Options);

internal sealed record VerificationResult(
    string Repository,
    string Command,
    int ExitCode,
    string StandardOutput,
    string StandardError);

internal static class CommitMessage
{
    public static bool IsWellFormed(string message)
        => AdoRegexes.CommitMessage().IsMatch(message);

    public static string Build(WorkspaceManifest manifest, string? overrideMessage = null)
    {
        if (!string.IsNullOrWhiteSpace(overrideMessage))
        {
            return EnsureWorkItemReference(overrideMessage, manifest);
        }

        return $"{Prefix(manifest)}({Ids(manifest)}): {manifest.Slug}";
    }

    public static string EnsureWorkItemReference(string message, WorkspaceManifest manifest)
    {
        var ids = new[] { manifest.TaskId, manifest.WorkItemId }
            .Concat(manifest.ParentWorkItems.Select(item => item.Id))
            .Concat((manifest.ChildTaskIds ?? new Dictionary<string, string>()).Values)
            .Where(id => !string.IsNullOrWhiteSpace(id))
            .Select(id => $"#{id}")
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

        return ids.Any(id => message.Contains(id, StringComparison.OrdinalIgnoreCase))
            ? message
            : $"{message} {ids.First()}";
    }

    private static string Prefix(WorkspaceManifest manifest)
        => manifest.Type.ToLowerInvariant() switch
        {
            "feat" => "feat",
            "fix" => "fix",
            "bug" => "bug",
            "chore" => "chore",
            "refactor" => "refactor",
            "test" => "test",
            _ => manifest.Type.ToLowerInvariant()
        };

    private static string Ids(WorkspaceManifest manifest)
        => string.Join(' ', manifest.ParentWorkItems.Select(item => $"#{item.Id}")
            .Concat(string.IsNullOrWhiteSpace(manifest.TaskId) ? [] : [$"#{manifest.TaskId}"])
            .Concat((manifest.ChildTaskIds ?? new Dictionary<string, string>()).Values.Select(id => $"#{id}"))
            .Distinct(StringComparer.OrdinalIgnoreCase));
}

internal static class AdoTaskNaming
{
    public static string ChildTaskTitle(string repository, string title)
    {
        var prefix = repository.Equals("front", StringComparison.OrdinalIgnoreCase)
            ? "FRONT"
            : repository.Equals("back", StringComparison.OrdinalIgnoreCase)
                ? "BACK"
                : repository.ToUpperInvariant();

        return $"[{prefix}] {title}";
    }
}

internal static class AdoWorkflowStates
{
    public static string? StartState(string? workItemType, TaskStartOptions? options)
    {
        var normalized = NormalizeType(workItemType);
        return normalized switch
        {
            "user story" => options?.UserStoryState ?? "En réalisation",
            "anomalie" => options?.AnomalyState ?? "En réalisation",
            "bug" => options?.BugState ?? "En développement",
            "task" => options?.TaskState ?? "En développement",
            "tache" => options?.TaskState ?? "En développement",
            _ => null
        };
    }

    public static string? FinishState(string? workItemType, TaskFinishOptions? options)
    {
        var normalized = NormalizeType(workItemType);
        return normalized switch
        {
            "bug" => options?.BugState ?? "PR en attente",
            "task" => options?.TaskState ?? "PR en attente",
            "tache" => options?.TaskState ?? "PR en attente",
            _ => null
        };
    }

    private static string NormalizeType(string? workItemType)
        => (workItemType ?? string.Empty).Trim().ToLowerInvariant();
}

internal sealed record TaskAddRepoOptions(string Repository, string? Workspace);

internal sealed record TaskCommitRequest(string? Workspace, bool Continue, bool Execute, string? Message);

internal sealed record TaskFinishRequest(
    string? Workspace,
    bool Continue,
    bool Execute,
    bool CreatePr,
    bool Ready,
    string? Message,
    bool SkipVerify,
    bool SkipAdo);
