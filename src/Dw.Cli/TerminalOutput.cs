using System.Text;

namespace Dw.Cli;

internal static class TerminalOutput
{
    public const string ColorAuto = "auto";
    public const string ColorAlways = "always";
    public const string ColorNever = "never";

    public static bool IsInteractive(TextWriter writer)
    {
        if (writer is StyledTerminalWriter styled)
        {
            return styled.IsInteractive;
        }

        return ReferenceEquals(writer, Console.Out)
            ? !Console.IsOutputRedirected
            : ReferenceEquals(writer, Console.Error) && !Console.IsErrorRedirected;
    }

    public static bool SupportsAnsi(TextWriter writer)
    {
        if (!IsInteractive(writer))
        {
            return false;
        }

        if (!string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("NO_COLOR")))
        {
            return false;
        }

        var term = Environment.GetEnvironmentVariable("TERM");
        if (string.Equals(term, "dumb", StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        return true;
    }

    public static bool ShouldUseColor(TextWriter writer, string? mode)
        => NormalizeMode(mode) switch
        {
            ColorAlways => true,
            ColorNever => false,
            _ => SupportsAnsi(writer)
        };

    public static string Red(string value, TextWriter writer)
        => Color(writer, "31", value);

    public static string Yellow(string value, TextWriter writer)
        => Color(writer, "33", value);

    public static string Green(string value, TextWriter writer)
        => Color(writer, "32", value);

    public static string Cyan(string value, TextWriter writer)
        => Color(writer, "36", value);

    public static string Dim(string value, TextWriter writer)
        => Color(writer, "2", value);

    public static string Bold(string value, TextWriter writer)
        => Color(writer, "1", value);

    public static TextWriter CreateStyledWriter(TextWriter writer, bool isError, string? mode)
        => ShouldUseColor(writer, mode) ? new StyledTerminalWriter(writer, isError) : writer;

    public static string NormalizeMode(string? mode)
    {
        var normalized = string.IsNullOrWhiteSpace(mode) ? ColorAuto : mode.Trim().ToLowerInvariant();
        return normalized switch
        {
            ColorAuto or ColorAlways or ColorNever => normalized,
            _ => throw new DwException($"Mode couleur inconnu: {mode}. Valeurs autorisees: auto, always, never.", 2)
        };
    }

    public static string StyleLine(TextWriter writer, string line, bool isError)
    {
        if (string.IsNullOrEmpty(line))
        {
            return line;
        }

        var trimmed = line.TrimStart();
        if (trimmed.StartsWith("{", StringComparison.Ordinal) ||
            trimmed.StartsWith("[", StringComparison.Ordinal) ||
            trimmed.StartsWith("\"", StringComparison.Ordinal))
        {
            return line;
        }

        if (isError || line.StartsWith("Erreur", StringComparison.OrdinalIgnoreCase))
        {
            return Bold(Red(line, writer), writer);
        }

        if (trimmed.StartsWith("# ", StringComparison.Ordinal) ||
            trimmed.StartsWith("## ", StringComparison.Ordinal))
        {
            return Bold(Cyan(line, writer), writer);
        }

        var styled = line
            .Replace(": Done", $": {Bold(Green("Done", writer), writer)}", StringComparison.Ordinal)
            .Replace("Done:", $"{Bold(Green("Done", writer), writer)}:", StringComparison.Ordinal);

        if (styled.StartsWith("Dry-run", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Relancer", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Teardown dry-run", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("PR non creee", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Teardown annule", StringComparison.OrdinalIgnoreCase))
        {
            return Yellow(styled, writer);
        }

        if (trimmed.StartsWith("- ", StringComparison.Ordinal) ||
            trimmed.StartsWith("* ", StringComparison.Ordinal))
        {
            var indentLength = line.Length - trimmed.Length;
            var indent = indentLength > 0 ? line[..indentLength] : string.Empty;
            return $"{indent}{Dim(trimmed[..2], writer)}{trimmed[2..]}";
        }

        var separatorIndex = styled.IndexOf(':');
        if (separatorIndex > 0 && separatorIndex <= 40)
        {
            var label = styled[..separatorIndex];
            var suffix = styled[separatorIndex..];
            if (!label.Contains("//", StringComparison.Ordinal) && !label.Contains('\\'))
            {
                return $"{Bold(Cyan(label, writer), writer)}{suffix}";
            }
        }

        if (styled.StartsWith("Workspace cree", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Workspace renomme", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Workspace synchronise", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Workspace supprime", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Repo ajoute", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Work items ajoutes", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Work items retires", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Binaire remplace", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Commits/push termines", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("PR creee", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Root rafraichi", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Repo ", StringComparison.OrdinalIgnoreCase) && styled.Contains(":", StringComparison.Ordinal))
        {
            return Green(styled, writer);
        }

        if (styled.StartsWith("Aucun", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Sync ignoree", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("PR ignoree", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("ADO ignore", StringComparison.OrdinalIgnoreCase))
        {
            return Yellow(styled, writer);
        }

        if (styled.StartsWith("Prochaine etape", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Puis, pour", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Et pour terminer", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Workspaces disponibles", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Project  WorkItem", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Preparation de l'upgrade", StringComparison.OrdinalIgnoreCase) ||
            styled.StartsWith("Schemas et contextes agents regeneres", StringComparison.OrdinalIgnoreCase))
        {
            return Bold(Cyan(styled, writer), writer);
        }

        if (trimmed.StartsWith("dw ", StringComparison.OrdinalIgnoreCase))
        {
            return Bold(styled, writer);
        }

        return styled;
    }

    private static string Color(TextWriter writer, string code, string value)
        => SupportsAnsi(writer) ? $"\u001b[{code}m{value}\u001b[0m" : value;
}

internal sealed class StyledTerminalWriter(TextWriter inner, bool isError) : TextWriter
{
    private readonly StringBuilder buffer = new();

    public bool IsInteractive => true;

    public override Encoding Encoding => inner.Encoding;

    public override void Write(char value)
    {
        if (value == '\r')
        {
            FlushBuffered();
            inner.Write(value);
            return;
        }

        if (value == '\n')
        {
            WriteStyledBufferedLine();
            inner.Write(value);
            return;
        }

        buffer.Append(value);
    }

    public override void Write(string? value)
    {
        if (string.IsNullOrEmpty(value))
        {
            return;
        }

        if (value.Contains('\r', StringComparison.Ordinal))
        {
            FlushBuffered();
            inner.Write(value);
            return;
        }

        foreach (var character in value)
        {
            Write(character);
        }
    }

    public override void WriteLine(string? value)
    {
        if (!string.IsNullOrEmpty(value))
        {
            FlushBuffered();
            inner.Write(TerminalOutput.StyleLine(this, value, isError));
        }

        inner.WriteLine();
    }

    public override void Flush()
    {
        FlushBuffered();
        inner.Flush();
    }

    private void WriteStyledBufferedLine()
    {
        if (buffer.Length == 0)
        {
            return;
        }

        var line = buffer.ToString();
        buffer.Clear();
        inner.Write(TerminalOutput.StyleLine(this, line, isError));
    }

    private void FlushBuffered()
    {
        if (buffer.Length == 0)
        {
            return;
        }

        inner.Write(buffer.ToString());
        buffer.Clear();
    }
}
