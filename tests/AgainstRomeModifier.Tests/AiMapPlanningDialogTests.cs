using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class AiMapPlanningDialogTests
{
    [Fact]
    public void All_roles_default_to_requested_laguna_model() => Run(() =>
    {
        using var dialog = new AiMapPlanningDialog(_ => Task.FromResult<IReadOnlyList<string>>(["gemma3:12b", "laguna-xs-2.1:latest"]),
            (_, _, _) => Task.FromResult(Result()), _ => new(0, 0, 0, 0));
        Pump(dialog.LoadModelsAsync());
        Assert.All(dialog.ModelBoxes, box => Assert.Equal("laguna-xs-2.1:latest", box.Text));
    });
    [Fact]
    public void Generation_reviews_three_models_and_only_explicit_apply_changes_host_once() => Run(() =>
    {
        int applied = 0;
        IReadOnlyList<MultiAiMapRoleRequest>? sent = null;
        using var dialog = Dialog((requests, _, _) => { sent = requests; return Task.FromResult(Result()); }, _ => { applied++; return new(3, 2, 0, 1); });
        dialog.Show(); Pump(dialog.LoadModelsAsync());
        for (int i = 0; i < 3; i++) dialog.ModelBoxes[i].Text = "model" + i;
        Pump(dialog.GenerateAsync());
        Assert.Equal(new[] { "model0", "model1", "model2" }, sent!.Select(x => x.Model));
        Assert.Equal(0, applied); Assert.True(dialog.ApplyButton.Enabled);
        Assert.Contains("hill", dialog.PreviewBox.Text); Assert.Contains("review summary", dialog.PreviewBox.Text);
        dialog.ApplyButton.PerformClick(); dialog.ApplyButton.PerformClick();
        Assert.Equal(1, applied); Assert.False(dialog.ApplyButton.Enabled);
    });

    [Fact]
    public void Failure_clears_old_plan_and_retry_accepts_partial_plan_with_diagnostics() => Run(() =>
    {
        int attempt = 0, applied = 0;
        using var dialog = Dialog((_, _, _) =>
        {
            attempt++;
            if (attempt == 2) throw new HttpRequestException("offline");
            var result = Result();
            return Task.FromResult(attempt == 3 ? result with { Roles = [new(AiMapDesignRole.Water, "broken", AiMapRoleStatus.Failed, 0, "water failed", [], null)] } : result);
        }, _ => { applied++; return new(0, 0, 0, 0); });
        dialog.Show(); Pump(dialog.GenerateAsync());
        Assert.True(dialog.ApplyButton.Enabled);
        Pump(dialog.GenerateAsync());
        Assert.False(dialog.ApplyButton.Enabled); Assert.Contains("offline", dialog.StatusLabel.Text);
        dialog.ApplyPlan(); Assert.Equal(0, applied);
        Pump(dialog.GenerateAsync());
        Assert.Contains("water failed", dialog.PreviewBox.Text); Assert.True(dialog.ApplyButton.Enabled);
        dialog.ApplyPlan(); Assert.Equal(1, applied);
    });

    [Fact]
    public void Cancel_ignores_late_success_and_allows_retry() => Run(() =>
    {
        var pending = new TaskCompletionSource<MultiAiMapPlanResult>();
        int attempts = 0;
        using var dialog = Dialog((_, _, _) => ++attempts == 1 ? pending.Task : Task.FromResult(Result()), _ => throw new InvalidOperationException("must not apply"));
        dialog.Show(); Task task = dialog.GenerateAsync();
        Assert.True(dialog.CancelGenerationButton.Enabled);
        dialog.CancelGenerationButton.PerformClick(); pending.SetResult(Result()); Pump(task);
        Assert.False(dialog.ApplyButton.Enabled); Assert.True(dialog.GenerateButton.Enabled);
        Pump(dialog.GenerateAsync()); Assert.True(dialog.ApplyButton.Enabled);
    });

    [Fact]
    public void Closing_during_generation_or_model_listing_never_updates_disposed_controls() => Run(() =>
    {
        var generation = new TaskCompletionSource<MultiAiMapPlanResult>();
        var listing = new TaskCompletionSource<IReadOnlyList<string>>();
        CancellationToken token = default;
        using var dialog = new AiMapPlanningDialog(_ => listing.Task, (_, _, ct) => { token = ct; return generation.Task; }, _ => throw new InvalidOperationException("must not apply"));
        dialog.Show(); Task load = dialog.LoadModelsAsync(); Task task = dialog.GenerateAsync();
        dialog.Close(); dialog.Dispose();
        Assert.True(token.IsCancellationRequested);
        listing.SetResult(new[] { "late" }); generation.SetResult(Result()); Pump(task); Pump(load);
    });

    [Fact]
    public void All_failed_roles_are_reviewable_and_apply_stays_disabled() => Run(() =>
    {
        using var dialog = Dialog((_, _, _) => Task.FromResult(new MultiAiMapPlanResult(null,
            [new(AiMapDesignRole.Terrain, "missing", AiMapRoleStatus.Failed, 0, "model not found", ["warning detail"], null)], false)),
            _ => throw new InvalidOperationException("must not apply"));
        dialog.Show(); Pump(dialog.GenerateAsync());
        Assert.False(dialog.ApplyButton.Enabled); Assert.True(dialog.GenerateButton.Enabled);
        Assert.Contains("model not found", dialog.PreviewBox.Text); Assert.Contains("warning detail", dialog.PreviewBox.Text);
    });

    [Fact]
    public void Model_listing_failure_preserves_manual_model_entry_for_retry() => Run(() =>
    {
        using var dialog = new AiMapPlanningDialog(_ => throw new HttpRequestException("Ollama unavailable"),
            (_, _, _) => Task.FromResult(Result()), _ => new(0, 0, 0, 0));
        dialog.Show(); Pump(dialog.LoadModelsAsync());
        Assert.Contains("Ollama unavailable", dialog.StatusLabel.Text);
        foreach (var box in dialog.ModelBoxes) { Assert.True(box.Enabled); box.Text = "manual"; }
        Pump(dialog.GenerateAsync()); Assert.True(dialog.ApplyButton.Enabled);
    });
    [Theory]
    [InlineData(-1)]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public void Changing_description_or_any_role_invalidates_reviewed_plan(int role) => Run(() =>
    {
        int applied = 0;
        using var dialog = Dialog((_, _, _) => Task.FromResult(Result()), _ => { applied++; return new(0, 0, 0, 0); });
        dialog.Show(); Pump(dialog.GenerateAsync()); Assert.True(dialog.ApplyButton.Enabled);
        if (role < 0) dialog.DescriptionBox.Text += " changed";
        else dialog.ModelBoxes[role].Text = "changed model";
        Assert.False(dialog.ApplyButton.Enabled); Assert.Empty(dialog.PreviewBox.Text);
        Assert.True(dialog.StatusLabel.Text.Contains("重新生成") || dialog.StatusLabel.Text.Contains("Generate again"));
        dialog.ApplyPlan(); Assert.Equal(0, applied);
        Pump(dialog.GenerateAsync()); dialog.ApplyPlan(); Assert.Equal(1, applied);
    });

    [Theory]
    [InlineData(false, 0)]
    [InlineData(false, 1)]
    [InlineData(false, 2)]
    [InlineData(true, 0)]
    [InlineData(true, 1)]
    [InlineData(true, 2)]
    public void Late_model_listing_preserves_generation_inputs_and_status(bool finishGenerationFirst, int response) => Run(() =>
    {
        var listing = new TaskCompletionSource<IReadOnlyList<string>>();
        var generation = new TaskCompletionSource<MultiAiMapPlanResult>();
        using var dialog = new AiMapPlanningDialog(_ => listing.Task, (_, _, _) => generation.Task, _ => new(0, 0, 0, 0));
        Task load = dialog.LoadModelsAsync(); dialog.Show();
        for (int i = 0; i < 3; i++) dialog.ModelBoxes[i].Text = "selected" + i;
        Task generated = dialog.GenerateAsync();
        if (finishGenerationFirst) { generation.SetResult(Result()); Pump(generated); }
        string status = dialog.StatusLabel.Text;
        if (response == 2) listing.SetException(new HttpRequestException("late listing error"));
        else listing.SetResult(response == 0 ? new[] { "different default" } : Array.Empty<string>());
        Pump(load);
        Assert.Equal(status, dialog.StatusLabel.Text);
        Assert.Equal(new[] { "selected0", "selected1", "selected2" }, dialog.ModelBoxes.Select(box => box.Text));
        if (!finishGenerationFirst) { generation.SetResult(Result()); Pump(generated); }
        Assert.True(dialog.ApplyButton.Enabled);
    });
    [Fact]
    public void Visual_preview_invalidates_disposes_and_preview_failure_blocks_apply_then_retry_succeeds() => Run(() =>
    {
        int previews = 0, applied = 0;
        System.Drawing.Bitmap? first = null;
        using var dialog = new AiMapPlanningDialog(_ => Task.FromResult<IReadOnlyList<string>>([]),
            (_, _, _) => Task.FromResult(Result()), _ => { applied++; return new(0, 0, 0, 0); },
            _ => {
                if (++previews == 2) throw new InvalidOperationException("preview unavailable");
                var bitmap = new System.Drawing.Bitmap(16, 16);
                first ??= bitmap;
                return new(bitmap, new(7, 2, 3, 1), true);
            });
        dialog.Show(); Pump(dialog.GenerateAsync());
        Assert.Same(first, dialog.PreviewImage.Image);
        Assert.True(dialog.ApplyButton.Enabled); Assert.Equal(0, applied);
        Assert.Contains("7", dialog.PreviewBox.Text); Assert.Contains("1/2", dialog.PreviewBox.Text);
        dialog.DescriptionBox.Text += " edited";
        Assert.Null(dialog.PreviewImage.Image); Assert.False(dialog.ApplyButton.Enabled);
        Assert.Throws<ArgumentException>(() => first!.GetPixel(0, 0));
        Pump(dialog.GenerateAsync());
        Assert.Null(dialog.PreviewImage.Image); Assert.False(dialog.ApplyButton.Enabled);
        Assert.Contains("preview unavailable", dialog.StatusLabel.Text);
        dialog.ApplyPlan(); Assert.Equal(0, applied);
        Pump(dialog.GenerateAsync());
        Assert.NotNull(dialog.PreviewImage.Image); dialog.ApplyPlan(); Assert.Equal(1, applied);
        var last = (System.Drawing.Bitmap)dialog.PreviewImage.Image!;
        dialog.Dispose(); Assert.Throws<ArgumentException>(() => last.GetPixel(0, 0));
    });

    [Fact]
    public void Role_progress_updates_ui_and_late_reports_cannot_overwrite_cancelled_or_finished_status() => Run(() =>
    {
        IProgress<AiMapRoleProgress>? progress = null;
        var pending = new TaskCompletionSource<MultiAiMapPlanResult>();
        using var dialog = new AiMapPlanningDialog(_ => Task.FromResult<IReadOnlyList<string>>([]),
            (_, _, _, updates) => { progress = updates; return pending.Task; }, _ => new(0, 0, 0, 0));
        dialog.Show(); Task task = dialog.GenerateAsync();
        progress!.Report(new(AiMapDesignRole.Water, "water-model", 1, false)); Application.DoEvents();
        Assert.Equal(1, dialog.GenerationProgress.Value); Assert.Contains("water-model", dialog.StatusLabel.Text);
        dialog.CancelGenerationButton.PerformClick(); pending.SetResult(Result()); Pump(task);
        string status = dialog.StatusLabel.Text;
        progress.Report(new(AiMapDesignRole.Materials, "late-model", 3, true)); Application.DoEvents();
        Assert.Equal(status, dialog.StatusLabel.Text); Assert.False(dialog.ApplyButton.Enabled);
    });

    private static AiMapPlanningDialog Dialog(Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, Task<MultiAiMapPlanResult>> generate,
        Func<AiMapPlan, AiMapApplyResult> apply) => new(_ => Task.FromResult<IReadOnlyList<string>>(["available"]), generate, apply);

    private static MultiAiMapPlanResult Result() => new(new AiMapPlan { Summary = "review summary", Features = [new() { Type = "hill", Location = "north", Radius = 4, Amount = 20 }] },
        [new(AiMapDesignRole.Terrain, "terrain", AiMapRoleStatus.Succeeded, 1, null, [], null)], false);

    private static void Pump(Task task)
    {
        var limit = DateTime.UtcNow.AddSeconds(10);
        while (!task.IsCompleted && DateTime.UtcNow < limit) { Application.DoEvents(); Thread.Sleep(1); }
        Assert.True(task.IsCompleted, "UI operation timed out"); task.GetAwaiter().GetResult();
    }

    private static void Run(Action body)
    {
        Exception? error = null;
        var thread = new Thread(() => { try { body(); } catch (Exception ex) { error = ex; } });
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(30)), "STA test timed out");
        if (error is not null) ExceptionDispatchInfo.Capture(error).Throw();
    }
}
