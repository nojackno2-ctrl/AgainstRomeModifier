using System.IO.Compression;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeSpriteCatalogTests
{
    private const string AlrList = "[AlrNames]\r\n0000,unit.alr,0000\r\n0001,broken.alr,0000\r\n0002,missing.alr,0000\r\n0003,trunk.alr\r\n0004,crown.alr\r\n";
    private const string AptList = "[AptNames]\r\n0000,house.apt,0000\r\n";

    [Fact]
    public void Alr_still_uses_direction_rows_team_palette_and_canvas_centre_ground_anchor()
    {
        using var catalog = Catalog();
        NativeSprite front = catalog.GetSprite("FigTest", team: 0)!;
        // Frame 0 sits at offset (7,11) in a 12x18 canvas; opaque pixels x=1..3 are cropped.
        Assert.Equal((3, 1), (front.Width, front.Height));
        Assert.Equal(new uint[] { 0xFF302010, 0, 0xFF302010 }, front.ArgbPixels);
        Assert.Equal((6 - 7 - 1, 9 - 11), (front.AnchorX, front.AnchorY));
        Assert.Equal("unit.alr", front.AssetName);
        Assert.Equal(new uint[] { 0xFFB0A090, 0, 0xFFB0A090 }, catalog.GetSprite("FigTest", team: 1)!.ArgbPixels);
        Assert.Equal(catalog.GetSprite("FigTest", team: 1)!.ArgbPixels, catalog.GetSprite("FigTest", team: 9)!.ArgbPixels);
        // Direction 1 starts at frame 1 * LayoutColumns(2) = frame 2, offset (2,3).
        NativeSprite side = catalog.GetSprite("figtest", direction: 1)!;
        Assert.Equal(new uint[] { 0xFF302010, 0xFF605040 }, side.ArgbPixels);
        Assert.Equal((6 - 2, 9 - 3), (side.AnchorX, side.AnchorY));
        Assert.Same(side, catalog.GetSprite("FigTest", direction: -1)); // wraps and is cached
        // Non-team palette type ignores the team.
        Assert.Equal(front.ArgbPixels, catalog.GetSprite("LanTest", team: 1)!.ArgbPixels);
    }

    [Theory]
    // Verified in game for 16 rows: 0->14, 45->12, 90->10, 135->8, 180->6.
    [InlineData(0f, 16, 14)]
    [InlineData(45f, 16, 12)]
    [InlineData(90f, 16, 10)]
    [InlineData(135f, 16, 8)]
    [InlineData(180f, 16, 6)]
    [InlineData(315f, 16, 0)]
    [InlineData(-45f, 16, 0)]
    [InlineData(360f, 16, 14)]
    [InlineData(22.5f, 16, 13)]
    [InlineData(0f, 32, 28)]   // cavalry rows scale the same mapping (unverified in game)
    [InlineData(90f, 32, 20)]
    [InlineData(123f, 1, 0)]
    [InlineData(float.NaN, 16, 0)]
    public void Scenario_angle_maps_to_direction_row(float degrees, int rows, int expected)
        => Assert.Equal(expected, NativeSpriteCatalog.DirectionForAngle(degrees, rows));

    [Fact]
    public void Angle_selects_the_direction_row_of_the_asset()
    {
        using var catalog = Catalog();
        // The fixture has 2 rows: angle 0 maps to row 14*2/16 = 1, angle 180 to row 0.
        Assert.Same(catalog.GetSprite("FigTest", direction: 1), catalog.GetSprite("FigTest", angleDegrees: 0));
        Assert.Same(catalog.GetSprite("FigTest", direction: 0), catalog.GetSprite("FigTest", angleDegrees: 180));
    }

    [Fact]
    public void Apt_still_uses_finished_stage_and_cropped_anchor()
    {
        using var catalog = Catalog();
        NativeSprite house = catalog.GetSprite("BauTest")!;
        Assert.Equal((3, 1), (house.Width, house.Height));
        Assert.Equal(new uint[] { 0xFF332211, 0, 0xFF332211 }, house.ArgbPixels);
        Assert.Equal((0, 15), (house.AnchorX, house.AnchorY));
    }

    [Fact]
    public void Layered_alr_composites_crown_over_trunk_on_a_shared_ground_anchor()
    {
        using var catalog = Catalog();
        NativeSprite tree = catalog.GetSprite("LanTree")!;
        Assert.Equal("trunk.alr+crown.alr", tree.AssetName);
        // Ground-relative: trunk x 0, y -3..-1; crown x -1..1, y -4..-3 => 3x4 union anchored at (1,4).
        Assert.Equal((3, 4, 1, 4), (tree.Width, tree.Height, tree.AnchorX, tree.AnchorY));
        const uint G = 0xFF00FF00, R = 0xFFFF0000;
        Assert.Equal(new uint[] { G, G, G, G, G, G, 0, R, 0, 0, R, 0 }, tree.ArgbPixels); // crown wins the overlap
        NativeSprite cover = catalog.GetSprite("LanCover")!; // alrml-only ground cover uses that layer alone
        Assert.Equal(("crown.alr", 3, 2, 1, 4), (cover.AssetName, cover.Width, cover.Height, cover.AnchorX, cover.AnchorY));
        NativeSprite trunkOnly = catalog.GetSprite("LanBrokenCrown")!; // an undecodable crown keeps the trunk
        Assert.Equal(("trunk.alr", 1, 3), (trunkOnly.AssetName, trunkOnly.Width, trunkOnly.Height));
    }

    /// <summary>v6 single-frame ALR: 8x8 canvas (ground anchor 4,4) with a palette-index-1 frame at the given offset.</summary>
    private static byte[] SolidAlr(int width, int height, int offsetX, int offsetY, uint color)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream);
        foreach (uint word in new uint[] { 0x41524C41, 6, 0, 1, 8, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 8, 8, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
        int pixels = width * height, padded = (pixels + 3) & ~3;
        writer.Write(-1); writer.Write((uint)(8 + padded)); writer.Write((uint)(offsetX | offsetY << 16));
        writer.Write((uint)(width | height << 11 | 2 << 22)); writer.Write((uint)pixels);
        writer.Write(0u); writer.Write(color);
        writer.Write(Enumerable.Repeat((byte)1, pixels).ToArray()); writer.Write(new byte[padded - pixels]);
        for (int y = 0; y <= height; y++) writer.Write((uint)(y * width));
        return stream.ToArray();
    }

    [Theory]
    [InlineData(new uint[] { 5, 2, 5, 25 }, 1250, 1000)] // gerhau02: last build stage, intact, frame 0
    [InlineData(new uint[] { 5, 2, 5, 1 }, 50, 40)]
    [InlineData(new uint[] { 1, 2, 1, 1 }, 2, 0)]
    [InlineData(new uint[] { 5, 2, 5, 25 }, 10, 9)] // inconsistent table falls back to the last frame
    public void Finished_apt_frame_is_last_construction_stage(uint[] layout, int frames, int expected)
        => Assert.Equal(expected, NativeSpriteCatalog.AptFinishedFrame(layout, frames));

    [Fact]
    public void Unknown_unmapped_missing_and_corrupt_assets_return_null()
    {
        int reads = 0;
        using var catalog = NativeSpriteCatalog.FromText(Objdef(), AlrList, AptList,
            name => { reads++; return name switch { "unit.alr" => NativeAlrDocumentTests.Fixture(6), "broken.alr" => [1, 2, 3], _ => null }; },
            _ => null);
        Assert.Null(catalog.GetSprite("Nope"));
        Assert.Null(catalog.GetSprite("ParNone"));
        Assert.Null(catalog.GetSprite("FigMissing"));
        Assert.Null(catalog.GetSprite("FigBroken"));
        Assert.Null(catalog.GetSprite("FigBroken"));
        Assert.Null(catalog.GetSprite("BauTest"));
        Assert.Equal(2, reads); // broken and missing archives are read once and cached
        Assert.True(catalog.TryGetDefinition(" FigTest ", out NativeSpriteDefinition definition));
        Assert.Equal(new NativeSpriteDefinition(10, "FigTest", 0, -1, 1, AnimationAdd: 0), definition);
    }

    [Fact]
    public void Open_reads_game_layout_read_only_and_returns_null_when_incomplete()
    {
        string root = Path.Combine(Path.GetTempPath(), "arm-sprite-catalog-" + Guid.NewGuid().ToString("N"));
        try
        {
            Directory.CreateDirectory(Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS"));
            File.WriteAllText(Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"), Objdef());
            File.WriteAllText(Path.Combine(root, "SYSTEM", "cl_alr.ini"), AlrList);
            Assert.Null(NativeSpriteCatalog.Open(root));
            File.WriteAllText(Path.Combine(root, "SYSTEM", "cl_apt.ini"), AptList);
            Zip(Path.Combine(root, "alr.dat"), "SYSTEM/DATA/ALR/unit.alr", NativeAlrDocumentTests.Fixture(6));
            Zip(Path.Combine(root, "apt.dat"), "SYSTEM/DATA/APT/house.apt", NativeAptDocumentTests.Fixture(3));
            using (var catalog = NativeSpriteCatalog.Open(root)!)
            {
                Assert.Equal(3, catalog.GetSprite("FigTest")!.ArgbPixels.Length);
                Assert.NotNull(catalog.GetSprite("BauTest"));
                using var concurrent = File.Open(Path.Combine(root, "alr.dat"), FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            }
            File.Delete(Path.Combine(root, "alr.dat")); // archives are released on dispose
        }
        finally { Directory.Delete(root, recursive: true); }
    }

    private static NativeSpriteCatalog Catalog() => NativeSpriteCatalog.FromText(Objdef(), AlrList, AptList,
        name => name switch
        {
            "unit.alr" => NativeAlrDocumentTests.Fixture(6),
            "trunk.alr" => SolidAlr(1, 3, 4, 1, 0x0000FF), // 1x3 red trunk standing on the ground anchor (4,4)
            "crown.alr" => SolidAlr(3, 2, 3, 0, 0x00FF00), // 3x2 green crown overlapping the trunk top
            "broken.alr" => [1, 2, 3],
            _ => null
        },
        name => name == "house.apt" ? NativeAptDocumentTests.Fixture(3) : null);

    private static string Objdef() => string.Join("\r\n",
        "[ObjectDefaults]",
        ";idx ,activ,alen ,rotsp,moves,alrid",
        Row(10, "FigTest", alr: 0, apt: -1, palette: 1),
        Row(11, "LanTest", alr: 0, apt: -1, palette: 0),
        Row(12, "BauTest", alr: -1, apt: 0, palette: 0),
        Row(13, "ParNone", alr: -1, apt: -1, palette: 0),
        Row(14, "FigBroken", alr: 1, apt: -1, palette: 0),
        Row(15, "FigMissing", alr: 2, apt: -1, palette: 0),
        Row(16, "LanTree", alr: 3, apt: -1, palette: 0, layer: 4, layerType: 2),
        Row(17, "LanCover", alr: -1, apt: -1, palette: 0, layer: 4, layerType: 10),
        Row(18, "LanBrokenCrown", alr: 3, apt: -1, palette: 0, layer: 1, layerType: 2));

    private static string Row(int id, string name, int alr, int apt, int palette, int layer = -1, int layerType = -1)
    {
        string[] columns = Enumerable.Repeat("   0", 60).ToArray();
        columns[0] = id.ToString(); columns[5] = alr.ToString(); columns[14] = apt.ToString();
        columns[8] = layer.ToString(); columns[10] = layerType.ToString();
        columns[17] = palette.ToString(); columns[52] = name;
        return string.Join(",", columns);
    }

    private static void Zip(string path, string entryName, byte[] content)
    {
        using var archive = ZipFile.Open(path, ZipArchiveMode.Create);
        using Stream stream = archive.CreateEntry(entryName).Open();
        stream.Write(content);
    }
}
