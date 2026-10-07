using System.IO.Compression;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
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
            var applied = form.ApplyAiMapPlan(plan);
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
