using System.Text;
using System.Net;

namespace Dw.Cli.Commands;

internal static class ChangelogRenderer
{
    internal static ChangelogFormat ParseChangelogFormat(string? format)
        => format?.Trim().ToLowerInvariant() switch
        {
            null or "" or "raw" => ChangelogFormat.Raw,
            "markdown" => ChangelogFormat.Markdown,
            "html" => ChangelogFormat.Html,
            _ => throw new DwException($"Format de changelog inconnu: {format}", 2)
        };

    internal static string RenderFlatChangelog(IReadOnlyList<WorkItemSnapshot> items, ChangelogFormat format, AzureDevOpsOptions options, bool markdownTable = false)
        => format switch
        {
            ChangelogFormat.Raw => string.Join(Environment.NewLine, items.Select(item => RenderRawLine(item))),
            ChangelogFormat.Markdown => markdownTable ? RenderFlatMarkdownTable(items, options) : RenderFlatMarkdown(items, options),
            ChangelogFormat.Html => RenderFlatHtml(items, options),
            _ => throw new InvalidOperationException("Format de changelog non pris en charge.")
        };

    internal static string RenderGroupedChangelog(IReadOnlyList<WorkItemGroup> groups, ChangelogFormat format, AzureDevOpsOptions options, bool markdownTable = false)
        => format switch
        {
            ChangelogFormat.Raw => RenderGroupedRaw(groups),
            ChangelogFormat.Markdown => markdownTable ? RenderGroupedMarkdownTable(groups, options) : RenderGroupedMarkdown(groups, options),
            ChangelogFormat.Html => RenderGroupedHtml(groups, options),
            _ => throw new InvalidOperationException("Format de changelog non pris en charge.")
        };

    private static string RenderGroupedRaw(IReadOnlyList<WorkItemGroup> groups)
    {
        var builder = new StringBuilder();
        for (var i = 0; i < groups.Count; i++)
        {
            var group = groups[i];
            builder.AppendLine(RenderRawLine(group.Parent));
            foreach (var item in group.Items)
            {
                builder.Append("  - ");
                builder.AppendLine(RenderRawLine(item));
            }

            if (i < groups.Count - 1)
            {
                builder.AppendLine();
            }
        }

        return builder.ToString().TrimEnd();
    }

    private static string RenderFlatMarkdown(IReadOnlyList<WorkItemSnapshot> items, AzureDevOpsOptions options)
        => string.Join(Environment.NewLine, new[] { "# Changelog", string.Empty }
            .Concat(items.Select(item => $"- {RenderMarkdownLine(item, options)}")));

    private static string RenderFlatMarkdownTable(IReadOnlyList<WorkItemSnapshot> items, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.AppendLine("# Changelog");
        builder.AppendLine();
        builder.AppendLine("| Work Item | Type | Etat | Titre |");
        builder.AppendLine("| --- | --- | --- | --- |");
        foreach (var item in items)
        {
            builder.AppendLine($"| {RenderMarkdownLink(item, options)} | {EscapeMarkdownTableCell(item.Type)} | {EscapeMarkdownTableCell(item.State)} | {EscapeMarkdownTableCell(item.Title)} |");
        }

        return builder.ToString().TrimEnd();
    }

    private static string RenderGroupedMarkdown(IReadOnlyList<WorkItemGroup> groups, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.AppendLine("# Changelog");
        builder.AppendLine();
        for (var i = 0; i < groups.Count; i++)
        {
            var group = groups[i];
            builder.AppendLine($"## {RenderMarkdownLine(group.Parent, options)}");
            foreach (var item in group.Items)
            {
                builder.AppendLine($"- {RenderMarkdownLine(item, options)}");
            }

            if (i < groups.Count - 1)
            {
                builder.AppendLine();
            }
        }

        return builder.ToString().TrimEnd();
    }

    private static string RenderGroupedMarkdownTable(IReadOnlyList<WorkItemGroup> groups, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.AppendLine("# Changelog");
        builder.AppendLine();
        for (var i = 0; i < groups.Count; i++)
        {
            var group = groups[i];
            builder.AppendLine($"## {RenderMarkdownLine(group.Parent, options)}");
            builder.AppendLine();
            builder.AppendLine("| Work Item | Type | Etat | Titre |");
            builder.AppendLine("| --- | --- | --- | --- |");

            if (group.Items.Count == 0)
            {
                builder.AppendLine($"| {RenderMarkdownLink(group.Parent, options)} | {EscapeMarkdownTableCell(group.Parent.Type)} | {EscapeMarkdownTableCell(group.Parent.State)} | {EscapeMarkdownTableCell(group.Parent.Title)} |");
            }
            else
            {
                foreach (var item in group.Items)
                {
                    builder.AppendLine($"| {RenderMarkdownLink(item, options)} | {EscapeMarkdownTableCell(item.Type)} | {EscapeMarkdownTableCell(item.State)} | {EscapeMarkdownTableCell(item.Title)} |");
                }
            }

            if (i < groups.Count - 1)
            {
                builder.AppendLine();
            }
        }

        return builder.ToString().TrimEnd();
    }

