using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeSpriteVisualStateTests
{
    [Fact]
    public void Alr_saved_linear_frame_includes_animation_and_direction_and_ignores_angle()
    {
        byte[] bytes = Alr();
        byte[] original = bytes.ToArray();
        using var catalog = AlrCatalog(bytes);
        // 2 animation banks, 3 directions, 4 steps: animation 1 / direction 1 / step 2 = 18.
        var state = State(18);
        NativeSprite sprite = catalog.GetSprite("Unit", team: 1, direction: -7, angleDegrees: 180, visualState: state)!;
        AssertPixels(sprite, 18, 1);
        Assert.Equal((3, 1, 3, 3), (sprite.Width, sprite.Height, sprite.AnchorX, sprite.AnchorY));
        Assert.Same(sprite, catalog.GetSprite(" unit ", 1, 0, float.NaN, State(18)));
        Assert.Equal(original, bytes);
    }

    [Fact]
    public void Alr_preview_starts_at_saved_step_and_wraps_only_within_saved_bank()
    {
        using var catalog = AlrCatalog(Alr());
        var state = State(18);
        NativeSpriteAnimation animation = catalog.GetAnimation("Unit", 1, 180, state)!;
        Assert.Equal(4, animation.Frames.Count);
        Assert.Equal(250d, animation.FrameDurationMs); // Existing preview estimate; no native tick claim.
        Assert.Same(catalog.GetSprite("Unit", 1, visualState: state), animation.Frames[0]);
        int[] expected = [18, 19, 16, 17];
        for (int i = 0; i < expected.Length; i++) AssertPixels(animation.Frames[i], expected[i], 1);
        Assert.Same(animation, catalog.GetAnimation("Unit", 1, 45, State(18)));
        Assert.NotSame(animation, catalog.GetAnimation("Unit", 0, visualState: state));
        Assert.NotSame(animation, catalog.GetAnimation("Unit", 1, visualState: State(19)));
        Assert.NotSame(animation, catalog.GetAnimation("Unit", 1));
    }

    [Fact]
    public void Full_state_and_palette_are_separate_cache_dimensions_even_for_same_visible_frame()
    {
        using var catalog = AlrCatalog(Alr());
        NativeSprite legacy = catalog.GetSprite("Unit", direction: 0)!;
        NativeSprite explicitZero = catalog.GetSprite("Unit", visualState: State(0))!;
        NativeSprite otherAxes = catalog.GetSprite("Unit", visualState: new(0, 1, 0, 0, 0))!;
        Assert.NotSame(legacy, explicitZero);
        Assert.NotSame(explicitZero, otherAxes);
        Assert.Equal(legacy.ArgbPixels, explicitZero.ArgbPixels);
        Assert.Equal(explicitZero.ArgbPixels, otherAxes.ArgbPixels);
        Assert.NotSame(explicitZero, catalog.GetSprite("Unit", team: 1, visualState: State(0)));
        Assert.NotSame(explicitZero, catalog.GetSprite("Unit", visualState: State(1)));
        Assert.NotSame(catalog.GetAnimation("Unit", visualState: State(0)),
            catalog.GetAnimation("Unit", visualState: new(0, 1, 0, 0, 0)));
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(short.MinValue)]
    [InlineData(24)]
    [InlineData(short.MaxValue)]
    public void Invalid_signed_alr_frames_return_null_without_legacy_fallback(short frame)
    {
        using var catalog = AlrCatalog(Alr());
        Assert.Null(catalog.GetSprite("Unit", visualState: State(frame)));
        Assert.Null(catalog.GetAnimation("Unit", visualState: State(frame)));
        AssertPixels(catalog.GetSprite("Unit", direction: 0)!, 0);
    }

    [Fact]
    public void Apt_native_axis_order_selects_asymmetric_pixels_and_seeded_axis_three_cycle()
    {
        byte[] bytes = Apt();
        byte[] original = bytes.ToArray();
        using var catalog = AptCatalog(bytes);
        // Layout [2,2,3,4], (((1*2)+1)*3+2)*4+2 = 46; swapping axes1/2 is invalid.
        var state = new NativeAnimationVisualState(-123, 1, 1, 2, 2);
        NativeSprite sprite = catalog.GetSprite("House", 99, -8, 123, state)!;
        AssertPixels(sprite, 46);
        Assert.Equal((3, 1, 0, 15), (sprite.Width, sprite.Height, sprite.AnchorX, sprite.AnchorY));
        NativeSpriteAnimation animation = catalog.GetAnimation("House", visualState: state)!;
        Assert.Same(sprite, animation.Frames[0]);
        Assert.Equal(250d, animation.FrameDurationMs);
        int[] expected = [46, 47, 44, 45];
        for (int i = 0; i < expected.Length; i++) AssertPixels(animation.Frames[i], expected[i]);
        Assert.Equal(original, bytes);
    }

    [Fact]
    public void Apt_cache_distinguishes_every_axis_and_full_state_from_legacy()
    {
        using var catalog = AptCatalog(Apt());
        NativeAnimationVisualState[] states =
        [new(0, 0, 0, 0, 0), new(0, 1, 0, 0, 0), new(0, 0, 1, 0, 0), new(0, 0, 0, 1, 0), new(0, 0, 0, 0, 1)];
        int[] indices = [0, 24, 12, 4, 1];
        for (int i = 0; i < states.Length; i++)
        {
            NativeSprite sprite = catalog.GetSprite("House", visualState: states[i])!;
            AssertPixels(sprite, indices[i]);
            Assert.Same(sprite, catalog.GetSprite("House", visualState: states[i]));
            Assert.Same(sprite, catalog.GetAnimation("House", visualState: states[i])!.Frames[0]);
        }
        var finished = new NativeAnimationVisualState(0, 1, 0, 0, 0);
        NativeSprite legacy = catalog.GetSprite("House")!;
        AssertPixels(legacy, 24);
        Assert.NotSame(legacy, catalog.GetSprite("House", visualState: finished));
        var otherAlr = new NativeAnimationVisualState(1, 1, 0, 0, 0);
        Assert.NotSame(catalog.GetSprite("House", visualState: finished), catalog.GetSprite("House", visualState: otherAlr));
        Assert.NotSame(catalog.GetAnimation("House", visualState: finished), catalog.GetAnimation("House", visualState: otherAlr));
        Assert.Same(catalog.GetAnimation("House"), catalog.GetAnimation("House", 9, 230));
        AssertPixels(catalog.GetAnimation("House")!.Frames[1], 25);
    }

    [Theory]
    [InlineData(-1, 0, 0, 0)]
    [InlineData(0, -1, 0, 0)]
    [InlineData(0, 0, -1, 0)]
    [InlineData(0, 0, 0, -1)]
    [InlineData(2, 0, 0, 0)]
    [InlineData(0, 2, 0, 0)]
    [InlineData(0, 0, 3, 0)]
    [InlineData(0, 0, 0, 4)] // Would otherwise alias the next axis2 bank.
    [InlineData(short.MinValue, 0, 0, 0)]
    [InlineData(0, 0, 0, short.MaxValue)]
    public void Invalid_apt_axes_never_clamp_or_alias_a_valid_frame(short a, short b, short c, short d)
    {
        using var catalog = AptCatalog(Apt());
        var state = new NativeAnimationVisualState(0, a, b, c, d);
        Assert.Null(catalog.GetSprite("House", visualState: state));
        Assert.Null(catalog.GetAnimation("House", visualState: state));
        AssertPixels(catalog.GetSprite("House")!, 24);
    }

    [Fact]
    public void Apt_large_layout_overflow_zero_axis_and_missing_frame_fail_safely()
    {
        Assert.False(NativeSpriteCatalog.TryAptStateFrame(
            [uint.MaxValue, uint.MaxValue, uint.MaxValue, uint.MaxValue], int.MaxValue,
            new(0, short.MaxValue, short.MaxValue, short.MaxValue, short.MaxValue), out _));
        Assert.False(NativeSpriteCatalog.TryAptStateFrame([2, 0, 3, 4], 48, new(0, 0, 0, 0, 0), out _));
        Assert.False(NativeSpriteCatalog.TryAptStateFrame([2, 2, 3], 48, new(0, 0, 0, 0, 0), out _));
        Assert.False(NativeSpriteCatalog.TryAptStateFrame([2, 2, 3, 4], 47, new(0, 1, 1, 2, 3), out _));
        byte[] bytes = Apt();
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(8 * 4), uint.MaxValue);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(10 * 4), uint.MaxValue);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(11 * 4), uint.MaxValue);
        using var catalog = AptCatalog(bytes);
        var state = new NativeAnimationVisualState(0, short.MaxValue, 1, short.MaxValue, short.MaxValue);
        Assert.Null(catalog.GetSprite("House", visualState: state));
        Assert.Null(catalog.GetAnimation("House", visualState: state));
    }

    [Fact]
    public void Incomplete_current_bank_keeps_explicit_still_but_returns_no_sequence()
    {
        using var alr = AlrCatalog(Alr(frameCount: 23));
        AssertPixels(alr.GetSprite("Unit", visualState: State(22))!, 22);
        Assert.Null(alr.GetAnimation("Unit", visualState: State(22)));
        using var apt = AptCatalog(Apt(frameCount: 47));
        var state = new NativeAnimationVisualState(0, 1, 1, 2, 2);
        AssertPixels(apt.GetSprite("House", visualState: state)!, 46);
        Assert.Null(apt.GetAnimation("House", visualState: state));
    }

    [Fact]
    public void Layering_preserves_overlay_palette_anchor_and_source_bytes()
    {
        byte[] bottom = Alr(), top = Alr(overlay: true);
        byte[] bottomBefore = bottom.ToArray(), topBefore = top.ToArray();
        using var catalog = NativeSpriteCatalog.FromText(Row("Tree", layer: 1) + "\n" + Row("Cover", alr: -1, layer: 1),
            "0,base.alr\n1,top.alr", "", name => name == "base.alr" ? bottom : top, _ => null);
        NativeSprite sprite = catalog.GetSprite("Tree", 1, visualState: State(18))!;
        Assert.Equal("base.alr+top.alr", sprite.AssetName);
        Assert.Equal((3, 1, 3, 3), (sprite.Width, sprite.Height, sprite.AnchorX, sprite.AnchorY));
        Assert.Equal(new uint[] { Color(19, 1), Color(2, 1), Color(1, 1) }, sprite.ArgbPixels);
        NativeSprite cover = catalog.GetSprite("Cover", 1, visualState: State(18))!;
        Assert.Equal(new uint[] { Color(2, 1) }, cover.ArgbPixels);
        Assert.Equal((1, 1, 2, 3), (cover.Width, cover.Height, cover.AnchorX, cover.AnchorY));
        Assert.Equal(bottomBefore, bottom); Assert.Equal(topBefore, top);
        AssertPixels(catalog.GetSprite("Tree", 1, direction: 0)!, 0, 1, layered: true);
    }

    [Fact]
    public void Explicit_overlay_frame_out_of_range_does_not_substitute_layer_zero()
    {
        using var catalog = NativeSpriteCatalog.FromText(Row("Tree", layer: 1), "0,base.alr\n1,top.alr", "",
            name => name == "base.alr" ? Alr() : Alr(frameCount: 1, overlay: true), _ => null);
        Assert.Null(catalog.GetSprite("Tree", visualState: State(18)));
        Assert.Null(catalog.GetAnimation("Tree", visualState: State(18)));
        Assert.NotNull(catalog.GetSprite("Tree", direction: 0));
    }

    [Fact]
    public void Legacy_alr_still_and_animation_keep_angle_direction_and_animation_zero()
    {
        using var catalog = AlrCatalog(Alr());
        AssertPixels(catalog.GetSprite("Unit")!, 8); // DefaultDirection 14 modulo 3 = 2.
        AssertPixels(catalog.GetSprite("Unit", angleDegrees: 180)!, 0);
        Assert.Same(catalog.GetSprite("Unit", direction: 0), catalog.GetSprite("Unit", direction: -3));
        NativeSpriteAnimation animation = catalog.GetAnimation("Unit", angleDegrees: 180)!;
        for (int i = 0; i < 4; i++) AssertPixels(animation.Frames[i], i);
    }

    [Fact]
    public void Saved_state_keeps_static_alr_animation_gate_and_missing_overlay_fallback()
    {
        string[] columns = Row("Tree", layer: 1).Split(',');
        columns[10] = "2"; // Existing stage-only tree gate.
        using var catalog = NativeSpriteCatalog.FromText(string.Join(",", columns), "0,base.alr\n1,missing.alr", "",
            name => name == "base.alr" ? Alr() : null, _ => null);
        AssertPixels(catalog.GetSprite("Tree", visualState: State(18))!, 18);
        Assert.Null(catalog.GetAnimation("Tree", visualState: State(18)));
        Assert.Null(catalog.GetAnimation("Tree"));
    }

    private static NativeAnimationVisualState State(short frame) => new(frame, 0, 0, 0, 0);
    private static uint Color(int index, int variant = 0) => 0xFF000000u | (uint)index << 16 | (uint)variant << 8;
    private static void AssertPixels(NativeSprite sprite, int frame, int variant = 0, bool layered = false)
        => Assert.Equal(new uint[] { Color(frame + 1, variant), layered ? Color(2, variant) : 0, Color(1, variant) }, sprite.ArgbPixels);
    private static NativeSpriteCatalog AlrCatalog(byte[] bytes)
        => NativeSpriteCatalog.FromText(Row("Unit"), "0,unit.alr", "", _ => bytes, _ => null);
    private static NativeSpriteCatalog AptCatalog(byte[] bytes)
        => NativeSpriteCatalog.FromText(Row("House", alr: -1, apt: 0), "", "0,house.apt", _ => null, _ => bytes);
    private static string Row(string name, int alr = 0, int apt = -1, int layer = -1)
        => string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
        {
            2 => "1000", 5 => alr.ToString(), 8 => layer.ToString(), 10 => "-1", 14 => apt.ToString(),
            17 => "1", 52 => name, _ => "   0"
        }));

    // Valid v6 indexed ALRA records, two palettes, asymmetric three-pixel scanlines.
    private static byte[] Alr(int frameCount = 24, bool overlay = false)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        foreach (uint word in new uint[] { 0x41524C41, 6, 0, (uint)frameCount, 8, 0, 4, 3, 2,
            0, 0, 0, 0, 0, 0, 8, 8, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
        const int colors = 49;
        for (int frame = 0; frame < frameCount; frame++)
        {
            writer.Write(-1); writer.Write(colors * 2 * 4 + 4); writer.Write(1u | 1u << 16);
            writer.Write(3u | 1u << 11 | (uint)colors << 22); writer.Write(3u);
            for (int variant = 0; variant < 2; variant++)
                for (int index = 0; index < colors; index++) writer.Write((uint)index | (uint)variant << 8);
            writer.Write(overlay ? new byte[] { 0, 2, 0, 0 } : new byte[] { (byte)(frame + 1), 0, 1, 0 });
            writer.Write(0u); writer.Write(3u);
        }
        return stream.ToArray();
    }

    // Valid APAT v3: every frame owns a distinct uncompressed diamond patch's top row.
    private static byte[] Apt(int frameCount = 48)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        uint[] header = new uint[28];
        header[0] = 0x54415041; header[1] = 3; header[2] = (uint)frameCount; header[3] = 112; header[4] = (uint)frameCount;
        header[6] = 64; header[7] = 31; header[8] = 2; header[9] = 2; header[10] = 3; header[11] = 4;
        header[13] = 8; header[20] = 49; header[21] = 1; header[23] = 64; header[24] = 31; header[27] = (uint)frameCount * 12;
        foreach (uint word in header) writer.Write(word);
        int position = 0;
        for (int y = 0; y < 31; y++) { writer.Write(position); position += y <= 15 ? (y + 1) * 4 : (31 - y) * 4; }
        for (int y = 0; y < 31; y++) writer.Write(y <= 15 ? (y + 1) * 4 : (31 - y) * 4);
        foreach (uint word in new uint[] { 32, 20, 30, 15, 0, 0, 0 }) writer.Write(word);
        for (int frame = 0; frame < frameCount; frame++) { writer.Write(frame); writer.Write(1); }
        for (uint index = 0; index < 49; index++) writer.Write(index);
        for (int frame = 0; frame < frameCount; frame++) { writer.Write(frame * 12); writer.Write(30); writer.Write(0); }
        writer.Write(0x54414450u);
        for (int frame = 0; frame < frameCount; frame++)
        {
            writer.Write(0x7FFFFFFEu); writer.Write(0u); writer.Write(new byte[] { (byte)(frame + 1), 0, 1, 0 });
        }
        return stream.ToArray();
    }
}
