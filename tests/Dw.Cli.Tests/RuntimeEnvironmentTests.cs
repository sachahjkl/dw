namespace Dw.Cli.Tests;

public sealed class RuntimeEnvironmentTests
{
    [Fact]
    public void IsNixManagedExecutablePath_returns_true_for_nix_store_path()
    {
        Assert.True(RuntimeEnvironment.IsNixManagedExecutablePath("/nix/store/abc123-dw/bin/dw"));
    }

    [Fact]
    public void IsNixManagedExecutablePath_returns_false_for_regular_path()
    {
        Assert.False(RuntimeEnvironment.IsNixManagedExecutablePath("/usr/local/bin/dw"));
    }
}
