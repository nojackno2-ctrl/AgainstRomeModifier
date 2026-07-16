using System.Text;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorPhase1Tests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "AgainstRomeMapEditorTests_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Clone_Creates_next_slot_preserves_unknown_files_and_registers_custom_map()
    {
        CreateSourceMap();
        var catalog = new EndlessMapCatalog();
        Assert.Equal(5, catalog.GetNextFreeSlot(_root));

        EndlessMapInfo cloned = new EndlessMapCloner(catalog).Clone(_root, 0, 5, "Test map");

        Assert.True(cloned.IsCustom);
        string clonePath = Path.Combine(_root, "MAPS", "ENDL_005");
        Assert.Equal(new byte[] { 1, 2, 3, 4 }, File.ReadAllBytes(Path.Combine(clonePath, "DATA", "unknown.dat")));
        Assert.Equal("Test map", PutTextDocument.Load(Path.Combine(clonePath, "TEXT", "US", "briefing.put")).GetValue("briefing_titel_1"));
        Assert.Contains("MAPS/ENDL_005/", ReadPfil(Path.Combine(clonePath, "Endlos_Rom_Siedlung1.sdl")));
        Assert.True(File.Exists(Path.Combine(clonePath, CustomMapManifest.MarkerFileName)));
        Assert.Contains(CustomMapManifest.Load(_root).Entries, x => x.Slot == 5 && x.SourceSlot == 0);
        Assert.False(Directory.Exists(Path.Combine(_root, "MAPS", "ENDL_005.tmp_arm")));
    }

    [Fact]
    public void Clone_rejects_names_that_the_game_encoding_cannot_store_before_copying_files()
    {
        CreateSourceMap();

        ArgumentException error = Assert.Throws<ArgumentException>(() => new EndlessMapCloner().Clone(_root, 0, 5, "地圖副本"));

        Assert.Contains("遊戲無法儲存的字元", error.Message);
        Assert.False(Directory.Exists(Path.Combine(_root, "MAPS", "ENDL_005")));
        Assert.False(Directory.Exists(Path.Combine(_root, "MAPS", "ENDL_005.tmp_arm")));
    }

    [Fact]
    public void Documents_Keep_pfil_header_and_only_change_requested_values()
    {
        string path = Path.Combine(_root, "boden.ini");
        Directory.CreateDirectory(_root);
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Waterlevel]\r\n120\r\n[WaterColor]\r\n0xffdfbf\r\n[WaterWarpShift]\r\n12\r\n[WaterBumpAmplitude]\r\n256\r\n[WaterBumpFrequency]\r\n4\r\n[FlashPropability]\r\n8\r\n"));
        byte[] original = File.ReadAllBytes(path);

        var document = BodenIniDocument.Load(path);
        document.SetValue("Waterlevel", "180");
        document.SetValue("WaterWarpShift", "14");
        document.SetValue("WaterBumpAmplitude", "512");
        document.SetValue("WaterBumpFrequency", "8");
        document.SetValue("FlashPropability", "16");
        document.Save();

        byte[] saved = File.ReadAllBytes(path);
        Assert.Equal(original.Take(16), saved.Take(16));
        string text = SyntheticFixture.Text(saved);
        Assert.Contains("[Waterlevel]\r\n180", text);
        Assert.Contains("[WaterColor]\r\n0xffdfbf", text);
        Assert.Contains("[WaterWarpShift]\r\n14", text);
        Assert.Contains("[WaterBumpAmplitude]\r\n512", text);
        Assert.Contains("[WaterBumpFrequency]\r\n8", text);
        Assert.Contains("[FlashPropability]\r\n16", text);
    }

    [Fact]
    public void PutTextDocument_RoundTrips_composite_briefing_and_team_names()
    {
        string path = Path.Combine(_root, "briefing.put");
        Directory.CreateDirectory(_root);
        File.WriteAllBytes(path, SyntheticFixture.Pfil(
            "var:briefing_titel_1 =\"Original\";\r\n" +
            "var:briefing_titel_2 =\"Subtitle\";\r\n" +
            "var:briefing_text =\"First line\\n\"\r\n+\"Second line\";\r\n" +
            "var:briefing_text_teamname0 =\"Player\";\r\n"));
        byte[] original = File.ReadAllBytes(path);

        var document = PutTextDocument.Load(path);
        Assert.Equal("First line\nSecond line", document.GetCompositeValue("briefing_text"));
        document.SetCompositeValue("briefing_text", "Changed line 1\nChanged \"line\" 2");
        document.SetValue("briefing_text_teamname0", "Romans");
        document.Save();

        var loaded = PutTextDocument.Load(path);
        Assert.Equal(original.Take(16), File.ReadAllBytes(path).Take(16));
        Assert.Equal("Changed line 1\nChanged \"line\" 2", loaded.GetCompositeValue("briefing_text"));
        Assert.Equal("Romans", loaded.GetValue("briefing_text_teamname0"));
        Assert.Equal("Original", loaded.GetValue("briefing_titel_1"));
    }

    [Fact]
    public void BodenTexturesDocument_Changes_exactly_one_tile_and_round_trips()
    {
        string path = Path.Combine(_root, "boden.txt");
        Directory.CreateDirectory(_root);
        string[] tiles = Enumerable.Range(0, 4096).Select(i => $"T{i:0000000}").ToArray();
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", tiles) + "\r\n"));

        var document = BodenTexturesDocument.Load(path);
        document.SetTexture(3, 2, "4BJ___51");
        document.Save();

        var loaded = BodenTexturesDocument.Load(path);
        Assert.Equal("4BJ___51", loaded.GetTexture(3, 2));
        Assert.Equal("T0000000", loaded.GetTexture(0, 0));
        Assert.Equal("T0004095", loaded.GetTexture(63, 63));
    }

    [Fact]
    public void BodenTexturesDocument_SetTextures_writes_whole_grid_in_one_pass()
    {
        string path = Path.Combine(_root, "boden.txt");
        Directory.CreateDirectory(_root);
        string[] tiles = Enumerable.Range(0, 4096).Select(i => $"T{i:0000000}").ToArray();
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", tiles) + "\r\n"));

        var document = BodenTexturesDocument.Load(path);
        string[] replacement = Enumerable.Range(0, 4096).Select(i => $"R{i:0000000}").ToArray();
        document.SetTextures(replacement);
        document.Save();

        var loaded = BodenTexturesDocument.Load(path);
        Assert.Equal("R0000000", loaded.GetTexture(0, 0));
        Assert.Equal("R0004095", loaded.GetTexture(63, 63));
        Assert.Equal(replacement, loaded.Textures);
    }

    [Fact]
    public void BodenTexturesDocument_SetTextures_rejects_wrong_count()
    {
        string path = Path.Combine(_root, "boden.txt");
        Directory.CreateDirectory(_root);
        string[] tiles = Enumerable.Range(0, 4096).Select(i => $"T{i:0000000}").ToArray();
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", tiles) + "\r\n"));

        var document = BodenTexturesDocument.Load(path);

        Assert.Throws<ArgumentException>(() => document.SetTextures(new[] { "only", "three", "items" }));
    }

    [Fact]
    public void TerrainEditHistory_change_survives_boden_save_and_reload()
    {
        string path = Path.Combine(_root, "boden.txt");
        Directory.CreateDirectory(_root);
        string[] tiles = Enumerable.Range(0, 4096).Select(i => $"T{i:0000000}").ToArray();
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", tiles) + "\r\n"));
        var document = BodenTexturesDocument.Load(path);
        var history = new TerrainEditHistory(document.Dimension, document.Textures);

        Assert.NotNull(history.Paint(7, 9, "NEW_TEXTURE"));
        Assert.True(history.CommitStroke());
        document.SetTexture(7, 9, history.Current[9 * 64 + 7]);
        document.Save();
        history.CommitBaseline();

        var reloaded = BodenTexturesDocument.Load(path);
        Assert.Equal("NEW_TEXTURE", reloaded.GetTexture(7, 9));
        Assert.False(history.IsDirty);
    }

    [Fact]
    public void Delete_Removes_custom_map_and_manifest_entry()
    {
        CreateSourceMap();
        var catalog = new EndlessMapCatalog();
        new EndlessMapCloner(catalog).Clone(_root, 0, 5, "Delete me");

        new EndlessMapDeleter(catalog).Delete(_root, 5);

        Assert.False(Directory.Exists(Path.Combine(_root, "MAPS", "ENDL_005")));
        Assert.DoesNotContain(CustomMapManifest.Load(_root).Entries, x => x.Slot == 5);
        Assert.Equal(5, catalog.GetNextFreeSlot(_root));
    }

    [Fact]
    public void Delete_Refuses_original_map()
    {
        CreateSourceMap();

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(() => new EndlessMapDeleter().Delete(_root, 0));

        Assert.Contains("只能刪除", error.Message);
        Assert.True(Directory.Exists(Path.Combine(_root, "MAPS", "ENDL_000")));
    }

    [Fact]
    public void GameMapCatalog_Lists_campaign_and_endless_maps()
    {
        CreateSourceMap();
        string campaign = Path.Combine(_root, "MAPS", "KAMP_000");
        CopyDirectory(Path.Combine(_root, "MAPS", "ENDL_000"), campaign);
        File.WriteAllBytes(Path.Combine(campaign, "minimap.bmp"), MinimalBitmap());
        File.WriteAllBytes(Path.Combine(_root, "MAPS", "ENDL_000", "minimap.bmp"), MinimalBitmap());
        File.WriteAllBytes(Path.Combine(campaign, "boden.txt"), SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n"));
        File.WriteAllBytes(Path.Combine(_root, "MAPS", "ENDL_000", "boden.txt"), SyntheticFixture.Pfil("[Dimension]\r\n64\r\n[Texturen]\r\n"));

        IReadOnlyList<GameMapInfo> maps = new GameMapCatalog().List(_root);

        Assert.Contains(maps, x => x.Id == "KAMP_000" && x.Category == "劇情戰役");
        Assert.Contains(maps, x => x.Id == "ENDL_000" && x.Category == "無盡模式");
    }

    [Fact]
    public void SdlSceneCatalog_Maps_reference_and_local_position_to_world_coordinates()
    {
        Directory.CreateDirectory(_root);
        string path = Path.Combine(_root, "scene.sdl");
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[settlement]\r\nrefpos=1632,159,5792\r\n[object0000]\r\nnamedef=BauRomHau00_Haupthaus\r\npos=-32.00,2.00,64.00\r\nteam=3\r\n"));

        MapSceneObject item = Assert.Single(SdlSceneCatalog.Load(path));

        Assert.Equal("建築", item.Kind); Assert.Equal(1600, item.WorldX); Assert.Equal(161, item.WorldY); Assert.Equal(5856, item.WorldZ); Assert.Equal(3, item.Team);
        Assert.Equal(0, item.ObjectIndex); Assert.Equal(-32, item.LocalX); Assert.Equal(2, item.LocalY); Assert.Equal(64, item.LocalZ);
    }

    [Fact]
    public void SdlSceneEditService_Writes_only_changed_fields_and_rollback_restores_custom_map()
    {
        string map = CreateCustomSceneMap(5, includeMarker: true);
        IReadOnlyList<MapSceneObject> baseline = SdlSceneCatalog.LoadDirectory(map);
        MapSceneObject original = Assert.Single(baseline);
        MapSceneObject edited = original with { Team = 4, LocalX = 96, LocalY = 3, LocalZ = -64 };
        IReadOnlyList<MapSceneObject> current = [edited];
        Assert.True(SdlSceneEditService.HasChanges(baseline, current));

        using (var rollback = new FileRollbackScope())
        {
            SdlSceneEditService.SaveChanges(map, baseline, current, rollback);
            var changed = SdlDocument.Load(Path.Combine(map, original.SourceFile));
            Assert.Equal("4", changed.Objects[0].GetValue("team"));
            Assert.Equal("96.00,3.00,-64.00", changed.Objects[0].GetValue("pos"));
        }

        var restored = SdlDocument.Load(Path.Combine(map, original.SourceFile));
        Assert.Equal("3", restored.Objects[0].GetValue("team"));
        Assert.Equal("-32.00,2.00,64.00", restored.Objects[0].GetValue("pos"));
    }

    [Fact]
    public void SdlSceneEditService_Commits_custom_map_change_and_rejects_original_or_unmarked_maps()
    {
        string custom = CreateCustomSceneMap(5, includeMarker: true);
        IReadOnlyList<MapSceneObject> baseline = SdlSceneCatalog.LoadDirectory(custom);
        IReadOnlyList<MapSceneObject> edited = [baseline[0] with { Team = 8 }];
        using (var rollback = new FileRollbackScope())
        {
            SdlSceneEditService.SaveChanges(custom, baseline, edited, rollback);
            rollback.Commit();
        }
        Assert.Equal("8", SdlDocument.Load(Path.Combine(custom, baseline[0].SourceFile)).Objects[0].GetValue("team"));
        using (var rollback = new FileRollbackScope())
        {
            SdlSceneEditService.SaveChanges(custom, edited, baseline, rollback);
            rollback.Commit();
        }
        Assert.Equal("3", SdlDocument.Load(Path.Combine(custom, baseline[0].SourceFile)).Objects[0].GetValue("team"));

        string original = CreateCustomSceneMap(0, includeMarker: true);
        IReadOnlyList<MapSceneObject> originalObjects = SdlSceneCatalog.LoadDirectory(original);
        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidOperationException>(() => SdlSceneEditService.SaveChanges(original, originalObjects, [originalObjects[0] with { Team = 1 }], rollback));

        string unmarked = CreateCustomSceneMap(6, includeMarker: false);
        IReadOnlyList<MapSceneObject> unmarkedObjects = SdlSceneCatalog.LoadDirectory(unmarked);
        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidOperationException>(() => SdlSceneEditService.SaveChanges(unmarked, unmarkedObjects, [unmarkedObjects[0] with { Team = 1 }], rollback));

        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidDataException>(() => SdlSceneEditService.SaveChanges(custom, baseline, [baseline[0] with { SourceFile = "..\\outside.sdl", Team = 1 }], rollback));
    }

    [Fact]
    public void SdlSceneEditService_Duplicates_template_and_removes_object_with_renumbering()
    {
        string map = CreateCustomSceneMapWithTwoObjects(7);
        IReadOnlyList<MapSceneObject> baseline = SdlSceneCatalog.LoadDirectory(map);
        Assert.Equal(2, baseline.Count);
        string sourceFile = baseline[0].SourceFile;

        using (var rollback = new FileRollbackScope())
        {
            SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                removals: [new SdlSceneObjectRemoval(sourceFile, 1)],
                additions: [new SdlSceneObjectAddition(sourceFile, 0, Team: 5, LocalX: 10, LocalY: 20, LocalZ: 30)]);
            rollback.Commit();
        }

        var document = SdlDocument.Load(Path.Combine(map, sourceFile));
        Assert.Equal(2, document.Objects.Count);
        Assert.Equal(new[] { 0, 1 }, document.Objects.Select(x => x.Index));
        // object0 = 原件；object1 = 由模板複製、覆寫 team/pos，其餘欄位沿用模板。
        Assert.Equal("BauRomHau00_Haupthaus", document.Objects[0].GetValue("namedef"));
        Assert.Equal("3", document.Objects[0].GetValue("team"));
        SdlObjectSection copy = document.Objects[1];
        Assert.Equal("BauRomHau00_Haupthaus", copy.GetValue("namedef"));
        Assert.Equal("1676", copy.GetValue("def"));
        Assert.Equal("0.00", copy.GetValue("angle"));
        Assert.Equal("5", copy.GetValue("team"));
        Assert.Equal("10.00,20.00,30.00", copy.GetValue("pos"));
        // 原 object0001（Turm）已刪除且無殘留編號。
        Assert.DoesNotContain("BauRomTur00_Turm", ReadPfil(Path.Combine(map, sourceFile)));
        Assert.DoesNotContain("[object0002]", ReadPfil(Path.Combine(map, sourceFile)));
    }

    [Fact]
    public void SdlSceneEditService_Copy_then_delete_original_acts_as_move()
    {
        string map = CreateCustomSceneMapWithTwoObjects(8);
        IReadOnlyList<MapSceneObject> baseline = SdlSceneCatalog.LoadDirectory(map);
        string sourceFile = baseline[0].SourceFile;

        using (var rollback = new FileRollbackScope())
        {
            SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                removals: [new SdlSceneObjectRemoval(sourceFile, 0)],
                additions: [new SdlSceneObjectAddition(sourceFile, 0, Team: 3, LocalX: 100, LocalY: 2, LocalZ: -50)]);
            rollback.Commit();
        }

        var document = SdlDocument.Load(Path.Combine(map, sourceFile));
        Assert.Equal(2, document.Objects.Count);
        Assert.Equal(new[] { 0, 1 }, document.Objects.Select(x => x.Index));
        Assert.Equal("BauRomTur00_Turm", document.Objects[0].GetValue("namedef"));
        Assert.Equal("BauRomHau00_Haupthaus", document.Objects[1].GetValue("namedef"));
        Assert.Equal("100.00,2.00,-50.00", document.Objects[1].GetValue("pos"));
    }

    [Fact]
    public void SdlSceneEditService_Rejects_invalid_structural_changes()
    {
        string map = CreateCustomSceneMapWithTwoObjects(9);
        IReadOnlyList<MapSceneObject> baseline = SdlSceneCatalog.LoadDirectory(map);
        string sourceFile = baseline[0].SourceFile;
        byte[] before = File.ReadAllBytes(Path.Combine(map, sourceFile));

        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidDataException>(() => SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                removals: [new SdlSceneObjectRemoval(sourceFile, 99)]));
        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidDataException>(() => SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                removals: [new SdlSceneObjectRemoval(sourceFile, 0), new SdlSceneObjectRemoval(sourceFile, 0)]));
        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidDataException>(() => SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                additions: [new SdlSceneObjectAddition(sourceFile, 99, 3, 0, 0, 0)]));
        using (var rollback = new FileRollbackScope())
            Assert.Throws<InvalidDataException>(() => SdlSceneEditService.SaveChanges(map, baseline, baseline, rollback,
                additions: [new SdlSceneObjectAddition(sourceFile, 0, Team: 99, 0, 0, 0)]));

        Assert.Equal(before, File.ReadAllBytes(Path.Combine(map, sourceFile)));
    }

    [Fact]
    public void SdlDocument_Parses_and_updates_settlement_and_object_fields_without_losing_unknown_lines()
    {
        string path = CreateEditableSdl();
        byte[] original = File.ReadAllBytes(path);
        var document = SdlDocument.Load(path);

        Assert.Equal("1632,159,5792", document.Settlement["refpos"]);
        SdlObjectSection item = Assert.Single(document.Objects);
        Assert.Equal("BauRomPal02_Palisadenecke", item.GetValue("namedef"));
        Assert.Equal("926298413", item.GetValue("objdefv0"));

        document.TranslateSettlement(64, 0, -128);
        document.SetObjectTeam(0, 3);
        document.SetObjectPosition(0, new SdlVector3(-256, 4.5f, 128));
        document.SetObjectAngle(0, 90);
        document.SetObjectDefinition(0, 1700, "BauRomHau00_Haupthaus");
        document.Save();

        var loaded = SdlDocument.Load(path);
        Assert.Equal(original.Take(16), File.ReadAllBytes(path).Take(16));
        Assert.Equal("1696,159,5664", loaded.Settlement["refpos"]);
        item = Assert.Single(loaded.Objects);
        Assert.Equal("3", item.GetValue("team"));
        Assert.Equal("-256.00,4.50,128.00", item.GetValue("pos"));
        Assert.Equal("90.00", item.GetValue("angle"));
        Assert.Equal("1700", item.GetValue("def"));
        Assert.Equal("BauRomHau00_Haupthaus", item.GetValue("namedef"));
        Assert.Contains("; unknown=keep-this-comment", ReadPfil(path));
    }

    [Fact]
    public void SdlDocument_Add_remove_and_renumber_objects_round_trips()
    {
        string path = CreateEditableSdl();
        var document = SdlDocument.Load(path);
        int added = document.AddObject(new Dictionary<string, string>
        {
            ["namedef"] = "FigRomTie00_Test",
            ["def"] = "42",
            ["pos"] = "1.00,2.00,3.00",
            ["team"] = "8",
            ["angle"] = "45.00",
            ["custom_field"] = "preserved"
        });
        Assert.Equal(1, added);
        document.RemoveObject(0);
        document.Save();

        var loaded = SdlDocument.Load(path);
        SdlObjectSection remaining = Assert.Single(loaded.Objects);
        Assert.Equal(0, remaining.Index);
        Assert.Equal("FigRomTie00_Test", remaining.GetValue("namedef"));
        Assert.Equal("preserved", remaining.GetValue("custom_field"));
        Assert.Contains("[object0000]", ReadPfil(path));
        Assert.DoesNotContain("[object0001]", ReadPfil(path));
    }

    [Fact]
    public void SdlDocument_Local_real_samples_noop_round_trip_preserves_decoded_text()
    {
        string sourceDirectory = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案/MAPS/ENDL_000"));
        if (!Directory.Exists(sourceDirectory)) return; // 專有樣本不進版控；CI 無此目錄時略過。
        string[] sources = Directory.GetFiles(sourceDirectory, "Endlos_*_Siedlung*.sdl", SearchOption.TopDirectoryOnly);
        Assert.Equal(8, sources.Length);
        Directory.CreateDirectory(_root);

        foreach (string source in sources)
        {
            string copy = Path.Combine(_root, Path.GetFileName(source));
            File.Copy(source, copy);
            byte[] before = GameLZSS.DecompressPfil(File.ReadAllBytes(copy));
            var document = SdlDocument.Load(copy);

            Assert.NotEmpty(document.Settlement["refpos"]);
            Assert.NotEmpty(document.Objects);
            document.Save();

            Assert.Equal(before, GameLZSS.DecompressPfil(File.ReadAllBytes(copy)));
        }
    }

    private static byte[] MinimalBitmap() => new byte[] { (byte)'B', (byte)'M' };
    private string CreateEditableSdl()
    {
        Directory.CreateDirectory(_root);
        string path = Path.Combine(_root, "editable.sdl");
        File.WriteAllBytes(path, SyntheticFixture.Pfil(
            "[settlement]\r\n" +
            "name=Endlos_Rom_Siedlung1\r\n" +
            "refpos=1632,159,5792\r\n" +
            "; unknown=keep-this-comment\r\n\r\n" +
            "[object0000]\r\n" +
            "namedef=BauRomPal02_Palisadenecke\r\n" +
            "def=1676\r\n" +
            "pos=-320.00,0.00,-512.00\r\n" +
            "team=8\r\n" +
            "nation=3\r\n" +
            "objdefn0=\r\n" +
            "objdefv0=926298413\r\n" +
            "angle=0.00\r\n"));
        return path;
    }
    private string CreateCustomSceneMap(int slot, bool includeMarker)
    {
        string map = Path.Combine(_root, "MAPS", $"ENDL_{slot:000}");
        Directory.CreateDirectory(map);
        if (includeMarker) File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        string path = Path.Combine(map, "Endlos_Rom_Siedlung1.sdl");
        File.WriteAllBytes(path, SyntheticFixture.Pfil(
            "[settlement]\r\nrefpos=1632,159,5792\r\n" +
            "[object0000]\r\nnamedef=BauRomHau00_Haupthaus\r\ndef=1676\r\npos=-32.00,2.00,64.00\r\nteam=3\r\nangle=0.00\r\n"));
        return map;
    }
    private string CreateCustomSceneMapWithTwoObjects(int slot)
    {
        string map = Path.Combine(_root, "MAPS", $"ENDL_{slot:000}");
        Directory.CreateDirectory(map);
        File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        File.WriteAllBytes(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"), SyntheticFixture.Pfil(
            "[settlement]\r\nrefpos=1632,159,5792\r\n" +
            "[object0000]\r\nnamedef=BauRomHau00_Haupthaus\r\ndef=1676\r\npos=-32.00,2.00,64.00\r\nteam=3\r\nangle=0.00\r\n" +
            "[object0001]\r\nnamedef=BauRomTur00_Turm\r\ndef=1700\r\npos=64.00,0.00,-96.00\r\nteam=4\r\nangle=90.00\r\n"));
        return map;
    }

    private static void CopyDirectory(string source, string destination)
    {
        Directory.CreateDirectory(destination);
        foreach (string file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
        {
            string target = Path.Combine(destination, Path.GetRelativePath(source, file)); Directory.CreateDirectory(Path.GetDirectoryName(target)!); File.Copy(file, target);
        }
    }

    private void CreateSourceMap()
    {
        string map = Path.Combine(_root, "MAPS", "ENDL_000");
        Directory.CreateDirectory(Path.Combine(map, "TEXT", "US"));
        Directory.CreateDirectory(Path.Combine(map, "DATA"));
        File.WriteAllBytes(Path.Combine(map, "TEXT", "US", "briefing.put"), SyntheticFixture.Pfil("var:briefing_titel_1 =\"原始地圖\";\r\n"));
        File.WriteAllBytes(Path.Combine(map, "boden.ini"), SyntheticFixture.Pfil("[Waterlevel]\r\n120\r\n[WaterColor]\r\n0xffdfbf\r\n[DayStartTime]\r\n6\r\n[DayEndTime]\r\n20\r\n[RainDropsOnWater]\r\n1\r\n"));
        File.WriteAllBytes(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"), SyntheticFixture.Pfil("[object0000]\r\nname    =MAPS/ENDL_000/Endlos_Rom_Siedlung1.sdl\r\n"));
        File.WriteAllBytes(Path.Combine(map, "DATA", "unknown.dat"), new byte[] { 1, 2, 3, 4 });
    }

    private static string ReadPfil(string path) => SyntheticFixture.GameEncoding.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
