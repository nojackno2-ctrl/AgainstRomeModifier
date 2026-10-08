using System.Drawing;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Live_ai_partial_redo_protects_locked_region_preview_undo_save_and_reload()
    {
        if (Environment.GetEnvironmentVariable("ARM_AI_ACCEPTANCE_LIVE") != "1") return;
        string output = Environment.GetEnvironmentVariable("ARM_AI_ACCEPTANCE_OUTPUT") ?? Path.Combine(Path.GetTempPath(), "ArmAiPartial_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Partial AI", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            var diskBefore = SnapshotDirectory(map);
            byte[] before = layers.Heights.ToArray(), collision = layers.Collision!.ToArray();
            string[] textures = GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures.ToArray();
            using var provider = new OllamaMapPlanner(); var planner = new MultiAiMapPlanner(provider);
            var materials = GetField<FloorMaterialCatalog>(form, "_floorMaterials").Materials.Select(m => new AiMaterialOption(m.Id, m.DisplayName)).ToArray();
            MultiAiMapPlanResult? generated = null; AiMapApplyResult? predicted = null, applied = null; AiMapEditScope? scope = null;
            using var dialog = new AiMapPlanningDialog(provider.ListModelsAsync,
                async (requests, description, token, progress) => generated = await planner.GeneratePlanAsync(requests, description, materials, 30, token, new LiveAcceptanceProgress(progress, output)),
                plan => applied = form.ApplyAiMapPlan(plan),
                plan => { scope = plan.EditScope; var preview = form.PreviewAiMapPlan(plan); predicted = preview.Changes; return preview; });
            dialog.RoleChecks[1].Checked = false; dialog.RoleChecks[2].Checked = false;
            dialog.BoundsBox.Text = "8,8,40,40"; dialog.LocksBox.Text = "20,20,8,8\r\n32,32,4,4";
            dialog.DescriptionBox.Text = "Set baseHeight to 110 and add a broad hill at north-center. Preserve locked areas. No water, materials or passability changes.";
            dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-20000, -20000);
            dialog.Show(); PumpLive(dialog.GenerateAsync());
            Assert.NotNull(generated); Assert.False(generated.HasFailures, generated.RawResponse); Assert.Single(generated.Roles);
            Assert.Equal(AiMapDesignRole.Terrain, generated.Roles[0].Role); Assert.True(dialog.ApplyButton.Enabled);
            Assert.NotNull(predicted); Assert.True(predicted.HeightSamplesChanged > 0); Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal(before, layers.Heights); AssertSnapshotUnchanged(map, diskBefore);
            dialog.PreviewImage.Image!.Save(Path.Combine(output, "partial-preview.png")); CaptureLiveDialog(dialog, output, "partial");
            dialog.ApplyPlan(); Assert.Equal(predicted, applied); Assert.Equal(textures, GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures);
            Assert.Equal(collision, layers.Collision);
            float step = (layers.VertexSize - 1) / 64f;
            for (int y = 0; y < layers.VertexSize; y++) for (int x = 0; x < layers.VertexSize; x++)
                if (!scope!.AllowsVertex(x / step, y / step, 64)) Assert.Equal(before[y * layers.VertexSize + x], layers.Heights[y * layers.VertexSize + x]);
            byte[] edited = layers.Heights.ToArray(); SetWorkflowMode(form, "Height"); Invoke(form, "Undo"); Assert.Equal(before, layers.Heights);
            Invoke(form, "Redo"); Assert.Equal(edited, layers.Heights); AssertSnapshotUnchanged(map, diskBefore);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            using var fresh = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Fresh partial", "Test"));
            _ = fresh.Handle; Invoke(fresh, "LoadSelectedMap");
            Assert.Equal(edited, GetField<TerrainHeightEditSession>(fresh, "_terrainLayers").Heights);
            Assert.Equal(collision, GetField<TerrainHeightEditSession>(fresh, "_terrainLayers").Collision);
            Assert.Equal(textures, GetField<BodenTexturesDocument>(fresh, "_texturesDocument").Textures);
            File.WriteAllText(Path.Combine(output, "partial-result.json"), JsonSerializer.Serialize(new { generated.Roles, generated.Plan, Preview = predicted, Applied = applied,
                Bounds = scope!.Bounds, Locks = scope.Locked, Verified = "live single role / preview isolation / protected height and shared edges / other layers unchanged / undo redo / save / fresh reload" }, LiveJsonOptions));
        }, TimeSpan.FromMinutes(12));
    }

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
