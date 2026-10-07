using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeSpriteAnimationTests
{
    [Fact]
    public void Alr_idle_uses_only_animation_zero_direction_team_and_cached_readonly_frames()
    {
        int reads = 0;
        using var catalog = NativeSpriteCatalog.FromText(Row("Unit"), "0,unit.alr", "",
            _ => { reads++; return Alr(); }, _ => null);
        NativeSpriteAnimation sequence = catalog.GetAnimation("Unit", team: 1, angleDegrees: 45)!;
        Assert.Equal(24, sequence.Frames.Count);
        Assert.Equal(1000 / 24d, sequence.FrameDurationMs);
        Assert.Equal(0xFF0000FFu, sequence.Frames[0].ArgbPixels[0]); // variant 1, direction 12
        Assert.Equal(0xFFFFFF00u, sequence.Frames[1].ArgbPixels[0]);
        Assert.Equal((4, 4), (sequence.Frames[0].AnchorX, sequence.Frames[0].AnchorY));
        Assert.Equal((3, 4), (sequence.Frames[1].AnchorX, sequence.Frames[1].AnchorY)); // differing crop/offset, same ground anchor
        Assert.Same(catalog.GetSprite("Unit", 1, angleDegrees: 45), sequence.Frames[0]);
        Assert.Same(sequence, catalog.GetAnimation(" unit ", 1, 405));
        Assert.Same(sequence, catalog.GetAnimation("Unit", 99, 45)); // clamped palette
        Assert.NotSame(sequence, catalog.GetAnimation("Unit", 0, 45));
        Assert.NotSame(sequence, catalog.GetAnimation("Unit", 1, 90));
        Assert.Equal(1, reads);
        Assert.Throws<NotSupportedException>(() => ((IList<NativeSprite>)sequence.Frames)[0] = sequence.Frames[1]);
    }

    [Theory]
    [InlineData(0d, 0)]
    [InlineData(41d, 0)]
    [InlineData(42d, 1)]
    [InlineData(500d, 12)]
    [InlineData(999d, 23)]
    [InlineData(1000d, 0)]
    [InlineData(1500d, 12)]
    [InlineData(-1d, 0)]
    [InlineData(double.NaN, 0)]
    public void Frame_selection_wraps_the_exact_cycle_without_rounding_drift(double ms, int index)
    {
        using var catalog = Catalog(Row("Unit"), Alr());
        Assert.Equal(index, catalog.GetAnimation("Unit")!.GetFrameIndex(ms));
    }

    [Theory]
    [InlineData(0, -1, "00000000", -1, 1000, false)] // scenery with several columns
    [InlineData(0, 0, "00008008", -1, 1000, true)] // non-team living unit
    [InlineData(0, 0, "00000000", -1, 1000, false)] // anadd alone is insufficient
    [InlineData(1, -1, "00000000", -1, 1000, true)] // team unit
    [InlineData(1, 0, "00008008", 2, 1000, false)] // tree stage safety
    [InlineData(1, 0, "00008008", -1, 1, false)]
    public void Only_living_unit_evidence_enables_alr(int palette, int add, string mask, int layerType, int cycle, bool expected)
    {
        using var catalog = Catalog(Row("Object", palette: palette, add: add, mask: mask, layerType: layerType, cycle: cycle), Alr(5, 1));
        Assert.Equal(expected, catalog.GetAnimation("Object") is not null);
        Assert.NotNull(catalog.GetSprite("Object"));
    }

    [Fact]
    public void Trees_layers_and_alrml_only_ground_cover_keep_the_alive_still_frame()
    {
        string definitions = Row("Tree", palette: 0, add: -1, mask: "0", layerType: 2, layer: 1, cycle: 1) + "\n" +
            Row("Cover", palette: 0, add: -1, mask: "0", alr: -1, layer: 1, cycle: 1);
        using var catalog = NativeSpriteCatalog.FromText(definitions, "0,trunk.alr\n1,crown.alr", "", _ => Alr(5, 1), _ => null);
        Assert.Null(catalog.GetAnimation("Tree")); Assert.Null(catalog.GetAnimation("Cover"));
        Assert.Equal(0xFFFF0000u, catalog.GetSprite("Tree")!.ArgbPixels[0]);
        Assert.Equal(0xFFFF0000u, catalog.GetSprite("Cover")!.ArgbPixels[0]);
    }

    [Theory]
    [InlineData(25, 3000, 120d)]
    [InlineData(50, 4500, 90d)]
    [InlineData(25, 5000, 200d)]
    [InlineData(50, 0, 90d)]
    public void Apt_selects_finished_intact_axis_three_and_caches_sequence(int frames, int cycle, double duration)
    {
        using var catalog = NativeSpriteCatalog.FromText(Row("House", alr: -1, apt: 0, cycle: cycle), "", "0,house.apt",
            _ => null, _ => Apt(frames));
        NativeSpriteAnimation sequence = catalog.GetAnimation("House")!;
        Assert.Equal(frames, sequence.Frames.Count); Assert.Equal(duration, sequence.FrameDurationMs);
        Assert.Same(catalog.GetSprite("House"), sequence.Frames[0]);
        Assert.Same(sequence, catalog.GetAnimation("House", 7, 190));
        Assert.Equal(0xFFFF0000u, sequence.Frames[0].ArgbPixels[0]);
        Assert.Equal(0xFF00FF00u, sequence.Frames[1].ArgbPixels[0]);
        Assert.Equal(sequence.Frames[0].AnchorX, sequence.Frames[1].AnchorX);
        Assert.Equal(sequence.Frames[0].AnchorY, sequence.Frames[1].AnchorY);
        Assert.Equal(1, sequence.GetFrameIndex(duration));
        Assert.Equal(0, sequence.GetFrameIndex(sequence.CycleDurationMs));
    }

    [Fact]
    public void Static_missing_corrupt_and_incomplete_assets_fall_back_to_existing_stills()
    {
        using var staticApt = NativeSpriteCatalog.FromText(Row("House", alr: -1, apt: 0), "", "0,house.apt", _ => null, _ => Apt(1));
        Assert.Null(staticApt.GetAnimation("House")); Assert.NotNull(staticApt.GetSprite("House"));
        using var incomplete = Catalog(Row("Unit"), Alr(24, 16, frameCount: 1));
        Assert.Null(incomplete.GetAnimation("Unit")); Assert.NotNull(incomplete.GetSprite("Unit"));
        using var corrupt = Catalog(Row("Unit"), [1, 2, 3]);
        Assert.Null(corrupt.GetAnimation("Unit")); Assert.Null(corrupt.GetAnimation("Missing"));
    }

    [Fact]
    public void Atlas_preserves_stills_and_rolls_back_whole_sequences_that_do_not_fit()
    {
        NativeSprite Sprite(string name) => new(30, 30, new uint[900], 15, 30, name);
        NativeSprite a = Sprite("a"), b = Sprite("b"), c = Sprite("c"), d = Sprite("d");
        var tooLarge = new NativeSpriteAnimation([a, c, d, Sprite("e")], 1000);
        var small = new NativeSpriteAnimation([b, c], 1000);
        NativeSpriteAtlas atlas = NativeSpriteAtlas.PackAnimations([a, b], [tooLarge, small], 64);
        Assert.True(atlas.TryGetUv(a, out _)); Assert.True(atlas.TryGetUv(b, out _));
        Assert.True(atlas.TryGetUv(c, out _)); Assert.False(atlas.TryGetUv(d, out _));
        Assert.Equal(3, atlas.Count); Assert.InRange(atlas.Width, 1, 64); Assert.InRange(atlas.Height, 1, 64);
    }

    private static NativeSpriteCatalog Catalog(string row, byte[] alr)
        => NativeSpriteCatalog.FromText(row, "0,unit.alr", "", _ => alr, _ => null);

    private static string Row(string name, int alr = 0, int apt = -1, int palette = 1, int add = 0,
        string mask = "00008008", int layerType = -1, int layer = -1, int cycle = 1000)
        => string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
        {
            0 => "0", 2 => cycle.ToString(), 5 => alr.ToString(), 6 => add.ToString(), 8 => layer.ToString(),
            10 => layerType.ToString(), 14 => apt.ToString(), 17 => palette.ToString(), 45 => mask, 52 => name, _ => "   0"
        }));

    private static byte[] Alr(int columns = 24, int rows = 16, int? frameCount = null)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        int count = frameCount ?? columns * rows * 2;
        foreach (uint word in new uint[] { 0x41524C41, 6, 0, (uint)count, 8, 0, (uint)columns, (uint)rows, 2,
            0, 0, 0, 0, 0, 0, 8, 8, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
        for (int i = 0; i < count; i++)
        {
            writer.Write(-1); writer.Write(28u); writer.Write((uint)(i % 2));
            writer.Write((uint)(2 | 1 << 11 | 3 << 22)); writer.Write(2u);
            foreach (uint color in new uint[] { 0, 0x0000FF, 0x00FF00, 0, 0xFF0000, 0x00FFFF }) writer.Write(color);
            byte index = (byte)(i % columns % 2 + 1);
            writer.Write(new byte[] { index, index, 0, 0 }); writer.Write(0u); writer.Write(2u);
        }
        return stream.ToArray();
    }

    private static byte[] Apt(int columns)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        uint[] header = new uint[28];
        int count = 5 * 2 * 5 * columns, finished = 4 * 2 * 5 * columns;
        header[0] = 0x54415041; header[1] = 3; header[2] = (uint)count; header[3] = 112; header[4] = 3;
        header[6] = 64; header[7] = 31; header[8] = 5; header[9] = 2; header[10] = 5; header[11] = (uint)columns;
        header[13] = 8; header[20] = 4; header[21] = 1; header[23] = 64; header[24] = 31; header[27] = 36;
        foreach (uint word in header) writer.Write(word);
        int position = 0;
        for (int y = 0; y < 31; y++) { writer.Write(position); position += y <= 15 ? (y + 1) * 4 : (31 - y) * 4; }
        for (int y = 0; y < 31; y++) writer.Write(y <= 15 ? (y + 1) * 4 : (31 - y) * 4);
        foreach (uint word in new uint[] { 32, 20, 30, 15, 0, 0, 0 }) writer.Write(word); // no opaque groups
        for (int i = 0; i < count; i++) { writer.Write(i >= finished && i < finished + columns ? (i - finished) % 2 : 2); writer.Write(1); }
        foreach (uint color in new uint[] { 0, 0x0000FF, 0x00FF00, 0xFF0000 }) writer.Write(color);
        for (int i = 0; i < 3; i++) { writer.Write(i * 12); writer.Write(30); writer.Write(0); }
        writer.Write(0x54414450u);
        for (byte i = 1; i <= 3; i++) { writer.Write(0x7FFFFFFEu); writer.Write(0u); writer.Write(new byte[] { i, i, i, i }); }
        return stream.ToArray();
    }
}
