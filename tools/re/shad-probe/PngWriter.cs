using System.Buffers.Binary;
using System.IO.Compression;

/// <summary>只用標準庫匯出 RGBA PNG；不依賴 System.Drawing 或外部圖像套件。</summary>
internal static class PngWriter
{
    public static void Write(string path, int width, int height, ReadOnlySpan<uint> pixels)
    {
        using var file = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
        file.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        byte[] header = new byte[13];
        BinaryPrimitives.WriteInt32BigEndian(header, width);
        BinaryPrimitives.WriteInt32BigEndian(header.AsSpan(4), height);
        header[8] = 8; header[9] = 6;
        Chunk(file, "IHDR", header);
        using var packed = new MemoryStream();
        using (var zlib = new ZLibStream(packed, CompressionLevel.Optimal, leaveOpen: true))
        {
            var row = new byte[1 + width * 4];
            for (int y = 0; y < height; y++)
            {
                for (int x = 0; x < width; x++)
                {
                    uint color = pixels[y * width + x]; int offset = 1 + x * 4;
                    row[offset] = (byte)(color >> 16); row[offset + 1] = (byte)(color >> 8);
                    row[offset + 2] = (byte)color; row[offset + 3] = (byte)(color >> 24);
                }
                zlib.Write(row);
            }
        }
        Chunk(file, "IDAT", packed.ToArray());
        Chunk(file, "IEND", []);
    }

    private static void Chunk(Stream file, string type, byte[] data)
    {
        byte[] name = System.Text.Encoding.ASCII.GetBytes(type), word = new byte[4];
        BinaryPrimitives.WriteInt32BigEndian(word, data.Length); file.Write(word); file.Write(name); file.Write(data);
        uint crc = uint.MaxValue;
        foreach (byte value in name.Concat(data))
        {
            crc ^= value;
            for (int bit = 0; bit < 8; bit++) crc = (crc >> 1) ^ ((crc & 1) != 0 ? 0xEDB88320u : 0);
        }
        BinaryPrimitives.WriteUInt32BigEndian(word, ~crc); file.Write(word);
    }
}