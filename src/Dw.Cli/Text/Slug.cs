using System.Globalization;
using System.Text;

namespace Dw.Cli.Text;

internal static class Slug
{
    public static string Normalize(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return string.Empty;
        }

        var normalized = value.Normalize(NormalizationForm.FormD);
        var builder = new StringBuilder(value.Length);
        var previousDash = false;

        foreach (var c in normalized)
        {
            var category = CharUnicodeInfo.GetUnicodeCategory(c);
            if (category == UnicodeCategory.NonSpacingMark)
            {
                continue;
            }

            var lower = char.ToLowerInvariant(c);
            if (char.IsAsciiLetterOrDigit(lower))
            {
                builder.Append(lower);
                previousDash = false;
            }
            else if (!previousDash)
            {
                builder.Append('-');
                previousDash = true;
            }
        }

        return builder.ToString().Trim('-');
    }

    public static string FromPhraseOrFallback(string? value, string fallback)
    {
        var normalized = Normalize(value ?? string.Empty);
        return string.IsNullOrWhiteSpace(normalized) ? Normalize(fallback) : normalized;
    }
}
