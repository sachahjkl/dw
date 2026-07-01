namespace Dw.Cli.Tests;

public sealed class AzureDevOpsUrisTests
{
    [Fact]
    public void WorkItem_builds_expected_7_1_uri()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");

        var uri = AzureDevOpsUris.WorkItem(options, "12345");

        Assert.Equal(
            "https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_apis/wit/workitems/12345?api-version=7.1",
            uri.AbsoluteUri);
    }

    [Fact]
    public void PullRequests_escapes_repository_name()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf/", "HOMMAGE EXPLOITATION");

        var uri = AzureDevOpsUris.PullRequests(options, "FRONT HOMMAGE EXPLOITATION");

        Assert.Equal(
            "https://dev.azure.com/digital-factory-ogf/HOMMAGE%20EXPLOITATION/_apis/git/repositories/FRONT%20HOMMAGE%20EXPLOITATION/pullrequests?api-version=7.1",
            uri.AbsoluteUri);
    }

    [Fact]
    public void CreateWorkItem_builds_task_uri()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");

        var uri = AzureDevOpsUris.CreateWorkItem(options, "Task");

        Assert.Equal(
            "https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_apis/wit/workitems/$Task?api-version=7.1",
            uri.AbsoluteUri);
    }

    [Fact]
    public void PullRequestWorkItems_builds_expected_uri()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");

        var uri = AzureDevOpsUris.PullRequestWorkItems(options, "gesco-front", 42);

        Assert.Equal(
            "https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_apis/git/repositories/gesco-front/pullRequests/42/workitems?api-version=7.1",
            uri.AbsoluteUri);
    }
}
