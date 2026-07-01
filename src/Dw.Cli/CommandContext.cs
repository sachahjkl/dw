using System.Diagnostics;

namespace Dw.Cli;

internal sealed record CommandContext(
    TextWriter Out,
    TextWriter Error,
    IClock Clock,
    IFileSystem FileSystem,
    IProcessRunner ProcessRunner,
    bool Verbose = false)
{
    public void Debug(string message)
    {
        if (Verbose)
        {
            Error.WriteLine($"[debug] {message}");
        }
    }

    [Conditional("DEBUG")]
    public static void Assert(bool condition, string message)
        => System.Diagnostics.Debug.Assert(condition, message);
}

internal interface IClock
{
    DateTimeOffset Now { get; }
}

internal sealed class SystemClock : IClock
{
    public DateTimeOffset Now => DateTimeOffset.Now;
}

internal sealed class DwException(string message, int exitCode = 1) : Exception(message)
{
    public int ExitCode { get; } = exitCode;
}
