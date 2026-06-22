using System.Text.Json;
using System.Text.RegularExpressions;

namespace Dw.Cli.Commands;

internal static class AdoCommand
{
    private static readonly string[] OptionsWithValue = ["--project", "--root", "--comments"];

    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "assigned" => Assigned(context, args.Skip(1).ToArray()),
            "work-item" => WorkItem(context, args.Skip(1).ToArray()),
            "context" => WorkItemContext(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Help(CommandContext context)
    {
        CliCatalog.WriteCommandHelp(context.Out, "ado");
        return 0;
    }

    private static int Assigned(CommandContext context, string[] args)
    {
        var top = CommandOptions.IntValue(args, "--top", 20, 1);
        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, args);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        var items = client.GetAssignedWorkItemsAsync(top, token).GetAwaiter().GetResult();
        if (items.Count == 0)
        {
            context.Out.WriteLine("Aucun work item assigne.");
            return 0;
        }

        var projectHint = ProjectName(args) is { } project ? $" --project {project}" : string.Empty;
        foreach (var item in items)
        {
            context.Out.WriteLine($"#{item.Id} [{item.Type}] {item.State} - {item.Title}");
            context.Out.WriteLine($"  Start: dw task start {item.Id}{projectHint}");
        }

        return 0;
    }

    private static int WorkItem(CommandContext context, string[] args)
    {
        var id = CommandOptions.FirstPositional(args, OptionsWithValue);
        if (string.IsNullOrWhiteSpace(id))
        {
            throw new DwException("Usage: dw ado work-item <id>", 2);
        }

        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, args);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        var item = client.GetWorkItemSnapshotAsync(id, token).GetAwaiter().GetResult();

        context.Out.WriteLine($"#{item.Id}");
        context.Out.WriteLine($"Type: {item.Type ?? "(inconnu)"}");
        context.Out.WriteLine($"Etat: {item.State ?? "(inconnu)"}");
        context.Out.WriteLine($"Titre: {item.Title ?? "(inconnu)"}");
        context.Out.WriteLine();
        context.Out.WriteLine($"Contexte complet: dw ado context {item.Id}{ProjectHint(args)}");
        return 0;
    }

    private static int WorkItemContext(CommandContext context, string[] args)
    {
        var id = CommandOptions.FirstPositional(args, OptionsWithValue);
        if (string.IsNullOrWhiteSpace(id))
        {
            throw new DwException("Usage: dw ado context <id> [--project <name>] [--summary] [--comments <n>]", 2);
        }

        var summaryOnly = CommandOptions.HasFlag(args, "--summary");
        var commentLimit = CommandOptions.IntValue(args, "--comments", 200, 0);

        var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, args);
        using var http = new HttpClient();
        var client = new AzureDevOpsClient(http, azureDevOps);
        using var document = client.GetWorkItemExpandedAsync(id, token).GetAwaiter().GetResult();
        var fields = document.RootElement.GetProperty("fields");

        PrintHeader(context, document.RootElement, fields);
        PrintCoreFields(context, fields);
        PrintLongField(context, "Description", FieldText(fields, "System.Description"));
        PrintAcceptanceFields(context, fields);

        if (!summaryOnly)
        {
            PrintRelations(context, document.RootElement, args);
            PrintComments(context, client, id, commentLimit, token);
        }

        return 0;
    }

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

    private static void PrintRelations(CommandContext context, JsonElement root, string[] args)
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
            var target = relatedId is not null
                ? $"#{relatedId} {name ?? rel}"
                : artifact?.Display ?? name ?? url ?? "(url absente)";
            context.Out.WriteLine($"- {rel}: {target}");
            if (!string.IsNullOrWhiteSpace(comment))
            {
                context.Out.WriteLine($"  {HtmlTextCleaner.StripMarkup(comment)}");
            }

            if (relatedId is not null &&
                rel is not null &&
                rel.Contains("Hierarchy-Reverse", StringComparison.OrdinalIgnoreCase))
            {
                context.Out.WriteLine($"  Parent context: dw ado context {relatedId}{ProjectHint(args)}");
            }
        }

        context.Out.WriteLine();
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

    private static string ProjectHint(string[] args)
    {
        var project = ProjectName(args);
        return string.IsNullOrWhiteSpace(project) ? string.Empty : $" --project {project}";
    }

    private static string? ProjectName(string[] args)
        => CommandOptions.OptionValue(args, "--project");

}
