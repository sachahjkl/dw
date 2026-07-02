namespace Dw.Cli.Commands;

internal static partial class TaskCommand
{
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
        var handoffSummaries = statuses
            .Where(status => status.IsGitRepository)
            .ToDictionary(
                status => status.Repository,
                status => WorkspaceHandoffService.ReadRequiredSummary(context.FileSystem, workspace, status.Repository),
                StringComparer.OrdinalIgnoreCase);

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");
        PrintStatuses(context, statuses);
        PrintHandoffSummaries(context, statuses, handoffSummaries);

        var changed = statuses.Where(status => status.IsGitRepository && status.HasChanges).ToList();
        var unpushed = statuses.Where(status => status.IsGitRepository && status.HasUnpushed).ToList();
        var actionable = changed.Count > 0 ? changed : unpushed;
        var pullRequestCandidates = request.CreatePr
            ? SelectPullRequestCandidates(context, statuses, actionable, projectConfig)
            : actionable;
        var stageCommit = changed.Count > 0;

        if (actionable.Count == 0 && pullRequestCandidates.Count == 0)
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

        if (stageCommit)
        {
            context.Out.WriteLine("Commits/push termines.");
        }
        else if (unpushed.Count > 0)
        {
            context.Out.WriteLine("Push termine.");
        }
        else if (request.CreatePr)
        {
            context.Out.WriteLine("Aucun commit local a pousser. Verification PR en cours.");
        }

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

        CreatePullRequests(context, adoContext, workflow, projectConfig, manifest, pullRequestCandidates, draft, verificationResults, handoffSummaries);
        return 0;
    }

    internal static IReadOnlyList<RepositoryStatus> SelectPullRequestCandidates(
        CommandContext context,
        IReadOnlyList<RepositoryStatus> statuses,
        IReadOnlyList<RepositoryStatus> actionable,
        ProjectConfig? projectConfig)
    {
        if (actionable.Count > 0)
        {
            return actionable;
        }

        return statuses
            .Where(status => status.IsGitRepository)
            .Where(status => HasReviewableCommits(context, status, projectConfig))
            .ToArray();
    }

    internal static bool HasReviewableCommits(CommandContext context, RepositoryStatus status, ProjectConfig? projectConfig)
    {
        var repo = projectConfig?.Repositories.GetValueOrDefault(status.Repository);
        var target = repo?.PullRequestTargetBranch ?? repo?.DefaultBranch ?? "main";
        var comparison = $"origin/{target}..HEAD";
        var result = context.ProcessRunner.RunAsync("git", ["rev-list", "--count", comparison], status.Path).GetAwaiter().GetResult();
        return result.ExitCode == 0
               && int.TryParse(result.StandardOutput.Trim(), CultureInfo.InvariantCulture, out var ahead)
               && ahead > 0;
    }

    private static void CreatePullRequests(
        CommandContext context,
        AdoContext adoContext,
        WorkflowConfig workflow,
        ProjectConfig? projectConfig,
        WorkspaceManifest manifest,
        IReadOnlyList<RepositoryStatus> changed,
        bool draft,
        IReadOnlyList<VerificationResult> verificationResults,
        IReadOnlyDictionary<string, WorkspaceHandoffSummary> handoffSummaries)
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
            var sourceRef = $"refs/heads/{manifest.BranchName}";
            var existing = adoContext.Client.TryFindActivePullRequestAsync(adoRepo, sourceRef, adoContext.Token).GetAwaiter().GetResult();
            if (existing is not null)
            {
                context.Out.WriteLine($"PR deja ouverte pour {status.Repository}: {existing.Url ?? "(url non retournee)"}");
                continue;
            }

            var request = new CreatePullRequestRequest(
                SourceRefName: sourceRef,
                TargetRefName: $"refs/heads/{target}",
                Title: PullRequestText.Title(manifest),
                Description: PullRequestText.Description(manifest, status, ReadPlan(context, status.Path), verificationResults, handoffSummaries[status.Repository]),
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

    private static void UpdateFinishStates(CommandContext context, AdoContext adoContext, WorkflowConfig workflow, WorkspaceManifest manifest)
    {
        foreach (var id in WorkItemIdsFor(manifest))
        {
            var item = adoContext.Client.GetWorkItemSnapshotAsync(id, adoContext.Token).GetAwaiter().GetResult();
            var state = AdoWorkflowStates.FinishState(item.Type ?? manifest.WorkItemType, workflow.TaskFinish);
            var label = WorkspaceManifest.FormatWorkItem(new WorkspaceWorkItem(item.Id, item.Type, item.Title, item.State));
            if (string.IsNullOrWhiteSpace(state))
            {
                context.Out.WriteLine($"ADO item {label}: etat inchange ({item.Type}).");
                continue;
            }

            if (string.Equals(item.State, state, StringComparison.OrdinalIgnoreCase))
            {
                context.Out.WriteLine($"ADO item {label}: deja en etat {state}.");
                continue;
            }

            UpdateWorkItemState(adoContext.Client, adoContext.Token, id, state, "dw task finish: PR ouverte");
            context.Out.WriteLine($"ADO item {label}: etat -> {state}");
        }
    }

    private static string ReadPlan(CommandContext context, string repositoryPath)
    {
        var workspace = Directory.GetParent(repositoryPath)?.FullName ?? repositoryPath;
        var planPath = Path.Combine(workspace, "plan.md");
        return context.FileSystem.FileExists(planPath) ? context.FileSystem.ReadAllText(planPath) : string.Empty;
    }

    private static void PrintHandoffSummaries(CommandContext context, IReadOnlyList<RepositoryStatus> statuses, IReadOnlyDictionary<string, WorkspaceHandoffSummary> handoffSummaries)
    {
        foreach (var status in statuses.Where(status => status.IsGitRepository))
        {
            if (!handoffSummaries.TryGetValue(status.Repository, out var summary))
            {
                continue;
            }

            context.Out.WriteLine();
            context.Out.WriteLine($"[handoff:{summary.Repository}] statut={summary.Status}");
            PrintSummaryList(context, "fait", summary.Done);
            PrintSummaryList(context, "decisions", summary.Decisions);
            PrintSummaryList(context, "risques", summary.Risks);
            PrintSummaryList(context, "blockers", summary.Blockers);
            PrintSummaryList(context, "follow-up", summary.FollowUp);
        }
    }

    private static void PrintSummaryList(CommandContext context, string label, IReadOnlyList<string> items)
    {
        if (items.Count == 0)
        {
            return;
        }

        context.Out.WriteLine($"  {label}: {string.Join(" | ", items)}");
    }

    private static IReadOnlyList<ResourceRef> WorkItemRefsFor(WorkspaceManifest manifest)
        => WorkItemIdsFor(manifest).Select(id => new ResourceRef(id)).ToArray();

    private static int? TryGetInt(System.Text.Json.JsonElement element, string property)
        => element.TryGetProperty(property, out var value) && value.TryGetInt32(out var id) ? id : null;
}
