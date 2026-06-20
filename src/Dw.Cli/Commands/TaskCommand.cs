namespace Dw.Cli.Commands;

internal static class TaskCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "start" => Start(context, args.Skip(1).ToArray()),
            "status" => Status(context),
            "finish" => Finish(context, args.Skip(1).ToArray()),
            "add-repo" => AddRepo(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Start(CommandContext context, string[] args)
    {
        if (args.Length == 0)
        {
            throw new DwException("Usage: dw task start <workItemId> [--project <name>] [--slug <text>]", 2);
        }

        var workItemId = args[0];
        var project = OptionValue(args, "--project") ?? "default";
        var taskId = OptionValue(args, "--task");
        var slug = OptionValue(args, "--slug") ?? $"work-item-{workItemId}";
        var type = OptionValue(args, "--type") ?? "feat";
        var only = OptionValue(args, "--only");
        var skipAdo = args.Any(arg => string.Equals(arg, "--skip-ado", StringComparison.OrdinalIgnoreCase));
        var createChildTasks = args.Any(arg => string.Equals(arg, "--create-child-tasks", StringComparison.OrdinalIgnoreCase));

        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        var config = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = ResolveProjectConfig(config, project);
        var repositories = ResolveRepositories(projectConfig, only);
        var adoContext = skipAdo
            ? null
            : TryCreateAdoContext(context, workflow, projectConfig, required: false);
        WorkItemSnapshot? workItem = null;
        IReadOnlyDictionary<string, string>? childTaskIds = null;

        if (adoContext is not null)
        {
            workItem = adoContext.Client.GetWorkItemSnapshotAsync(workItemId, adoContext.Token).GetAwaiter().GetResult();
            context.Out.WriteLine($"ADO item {workItem.Id}: {workItem.Type} - {workItem.Title}");

            if (workflow.TaskStart?.UpdateWorkItemState ?? true)
            {
                var startState = AdoWorkflowStates.StartState(workItem.Type, workflow.TaskStart);
                if (!string.IsNullOrWhiteSpace(startState) &&
                    !string.Equals(workItem.State, startState, StringComparison.OrdinalIgnoreCase))
                {
                    UpdateWorkItemState(adoContext.Client, adoContext.Token, workItem.Id, startState, "dw task start");
                    context.Out.WriteLine($"ADO item {workItem.Id}: etat -> {startState}");
                }
            }

            if (createChildTasks || (workflow.TaskStart?.CreateChildTasks ?? false))
            {
                childTaskIds = CreateChildTasks(context, adoContext, workItem, repositories);
                if (string.IsNullOrWhiteSpace(taskId) && childTaskIds.Count == 1)
                {
                    taskId = childTaskIds.Values.First();
                }
            }
        }
        else if (!skipAdo && workflow.AzureDevOps is not null)
        {
            context.Out.WriteLine("ADO ignore: aucun token silencieux disponible. Utiliser dw auth login, DW_ADO_TOKEN, ou --skip-ado.");
        }

        var subject = GitBranchNames.BuildSubjectName(type, workItemId, slug);
        var branchName = GitBranchNames.Build(type, workItemId, taskId, slug);
        var projectRoot = Path.Combine(root, "projects", project);
        var workspace = Path.Combine(projectRoot, "workspaces", subject);

        context.FileSystem.CreateDirectory(workspace);
        var git = new GitWorktreeService(context.ProcessRunner, context.FileSystem);
        var results = new List<GitWorktreeResult>();

        foreach (var repositoryKey in repositories)
        {
            var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
                ?? new RepositoryConfig("", "main", Folder: repositoryKey);

            var folder = string.IsNullOrWhiteSpace(repository.Folder)
                ? repositoryKey
                : repository.Folder;

            var result = git.PrepareAsync(
                projectRoot,
                repositoryKey,
                repository,
                branchName,
                Path.Combine(workspace, folder)).GetAwaiter().GetResult();

            if (result.Status == GitWorktreeStatus.Failed)
            {
                throw new DwException($"Creation worktree impossible pour {repositoryKey}: {result.Message}");
            }

            results.Add(result);
        }

        var manifest = new WorkspaceManifest(
            Schema: 1,
            WorkItemId: workItemId,
            TaskId: taskId,
            Project: project,
            Type: type,
            Slug: slug,
            BranchName: branchName,
            CreatedAt: context.Clock.Now,
            Repositories: repositories,
            Status: "created",
            WorkItemType: workItem?.Type,
            WorkItemTitle: workItem?.Title,
            ChildTaskIds: childTaskIds);

        context.FileSystem.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(manifest));
        context.FileSystem.WriteAllText(Path.Combine(workspace, "plan.md"), Templates.PlanMd(workItemId, project));

        context.Out.WriteLine($"Workspace cree: {workspace}");
        context.Out.WriteLine($"Branche cible: {branchName}");
        foreach (var result in results)
        {
            context.Out.WriteLine($"Repo {result.Repository}: {result.Status} - {result.Message}");
        }

        context.Out.WriteLine("Prochaine etape: ouvrir ce dossier avec OpenCode/Codex et executer dw agent context.");
        return 0;
    }

    private static int Status(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        context.Out.WriteLine($"Root: {root}");
        context.Out.WriteLine("Workspaces detectes:");

        var files = context.FileSystem.EnumerateFiles(Path.Combine(root, "projects"), "task.json", SearchOption.AllDirectories).ToList();
        if (files.Count == 0)
        {
            context.Out.WriteLine("  Aucun workspace task trouve.");
            return 0;
        }

        foreach (var file in files)
        {
            context.Out.WriteLine($"  {Path.GetDirectoryName(file)}");
        }

        return 0;
    }

    private static int AddRepo(CommandContext context, string[] args)
    {
        var repositoryKey = args.FirstOrDefault(arg => !arg.StartsWith("-", StringComparison.Ordinal));
        if (string.IsNullOrWhiteSpace(repositoryKey))
        {
            throw new DwException("Usage: dw task add-repo <repo> [--workspace <path>]", 2);
        }

        var workspace = OptionValue(args, "--workspace") ?? Environment.CurrentDirectory;
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

    private static int Finish(CommandContext context, string[] args)
    {
        var workspace = OptionValue(args, "--workspace") ?? Environment.CurrentDirectory;
        workspace = Path.GetFullPath(workspace);
        var execute = args.Any(arg => string.Equals(arg, "--execute", StringComparison.OrdinalIgnoreCase));
        var createPr = args.Any(arg => string.Equals(arg, "--create-pr", StringComparison.OrdinalIgnoreCase));
        var draft = !args.Any(arg => string.Equals(arg, "--ready", StringComparison.OrdinalIgnoreCase));
        var message = OptionValue(args, "--message");
        var skipVerify = args.Any(arg => string.Equals(arg, "--skip-verify", StringComparison.OrdinalIgnoreCase));
        var skipAdo = args.Any(arg => string.Equals(arg, "--skip-ado", StringComparison.OrdinalIgnoreCase));

        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var statuses = new GitRepositoryStatusService(context.ProcessRunner, context.FileSystem)
            .GetStatusesAsync(workspace)
            .GetAwaiter()
            .GetResult();

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");

        foreach (var status in statuses)
        {
            context.Out.WriteLine();
            context.Out.WriteLine($"[{status.Repository}] {status.Path}");
            context.Out.WriteLine(status.IsGitRepository
                ? status.HasChanges ? "Changements detectes:" : "Aucun changement."
                : "Pas un repo Git utilisable.");

            if (!string.IsNullOrWhiteSpace(status.Detail))
            {
                context.Out.WriteLine(status.Detail);
            }
        }

        var changed = statuses.Where(status => status.IsGitRepository && status.HasChanges).ToList();
        if (changed.Count == 0)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Rien a terminer.");
            return 0;
        }

        if (!execute)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Dry-run uniquement. Relancer avec --execute --message \"...\" pour committer/pousser.");
            return 0;
        }

        if (string.IsNullOrWhiteSpace(message))
        {
            throw new DwException("task finish --execute exige --message.", 2);
        }

        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = projects.Projects.GetValueOrDefault(manifest.Project);
        var verificationResults = Array.Empty<VerificationResult>();

        if (!skipVerify && (workflow.TaskFinish?.RunVerification ?? true))
        {
            verificationResults = RunVerification(context, workflow, changed).ToArray();
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

        foreach (var status in changed)
        {
            RunGitOrThrow(context, status.Path, "add", ".");
            RunGitOrThrow(context, status.Path, "commit", "-m", CommitMessage.EnsureWorkItemReference(message, manifest));
            RunGitOrThrow(context, status.Path, "push", "-u", "origin", manifest.BranchName);
        }

        context.Out.WriteLine("Commits/push termines.");

        if (!createPr)
        {
            context.Out.WriteLine("PR non creee. Relancer avec --create-pr pour ouvrir les PR ADO.");
            return 0;
        }

        if (skipAdo)
        {
            throw new DwException("--create-pr ne peut pas etre combine avec --skip-ado.", 2);
        }

        var adoContext = skipAdo ? null : TryCreateAdoContext(context, workflow, projectConfig, required: true);
        CreatePullRequests(context, adoContext!, workflow, projectConfig, manifest, changed, draft, verificationResults);
        return 0;
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

    private static void RunGitOrThrow(CommandContext context, string workingDirectory, params string[] args)
    {
        var result = context.ProcessRunner.RunAsync("git", args, workingDirectory).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException(result.StandardError.Trim());
        }
    }

    private static AdoContext? TryCreateAdoContext(CommandContext context, WorkflowConfig workflow, ProjectConfig? projectConfig, bool required)
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

        return new AdoContext(new AzureDevOpsClient(new HttpClient(), options), token, options);
    }

    private static AzureDevOpsOptions? ResolveAzureDevOpsOptions(WorkflowConfig workflow, ProjectConfig? projectConfig)
    {
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

    private static IReadOnlyDictionary<string, string> CreateChildTasks(
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
                    new JsonPatchOperation("add", "/relations/-", new
                    {
                        rel = "System.LinkTypes.Hierarchy-Reverse",
                        url = AzureDevOpsUris.WorkItemApiUrl(adoContext.Options, parent.Id).AbsoluteUri,
                        attributes = new { comment = "creation dw task start" }
                    })
                ],
                adoContext.Token).GetAwaiter().GetResult();
            var id = TryGetString(document.RootElement, "id") ?? throw new DwException("ADO n'a pas retourne l'id de la tache creee.");
            created[repository] = id;
            context.Out.WriteLine($"ADO task creee [{repository}]: #{id} {title}");
        }

        return created;
    }

    private static void UpdateWorkItemState(AzureDevOpsClient client, TokenResult token, string workItemId, string state, string history)
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
                context.Out.WriteLine($"Verification [{status.Repository}]: {command}");
                var result = RunShell(context, status.Path, command);
                yield return new VerificationResult(status.Repository, command, result.ExitCode, result.StandardOutput, result.StandardError);
            }
        }
    }

    private static ProcessResult RunShell(CommandContext context, string workingDirectory, string command)
    {
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
        var ids = new List<string>();
        if (!string.IsNullOrWhiteSpace(manifest.TaskId))
        {
            ids.Add(manifest.TaskId);
        }

        if (manifest.ChildTaskIds is not null)
        {
            ids.AddRange(manifest.ChildTaskIds.Values.Where(id => !string.IsNullOrWhiteSpace(id)));
        }

        if (ids.Count == 0)
        {
            ids.Add(manifest.WorkItemId);
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

    private static int Help(CommandContext context)
    {
        context.Out.WriteLine("Usage: dw task <start|status|add-repo|finish>");
        return 0;
    }

    private static string? OptionValue(string[] args, string name)
    {
        for (var i = 0; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
            {
                return args[i + 1];
            }
        }

        return null;
    }

    private static ProjectConfig? ResolveProjectConfig(DevWorkflowConfig config, string project)
        => config.Projects.GetValueOrDefault(project);

    private static IReadOnlyList<string> ResolveRepositories(ProjectConfig? projectConfig, string? only)
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
    public static string EnsureWorkItemReference(string message, WorkspaceManifest manifest)
    {
        var ids = new[] { manifest.TaskId, manifest.WorkItemId }
            .Where(id => !string.IsNullOrWhiteSpace(id))
            .Select(id => $"#{id}")
            .ToArray();

        return ids.Any(id => message.Contains(id, StringComparison.OrdinalIgnoreCase))
            ? message
            : $"{message} {ids.First()}";
    }
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

        return $"[{prefix}][AI] {title}";
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
