namespace Dw.Cli;

internal sealed record CommandContext(
    TextWriter Out,
    TextWriter Error,
    IClock Clock,
    IFileSystem FileSystem,
    IProcessRunner ProcessRunner);

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
