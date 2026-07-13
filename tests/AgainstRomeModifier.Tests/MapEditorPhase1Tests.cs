using System.Text;
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
    }

    private static byte[] MinimalBitmap() => new byte[] { (byte)'B', (byte)'M' };
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
