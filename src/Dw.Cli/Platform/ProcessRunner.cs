using System.Diagnostics;

namespace Dw.Cli.Platform;

internal interface IProcessRunner
{
    Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null);
    Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null);
}

internal sealed record ProcessResult(int ExitCode, string StandardOutput, string StandardError);

internal sealed class ProcessRunner : IProcessRunner
{
    public async Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
        => await RunCoreAsync(fileName, arguments, null, workingDirectory);

    public async Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        => await RunCoreAsync(fileName, null, arguments, workingDirectory);

    private static async Task<ProcessResult> RunCoreAsync(
        string fileName,
        string? arguments,
        IReadOnlyList<string>? argumentList,
        string? workingDirectory)
    {
        using var process = new Process();
        process.StartInfo = new ProcessStartInfo
        {
            FileName = fileName,
            WorkingDirectory = workingDirectory ?? Environment.CurrentDirectory,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };

        if (argumentList is not null)
        {
            foreach (var argument in argumentList)
            {
                process.StartInfo.ArgumentList.Add(argument);
            }
        }
        else
        {
            process.StartInfo.Arguments = arguments ?? string.Empty;
        }

        process.Start();
        var stdout = process.StandardOutput.ReadToEndAsync();
        var stderr = process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();

        return new ProcessResult(process.ExitCode, await stdout, await stderr);
    }
}
