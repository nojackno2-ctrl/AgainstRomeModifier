using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    // Explicit read-only source opt-in; all encoded output goes to this test's TEMP directory.
    [Theory]
    [InlineData("TUTOR_01")]
    [InlineData("ENDL_000")]
    public void Water_collision_roundtrip_uses_real_map_layers_without_modifying_source(string mapName)
    {
        if (Environment.GetEnvironmentVariable("ARM_TERRAIN_READONLY_PATH") is not { } game) return;
        string map = Path.Combine(game, "MAPS", mapName);
        string heightPath = Path.Combine(map, "boden.bmp"), collisionPath = Path.Combine(map, "collision.bmp");
        byte[] originalHeight = File.ReadAllBytes(heightPath), originalCollision = File.ReadAllBytes(collisionPath);
        TerrainLayer height = TerrainLayerFiles.Read(heightPath)!, collision = TerrainLayerFiles.Read(collisionPath)!;
        Assert.Equal(height.Width - 1, collision.Width);
        Assert.All(collision.Argb, argb =>
        {
            Assert.Equal((byte)argb, (byte)(argb >> 8));
            Assert.Equal((byte)argb, (byte)(argb >> 16));
        });
        var session = new TerrainHeightEditSession(height.Width, height.Green, null, collision.Width, collision.Green);
        int index = Array.IndexOf(collision.Green, (byte)0);
        Assert.True(index >= 0);
        TerrainLayerStroke result = session.PaintWater(index % collision.Width + .5f, index / collision.Width + .5f, 16, 64, 0, 255);
        Assert.NotEmpty(result.Heights);
        Assert.NotEmpty(result.Collision);
        Directory.CreateDirectory(_root);
        string encodedHeight = Path.Combine(_root, mapName + "-boden.bmp"), encodedCollision = Path.Combine(_root, mapName + "-collision.bmp");
        File.WriteAllBytes(encodedHeight, TerrainLayerFiles.EncodeWithGreen(height, session.Heights));
        File.WriteAllBytes(encodedCollision, TerrainLayerFiles.EncodeWithGreen(collision, session.Collision!));
        Assert.Equal(session.Heights, TerrainLayerFiles.Read(encodedHeight)!.Green);
        Assert.Equal(session.Collision, TerrainLayerFiles.Read(encodedCollision)!.Green);
        TerrainLayer reloaded = TerrainLayerFiles.Read(encodedHeight)!;
        for (int pixel = 0; pixel < height.Green.Length; pixel++)
            if (height.Green[pixel] == session.Heights[pixel]) Assert.Equal(height.Argb[pixel], reloaded.Argb[pixel]);
        Assert.Equal(originalHeight, File.ReadAllBytes(heightPath));
        Assert.Equal(originalCollision, File.ReadAllBytes(collisionPath));
    }

    [Fact]
    public void Water_brush_updates_collision_with_height_undo_redo_and_save_reload()
    {
        string map = CreateFixture();
        byte[]? savedHeights = null, savedCollision = null;
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Water", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            GetField<ToolStripComboBox>(form, "_terrainOperation").SelectedIndex = 5;
            GetField<ToolStripComboBox>(form, "_terrainStrength").SelectedIndex = 2;
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            for (int pass = 0; pass < 8; pass++)
            {
                byte[] beforeHeight = layers.Heights.ToArray(), beforeCollision = layers.Collision!.ToArray();
                Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
                Invoke(form, "CommitStroke");
                byte[] afterHeight = layers.Heights.ToArray(), afterCollision = layers.Collision!.ToArray();
                Invoke(form, "Undo");
                Assert.Equal(beforeHeight, layers.Heights);
                Assert.Equal(beforeCollision, layers.Collision);
                Invoke(form, "Redo");
                Assert.Equal(afterHeight, layers.Heights);
                Assert.Equal(afterCollision, layers.Collision);
            }
            Assert.Contains((byte)255, layers.Collision!);
            Assert.Equal(0, layers.Collision![0]);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            savedHeights = layers.Heights.ToArray();
            savedCollision = layers.Collision.ToArray();
            Assert.Equal(savedCollision, TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"))!.Green);
        });
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Water", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            Assert.Equal(savedHeights, layers.Heights);
            Assert.Equal(savedCollision, layers.Collision);
            Assert.False(layers.IsDirty);
        });
    }
}
