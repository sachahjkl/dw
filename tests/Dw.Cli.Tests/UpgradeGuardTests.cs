namespace Dw.Cli.Tests;

public sealed class UpgradeGuardTests
{
    [Fact]
    public void EnsureSupportedHost_rejects_nix_managed_install()
    {
        var ex = Assert.Throws<DwException>(() => UpgradeCommand.EnsureSupportedHost("/nix/store/abc123-dw/bin/dw"));

        Assert.Contains("installation Nix", ex.Message);
    }

    [Fact]
    public void EnsureSupportedHost_allows_regular_install()
    {
        UpgradeCommand.EnsureSupportedHost("/usr/local/bin/dw");
    }
}
