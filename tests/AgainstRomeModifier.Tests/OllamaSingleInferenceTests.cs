using System.Net;
using System.Text;
using System.Text.Json;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class OllamaSingleInferenceTests
{
    [Fact]
    public async Task Different_planner_instances_share_one_inference_slot_and_cancelled_waiter_never_sends()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        int active = 0, peak = 0, secondCalls = 0;
        using var first = new OllamaMapPlanner("http://test", new Handler(async token =>
        {
            peak = Math.Max(peak, Interlocked.Increment(ref active)); entered.SetResult();
            try { await release.Task.WaitAsync(token); return Response(); }
            finally { Interlocked.Decrement(ref active); }
        }));
        using var second = new OllamaMapPlanner("http://test", new Handler(_ =>
        {
            secondCalls++; peak = Math.Max(peak, Interlocked.Increment(ref active));
            Interlocked.Decrement(ref active); return Task.FromResult(Response());
        }));
        var running = first.GeneratePlanAsync("laguna-xs-2.1:latest", "test", [], 30, default);
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
            using var cancellation = new CancellationTokenSource();
            var queued = second.GeneratePlanAsync("laguna-xs-2.1:latest", "test", [], 30, cancellation.Token);
            Assert.Equal(0, secondCalls); cancellation.Cancel();
            await Assert.ThrowsAnyAsync<OperationCanceledException>(() => queued);
            Assert.Equal(0, secondCalls);
        }
        finally { release.TrySetResult(); await running; }
        await second.GeneratePlanAsync("laguna-xs-2.1:latest", "retry", [], 30, default);
        Assert.Equal(1, secondCalls); Assert.Equal(1, peak);
    }

    private static HttpResponseMessage Response() => new(HttpStatusCode.OK)
    {
        Content = new StringContent("{\"message\":{\"content\":\"{\\\"features\\\":[{\\\"type\\\":\\\"hill\\\",\\\"location\\\":\\\"center\\\"}]}\"}}", Encoding.UTF8, "application/json")
    };
    private sealed class Handler(Func<CancellationToken, Task<HttpResponseMessage>> respond) : HttpMessageHandler
    {
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            using var body = JsonDocument.Parse(await request.Content!.ReadAsStringAsync(cancellationToken));
            Assert.False(body.RootElement.GetProperty("think").GetBoolean());
            return await respond(cancellationToken);
        }
    }
}
