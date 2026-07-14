using System.Drawing;
using System.Numerics;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditor3DTests
{
    [Fact]
    public void MapSelection_form_constructs_with_preview_layout()
    {
        using var form = new MapSelectionForm(Path.GetTempPath());

        Assert.Equal("Against Rome 地圖選單", form.Text);
    }

    [Fact]
    public void MapSelection_excludes_campaign_story_maps_but_keeps_other_original_and_custom_maps()
    {
        var campaign = new GameMapInfo("KAMP_001", "campaign", false, "Story", "劇情戰役");
        var historical = new GameMapInfo("HIST_001", "historical", false, "Historical", "歷史戰役");
        var originalEndless = new GameMapInfo("ENDL_000", "endless", false, "Endless", "無盡模式");
        var custom = new GameMapInfo("ENDL_005", "custom", true, "Custom", "無盡模式");

        Assert.False(MapSelectionForm.IsSelectableMap(campaign));
        Assert.True(MapSelectionForm.IsSelectableMap(historical));
        Assert.True(MapSelectionForm.IsSelectableMap(custom));
        Assert.Same(originalEndless, MapSelectionForm.SelectNewMapTemplate([campaign, custom, historical, originalEndless]));
        Assert.Equal("Historical - Copy", MapSelectionForm.SuggestedCopyName(historical));
        Assert.True(MapTextDocument.CanEncodeGameText(MapSelectionForm.SuggestedCopyName(historical)));
        Assert.False(MapTextDocument.CanEncodeGameText("Historical - 副本"));
    }

    [Fact]
    public void Only_endless_or_custom_maps_can_be_cloned_to_custom()
    {
        var historical = new GameMapInfo("HIST_001", "historical", false, "Historical", "歷史戰役");
        var multiplayer = new GameMapInfo("MP_001", "mp", false, "MP", "多人地圖");
        var originalEndless = new GameMapInfo("ENDL_000", "endless", false, "Endless", "無盡模式");
        var custom = new GameMapInfo("ENDL_005", "custom", true, "Custom", "無盡模式");

        Assert.True(MapSelectionForm.CanCloneToCustom(originalEndless));
        Assert.True(MapSelectionForm.CanCloneToCustom(custom));
        Assert.False(MapSelectionForm.CanCloneToCustom(historical));
        Assert.False(MapSelectionForm.CanCloneToCustom(multiplayer));
        // 沒有可用的無盡範本時不得退回其他類型。
        Assert.Null(MapSelectionForm.SelectNewMapTemplate([historical, multiplayer]));
    }

    [Fact]
    public void MapSelection_preview_is_detached_from_the_minimap_file()
    {
        string directory = Path.Combine(Path.GetTempPath(), "arm-map-preview-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        string path = Path.Combine(directory, "minimap.bmp");
        try
        {
            using (var source = new Bitmap(8, 8)) source.Save(path, System.Drawing.Imaging.ImageFormat.Bmp);
            using Image? preview = MapSelectionForm.LoadPreviewImage(directory);

            Assert.NotNull(preview);
            Assert.Equal(new Size(8, 8), preview.Size);
            File.Delete(path);
            Assert.False(File.Exists(path));
        }
        finally { Directory.Delete(directory, recursive: true); }
    }

    [Fact]
    public void TerrainEditHistory_groups_a_stroke_and_keeps_dirty_undo_redo_and_save_baseline_consistent()
    {
        var history = new TerrainEditHistory(2, ["grass", "grass", "water", "rock"]);

        Assert.Equal(new TerrainTextureChange(1, 0, "grass", "sand"), history.Paint(1, 0, "sand"));
        Assert.Equal(new TerrainTextureChange(1, 0, "sand", "mud"), history.Paint(1, 0, "mud"));
        Assert.True(history.CommitStroke());
        Assert.True(history.IsDirty);
        Assert.True(history.CanUndo);
        Assert.Equal("mud", history.Current[1]);

        IReadOnlyList<TerrainTextureChange> undone = Assert.IsAssignableFrom<IReadOnlyList<TerrainTextureChange>>(history.Undo());
        Assert.Single(undone);
        Assert.Equal(new TerrainTextureChange(1, 0, "grass", "mud"), undone[0]);
        Assert.Equal("grass", history.Current[1]);
        Assert.False(history.IsDirty);
        Assert.True(history.CanRedo);

        Assert.NotNull(history.Redo());
        Assert.Equal("mud", history.Current[1]);
        Assert.True(history.IsDirty);
        history.CommitBaseline();
        Assert.False(history.IsDirty);

        Assert.NotNull(history.Undo());
        Assert.True(history.IsDirty);
        IReadOnlyList<TerrainTextureChange> reset = history.ResetToBaseline();
        Assert.Single(reset);
        Assert.Equal("mud", history.Current[1]);
        Assert.False(history.IsDirty);
        Assert.False(history.CanUndo);
        Assert.False(history.CanRedo);
    }

    [Fact]
    public void TerrainEditHistory_drops_a_stroke_that_returns_to_its_original_texture()
    {
        var history = new TerrainEditHistory(1, ["grass"]);

        history.Paint(0, 0, "sand");
        history.Paint(0, 0, "grass");

        Assert.False(history.CommitStroke());
        Assert.False(history.IsDirty);
        Assert.False(history.CanUndo);

        history.Paint(0, 0, "sand");
        history.Paint(0, 0, "grass");
        history.Paint(0, 0, "rock");
        Assert.True(history.CommitStroke());
        IReadOnlyList<TerrainTextureChange> stroke = Assert.IsAssignableFrom<IReadOnlyList<TerrainTextureChange>>(history.Undo());
        Assert.Single(stroke);
        Assert.Equal(new TerrainTextureChange(0, 0, "grass", "rock"), stroke[0]);
    }

    [Fact]
    public void FloorMaterialCatalog_exposes_base_pigments_instead_of_transition_fragments()
    {
        var catalog = new FloorMaterialCatalog(["4B1___50", "4B1___51", "4BA___50", "4U13__10", "L5B09T1A", "AA_Brush01"]);

        Assert.Equal(2, catalog.Materials.Count);
        FloorMaterial first = Assert.Single(catalog.Materials, material => material.Id == "B1");
        Assert.Equal("土地", first.Category);
        Assert.Equal("淺灰泥地", first.DisplayName);
        Assert.Equal("4B1___50", first.RepresentativeTexture);
        Assert.Equal(new[] { "4B1___50", "4B1___51" }, first.Variants);
        Assert.Same(first, catalog.FindByTexture("4B1___51"));
        Assert.Null(catalog.FindByTexture("4U13__10"));
        Assert.All(Enumerable.Range(0, 32), x => Assert.Equal(first.RepresentativeTexture, first.PickVariant(x, 7)));
        FloorMaterial grass = Assert.Single(catalog.Materials, material => material.Id == "BA");
        Assert.Equal("草地", grass.Category);
        Assert.Equal("深綠草地", grass.DisplayName);
        Assert.Equal("BA", catalog.Materials[0].Id);

        string[] verifiedCodes = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "G", "I", "J", "K", "L", "M", "O", "R", "S", "T", "U", "V", "W", "X"];
        var completeCatalog = new FloorMaterialCatalog(verifiedCodes.Select(code => $"4B{code}___50"));
        Assert.Equal(28, completeCatalog.Materials.Count);
        Assert.DoesNotContain(completeCatalog.Materials, material => material.Category == "其他地表" || material.DisplayName == "其他地表");
        Assert.Equal(28, completeCatalog.Materials.Select(material => material.DisplayName).Distinct().Count());
    }

    [Fact]
    public void FloorMaterialCatalog_resolves_two_material_edges_without_exposing_transition_tiles_to_the_player()
    {
        var catalog = new FloorMaterialCatalog(["4BA___50", "4BB___50", "4UAB__30", "4UAB__31", "4UAB__40", "4UAB__60"]);
        string?[] materials = Enumerable.Repeat<string?>("BA", 9).ToArray();
        materials[5] = "BB"; // 新材質位於舊地表右側；過渡片留在舊地表格，第二種材質顯示於右半（數字鍵 6）。

        Assert.Equal("4UAB__60", catalog.ResolveTexture(materials, 3, 1, 1));
        Assert.Equal("4BB___50", catalog.ResolveTexture(materials, 3, 2, 1));
        Assert.Equal("BA", catalog.FindByTexture("4UAB__60")?.Id);

        materials[7] = "BB"; // 第二種材質同時位於右側與下方，過渡區位於右下角（數字鍵 3）。
        string? corner = catalog.ResolveTexture(materials, 3, 1, 1);
        Assert.StartsWith("4UAB__3", corner);
        Assert.Equal(corner, catalog.ResolveTexture(materials, 3, 1, 1));

        IReadOnlyList<int> painted = catalog.ApplyPlayerMaterial(materials, 3, 1, 1, "BB");
        Assert.Equal(new[] { 4 }, painted); // 玩家點的格子本身必須成為完整的新材質，不得被過渡格吞掉。
        Assert.Equal("BB", materials[4]);
        Assert.Equal("4BB___50", catalog.ResolveTexture(materials, 3, 1, 1));

        string?[] directPaint = Enumerable.Repeat<string?>("BA", 9).ToArray();
        Assert.Equal(new[] { 4 }, catalog.ApplyPlayerMaterial(directPaint, 3, 1, 1, "BB"));
        Assert.Equal("BB", directPaint[4]);
    }

    [Fact]
    public void HeightField_Interpolates_and_clamps_samples()
    {
        var field = new TerrainHeightField(3, 3, new byte[] { 0, 100, 200, 30, 130, 230, 60, 160, 255 }, 10);

        Assert.Equal(130f / 255f * 10f, field.SampleHeight(1, 1), 4);
        Assert.Equal(65f / 255f * 10f, field.SampleHeight(.5f, .5f), 4);
        Assert.Equal(0, field.SampleHeight(-10, -3), 4);
        Assert.Equal(10, field.SampleHeight(9, 9), 4);
    }

    [Fact]
    public void HeightField_Maps_the_complete_pixel_grid_to_map_tile_coordinates()
    {
        var samples = new byte[9 * 9];
        samples[4 * 9 + 4] = 255;
        var field = new TerrainHeightField(9, 9, samples, heightScale: 8, tileWidth: 2, tileHeight: 2);

        Assert.Equal(8, field.SampleHeight(1, 1), 4);
        Assert.Equal(0, field.SampleHeight(.5f, .5f), 4);
        Assert.Equal(2, field.TileWidth);
        Assert.Equal(2, field.TileHeight);
    }

    [Fact]
    public void MeshBuilder_uses_four_vertices_per_tile_for_independent_atlas_uvs()
    {
        var field = new TerrainHeightField(3, 3, new byte[9], 6);
        TerrainMeshData mesh = TerrainMeshBuilder.Build(field, 2, (_, _) => new[] { Vector2.Zero, Vector2.UnitX, Vector2.One, Vector2.UnitY });

        Assert.Equal(16, mesh.Vertices.Length);
        Assert.Equal(24, mesh.Indices.Length);
        Assert.All(mesh.Vertices, vertex => Assert.Equal(Vector3.UnitY, vertex.Normal));
        Assert.Equal(new uint[] { 0, 2, 1, 0, 3, 2 }, mesh.Indices.Take(6));
        for (int index = 0; index < mesh.Indices.Length; index += 3)
        {
            Vector3 a = mesh.Vertices[mesh.Indices[index]].Position;
            Vector3 b = mesh.Vertices[mesh.Indices[index + 1]].Position;
            Vector3 c = mesh.Vertices[mesh.Indices[index + 2]].Position;
            Vector3 geometricNormal = Vector3.Normalize(Vector3.Cross(b - a, c - a));
            Assert.True(Vector3.Dot(geometricNormal, Vector3.UnitY) > .99f, $"Triangle {index / 3} faces away from +Y.");
        }
    }

    [Fact]
    public void RayPicker_returns_nearest_tile_and_rejects_outside_ray()
    {
        var field = new TerrainHeightField(5, 5, new byte[25]);

        Assert.True(TerrainRayPicker.TryPick(field, new TerrainRay(new Vector3(1.8f, 4, 2.2f), -Vector3.UnitY), out int x, out int y));
        Assert.Equal(1, x); Assert.Equal(2, y);
        Assert.False(TerrainRayPicker.TryPick(field, new TerrainRay(new Vector3(-2, 4, 2), -Vector3.UnitY), out _, out _));
    }

    [Fact]
    public void EditorCamera_clamps_pitch_and_distance()
    {
        var camera = new EditorCamera();
        camera.Rotate(0, 500); camera.Zoom(.001f);
        Assert.Equal(80, camera.PitchDegrees); Assert.Equal(camera.MinDistance, camera.Distance);
        camera.Rotate(0, -1000); camera.Zoom(1000);
        Assert.Equal(20, camera.PitchDegrees); Assert.Equal(camera.MaxDistance, camera.Distance);
        Assert.NotEqual(camera.Position, camera.Target);
    }

    [Fact]
    public void AtlasLayout_is_non_overlapping_and_stays_inside_bounds()
    {
        IReadOnlyDictionary<string, AtlasRect> layout = FloorTextureAtlas.Layout(new[] { "a", "b", "a", "c" }, 2048);

        Assert.Equal(3, layout.Count);
        Assert.All(layout.Values, rect => Assert.InRange(rect.X + rect.Width, 1, 2048));
        Assert.DoesNotContain(layout.Values, first => layout.Values.Any(second => first != second && first.X < second.X + second.Width && second.X < first.X + first.Width && first.Y < second.Y + second.Height && second.Y < first.Y + first.Height));
    }
}
