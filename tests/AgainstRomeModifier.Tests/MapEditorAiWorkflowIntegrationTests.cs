using System.IO.Compression;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Preview_keeps_existing_redo_and_pending_height_stroke(bool collision)
    {
        var source = new TerrainHeightEditSession(9, Enumerable.Repeat((byte)80, 81).ToArray(), null,
            collision ? 8 : 0, collision ? new byte[64] : null);
        source.PaintHeight(4, 4, 3, TerrainHeightOperation.Raise, 10); source.CommitStroke();
        byte[] edited = source.Heights.ToArray();
        Assert.NotNull(source.Undo());
        byte[] pending = source.Heights.ToArray();
        bool undo = source.CanUndo, redo = source.CanRedo;
        var plan = new AiMapPlan { BaseHeight = 20, Features = [new() { Type = "blocked", X = 2, Y = 2, Radius = 1 }] };
        var preview = AiMapPlanPreviewBuilder.Build(plan, source, null, 4, 30);
        using (preview.Image)
        {
            Assert.Equal(pending, source.Heights); Assert.Equal(undo, source.CanUndo); Assert.Equal(redo, source.CanRedo);
            Assert.Equal(collision, preview.HasCollision);
        }
        // Redo remains the user's prior edit, not the preview's whole-map flattening.
        Assert.NotNull(source.Redo());
        Assert.Equal(edited[4 * 9 + 4], source.Heights[4 * 9 + 4]);
        Assert.NotEqual(20, source.Heights[4 * 9 + 4]);
        source.PaintHeight(0, 0, 1, TerrainHeightOperation.Raise, 3);
        pending = source.Heights.ToArray();
        var second = AiMapPlanPreviewBuilder.Build(plan, source, null, 4, 30);
        using (second.Image) Assert.Equal(pending, source.Heights);
        Assert.True(source.CommitStroke());
        Assert.NotNull(source.Undo());
        Assert.Equal(80, source.Heights[0]);
    }

    [Fact]
    public void Ai_plan_preserves_accepted_materials_and_supports_mode_undo_save_and_reload()
    {
        string map = CreateFixture("ENDL_010");
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/4BC___51.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)170, 128 * 128).ToArray()));
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_010", map, true, "AI workflow", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] originalHeights = layers.Heights.ToArray();
            byte[] originalCollision = layers.Collision!.ToArray();
            var textures = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string[] originalTextures = textures.Textures.ToArray();
            var originalFiles = SnapshotDirectory(map);
            var plan = new AiMapPlan
            {
                BaseHeight = 90, BaseMaterial = "BC",
                Features = [new() { Type = "material", X = 20, Y = 20, Radius = 3, Material = "BB" },
                    new() { Type = "blocked", X = 32, Y = 32, Radius = 3 }]
            };
            var preview = form.PreviewAiMapPlan(plan);
            using (preview.Image)
            {
                Assert.Equal(originalHeights, layers.Heights);
                Assert.Equal(originalCollision, layers.Collision);
                Assert.Equal(originalTextures, textures.Textures);
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                Assert.False(layers.CanUndo);
                AssertSnapshotUnchanged(map, originalFiles);
                Assert.Equal(System.Drawing.Color.FromArgb(215, 145, 45), preview.Image.GetPixel(0, 0));
                Assert.Equal(System.Drawing.Color.FromArgb(180, 60, 200), preview.Image.GetPixel(80, 80));
                Assert.Equal(195, preview.Image.GetPixel(128, 128).R);
            }
            var applied = form.ApplyAiMapPlan(plan);
            Assert.Equal(preview.Changes, applied);
            Assert.Equal(2, applied.MaterialStrokes);
            Assert.Equal(1, applied.RejectedMaterialStrokes); // Missing BB/BC transitions reject only this area.
            Assert.True(applied.HeightSamplesChanged > 0);
            Assert.True(applied.CollisionPixelsChanged > 0);
            Assert.All(textures.Textures, texture => Assert.Equal("4BC___51", texture));
            Assert.All(layers.Heights, height => Assert.Equal(90, height));
            foreach (var (path, bytes) in originalFiles) Assert.Equal(bytes, File.ReadAllBytes(Path.Combine(map, path)));

            SetWorkflowMode(form, "Texture");
            Invoke(form, "Undo");
            Assert.Equal(originalTextures, textures.Textures);
            Assert.All(layers.Heights, height => Assert.Equal(90, height));
            Invoke(form, "Redo");
            Assert.All(textures.Textures, texture => Assert.Equal("4BC___51", texture));

            // Height and collision share one layer history; AI records their combined edit once.
            SetWorkflowMode(form, "Collision");
            Invoke(form, "Undo");
            Assert.Equal(originalHeights, layers.Heights);
            Assert.Equal(originalCollision, layers.Collision);
            Assert.All(textures.Textures, texture => Assert.Equal("4BC___51", texture));
            SetWorkflowMode(form, "Height");
            Invoke(form, "Redo");
            Assert.All(layers.Heights, height => Assert.Equal(90, height));
            Assert.Contains((byte)255, layers.Collision!);

            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.All(TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"))!.Green, height => Assert.Equal(90, height));
            Assert.All(BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")).Textures, texture => Assert.Equal("4BC___51", texture));
            Assert.Equal(layers.Collision, TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"))!.Green);
            Invoke(form, "LoadSelectedMap");
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            var reopened = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            Assert.All(reopened.Heights, height => Assert.Equal(90, height));
            Assert.False(reopened.CanUndo);
            SetWorkflowMode(form, "Texture");
            Invoke(form, "Undo");
            Assert.All(GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures, texture => Assert.Equal("4BC___51", texture));
        });
    }

    private static void SetWorkflowMode(MapEditorForm form, string name)
    {
        Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
        Invoke(form, "SetEditMode", Enum.Parse(mode, name));
    }
}
