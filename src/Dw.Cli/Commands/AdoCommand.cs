using System.Text.Json;
using System.Text.RegularExpressions;
using Dw.Cli.Cli;

namespace Dw.Cli.Commands;

internal static class AdoCommand
{
    internal static int Assigned(CommandContext context, string? configuredRoot, string? projectName, int top, bool includeFinalStates, bool groupByParent, bool json)
    {
        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, configuredRoot, projectName);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        var items = FilterAssignedItems(client.GetAssignedWorkItemsAsync(top, token).GetAwaiter().GetResult(), includeFinalStates);
        if (items.Count == 0)
        {
            context.Out.WriteLine(includeFinalStates
                ? "Aucun work item assigne."
                : "Aucun work item assigne hors etats finaux.");
            return 0;
        }

        if (groupByParent)
        {
            var groups = GroupAssignedItemsByParent(client, token, items, projectName);
            if (json)
            {
                context.Out.WriteLine(JsonSerializer.Serialize(groups));
                return 0;
            }

            foreach (var group in groups)
            {
                context.Out.WriteLine($"#{group.Parent.Id} [{group.Parent.Type}] {group.Parent.State} - {group.Parent.Title}");
                if (group.Items.Count > 0)
                {
                    context.Out.WriteLine($"  Start: {group.SuggestedStartCommand}");
                }

                foreach (var item in group.Items)
                {
                    context.Out.WriteLine($"  - #{item.Id} [{item.Type}] {item.State} - {item.Title}");
                }

                context.Out.WriteLine();
            }

            return 0;
        }

        if (json)
        {
            context.Out.WriteLine(JsonSerializer.Serialize(items));
            return 0;
        }

        var projectHint = ProjectHint(projectName);
        foreach (var item in items)
        {
            context.Out.WriteLine($"#{item.Id} [{item.Type}] {item.State} - {item.Title}");
            context.Out.WriteLine($"  Start: dw task start {item.Id}{projectHint}");
        }

