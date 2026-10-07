using System.Drawing;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Live_ai_dialog_preview_apply_undo_save_and_fresh_reload()
    {
        if (Environment.GetEnvironmentVariable("ARM_AI_ACCEPTANCE_LIVE") != "1") return;
        string output = Environment.GetEnvironmentVariable("ARM_AI_ACCEPTANCE_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmAiAcceptance_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string model = Environment.GetEnvironmentVariable("ARM_OLLAMA_MODEL") ?? "laguna-xs-2.1:latest";
        string map = CreateFixture();
        RunInSta(() =>
        {
            Language previous = Loc.CurrentLanguage;
            Loc.OverrideLanguageForTesting(Language.TraditionalChinese);
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Live AI acceptance", "Test"));
                _ = form.Handle; Invoke(form, "LoadSelectedMap");
                var before = SnapshotDirectory(map);
                var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
                byte[] originalHeight = layers.Heights.ToArray(), originalCollision = layers.Collision!.ToArray();
                string[] originalTextures = GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures.ToArray();
                using var provider = new OllamaMapPlanner();
                var planner = new MultiAiMapPlanner(provider);
                var materials = GetField<FloorMaterialCatalog>(form, "_floorMaterials").Materials
                    .Select(item => new AiMaterialOption(item.Id, item.DisplayName)).ToArray();
                MultiAiMapPlanResult? generated = null;
                AiMapApplyResult? predicted = null, applied = null;
                int applyCalls = 0;
                using var dialog = new AiMapPlanningDialog(provider.ListModelsAsync,
                    async (requests, description, token, progress) =>
                    {
                        generated = await planner.GeneratePlanAsync(requests, description, materials, 30, token,
                            new LiveAcceptanceProgress(progress, output));
                        return generated;
                    }, plan => { applyCalls++; return applied = form.ApplyAiMapPlan(plan); },
                    plan => { var preview = form.PreviewAiMapPlan(plan); predicted = preview.Changes; return preview; });
                dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-20000, -20000);
                foreach (var box in dialog.ModelBoxes) box.Text = model;
                dialog.DescriptionBox.Text = "A wide flat meadow, a mountain in the north-east, a small lake in the south-west, a shallow west-to-east river. Use available ground materials, preserve dry open space. Add a small blocked area in the mountains.";
                dialog.Show(); PumpLive(dialog.GenerateAsync());
                Assert.NotNull(generated); Assert.False(generated.IsCancelled); Assert.False(generated.HasFailures, generated.RawResponse);
                Assert.True(dialog.ApplyButton.Enabled); Assert.NotNull(dialog.PreviewImage.Image);
                Assert.Equal(0, applyCalls); AssertSnapshotUnchanged(map, before);
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                dialog.PreviewImage.Image!.Save(Path.Combine(output, "preview.png"));
                CaptureLiveDialog(dialog, output, "zh");
                Loc.OverrideLanguageForTesting(Language.English);
                using (var english = new AiMapPlanningDialog(_ => Task.FromResult<IReadOnlyList<string>>([model]),
                    (_, _, _) => Task.FromResult(generated!), _ => throw new InvalidOperationException("visual review only"), form.PreviewAiMapPlan))
                {
                    english.StartPosition = FormStartPosition.Manual; english.Location = new Point(-20000, -20000);
                    english.Show(); PumpLive(english.GenerateAsync()); CaptureLiveDialog(english, output, "en");
                }
                Loc.OverrideLanguageForTesting(Language.TraditionalChinese);
                dialog.ApplyPlan(); Assert.Equal(1, applyCalls); Assert.Equal(predicted, applied);
                Assert.True(applied!.HeightSamplesChanged > 0); AssertSnapshotUnchanged(map, before);
                SetWorkflowMode(form, "Texture"); Invoke(form, "Undo");
                Assert.Equal(originalTextures, GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures);
                SetWorkflowMode(form, "Height"); Invoke(form, "Undo");
                Assert.Equal(originalHeight, layers.Heights); Assert.Equal(originalCollision, layers.Collision);
                Invoke(form, "Redo"); SetWorkflowMode(form, "Texture"); Invoke(form, "Redo");
                byte[] nextHeight = layers.Heights.ToArray(), nextCollision = layers.Collision!.ToArray();
                string[] nextTextures = GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures.ToArray();
                Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Fresh", "Test"));
                _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
                var fresh = GetField<TerrainHeightEditSession>(reopened, "_terrainLayers");
                Assert.Equal(nextHeight, fresh.Heights); Assert.Equal(nextCollision, fresh.Collision);
                Assert.Equal(nextTextures, GetField<BodenTexturesDocument>(reopened, "_texturesDocument").Textures);
                Assert.False(GetProperty<bool>(reopened, "IsDirty")); Assert.False(fresh.CanUndo);
                File.WriteAllText(Path.Combine(output, "result.json"), JsonSerializer.Serialize(new {
                    Model = model, generated.Roles, generated.Plan, Preview = predicted, Applied = applied,
                    dialog.DeviceDpi, Verified = "real generation / preview isolation / apply equality / undo redo / save / fresh form reload",
                    Raw = generated.RawResponse }, LiveJsonOptions));
            }
            finally { Loc.OverrideLanguageForTesting(previous); }
        }, TimeSpan.FromMinutes(12));
    }

    private static readonly JsonSerializerOptions LiveJsonOptions = new() { WriteIndented = true };

    private sealed class LiveAcceptanceProgress(IProgress<AiMapRoleProgress> ui, string output) : IProgress<AiMapRoleProgress>
    {
        public void Report(AiMapRoleProgress progress)
        {
            ui.Report(progress);
            File.AppendAllText(Path.Combine(output, "progress.log"), $"{DateTime.UtcNow:O} {progress}\n");
        }
    }
    private static void PumpLive(Task task)
    {
        var limit = DateTime.UtcNow.AddMinutes(10);
        while (!task.IsCompleted && DateTime.UtcNow < limit) { Application.DoEvents(); Thread.Sleep(10); }
        Assert.True(task.IsCompleted, "Live generation exceeded ten minutes."); task.GetAwaiter().GetResult();
    }
    private static void CaptureLiveDialog(Form dialog, string output, string language)
    {
        foreach (Size size in new[] { new Size(880, 740), dialog.MinimumSize })
        {
            dialog.Size = size; dialog.PerformLayout(); Application.DoEvents();
            using var bitmap = new Bitmap(dialog.Width, dialog.Height);
            dialog.DrawToBitmap(bitmap, new Rectangle(Point.Empty, dialog.Size));
            bitmap.Save(Path.Combine(output, $"ai-{language}-{size.Width}x{size.Height}.png"));
        }
    }
}
