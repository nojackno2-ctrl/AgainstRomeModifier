
namespace AgainstRomeModifier.Core.Services;

/// <summary>
/// 解碼後的點陣影像：<see cref="Bgra"/> 為緊密排列（無 stride padding）、由上而下的
/// BGRA 像素緩衝，長度必為 Width*Height*4。UI 端再依需要包成 <c>System.Drawing.Bitmap</c>。
/// </summary>
public sealed record TgaImage(int Width, int Height, byte[] Bgra);

/// <summary>
/// 《Against Rome》UI 圖示 / 存檔預覽用的 TGA 解碼器。支援 8 位元索引色（附 24 位元調色盤）
/// 與 24/32 位元真彩色兩種格式。此類別不依賴 System.Drawing，讓解析邏輯集中於 Core 並可被測試；
/// 原本 Modifier 與 SaveManager 各自維護一份完全相同的解碼副本，現統一由此提供。
/// </summary>
public static class TgaDecoder
{
    /// <summary>解碼 TGA 位元組；格式不支援或資料不足時回傳 null。輸出一律為由上而下的 BGRA。</summary>
    public static TgaImage? Decode(byte[] tgaBytes)
    {
        if (tgaBytes == null || tgaBytes.Length < 18) return null;
        int idLength = tgaBytes[0];
        int imageType = tgaBytes[2];
        int width = BitConverter.ToUInt16(tgaBytes, 12);
        int height = BitConverter.ToUInt16(tgaBytes, 14);
        int pixelDepth = tgaBytes[16];
        int descriptor = tgaBytes[17];

        if (width <= 0 || height <= 0) return null;
        // 標頭的寬高各可到 65535，(long)w*h*4 會溢位 int，導致 new byte[負數]。
        // 損毀或惡意的 savepic.tga 不該讓解碼器以奇怪的例外收場。
        if ((long)width * height * 4 > int.MaxValue) return null;
        bool topToBottom = (descriptor & 0x20) != 0;
        int rowBytes = width * 4;
        byte[] bgra = new byte[rowBytes * height];

        if (imageType == 1)
        {
            int colorMapType = tgaBytes[1];
            if (colorMapType != 1 || pixelDepth != 8) return null;
            int colorMapLength = BitConverter.ToUInt16(tgaBytes, 5);
            int colorMapEntrySize = tgaBytes[7];
            if (colorMapEntrySize != 24) return null;
            int colorMapOffset = 18 + idLength;
            int pixelDataOffset = colorMapOffset + colorMapLength * 3;

            if (pixelDataOffset + width * height > tgaBytes.Length) return null;

            // 調色盤：以純黑（0,0,0）代表透明，其餘為不透明；與遊戲的索引色 UI sprite 一致。
            byte[] palette = new byte[colorMapLength * 4];
            for (int i = 0; i < colorMapLength; i++)
            {
                int entryOffset = colorMapOffset + i * 3;
                if (entryOffset + 2 >= tgaBytes.Length) break;
                byte b = tgaBytes[entryOffset];
                byte g = tgaBytes[entryOffset + 1];
                byte r = tgaBytes[entryOffset + 2];
                bool transparent = r == 0 && g == 0 && b == 0;
                int p = i * 4;
                palette[p] = b;
                palette[p + 1] = g;
                palette[p + 2] = r;
                palette[p + 3] = (byte)(transparent ? 0 : 255);
            }

            for (int y = 0; y < height; y++)
            {
                int targetY = topToBottom ? y : (height - 1 - y);
                int targetOffset = targetY * rowBytes;
                int rowDataOffset = pixelDataOffset + y * width;
                for (int x = 0; x < width; x++)
                {
                    int pixelOffset = rowDataOffset + x;
                    if (pixelOffset >= tgaBytes.Length) break;
                    int index = tgaBytes[pixelOffset];
                    int pixel = targetOffset + x * 4;
                    if (index < colorMapLength)
                    {
                        bgra[pixel] = palette[index * 4];
                        bgra[pixel + 1] = palette[index * 4 + 1];
                        bgra[pixel + 2] = palette[index * 4 + 2];
                        bgra[pixel + 3] = palette[index * 4 + 3];
                    }
                    else
                    {
                        // 索引超出調色盤：對齊原實作的 Color.Transparent（B=G=R=255, A=0）。
                        bgra[pixel] = 255;
                        bgra[pixel + 1] = 255;
                        bgra[pixel + 2] = 255;
                        bgra[pixel + 3] = 0;
                    }
                }
            }

            return new TgaImage(width, height, bgra);
        }

        if (imageType == 2)
        {
            if (pixelDepth != 24 && pixelDepth != 32) return null;
            int pixelDataOffset = 18 + idLength;
            int bytesPerPixel = pixelDepth / 8;

            if (pixelDataOffset + width * height * bytesPerPixel > tgaBytes.Length) return null;

            for (int y = 0; y < height; y++)
            {
                int targetY = topToBottom ? y : (height - 1 - y);
                int targetOffset = targetY * rowBytes;
                int rowDataOffset = pixelDataOffset + y * width * bytesPerPixel;
                for (int x = 0; x < width; x++)
                {
                    int pixelOffset = rowDataOffset + x * bytesPerPixel;
                    if (pixelOffset + 2 >= tgaBytes.Length) break;
                    int pixel = targetOffset + x * 4;
                    bgra[pixel] = tgaBytes[pixelOffset];
                    bgra[pixel + 1] = tgaBytes[pixelOffset + 1];
                    bgra[pixel + 2] = tgaBytes[pixelOffset + 2];
                    bgra[pixel + 3] = (bytesPerPixel == 4 && pixelOffset + 3 < tgaBytes.Length)
                        ? tgaBytes[pixelOffset + 3]
                        : (byte)255;
                }
            }

            return new TgaImage(width, height, bgra);
        }

        return null;
    }
}
