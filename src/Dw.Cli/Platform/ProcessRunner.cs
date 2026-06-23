using System.Diagnostics;

namespace Dw.Cli.Platform;

internal interface IProcessRunner
{
    Task<ProcessResult> RunAsync(ProcessRequest request);
    Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null);
    Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null);
    Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment);
    Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment);
}

internal sealed record ProcessRequest(
    string FileName,
    IReadOnlyList<string>? Arguments = null,
    string? ArgumentString = null,
    string? WorkingDirectory = null,
    IReadOnlyDictionary<string, string>? Environment = null,
    bool Interactive = false);

internal sealed record ProcessResult(int ExitCode, string StandardOutput, string StandardError);

internal sealed class ProcessRunner : IProcessRunner
{
public async Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
        => await RunAsync(new ProcessRequest(fileName, ArgumentString: arguments, WorkingDirectory: workingDirectory));

    public async Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        => await RunAsync(new ProcessRequest(fileName, Arguments: arguments, WorkingDirectory: workingDirectory));

    public async Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
        => await RunAsync(new ProcessRequest(fileName, Arguments: arguments, WorkingDirectory: workingDirectory, Environment: environment));

    public async Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
        => (await RunAsync(new ProcessRequest(fileName, Arguments: arguments, WorkingDirectory: workingDirectory, Environment: environment, Interactive: true))).ExitCode;

    public async Task<ProcessResult> RunAsync(ProcessRequest request)
    {
        using var process = new Process();
        var arguments = request.Arguments is not null && IsGit(request.FileName)
            ? GitArguments(request.Arguments)
            : request.Arguments;
        process.StartInfo = new ProcessStartInfo
        {
            FileName = request.FileName,
            WorkingDirectory = request.WorkingDirectory ?? Environment.CurrentDirectory,
            RedirectStandardOutput = !request.Interactive,
            RedirectStandardError = !request.Interactive,
            RedirectStandardInput = false,
            UseShellExecute = false,
            CreateNoWindow = !request.Interactive
        };

        if (arguments is not null)
        {
            foreach (var argument in arguments)
            {
                process.StartInfo.ArgumentList.Add(argument);
            }
        }
        else
        {
            process.StartInfo.Arguments = request.ArgumentString ?? string.Empty;
        }

        if (request.Environment is not null)
        {
            foreach (var variable in request.Environment)
            {
                process.StartInfo.Environment[variable.Key] = variable.Value;
            }
        }

        process.Start();
        if (request.Interactive)
        {
            await process.WaitForExitAsync();
            return new ProcessResult(process.ExitCode, string.Empty, string.Empty);
        }

        var stdout = process.StandardOutput.ReadToEndAsync();
        var stderr = process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();

        return new ProcessResult(process.ExitCode, await stdout, await stderr);
    }

    private static bool IsGit(string fileName)
        => string.Equals(fileName, "git", StringComparison.OrdinalIgnoreCase)
           || string.Equals(fileName, "git.exe", StringComparison.OrdinalIgnoreCase);

    private static IReadOnlyList<string> GitArguments(IReadOnlyList<string> arguments)
        => arguments.Count >= 2 && arguments[0] == "-c" && arguments[1] == "core.longpaths=true"
            ? arguments
            : ["-c", "core.longpaths=true", .. arguments];
}