    private static string RenderFlatHtml(IReadOnlyList<WorkItemSnapshot> items, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.AppendLine("<h1>Changelog</h1>");
        builder.AppendLine("<ul>");
        foreach (var item in items)
        {
            builder.Append("  <li>");
            builder.Append(RenderHtmlLine(item, options));
            builder.AppendLine("</li>");
        }

        builder.Append("</ul>");
        return builder.ToString();
    }

    private static string RenderGroupedHtml(IReadOnlyList<WorkItemGroup> groups, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.AppendLine("<h1>Changelog</h1>");
        foreach (var group in groups)
        {
            builder.Append("<h2>");
            builder.Append(RenderHtmlLine(group.Parent, options));
            builder.AppendLine("</h2>");
            if (group.Items.Count == 0)
            {
                continue;
            }

            builder.AppendLine("<ul>");
            foreach (var item in group.Items)
            {
                builder.Append("  <li>");
                builder.Append(RenderHtmlLine(item, options));
                builder.AppendLine("</li>");
            }

            builder.AppendLine("</ul>");
        }

        return builder.ToString().TrimEnd();
    }

    private static string RenderRawLine(WorkItemSnapshot item)
    {
        var builder = new StringBuilder();
        builder.Append('#').Append(item.Id);
        if (!string.IsNullOrWhiteSpace(item.Type))
        {
            builder.Append(" [").Append(item.Type).Append(']');
        }

        if (!string.IsNullOrWhiteSpace(item.State))
        {
            builder.Append(' ').Append(item.State);
        }

        if (!string.IsNullOrWhiteSpace(item.Title))
        {
            builder.Append(" - ").Append(item.Title);
        }

        return builder.ToString();
    }

    private static string RenderMarkdownLine(WorkItemSnapshot item, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.Append(RenderMarkdownLink(item, options));
        if (!string.IsNullOrWhiteSpace(item.Type))
        {
            builder.Append(" [").Append(item.Type).Append(']');
        }

        if (!string.IsNullOrWhiteSpace(item.State))
        {
            builder.Append(' ').Append(item.State);
        }

        if (!string.IsNullOrWhiteSpace(item.Title))
        {
            builder.Append(" - ").Append(item.Title);
        }

        return builder.ToString();
    }

    private static string RenderMarkdownLink(WorkItemSnapshot item, AzureDevOpsOptions options)
        => $"[#{item.Id}]({AzureDevOpsUris.WorkItemWebUrl(options, item.Id).AbsoluteUri})";

    private static string EscapeMarkdownTableCell(string? value)
        => string.IsNullOrWhiteSpace(value)
            ? string.Empty
            : value.Replace("|", "\\|", StringComparison.Ordinal).Replace(Environment.NewLine, "<br />", StringComparison.Ordinal).Replace("\n", "<br />", StringComparison.Ordinal);

    private static string RenderHtmlLine(WorkItemSnapshot item, AzureDevOpsOptions options)
    {
        var builder = new StringBuilder();
        builder.Append("<a href=\"")
            .Append(WebUtility.HtmlEncode(AzureDevOpsUris.WorkItemWebUrl(options, item.Id).AbsoluteUri))
            .Append("\">#")
            .Append(WebUtility.HtmlEncode(item.Id))
            .Append("</a>");
        if (!string.IsNullOrWhiteSpace(item.Type))
        {
            builder.Append(" [").Append(WebUtility.HtmlEncode(item.Type)).Append(']');
        }

        if (!string.IsNullOrWhiteSpace(item.State))
        {
            builder.Append(' ').Append(WebUtility.HtmlEncode(item.State));
        }

        if (!string.IsNullOrWhiteSpace(item.Title))
        {
            builder.Append(" - ").Append(WebUtility.HtmlEncode(item.Title));
        }

        return builder.ToString();
    }
}

internal enum ChangelogFormat
{
    Raw,
    Markdown,
    Html
}
