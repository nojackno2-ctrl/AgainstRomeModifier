using AgainstRomeModifier;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeMapEditor;

/// <summary>地圖灰階圖層（boden／emboss／collision.bmp）的讀寫與高度相依快取的失效處理。</summary>
internal static class TerrainLayerFiles
{
    /// <summary>
    /// 以「全部高度總和」為標頭鍵的快取。遊戲在缺檔或鍵不符時會重算並寫回，
    /// 但總和相同的編輯（例如一處升高、另一處等量降低）會誤用舊快取，因此高度變更時一律刪除。
    /// </summary>
    public static readonly string[] HeightDependentCaches = ["skydens.dat", "visible.dat", "cliprect.dat", "shadows.dat"];

    /// <summary>讀取 BMP 的完整像素（ARGB，影像由上而下）與綠通道。</summary>
    public static TerrainLayer? Read(string path)
    {
        if (!File.Exists(path)) return null;
        using var bitmap = new Bitmap(path);
        int[] argb = BitmapPixels.Read(bitmap);
        var green = new byte[argb.Length];
        for (int index = 0; index < argb.Length; index++) green[index] = (byte)(argb[index] >> 8);
        return new TerrainLayer(bitmap.Width, bitmap.Height, argb, green);
    }

    /// <summary>
    /// 只改寫數值有變的像素（寫成 R=G=B 灰階，遊戲讀綠通道、collision 讀三通道平均），
    /// 其餘像素逐位保留原檔的三通道值，輸出與原版相同的 24-bit bottom-up BMP。
    /// </summary>
    public static byte[] EncodeWithGreen(TerrainLayer original, IReadOnlyList<byte> values)
    {
        if (values.Count != original.Width * original.Height) throw new ArgumentException("圖層數值數量與 BMP 尺寸不符。", nameof(values));
        int width = original.Width, height = original.Height, stride = (width * 3 + 3) & ~3;
        int imageSize = stride * height, fileSize = 54 + imageSize;
        var bytes = new byte[fileSize];
        bytes[0] = (byte)'B'; bytes[1] = (byte)'M';
        BitConverter.TryWriteBytes(bytes.AsSpan(2), fileSize);
        BitConverter.TryWriteBytes(bytes.AsSpan(10), 54);
        BitConverter.TryWriteBytes(bytes.AsSpan(14), 40);
        BitConverter.TryWriteBytes(bytes.AsSpan(18), width);
        BitConverter.TryWriteBytes(bytes.AsSpan(22), height);
        BitConverter.TryWriteBytes(bytes.AsSpan(26), (short)1);
        BitConverter.TryWriteBytes(bytes.AsSpan(28), (short)24);
        BitConverter.TryWriteBytes(bytes.AsSpan(34), imageSize);
        for (int y = 0; y < height; y++)
        {
            int row = 54 + (height - 1 - y) * stride; // BMP 由下而上儲存。
            for (int x = 0; x < width; x++)
            {
                int index = y * width + x, pixel = original.Argb[index];
                byte value = values[index];
                int offset = row + x * 3;
                if (value == original.Green[index])
                {
                    bytes[offset] = (byte)pixel; bytes[offset + 1] = (byte)(pixel >> 8); bytes[offset + 2] = (byte)(pixel >> 16);
                }
                else bytes[offset] = bytes[offset + 1] = bytes[offset + 2] = value;
            }
        }
        return bytes;
    }

    /// <summary>在交易內寫入圖層；呼叫端負責 Commit。</summary>
    public static void Write(string path, TerrainLayer original, IReadOnlyList<byte> values, FileRollbackScope rollback)
        => SafeFileWriter.WriteAllBytes(path, EncodeWithGreen(original, values), rollback);

    /// <summary>刪除高度相依快取（先納入回滾交易，失敗時會還原）。回傳實際刪除的檔名。</summary>
    public static IReadOnlyList<string> InvalidateHeightCaches(string mapDirectory, FileRollbackScope rollback)
    {
        var removed = new List<string>();
        foreach (string name in HeightDependentCaches)
        {
            string path = Path.Combine(mapDirectory, name);
            if (!File.Exists(path)) continue;
            rollback.TrackFile(path);
            File.SetAttributes(path, FileAttributes.Normal);
            File.Delete(path);
            removed.Add(name);
        }
        return removed;
    }
}

internal sealed record TerrainLayer(int Width, int Height, int[] Argb, byte[] Green);
