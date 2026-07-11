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
        File.WriteAllBytes(path, SyntheticFixture.Pfil("[Waterlevel]\r\n120\r\n[WaterColor]\r\n0xffdfbf\r\n"));
        byte[] original = File.ReadAllBytes(path);

        var document = BodenIniDocument.Load(path);
        document.SetValue("Waterlevel", "180");
        document.Save();

        byte[] saved = File.ReadAllBytes(path);
        Assert.Equal(original.Take(16), saved.Take(16));
        string text = SyntheticFixture.Text(saved);
        Assert.Contains("[Waterlevel]\r\n180", text);
        Assert.Contains("[WaterColor]\r\n0xffdfbf", text);
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
