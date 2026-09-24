// Local integration-test fixture only. This is not a customer backend.
using System.Collections.Concurrent;
using System.Text.Json;
using Microsoft.AspNetCore.SignalR;

var builder = WebApplication.CreateBuilder(args);
builder.Logging.SetMinimumLevel(LogLevel.Warning);
builder.Services.AddSignalR(); // Includes the default 32 KiB receive limit.
builder.Services.AddSingleton<FixtureState>();
var app = builder.Build();
app.Use(async (context, next) =>
{
    if (context.Request.Path.StartsWithSegments("/telemetry") &&
        context.Request.Headers.Authorization != "Bearer fixture-token")
    {
        context.Response.StatusCode = 401;
        return;
    }
    await next();
});
app.MapHub<TelemetryHub>("/telemetry");
app.MapGet("/state", (FixtureState state) => new { messages = state.Messages.ToArray(), connections = state.Connections.Count });
app.MapPost("/reset", (FixtureState state) => { state.Messages.Clear(); return Results.Ok(); });
app.MapPost("/disconnect", (FixtureState state) =>
{
    foreach (var abort in state.Connections.Values) abort();
    return Results.Ok();
});
app.Run();

class FixtureState
{
    public ConcurrentQueue<object> Messages { get; } = new();
    public ConcurrentDictionary<string, Action> Connections { get; } = new();
}

class TelemetryHub(FixtureState state) : Hub
{
    public override Task OnConnectedAsync()
    {
        // Simulate transport loss, not a deliberate non-reconnectable hub close.
        var http = Context.GetHttpContext()!;
        state.Connections[Context.ConnectionId] = http.Abort;
        return base.OnConnectedAsync();
    }
    public override Task OnDisconnectedAsync(Exception? error)
    {
        state.Connections.TryRemove(Context.ConnectionId, out _);
        return base.OnDisconnectedAsync(error);
    }
    public Task ReportTelemetry(JsonElement envelope)
    {
        state.Messages.Enqueue(new { connectionId = Context.ConnectionId, envelope = envelope.Clone() });
        return Task.CompletedTask;
    }
}
