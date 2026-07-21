using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 鎖定 TgaDecoder 的解碼行為（原本內嵌在 UI 且無測試）：真彩色的垂直翻轉、
/// 索引色的純黑透明特例、無效輸入回傳 null。輸出一律為由上而下的緊密 BGRA。
/// </summary>
public sealed class TgaDecoderTests
{
    [Fact]
    public void Truecolor_24bpp_bottom_to_top_is_flipped_to_top_down()
    {
        // 2x2、24bpp、descriptor=0（bottom-to-top）。檔案第一列是影像最下列。
        byte[] header =
        {
            0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            2, 0, // width = 2
            2, 0, // height = 2
            24,   // pixel depth
            0     // descriptor（bottom-to-top）
        };
        byte[] pixels =
        {
            10, 20, 30, 40, 50, 60,      // 檔案列 0 = 影像最下列
            70, 80, 90, 100, 110, 120,   // 檔案列 1 = 影像最上列
        };
        byte[] tga = Concat(header, pixels);

        TgaImage? image = TgaDecoder.Decode(tga);

        Assert.NotNull(image);
        Assert.Equal(2, image!.Width);
        Assert.Equal(2, image.Height);
        // 上列（輸出 row 0）應為檔案列 1，BGRA。
        Assert.Equal(new byte[] { 70, 80, 90, 255, 100, 110, 120, 255 }, image.Bgra[0..8]);
        // 下列（輸出 row 1）應為檔案列 0。
        Assert.Equal(new byte[] { 10, 20, 30, 255, 40, 50, 60, 255 }, image.Bgra[8..16]);
    }

    [Fact]
    public void Indexed_8bpp_maps_pure_black_palette_entry_to_transparent()
    {
        // 2x1、8bpp 索引色，調色盤 2 筆：entry0 純黑（透明）、entry1 不透明。
        byte[] header =
        {
            0, 1, 1, 0, 0,
            2, 0, // color map length = 2
            24,   // color map entry size
            0, 0, 0, 0,
            2, 0, // width = 2
            1, 0, // height = 1
            8,    // pixel depth
            0     // descriptor
        };
        byte[] palette =
        {
            0, 0, 0,      // entry0：純黑 → 透明
            11, 22, 33,   // entry1：BGR
        };
        byte[] indices = { 1, 0 }; // pixel0 = entry1, pixel1 = entry0
        byte[] tga = Concat(Concat(header, palette), indices);

        TgaImage? image = TgaDecoder.Decode(tga);

        Assert.NotNull(image);
        Assert.Equal(2, image!.Width);
        Assert.Equal(1, image.Height);
        Assert.Equal(new byte[] { 11, 22, 33, 255 }, image.Bgra[0..4]); // 不透明
        Assert.Equal(new byte[] { 0, 0, 0, 0 }, image.Bgra[4..8]);      // 純黑 → 全透明
    }

    [Theory]
    [InlineData(0)]   // 空陣列
    [InlineData(10)]  // 少於 18 位元組標頭
    public void Too_short_input_returns_null(int length)
    {
        Assert.Null(TgaDecoder.Decode(new byte[length]));
    }

    [Fact]
    public void Unsupported_image_type_returns_null()
    {
        byte[] header = new byte[18];
        header[2] = 9; // 非 1/2 的影像類型
        header[12] = 2; header[14] = 2; header[16] = 24;
        Assert.Null(TgaDecoder.Decode(header));
    }

    private static byte[] Concat(byte[] a, byte[] b)
    {
        byte[] result = new byte[a.Length + b.Length];
        System.Array.Copy(a, 0, result, 0, a.Length);
        System.Array.Copy(b, 0, result, a.Length, b.Length);
        return result;
    }
}
