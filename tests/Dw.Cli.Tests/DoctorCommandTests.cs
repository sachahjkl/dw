namespace Dw.Cli.Tests;

public sealed class DoctorCommandTests
{
    [Fact]
    public void HasDotNet10Runtime_detects_runtime_line_with_10_prefix()
    {
        var output = """
Microsoft.AspNetCore.App 9.0.1 [C:\\dotnet\\shared\\Microsoft.AspNetCore.App]
Microsoft.NETCore.App 10.0.0 [C:\\dotnet\\shared\\Microsoft.NETCore.App]
""";

        Assert.True(DoctorCommand.HasDotNet10Runtime(output));
    }

    [Fact]
    public void HasDotNet10Runtime_rejects_output_without_matching_runtime()
    {
        var output = """
Microsoft.AspNetCore.App 10.0.0 [C:\\dotnet\\shared\\Microsoft.AspNetCore.App]
Microsoft.NETCore.App 9.0.8 [C:\\dotnet\\shared\\Microsoft.NETCore.App]
""";

        Assert.False(DoctorCommand.HasDotNet10Runtime(output));
    }
}
