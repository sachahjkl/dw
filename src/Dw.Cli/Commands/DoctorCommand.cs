namespace Dw.Cli.Commands;

internal static class DoctorCommand
{
    public static async Task<int> RunAsync(CommandContext context, string[] args)
    {
        var fix = args.Any(arg => arg.Equals("--fix", StringComparison.OrdinalIgnoreCase));
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;

        var filesystemChecks = new List<DoctorCheck>
        {
            new("Root DevWorkflow", context.FileSystem.DirectoryExists(root), root, $"Executer: dw init --root \"{root}\""),
            new("Configuration utilisateur", context.FileSystem.FileExists(AppPaths.UserSettingsPath), AppPaths.UserSettingsPath, "Executer: dw init"),
            CheckDefaultAgent(context, root)
        };
        var commandCheckTasks = new[]
        {
            CheckCommand(context, "git", "--version", "Git", "Installer Git puis relancer dw doctor"),
            CheckCommand(context, "dotnet", "--list-runtimes", ".NET 8 runtime", "Installer .NET 8 Runtime: https://dotnet.microsoft.com/download/dotnet/8.0", output => output.Contains("Microsoft.NETCore.App 8.", StringComparison.OrdinalIgnoreCase)),
            OperatingSystem.IsWindows()
                ? CheckCommand(context, "cmd", "/c npm --version", "npm", "Installer Node.js/npm pour permettre aux MCP locaux OpenCode de demarrer via npx")
                : CheckCommand(context, "npm", "--version", "npm", "Installer Node.js/npm pour permettre aux MCP locaux OpenCode de demarrer via npx"),
            CheckCommand(context, "opencode", "--version", "OpenCode", "Installer OpenCode selon la procedure d'equipe, puis verifier le PATH")
        };
        var checks = filesystemChecks.Concat(await Task.WhenAll(commandCheckTasks)).ToList();

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
        => await CheckCommand(context, [fileName], arguments, name, remediation, validator);

    private static async Task<DoctorCheck> CheckCommand(
        CommandContext context,
        IReadOnlyList<string> fileNames,
        string arguments,
        string name,
        string remediation,
        Func<string, bool>? validator = null)
    {
        foreach (var fileName in fileNames)
        {
            try
            {
                var result = await context.ProcessRunner.RunAsync(fileName, arguments);
                var output = (result.StandardOutput + Environment.NewLine + result.StandardError).Trim();
                var passed = result.ExitCode == 0 && (validator?.Invoke(output) ?? true);
                var firstLine = output.Split([Environment.NewLine], StringSplitOptions.RemoveEmptyEntries).FirstOrDefault();
                if (passed)
                {
                    return new DoctorCheck(name, true, firstLine ?? fileName, remediation);
                }
            }
            catch
            {
                // Try next executable name, e.g. npm.cmd on Windows.
            }
        }

        return new DoctorCheck(name, false, null, remediation);
    }

    private static DoctorCheck CheckDefaultAgent(CommandContext context, string root)
    {
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var agentName = workflow.Agent?.Default ?? AgentDefaults.DefaultAgent;
        try
        {
            var adapter = AgentAdapterRegistry.Resolve(agentName);
            return new DoctorCheck("Agent par defaut", true, adapter.Name, "Configurer: dw agent config set-default opencode");
        }
        catch (DwException ex)
        {
            return new DoctorCheck("Agent par defaut", false, agentName, ex.Message);
        }
    }

    private sealed record DoctorCheck(string Name, bool Passed, string? Detail, string Remediation);
}
