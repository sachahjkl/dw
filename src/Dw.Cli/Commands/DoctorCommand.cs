namespace Dw.Cli.Commands;

internal static class DoctorCommand
{
    public static async Task<int> RunAsync(CommandContext context, string[] args)
    {
        var fix = args.Any(arg => arg.Equals("--fix", StringComparison.OrdinalIgnoreCase));
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;

        var checks = new List<DoctorCheck>
        {
            new("Root DevWorkflow", context.FileSystem.DirectoryExists(root), root, $"Executer: dw init --root \"{root}\""),
            new("Configuration utilisateur", context.FileSystem.FileExists(AppPaths.UserSettingsPath), AppPaths.UserSettingsPath, "Executer: dw init"),
            await CheckCommand(context, "git", "--version", "Git", "Installer Git puis relancer dw doctor"),
            await CheckCommand(context, "dotnet", "--list-runtimes", ".NET 8 runtime", "Installer .NET 8 Runtime: https://dotnet.microsoft.com/download/dotnet/8.0", output => output.Contains("Microsoft.NETCore.App 8.", StringComparison.OrdinalIgnoreCase)),
            await CheckCommand(context, "opencode", "--version", "OpenCode", "Installer OpenCode selon la procedure d'equipe, puis verifier le PATH")
        };

        if (fix && !context.FileSystem.DirectoryExists(root))
        {
            InitCommand.Run(context, ["--root", root]);
            checks[0] = checks[0] with { Passed = true };
        }

        foreach (var check in checks)
        {
            context.Out.WriteLine($"{(check.Passed ? "[OK]  " : "[WARN]")} {check.Name}");
            if (!string.IsNullOrWhiteSpace(check.Detail))
            {
                context.Out.WriteLine($"      {check.Detail}");
            }

            if (!check.Passed)
            {
                context.Out.WriteLine($"      {check.Remediation}");
            }
        }

        return checks.All(check => check.Passed) ? 0 : 1;
    }

    private static async Task<DoctorCheck> CheckCommand(
        CommandContext context,
        string fileName,
        string arguments,
        string name,
        string remediation,
        Func<string, bool>? validator = null)
    {
        try
        {
            var result = await context.ProcessRunner.RunAsync(fileName, arguments);
            var output = (result.StandardOutput + Environment.NewLine + result.StandardError).Trim();
            var passed = result.ExitCode == 0 && (validator?.Invoke(output) ?? true);
            var firstLine = output.Split([Environment.NewLine], StringSplitOptions.RemoveEmptyEntries).FirstOrDefault();
            return new DoctorCheck(name, passed, firstLine ?? fileName, remediation);
        }
        catch
        {
            return new DoctorCheck(name, false, null, remediation);
        }
    }

    private sealed record DoctorCheck(string Name, bool Passed, string? Detail, string Remediation);
}
