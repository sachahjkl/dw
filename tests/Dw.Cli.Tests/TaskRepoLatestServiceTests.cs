namespace Dw.Cli.Tests;

public sealed class TaskRepoLatestServiceTests
{
    [Fact]
    public void ResolveRemoteSourceBranch_returns_origin_default_branch()
    {
        var sourceBranch = TaskRepoLatestService.ResolveRemoteSourceBranch("develop");

        Assert.Equal("origin/develop", sourceBranch);
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 7, 2, 12, 0, 0, TimeSpan.Zero);
    }

}
