using System.Text.Json;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class MultiAiMapPlannerTests
{
    private static readonly AiMaterialOption[] Materials = [new("Grass", "草地"), new("Sand", "沙地")];

    [Fact]
    public async Task Progress_reports_role_start_and_finish_in_order_including_partial_failure()
    {
        var updates = new List<AiMapRoleProgress>();
        var progress = new CaptureProgress(updates.Add);
        var planner = new MultiAiMapPlanner((model, prompt, _, _, _) =>
        {
            var role = Role(prompt);
            Assert.Equal(role, updates[^1].Role); Assert.False(updates[^1].Finished);
            Assert.Equal(model, updates[^1].Model);
            return role == AiMapDesignRole.Water
                ? Task.FromException<(AiMapPlan, string)>(new HttpRequestException("water failed"))
                : Task.FromResult(Response(Plan(role)));
        });
        var requests = Enum.GetValues<AiMapDesignRole>().Select(role => new MultiAiMapRoleRequest(role, role + "-model")).ToArray();
        var result = await planner.GeneratePlanAsync(requests, "test", Materials, 60, default, progress);
        Assert.True(result.HasFailures);
        Assert.Equal(new[] { 0, 1, 1, 2, 2, 3 }, updates.Select(update => update.CompletedRoles));
        Assert.Equal(new[] { false, true, false, true, false, true }, updates.Select(update => update.Finished));
        Assert.Equal(AiMapRoleStatus.Failed, updates[3].Status);
    }

    private sealed class CaptureProgress(Action<AiMapRoleProgress> report) : IProgress<AiMapRoleProgress>
    {
        public void Report(AiMapRoleProgress value) => report(value);
    }

    [Fact]
    public async Task Calls_are_serial_and_merge_in_role_order()
    {
        int active = 0, peak = 0;
        var observed = new List<AiMapDesignRole>();
        var planner = new MultiAiMapPlanner(async (_, prompt, _, _, token) =>
        {
            AiMapDesignRole role = Role(prompt);
            peak = Math.Max(peak, Interlocked.Increment(ref active));
            observed.Add(role);
            await Task.Delay(20, token);
            Interlocked.Decrement(ref active);
            return Response(Plan(role));
        });
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "山谷河流草地", Materials, 60, default);
        Assert.Equal(1, peak);
        Assert.Equal(Enum.GetValues<AiMapDesignRole>(), observed);
        Assert.False(result.HasFailures);
        Assert.Equal(new[] { "hill", "lake", "material" }, result.Plan!.Features.Select(feature => feature.Type));
        Assert.Equal(80, result.Plan.BaseHeight); Assert.Equal("Grass", result.Plan.BaseMaterial);
    }

    [Fact]
    public async Task Per_role_models_are_routed_and_input_order_does_not_change_merge_order()
    {
        var observed = new Dictionary<AiMapDesignRole, string>();
        var planner = new MultiAiMapPlanner((model, prompt, _, _, _) =>
        {
            AiMapDesignRole role = Role(prompt);
            observed.Add(role, model);
            return Task.FromResult(Response(Plan(role)));
        });
        MultiAiMapRoleRequest[] requests = [new(AiMapDesignRole.Materials, "surface-model"),
            new(AiMapDesignRole.Water, "water-model"), new(AiMapDesignRole.Terrain, "terrain-model")];
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync(requests, "test", Materials, 60, default);
        Assert.Equal(new[] { "terrain-model", "water-model", "surface-model" }, result.Roles.Select(role => role.Model));
        Assert.Equal("water-model", observed[AiMapDesignRole.Water]);
        Assert.Equal(new[] { "hill", "lake", "material" }, result.Plan!.Features.Select(feature => feature.Type));
    }

    [Fact]
    public async Task Partial_failure_retains_preview_and_reports_failed_role()
    {
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) => Role(prompt) == AiMapDesignRole.Water
            ? Task.FromException<(AiMapPlan, string)>(new HttpRequestException("water unavailable"))
            : Task.FromResult(Response(Plan(Role(prompt)))));
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.True(result.HasFailures);
        Assert.False(result.IsCancelled);
        Assert.Equal(new[] { "hill", "material" }, result.Plan!.Features.Select(feature => feature.Type));
        Assert.Equal(AiMapRoleStatus.Failed, result.Roles[1].Status);
        Assert.Contains("water unavailable", result.Roles[1].Error);
        Assert.Contains("water unavailable", result.RawResponse);
    }

    [Fact]
    public async Task All_failures_return_no_preview_and_three_errors()
    {
        var planner = new MultiAiMapPlanner((_, _, _, _, _) =>
            Task.FromException<(AiMapPlan, string)>(new InvalidDataException("broken response")));
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.Null(result.Plan);
        Assert.True(result.HasFailures);
        Assert.All(result.Roles, role => Assert.Equal("broken response", role.Error));
    }

    [Fact]
    public async Task Global_cancellation_returns_promptly_even_if_planner_ignores_token()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        int count = 0;
        var planner = new MultiAiMapPlanner(async (_, prompt, _, _, _) =>
        {
            if (Interlocked.Increment(ref count) == 1) entered.SetResult();
            await release.Task; // Deliberately ignores the cancellation token.
            return Response(Plan(Role(prompt)));
        });
        using var source = new CancellationTokenSource();
        Task<MultiAiMapPlanResult> generating = planner.GeneratePlanAsync("local", "test", Materials, 60, source.Token);
        await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
        source.Cancel();
        MultiAiMapPlanResult result = await generating.WaitAsync(TimeSpan.FromSeconds(5));
        Assert.True(result.IsCancelled);
        Assert.Null(result.Plan);
        Assert.All(result.Roles, role => Assert.Equal(AiMapRoleStatus.Cancelled, role.Status));
        release.SetResult();
    }

    [Fact]
    public async Task Cancelled_role_without_global_cancellation_allows_other_roles_preview()
    {
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) => Role(prompt) == AiMapDesignRole.Water
            ? Task.FromException<(AiMapPlan, string)>(new OperationCanceledException("timeout"))
            : Task.FromResult(Response(Plan(Role(prompt)))));
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.False(result.IsCancelled);
        Assert.NotNull(result.Plan);
        Assert.Equal(AiMapRoleStatus.Cancelled, result.Roles[1].Status);
        Assert.Contains("timeout", result.Roles[1].Error);
    }

    [Fact]
    public async Task Precancelled_generation_never_invokes_planner()
    {
        int calls = 0;
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) =>
        {
            calls++;
            return Task.FromResult(Response(Plan(Role(prompt))));
        });
        using var source = new CancellationTokenSource();
        source.Cancel();
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, source.Token);
        Assert.True(result.IsCancelled);
        Assert.Equal(0, calls);
        Assert.Null(result.Plan);
    }

    [Fact]
    public async Task Scope_and_budget_are_enforced_with_visible_warnings_and_no_shared_plan_mutation()
    {
        var originals = Enum.GetValues<AiMapDesignRole>().ToDictionary(role => role, role =>
        {
            AiMapPlan plan = Plan(role);
            plan.BaseHeight = 999;
            plan.BaseMaterial = "Grass";
            AiMapFeature template = plan.Features[0];
            plan.Features = Enumerable.Range(0, 20).Select(index => new AiMapFeature
            { Type = template.Type, X = index, Radius = 2, Material = "Sand" }).ToList();
            plan.Features.Add(new() { Type = role == AiMapDesignRole.Terrain ? "lake" : "hill", X = 63 });
            return plan;
        });
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) => Task.FromResult(Response(originals[Role(prompt)])));
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.Equal(AiMapPlan.MaxFeatures, result.Plan!.Features.Count);
        Assert.Equal(255, result.Plan.BaseHeight); // Same normalization as single-AI path.
        Assert.All(result.Roles, role => Assert.Equal(MultiAiMapPlanner.FeaturesPerRole, role.AcceptedFeatures));
        Assert.All(result.Roles, role => Assert.Contains(role.Warnings, warning => warning.Contains("截斷 12")));
        Assert.All(result.Roles, role => Assert.Contains(role.Warnings, warning => warning.Contains("範圍")));
        Assert.All(result.Plan.Features.Take(16), feature => Assert.Null(feature.Material));
        Assert.All(originals.Values.SelectMany(plan => plan.Features.Take(20)), feature => Assert.Equal("Sand", feature.Material));
    }

    [Fact]
    public async Task Removing_attached_materials_deduplicates_and_reports_actual_accepted_count()
    {
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) =>
        {
            AiMapPlan plan = Plan(Role(prompt));
            if (Role(prompt) == AiMapDesignRole.Terrain)
                plan.Features = [new() { Type = "hill", X = 10, Radius = 4, Material = "Grass" },
                    new() { Type = "hill", X = 10, Radius = 4, Material = "Sand" }];
            return Task.FromResult(Response(plan));
        });
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.Equal(1, result.Roles[0].AcceptedFeatures);
        Assert.Equal(3, result.Plan!.Features.Count);
        Assert.Contains(result.Roles[0].Warnings, warning => warning.Contains("重複的 1"));
    }

    [Fact]
    public async Task Raw_normalization_loss_is_reported_including_MaxFeatures()
    {
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) =>
        {
            AiMapPlan plan = Plan(Role(prompt));
            string raw = JsonSerializer.Serialize(new { features = Enumerable.Range(0, 30).Select(_ => new { type = "unknown" }) });
            return Task.FromResult((plan, raw));
        });
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.All(result.Roles, role => Assert.Contains(role.Warnings, warning => warning.Contains("29")));
    }

    [Fact]
    public async Task Out_of_scope_only_role_is_failed_instead_of_silent_empty_success()
    {
        var planner = new MultiAiMapPlanner((_, _, _, _, _) => Task.FromResult(Response(Plan(AiMapDesignRole.Terrain))));
        MultiAiMapPlanResult result = await planner.GeneratePlanAsync("local", "test", Materials, 60, default);
        Assert.NotNull(result.Plan);
        Assert.Equal(AiMapRoleStatus.Succeeded, result.Roles[0].Status);
        Assert.All(result.Roles.Skip(1), role => Assert.Equal(AiMapRoleStatus.Failed, role.Status));
    }

    [Fact]
    public async Task Invalid_role_requests_fail_before_invocation()
    {
        int calls = 0;
        var planner = new MultiAiMapPlanner((_, prompt, _, _, _) =>
        { calls++; return Task.FromResult(Response(Plan(Role(prompt)))); });
        await Assert.ThrowsAsync<ArgumentException>(() => planner.GeneratePlanAsync(
            [new(AiMapDesignRole.Terrain, "a"), new(AiMapDesignRole.Terrain, "b"), new(AiMapDesignRole.Materials, "c")],
            "test", Materials, 60, default));
        Assert.Equal(0, calls);
    }

    [Theory]
    [InlineData("Terrain", "baseHeight", "Do not set baseMaterial")]
    [InlineData("Water", "lake and river", "Do not set baseHeight")]
    [InlineData("Materials", "material features", "Do not set baseHeight")]
    public void Prompts_constrain_each_role(string role, string expected, string forbidden)
    {
        string prompt = MultiAiMapPlanner.BuildRolePrompt(Enum.Parse<AiMapDesignRole>(role), "使用者的地圖要求");
        Assert.Contains(expected, prompt);
        Assert.Contains(forbidden, prompt);
        Assert.Contains("at most 8 features", prompt);
        Assert.Contains("使用者的地圖要求", prompt);
    }

    private static AiMapDesignRole Role(string prompt) => Enum.GetValues<AiMapDesignRole>()
        .Single(role => prompt.StartsWith($"You are the {role} specialist", StringComparison.Ordinal));

    private static (AiMapPlan, string) Response(AiMapPlan plan) => (plan, JsonSerializer.Serialize(plan));

    private static AiMapPlan Plan(AiMapDesignRole role) => role switch
    {
        AiMapDesignRole.Terrain => new() { BaseHeight = 80, Features = [new() { Type = "hill", Location = "north", Radius = 4 }] },
        AiMapDesignRole.Water => new() { Features = [new() { Type = "lake", Location = "south", Radius = 4 }] },
        _ => new() { BaseMaterial = "Grass", Features = [new() { Type = "material", Location = "east", Radius = 4, Material = "Sand" }] },
    };
}
