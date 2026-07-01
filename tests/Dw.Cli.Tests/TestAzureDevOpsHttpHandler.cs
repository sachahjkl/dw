using System.Net;
using System.Net.Http.Headers;
using System.Text;

namespace Dw.Cli.Tests;

public sealed class TestAzureDevOpsHttpHandler : HttpMessageHandler
{
    private readonly List<(Predicate<HttpRequestMessage> Match, Func<HttpResponseMessage> CreateResponse)> _handlers = [];
    private readonly List<HttpRequestMessage> _capturedRequests = [];

    public IReadOnlyList<HttpRequestMessage> CapturedRequests => _capturedRequests;

    public void SetupGet(string pathContains, HttpStatusCode statusCode, string responseJson)
    {
        _handlers.Add((
            req => req.Method == HttpMethod.Get
                && req.RequestUri is not null
                && req.RequestUri.AbsoluteUri.Contains(pathContains, StringComparison.OrdinalIgnoreCase),
            () => CreateResponse(statusCode, responseJson)));
    }

    public void SetupPost(string pathContains, HttpStatusCode statusCode, string responseJson)
    {
        _handlers.Add((
            req => req.Method == HttpMethod.Post
                && req.RequestUri is not null
                && req.RequestUri.AbsoluteUri.Contains(pathContains, StringComparison.OrdinalIgnoreCase),
            () => CreateResponse(statusCode, responseJson)));
    }

    protected override Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        _capturedRequests.Add(request);
        foreach (var (match, createResponse) in _handlers)
        {
            if (match(request))
            {
                return Task.FromResult(createResponse());
            }
        }

        return Task.FromResult(
            new HttpResponseMessage(HttpStatusCode.NotFound) { Content = new StringContent("{}") });
    }

    private static HttpResponseMessage CreateResponse(HttpStatusCode statusCode, string json)
        => new(statusCode) { Content = new StringContent(json, Encoding.UTF8, new MediaTypeHeaderValue("application/json")) };
}
