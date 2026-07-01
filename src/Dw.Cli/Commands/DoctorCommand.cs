namespace Dw.Cli.Commands;

internal static class DoctorCommand
{
    public static async Task<int> RunAsync(CommandContext context, bool fix)
    {
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
            CheckCommand(context, "dotnet", "--list-runtimes", ".NET 10 runtime", "Installer .NET 10 Runtime: https://dotnet.microsoft.com/download/dotnet/10.0", output => output.Contains("Microsoft.NETCore.App 10.", StringComparison.OrdinalIgnoreCase)),
            CheckNodePackageManager(context),
            CheckCommand(context, "opencode", "--version", "OpenCode", "Installer OpenCode selon la procedure d'equipe, puis verifier le PATH")
        };
        var checks = filesystemChecks.Concat(await Task.WhenAll(commandCheckTasks)).ToList();

        if (fix && !context.FileSystem.DirectoryExists(root))
        {
            InitCommand.Run(context, new InitRequest(root, "business", NoSave: false, DryRun: false));
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

    private static async Task<DoctorCheck> CheckNodePackageManager(CommandContext context)
    {
        var remediation = "Installer pnpm, ou Node.js/npm si pnpm est indisponible.";
        var candidates = OperatingSystem.IsWindows()
            ? new[]
            {
                (FileName: "cmd", Arguments: "/c pnpm --version", DetailPrefix: "pnpm"),
                (FileName: "cmd", Arguments: "/c npm --version", DetailPrefix: "npm")
            }
            :
            [
                (FileName: "pnpm", Arguments: "--version", DetailPrefix: "pnpm"),
                (FileName: "npm", Arguments: "--version", DetailPrefix: "npm")
            ];

        foreach (var candidate in candidates)
        {
            try
            {
                var result = await context.ProcessRunner.RunAsync(candidate.FileName, candidate.Arguments);
                var output = (result.StandardOutput + Environment.NewLine + result.StandardError).Trim();
                var firstLine = output.Split([Environment.NewLine], StringSplitOptions.RemoveEmptyEntries).FirstOrDefault();
                if (result.ExitCode == 0)
                {
                    return new DoctorCheck("pnpm/npm", true, firstLine is null ? candidate.DetailPrefix : $"{candidate.DetailPrefix} {firstLine}", remediation);
                }
            }
            catch
            {
                // Try npm fallback when pnpm is unavailable.
            }
        }

        return new DoctorCheck("pnpm/npm", false, null, remediation);
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