        return 0;
    }

    internal static IReadOnlyList<WorkItemSnapshot> FilterAssignedItems(IReadOnlyList<WorkItemSnapshot> items, bool includeFinalStates)
        => includeFinalStates
            ? items
            : items.Where(item => !TaskCommand.IsFinalState(item.Type, item.State)).ToArray();

    internal static int WorkItem(CommandContext context, string? configuredRoot, string? projectName, string id, bool json)
    {
        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, configuredRoot, projectName);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        var selection = WorkItemSet.Parse(id);
        var items = selection.Ids
            .Select(itemId => client.GetWorkItemSnapshotAsync(itemId, token).GetAwaiter().GetResult())
            .ToArray();

        if (json)
        {
            context.Out.WriteLine(JsonSerializer.Serialize(items));
            return 0;
        }

        for (var i = 0; i < items.Length; i++)
        {
            var item = items[i];

            if (i > 0)
            {
                context.Out.WriteLine();
                context.Out.WriteLine("---");
            }

            context.Out.WriteLine($"#{item.Id}");
            context.Out.WriteLine($"Type: {item.Type ?? "(inconnu)"}");
            context.Out.WriteLine($"Etat: {item.State ?? "(inconnu)"}");
            context.Out.WriteLine($"Titre: {item.Title ?? "(inconnu)"}");
            context.Out.WriteLine();
            context.Out.WriteLine($"Contexte complet: dw ado context {item.Id}{ProjectHint(projectName)}");
        }

        return 0;
    }

    internal static int Changelog(CommandContext context, string? configuredRoot, string? projectName, string ids, bool fromPullRequests, bool fromGit, string? repository, bool groupByParent, string? format, bool markdownTable, bool idsOnly, string? gitTo)
    {
        if (fromPullRequests && fromGit)
        {
            throw new DwException("Choisir soit --from-pr, soit --from-git, pas les deux.", 2);
        }

        var mode = fromGit ? ChangelogSourceMode.Git : ChangelogSourceMode.PullRequests;
        var outputFormat = ChangelogRenderer.ParseChangelogFormat(format);
        if (markdownTable && outputFormat != ChangelogFormat.Markdown)
        {
            throw new DwException("L'option --table est uniquement disponible avec --format markdown.", 2);
        }

        if (idsOnly && markdownTable)
        {
            throw new DwException("Les options --ids-only et --table ne peuvent pas etre combinees.", 2);
        }

        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, configuredRoot, projectName);
        var projectConfig = ResolveProjectConfig(context, configuredRoot, projectName);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);

        var workItemIds = mode == ChangelogSourceMode.Git
            ? ExtractWorkItemIdsFromGitRange(context, ids, gitTo)
            : GetWorkItemIdsFromPullRequests(client, token, projectConfig, repository, ids);

        if (workItemIds.Count == 0)
        {
            context.Out.WriteLine(mode == ChangelogSourceMode.Git
                ? "Aucun work item detecte dans les messages de commit de la plage git."
                : "Aucun work item detecte pour les pull requests donnees.");
            return 0;
        }

        if (idsOnly)
        {
            context.Out.WriteLine(string.Join(' ', workItemIds));
            return 0;
        }

        var items = client.GetWorkItemSnapshotsAsync(workItemIds, token).GetAwaiter().GetResult();
        if (items.Count == 0)
        {
            context.Out.WriteLine("Aucun work item resolu dans Azure DevOps.");
            return 0;
        }

        var rendered = groupByParent
            ? ChangelogRenderer.RenderGroupedChangelog(GroupWorkItemsByParent(client, token, items), outputFormat, azureDevOps, markdownTable)
            : ChangelogRenderer.RenderFlatChangelog(items.OrderBy(item => item.Id, StringComparer.OrdinalIgnoreCase).ToArray(), outputFormat, azureDevOps, markdownTable);
        context.Out.WriteLine(rendered);
        return 0;
    }

    internal static int WorkItemContext(CommandContext context, string? configuredRoot, string? projectName, string id, bool summaryOnly, int commentLimit, bool json)
    {
        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, configuredRoot, projectName);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        var selection = WorkItemSet.Parse(id);
        if (json)
        {
            var payload = selection.Ids
                .Select(itemId => client.GetWorkItemExpandedAsync(itemId, token).GetAwaiter().GetResult().RootElement.GetRawText())
                .ToArray();
            context.Out.WriteLine($"[{string.Join(',', payload)}]");
            return 0;
        }

        for (var i = 0; i < selection.Ids.Count; i++)
        {
            using var document = client.GetWorkItemExpandedAsync(selection.Ids[i], token).GetAwaiter().GetResult();
            var fields = document.RootElement.GetProperty("fields");

            if (i > 0)
            {
                context.Out.WriteLine();
                context.Out.WriteLine("---");
                context.Out.WriteLine();
            }

            PrintHeader(context, document.RootElement, fields);
            PrintCoreFields(context, fields);
            PrintLongField(context, "Description", FieldText(fields, "System.Description"));
            PrintAcceptanceFields(context, fields);

            if (!summaryOnly)
            {
                PrintRelations(context, document.RootElement, projectName);
                PrintComments(context, client, selection.Ids[i], commentLimit, token);
            }
        }

        return 0;
    }

    private static IReadOnlyList<AssignedWorkItemGroup> GroupAssignedItemsByParent(AzureDevOpsClient client, TokenResult token, IReadOnlyList<WorkItemSnapshot> items, string? projectName)
        => GroupWorkItemsByParent(client, token, items)
            .Select(group => new AssignedWorkItemGroup(
                group.Parent,
                group.Items,
                $"dw task start {BuildSuggestedStartIds(group.Parent, group.Items)}{ProjectHint(projectName)}"))
            .ToArray();

    internal static IReadOnlyList<WorkItemGroup> GroupWorkItemsByParent(AzureDevOpsClient client, TokenResult token, IReadOnlyList<WorkItemSnapshot> items)
    {
        var groups = new Dictionary<string, List<WorkItemSnapshot>>(StringComparer.OrdinalIgnoreCase);
        var parents = new Dictionary<string, WorkItemSnapshot>(StringComparer.OrdinalIgnoreCase);

        foreach (var item in items)
        {
            var parentId = client.GetRelatedWorkItemIdsAsync(item.Id, "System.LinkTypes.Hierarchy-Reverse", token).GetAwaiter().GetResult().FirstOrDefault();
            if (string.IsNullOrWhiteSpace(parentId))
            {
                parentId = item.Id;
                parents[parentId] = item;
            }
            else if (!parents.ContainsKey(parentId))
            {
                parents[parentId] = client.GetWorkItemSnapshotAsync(parentId, token).GetAwaiter().GetResult();
            }

            if (!groups.TryGetValue(parentId, out var children))
            {
                children = [];
                groups[parentId] = children;
            }

            if (!string.Equals(parentId, item.Id, StringComparison.OrdinalIgnoreCase))
            {
                children.Add(item);
            }
        }

        return groups
            .Select(group => new WorkItemGroup(
                parents[group.Key],
                group.Value
                    .OrderBy(item => item.Id, StringComparer.OrdinalIgnoreCase)
                    .ToArray()))
            .OrderBy(group => group.Parent.Id, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    internal static IReadOnlyList<string> ExtractWorkItemIdsFromCommitMessages(string commitLog)
        => AdoRegexes.WorkItemReference()
            .Matches(commitLog ?? string.Empty)
            .Select(match => match.Groups["id"].Value)
            .Where(id => !string.IsNullOrWhiteSpace(id))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static IReadOnlyList<string> ExtractWorkItemIdsFromGitRange(CommandContext context, string from, string? to)
    {
        if (string.IsNullOrWhiteSpace(to))
        {
            throw new DwException("Le mode --from-git attend 2 refs git: source et target.", 2);
        }

        var result = context.ProcessRunner.RunAsync("git", ["log", "--format=%B%x1e", $"{from}..{to}"], Environment.CurrentDirectory).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException($"git log a echoue: {FirstNonEmpty(result.StandardError, result.StandardOutput)}");
        }

        return ExtractWorkItemIdsFromCommitMessages(result.StandardOutput);
    }

    internal static IReadOnlyList<string> GetWorkItemIdsFromPullRequests(AzureDevOpsClient client, TokenResult token, ProjectConfig? projectConfig, string? repository, string source)
    {
        var pullRequestIds = WorkItemSet.Parse(source).Ids;
        var repositories = ResolveAdoRepositories(projectConfig, repository);
        if (repositories.Count == 0)
        {
            throw new DwException($"Le mode PR requiert {OptionNames.Repo}, ou un {OptionNames.Project} avec des repositories AzureDevOpsRepository configures.", 2);
        }

        var workItemIds = new List<string>();
        foreach (var pullRequestId in pullRequestIds)
        {
            if (!int.TryParse(pullRequestId, CultureInfo.InvariantCulture, out var numericPullRequestId))
            {
                throw new DwException($"ID de pull request invalide: {pullRequestId}", 2);
            }

            var matches = repositories
                .Select(repo => new PullRequestLookup(repo, client.TryGetPullRequestWorkItemIdsAsync(repo, numericPullRequestId, token).GetAwaiter().GetResult()))
                .Where(result => result.WorkItemIds is not null)
                .ToArray();

            if (matches.Length == 0)
            {
                throw new DwException($"Pull request #{pullRequestId} introuvable dans les repos Azure DevOps testes: {string.Join(", ", repositories)}");
            }

            if (matches.Length > 1)
            {
                throw new DwException($"Pull request #{pullRequestId} trouvee dans plusieurs repos ({string.Join(", ", matches.Select(match => match.Repository))}). Preciser --repo.", 2);
            }

            workItemIds.AddRange(matches[0].WorkItemIds!);
        }

        return workItemIds.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
    }

    private static IReadOnlyList<string> ResolveAdoRepositories(ProjectConfig? projectConfig, string? repository)
    {
        if (!string.IsNullOrWhiteSpace(repository))
        {
            return repository
                .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(repo => ResolveAdoRepository(projectConfig, repo))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray();
        }

        return projectConfig?.Repositories.Values
            .Select(repo => repo.AzureDevOpsRepository)
            .Where(repo => !string.IsNullOrWhiteSpace(repo))
            .Cast<string>()
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray() ?? [];
    }

    private static string ResolveAdoRepository(ProjectConfig? projectConfig, string repository)
    {
        if (projectConfig is not null && projectConfig.Repositories.TryGetValue(repository, out var configured) && !string.IsNullOrWhiteSpace(configured.AzureDevOpsRepository))
        {
            return configured.AzureDevOpsRepository;
        }

        return repository;
    }

    private static ProjectConfig? ResolveProjectConfig(CommandContext context, string? configuredRoot, string? projectName)
    {
        if (string.IsNullOrWhiteSpace(projectName))
        {
            return null;
        }

        var root = RootResolver.Resolve(context, configuredRoot);
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        return DevWorkflowConfigLoader.ResolveProject(projects, projectName);
    }

    private static string FirstNonEmpty(params string?[] values)
        => values.FirstOrDefault(value => !string.IsNullOrWhiteSpace(value)) ?? "erreur inconnue";

    private static string BuildSuggestedStartIds(WorkItemSnapshot parent, IReadOnlyList<WorkItemSnapshot> children)
        => string.Join(',', new[] { parent.Id }.Concat(children.Select(item => item.Id)).Distinct(StringComparer.OrdinalIgnoreCase));

    private static void PrintHeader(CommandContext context, JsonElement root, JsonElement fields)
    {
        context.Out.WriteLine($"# Work Item #{root.GetProperty("id").GetInt32()}");
        context.Out.WriteLine();
        context.Out.WriteLine($"Title: {FieldText(fields, "System.Title") ?? "(inconnu)"}");
        context.Out.WriteLine($"Type: {FieldText(fields, "System.WorkItemType") ?? "(inconnu)"}");
        context.Out.WriteLine($"State: {FieldText(fields, "System.State") ?? "(inconnu)"}");
        context.Out.WriteLine($"Assigned To: {IdentityText(fields, "System.AssignedTo") ?? "(non assigne)"}");
        context.Out.WriteLine($"Area: {FieldText(fields, "System.AreaPath") ?? "(inconnu)"}");
        context.Out.WriteLine($"Iteration: {FieldText(fields, "System.IterationPath") ?? "(inconnu)"}");
        context.Out.WriteLine($"Tags: {FieldText(fields, "System.Tags") ?? "(aucun)"}");
        context.Out.WriteLine();
    }

    private static void PrintCoreFields(CommandContext context, JsonElement fields)
    {
        context.Out.WriteLine("## Core");
        PrintField(context, "Created By", IdentityText(fields, "System.CreatedBy"));
        PrintField(context, "Created Date", FieldText(fields, "System.CreatedDate"));
        PrintField(context, "Changed By", IdentityText(fields, "System.ChangedBy"));
        PrintField(context, "Changed Date", FieldText(fields, "System.ChangedDate"));
        PrintField(context, "Priority", FieldText(fields, "Microsoft.VSTS.Common.Priority"));
        PrintField(context, "Value Area", FieldText(fields, "Microsoft.VSTS.Common.ValueArea"));
        context.Out.WriteLine();
    }

    private static void PrintAcceptanceFields(CommandContext context, JsonElement fields)
    {
        var printed = false;
        foreach (var property in fields.EnumerateObject())
        {
            if (!IsContextField(property.Name))
            {
                continue;
            }

            if (!printed)
            {
                context.Out.WriteLine("## Product / Acceptance Context");
                printed = true;
            }

            PrintLongField(context, FriendlyFieldName(property.Name), ElementText(property.Value));
        }

        if (!printed)
        {
            context.Out.WriteLine("## Product / Acceptance Context");
            context.Out.WriteLine("(aucun champ acceptance/product detecte)");
            context.Out.WriteLine();
        }
    }

    private static void PrintRelations(CommandContext context, JsonElement root, string? projectName)
    {
        context.Out.WriteLine("## Relations");
        if (!root.TryGetProperty("relations", out var relations) || relations.ValueKind != JsonValueKind.Array)
        {
            context.Out.WriteLine("(aucune relation)");
            context.Out.WriteLine();
            return;
        }

        foreach (var relation in relations.EnumerateArray())
        {
            var rel = relation.TryGetProperty("rel", out var relProperty) ? relProperty.GetString() : "(relation)";
            var url = relation.TryGetProperty("url", out var urlProperty) ? urlProperty.GetString() : null;
            var name = TryRelationAttribute(relation, "name");
            var comment = TryRelationAttribute(relation, "comment");
            var relatedId = WorkItemIdFromRelationUrl(url);
            var artifact = AdoArtifactLink.TryParse(url);
            var target = DescribeRelationTarget(rel, relatedId, artifact, name, url);
            context.Out.WriteLine($"- {rel}: {target}");
            if (!string.IsNullOrWhiteSpace(comment))
            {
                context.Out.WriteLine($"  {HtmlTextCleaner.StripMarkup(comment)}");
            }

            if (relatedId is not null &&
                rel is not null &&
                rel.Contains("Hierarchy-Reverse", StringComparison.OrdinalIgnoreCase))
            {
                context.Out.WriteLine($"  Parent context: dw ado context {relatedId}{ProjectHint(projectName)}");
            }
        }

        context.Out.WriteLine();
    }

    internal static string DescribeRelationTarget(string? rel, string? relatedId, AdoArtifactLink? artifact, string? name, string? url)
    {
        if (relatedId is not null)
        {
            return $"#{relatedId} {name ?? rel}";
        }

        if (artifact is not null)
        {
            return artifact.Display;
        }

        if (!string.IsNullOrWhiteSpace(rel)
            && rel.Contains("AttachedFile", StringComparison.OrdinalIgnoreCase)
            && !string.IsNullOrWhiteSpace(name)
            && !string.IsNullOrWhiteSpace(url))
        {
            return $"{name} ({url})";
        }

        return name ?? url ?? "(url absente)";
    }

    private static void PrintComments(CommandContext context, AzureDevOpsClient client, string id, int limit, TokenResult token)
    {
        context.Out.WriteLine("## Comments");
        if (limit == 0)
        {
            context.Out.WriteLine("(comments disabled by --comments 0)");
            return;
        }

        var printed = 0;
        string? continuation = null;
        do
        {
            var top = Math.Min(100, limit - printed);
            using var comments = client.GetWorkItemCommentsAsync(id, top, continuation, token).GetAwaiter().GetResult();
            if (comments.RootElement.TryGetProperty("comments", out var array) && array.ValueKind == JsonValueKind.Array)
            {
                foreach (var comment in array.EnumerateArray())
                {
                    printed++;
                    var author = IdentityFromElement(comment, "createdBy") ?? "(auteur inconnu)";
                    var date = comment.TryGetProperty("createdDate", out var dateProperty) ? dateProperty.GetString() : null;
                    var text = comment.TryGetProperty("text", out var textProperty) ? textProperty.GetString() : null;
                    context.Out.WriteLine($"### {printed}. {author} - {date ?? "(date inconnue)"}");
                    context.Out.WriteLine(HtmlTextCleaner.StripMarkup(text));
                    context.Out.WriteLine();
                    if (printed >= limit)
                    {
                        break;
                    }
                }
            }

            continuation = comments.RootElement.TryGetProperty("continuationToken", out var tokenProperty)
                ? tokenProperty.GetString()
                : null;
        }
        while (!string.IsNullOrWhiteSpace(continuation) && printed < limit);

        if (printed == 0)
        {
            context.Out.WriteLine("(aucun commentaire)");
        }
    }

    private static void PrintField(CommandContext context, string label, string? value)
        => context.Out.WriteLine($"- {label}: {value ?? "(vide)"}");

    private static void PrintLongField(CommandContext context, string label, string? value)
    {
        context.Out.WriteLine($"## {label}");
        context.Out.WriteLine(string.IsNullOrWhiteSpace(value) ? "(vide)" : HtmlTextCleaner.StripMarkup(value));
        context.Out.WriteLine();
    }

    private static string? FieldText(JsonElement fields, string name)
        => fields.TryGetProperty(name, out var value) ? ElementText(value) : null;

    private static string? IdentityText(JsonElement fields, string name)
        => fields.TryGetProperty(name, out var value) ? IdentityText(value) : null;

    private static string? IdentityFromElement(JsonElement element, string name)
        => element.TryGetProperty(name, out var value) ? IdentityText(value) : null;

    private static string? IdentityText(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object && value.TryGetProperty("displayName", out var displayName))
        {
            return displayName.GetString();
        }

        return ElementText(value);
    }

    private static string? ElementText(JsonElement value)
        => value.ValueKind switch
        {
            JsonValueKind.String => value.GetString(),
            JsonValueKind.Number => value.GetRawText(),
            JsonValueKind.True => "true",
            JsonValueKind.False => "false",
            JsonValueKind.Object when value.TryGetProperty("displayName", out var displayName) => displayName.GetString(),
            JsonValueKind.Null => null,
            _ => value.GetRawText()
        };

    private static string? TryRelationAttribute(JsonElement relation, string name)
    {
        if (!relation.TryGetProperty("attributes", out var attributes) || attributes.ValueKind != JsonValueKind.Object)
        {
            return null;
        }

        return attributes.TryGetProperty(name, out var value) ? ElementText(value) : null;
    }

    private static string? WorkItemIdFromRelationUrl(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }

        var match = AdoRegexes.WorkItemRelationUrl().Match(url);
        return match.Success ? match.Groups[1].Value : null;
    }

    private static bool IsContextField(string fieldName)
    {
        var normalized = fieldName.Replace(".", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("_", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace(" ", string.Empty, StringComparison.OrdinalIgnoreCase);
        return normalized.Contains("acceptance", StringComparison.OrdinalIgnoreCase)
               || normalized.Contains("productowner", StringComparison.OrdinalIgnoreCase)
               || normalized.Contains("product", StringComparison.OrdinalIgnoreCase)
               || normalized.Contains("businessvalue", StringComparison.OrdinalIgnoreCase)
               || fieldName.Equals("Microsoft.VSTS.Common.AcceptanceCriteria", StringComparison.OrdinalIgnoreCase);
    }

    private static string FriendlyFieldName(string fieldName)
        => fieldName
            .Replace("System.", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("Microsoft.VSTS.Common.", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("Custom.", string.Empty, StringComparison.OrdinalIgnoreCase);

    private static string ProjectHint(string? projectName)
        => string.IsNullOrWhiteSpace(projectName) ? string.Empty : $" {OptionNames.Project} {projectName}";

}

internal sealed record AssignedWorkItemGroup(WorkItemSnapshot Parent, IReadOnlyList<WorkItemSnapshot> Items, string SuggestedStartCommand);
internal sealed record WorkItemGroup(WorkItemSnapshot Parent, IReadOnlyList<WorkItemSnapshot> Items);
internal sealed record PullRequestLookup(string Repository, IReadOnlyList<string>? WorkItemIds);
internal enum ChangelogSourceMode
{
    PullRequests,
    Git
}

