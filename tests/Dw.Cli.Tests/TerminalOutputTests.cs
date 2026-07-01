namespace Dw.Cli.Tests;

public sealed class TerminalOutputTests
{
    [Fact]
    public void StyledTerminalWriter_colors_status_lines()
    {
        var previousTerm = Environment.GetEnvironmentVariable("TERM");
        var previousNoColor = Environment.GetEnvironmentVariable("NO_COLOR");
        using var inner = new StringWriter();
        using var writer = new StyledTerminalWriter(inner, isError: false);
        try
        {
            Environment.SetEnvironmentVariable("TERM", "xterm-256color");
            Environment.SetEnvironmentVariable("NO_COLOR", null);
            writer.WriteLine("Workspace cree: S:/dw");
            writer.Flush();

            var text = inner.ToString();
            Assert.Contains("\u001b[", text, StringComparison.Ordinal);
            Assert.Contains("Workspace cree", text);
        }
        finally
        {
            Environment.SetEnvironmentVariable("TERM", previousTerm);
            Environment.SetEnvironmentVariable("NO_COLOR", previousNoColor);
        }
    }

    [Fact]
    public void StyledTerminalWriter_preserves_json_lines()
    {
        using var inner = new StringWriter();
        using var writer = new StyledTerminalWriter(inner, isError: false);

        writer.WriteLine("{\"schema\":1}");
        writer.Flush();

        Assert.Equal("{\"schema\":1}" + Environment.NewLine, inner.ToString());
    }
}
