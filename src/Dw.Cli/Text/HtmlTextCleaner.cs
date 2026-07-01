using System.Net;

namespace Dw.Cli.Text;

internal static class HtmlTextCleaner
{
    public static string StripMarkup(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return string.Empty;
        }

        var normalized = value.Replace("<br>", "\n", StringComparison.OrdinalIgnoreCase)
            .Replace("<br/>", "\n", StringComparison.OrdinalIgnoreCase)
            .Replace("<br />", "\n", StringComparison.OrdinalIgnoreCase)
            .Replace("</p>", "\n", StringComparison.OrdinalIgnoreCase)
            .Replace("</div>", "\n", StringComparison.OrdinalIgnoreCase)
            .Replace("</li>", "\n", StringComparison.OrdinalIgnoreCase);
        normalized = AdoRegexes.HtmlListItem().Replace(normalized, "- ");
        normalized = AdoRegexes.HtmlTag().Replace(normalized, string.Empty);
        normalized = WebUtility.HtmlDecode(normalized);
        normalized = AdoRegexes.TrailingWhitespaceBeforeNewLine().Replace(normalized, Environment.NewLine);
        normalized = AdoRegexes.ExcessBlankLines().Replace(normalized, Environment.NewLine + Environment.NewLine);
        return normalized.Trim();
    }
}
