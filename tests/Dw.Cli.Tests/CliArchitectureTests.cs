namespace Dw.Cli.Tests;

public sealed class CliArchitectureTests
{
    [Fact]
    public void Cli_does_not_keep_custom_help_or_completion_framework()
    {
        var sourceRoot = FindSourceRoot();
        var files = Directory.GetFiles(sourceRoot, "*.cs", SearchOption.AllDirectories);

        Assert.DoesNotContain(files, file => Path.GetFileName(file) is "CliCatalog.cs" or "HelpCommand.cs" or "CompletionCommand.cs");
    }

    [Fact]
    public void Command_handlers_do_not_hardcode_usage_text()
    {
        var sourceRoot = FindSourceRoot();
        var files = Directory.GetFiles(Path.Combine(sourceRoot, "Commands"), "*.cs", SearchOption.AllDirectories)
            .Concat(Directory.GetFiles(Path.Combine(sourceRoot, "Workspaces"), "*.cs", SearchOption.AllDirectories));

        var offenders = files
            .SelectMany(file => File.ReadLines(file).Select((line, index) => new { file, line, lineNumber = index + 1 }))
            .Where(match => match.line.Contains("Usage:", StringComparison.Ordinal))
            .Select(match => $"{Path.GetRelativePath(sourceRoot, match.file)}:{match.lineNumber}")
            .ToArray();

        Assert.Empty(offenders);
    }

    private static string FindSourceRoot()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null)
        {
            var candidate = Path.Combine(directory.FullName, "src", "Dw.Cli");
            if (Directory.Exists(candidate))
            {
                return candidate;
            }

            directory = directory.Parent;
        }

        throw new DirectoryNotFoundException("src/Dw.Cli introuvable.");
    }
}
