using System.Buffers.Binary;
using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class MapLightingTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Bitmap_decodes_bgr_padding_and_row_orientation_to_owned_rgb(bool topDown)
    {
        byte[] bytes = Bitmap(3, 2, topDown, (x, y) => new Vector3(x + 10, y + 30, x + y + 50));
        var image = MapLightingBitmap.Parse(bytes);
        Array.Fill(bytes, (byte)0);
        Assert.Equal(3, image.Width); Assert.Equal(2, image.Height);
        Assert.Equal(new Vector3(12, 31, 53), image.ColorAt(2, 1));
        Assert.Equal(new Vector3(10, 30, 50), image.ColorAt(0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => image.ColorAt(-1, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => image.ColorAt(0, 2));
    }

    [Fact]
    public void Daynight_preserves_unused_rows_and_uses_native_quantized_interpolation()
    {
        var table = MapLightingDayNight.Parse(Bitmap(24, 6, false,
            (x, y) => y == 0 ? new Vector3(x * 10, 100 + x, 200 - x) : new Vector3(5, y, 9)));
        Assert.Equal(new Vector3(5, 4, 9), table.RawColor(8, 4));
        Assert.Equal(new Vector3(20, 102, 198) / 256, table.AmbientAt(2, 0));
        Assert.Equal(new Vector3(25, 102, 197) / 256, table.AmbientAt(2, 30));
        // 權重分開 floor，minute=1 時權重和 65535，整數色道有額外量化。
        Assert.Equal(new Vector3(20, 102, 197) / 256, table.AmbientAt(2, 1));
        Assert.Equal(new Vector3(115, 111, 188) / 256, table.AmbientAt(23, 30));
        Assert.Throws<ArgumentOutOfRangeException>(() => table.AmbientAt(24, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => table.AmbientAt(0, 60));
        Assert.Throws<NotSupportedException>(() => MapLightingDayNight.Parse(Bitmap(3, 2, false, (_, _) => Vector3.One)));
    }

    [Theory]
    [InlineData(2, 99)]
    [InlineData(10, 53)]
    [InlineData(18, 0)]
    [InlineData(22, int.MinValue)]
    [InlineData(34, 99)]
    public void Bitmap_rejects_bad_extents(int offset, int value)
    {
        byte[] bytes = Bitmap(3, 2, false, (_, _) => Vector3.One);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
        Assert.Throws<InvalidDataException>(() => MapLightingBitmap.Parse(bytes));
    }

    [Fact]
    public void Bitmap_rejects_truncated_payload_even_with_consistent_file_size()
    {
        byte[] valid = Bitmap(3, 2, false, (_, _) => Vector3.One);
        for (int length = 0; length < valid.Length; length++)
        {
            byte[] prefix = valid[..length];
            if (length >= 6) BinaryPrimitives.WriteInt32LittleEndian(prefix.AsSpan(2), length);
            Assert.Throws<InvalidDataException>(() => MapLightingBitmap.Parse(prefix));
        }
    }

    [Fact]
    public void Bitmap_accepts_zero_image_size_and_trailer_but_rejects_other_depths()
    {
        byte[] bytes = Bitmap(3, 2, false, (_, _) => Vector3.One);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(34), 0);
        Assert.Equal(Vector3.One, MapLightingBitmap.Parse(bytes).ColorAt(0, 0));
        byte[] trailer = new byte[bytes.Length + 2]; bytes.CopyTo(trailer, 0);
        BinaryPrimitives.WriteInt32LittleEndian(trailer.AsSpan(2), trailer.Length);
        Assert.Equal(Vector3.One, MapLightingBitmap.Parse(trailer).ColorAt(0, 0));
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(28), 8);
        Assert.Throws<NotSupportedException>(() => MapLightingBitmap.Parse(bytes));
    }

    [Fact]
    public void Shadow_layout_is_slice_then_z_then_lsb_x_and_preserves_cache_metadata()
    {
        byte[] bytes = Shadows();
        bytes[32 + 17 * 8192 + 19 * 32 + 1] = 0x81;
        var map = MapLightingShadowMap.ParseDecoded(bytes);
        Array.Fill(bytes, (byte)0);
        Assert.True(map.IsOccluded(17, 8, 19)); Assert.True(map.IsOccluded(17, 15, 19));
        Assert.False(map.IsOccluded(17, 9, 19)); Assert.False(map.IsOccluded(18, 8, 19));
        Assert.Equal(16000u, map.Header[5]); Assert.Equal(5000u, map.Header[6]);
        Assert.Throws<ArgumentOutOfRangeException>(() => map.IsOccluded(128, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => map.IsOccluded(0, 256, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => map.IsOccluded(0, 0, -1));
    }

    [Fact]
    public void Shadow_temporal_interpolation_wraps_midnight_and_has_native_floor()
    {
        byte[] bytes = Shadows(); bytes[32] = 1;
        var map = MapLightingShadowMap.ParseDecoded(bytes);
        Assert.Equal(255, map.OcclusionAt(0, 0, 0));
        Assert.Equal(128, map.OcclusionAt(337, 0, 0));
        Assert.Equal(0, map.OcclusionAt(675, 0, 0));
        Assert.Equal(254, map.OcclusionAt(86399, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => map.OcclusionAt(86400, 0, 0));
    }

    [Fact]
    public void Shadow_spatial_sampling_preserves_65535_weights_and_native_darkening()
    {
        byte[] bytes = Shadows(); Array.Fill(bytes, (byte)255, 32, bytes.Length - 32);
        var map = MapLightingShadowMap.ParseDecoded(bytes);
        Assert.Equal(254, map.SampleOcclusion(0, 128, 192));
        Assert.Equal(254, map.SampleOcclusion(0, 32767, -1));
        Assert.Equal(1f, map.ShadowFactor(0, 128, 192, 0));
        Assert.Equal(2f / 256, map.ShadowFactor(0, 128, 192, 1));
        Assert.Throws<ArgumentOutOfRangeException>(() => map.ShadowFactor(0, 0, 0, float.NaN));
        Assert.Throws<ArgumentOutOfRangeException>(() => map.SampleOcclusion(0, int.MaxValue, 0));
        bytes = Shadows(); bytes[32] = 1; bytes[32 + 32] = 1;
        map = MapLightingShadowMap.ParseDecoded(bytes);
        Assert.Equal(127, map.SampleOcclusion(0, 32, 0));
    }

    [Fact]
    public void Shadow_rejects_extent_and_unsupported_dimensions()
    {
        byte[] bytes = Shadows();
        Assert.Throws<InvalidDataException>(() => MapLightingShadowMap.ParseDecoded(bytes.AsSpan(0, bytes.Length - 1)));
        Assert.Throws<InvalidDataException>(() => MapLightingShadowMap.ParseDecoded(new byte[bytes.Length + 1]));
        BinaryPrimitives.WriteInt32LittleEndian(bytes, 257);
        Assert.Throws<NotSupportedException>(() => MapLightingShadowMap.ParseDecoded(bytes));
    }

    [Fact]
    public void Local_light_uses_per_channel_max_and_squared_distance_not_addition()
    {
        var ambient = new Vector3(.2f, .7f, .6f);
        Assert.Equal(new Vector3(.75f, .7f, .6f), MapLightingModel.ApplyLocalLight(ambient, new Vector3(1, .5f, 0), 25, 100));
        Assert.Equal(ambient, MapLightingModel.ApplyLocalLight(ambient, Vector3.One, 100, 100));
        Assert.Equal(Vector3.One, MapLightingModel.ApplyLocalLight(Vector3.Zero, new Vector3(2), 0, 100));
        Assert.Throws<ArgumentOutOfRangeException>(() => MapLightingModel.ApplyLocalLight(ambient, Vector3.One, -1, 100));
        Assert.Throws<ArgumentOutOfRangeException>(() => MapLightingModel.ApplyLocalLight(ambient, Vector3.One, 0, 0));
    }

    [Fact]
    public void Sprite_gain_has_shadow_floor_and_squared_extra_byte_factor()
    {
        float scale = BitConverter.Int32BitsToSingle(0x3B7FFF34);
        var ambient = new Vector3(.2f, .7f, .6f);
        Assert.Equal(ambient * (255 * scale * 255 * scale), MapLightingModel.SpriteGain(ambient, 1, 255));
        Assert.Equal(ambient * (.293f * 128 * scale * 128 * scale), MapLightingModel.SpriteGain(ambient, 0, 128));
        Assert.Equal(Vector3.Zero, MapLightingModel.SpriteGain(ambient, 1, 0));
    }

    private static byte[] Shadows()
    {
        byte[] bytes = new byte[MapLightingShadowMap.DecodedLength];
        int[] header = [256, 256, 128, 6, 0, 16000, 5000, 12345];
        for (int i = 0; i < header.Length; i++) BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(i * 4), header[i]);
        return bytes;
    }

    private static byte[] Bitmap(int width, int height, bool topDown, Func<int, int, Vector3> pixel)
    {
        int stride = (width * 3 + 3) & ~3; byte[] bytes = new byte[54 + stride * height];
        bytes[0] = (byte)'B'; bytes[1] = (byte)'M';
        foreach (var (offset, value) in new[] { (2, bytes.Length), (10, 54), (14, 40), (18, width), (22, topDown ? -height : height), (34, stride * height) })
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(26), 1);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(28), 24);
        for (int y = 0; y < height; y++)
            for (int x = 0; x < width; x++)
            {
                Vector3 c = pixel(x, y); int i = 54 + (topDown ? y : height - 1 - y) * stride + x * 3;
                bytes[i] = (byte)c.Z; bytes[i + 1] = (byte)c.Y; bytes[i + 2] = (byte)c.X;
            }
        return bytes;
    }
}
