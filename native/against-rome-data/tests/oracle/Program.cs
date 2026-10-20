using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length < 1)
{
    Console.WriteLine("Usage: AlrOracle <cases_directory>");
    return 1;
}

string casesDir = Path.GetFullPath(args[0]);
if (!Directory.Exists(casesDir))
{
    Console.Error.WriteLine($"Directory not found: {casesDir}");
    return 1;
}

var inputFiles = Directory.GetFiles(casesDir, "*.in").OrderBy(f => f, StringComparer.Ordinal).ToArray();
Console.WriteLine($"Found {inputFiles.Length} input files in {casesDir}");

var framesField = typeof(NativeAlrDocument).GetField("_frames", BindingFlags.NonPublic | BindingFlags.Instance)!;

foreach (string path in inputFiles)
{
    string name = Path.GetFileNameWithoutExtension(path);
    byte[] source = File.ReadAllBytes(path);
    string statusFile = Path.Combine(casesDir, $"{name}.status");

    NativeAlrDocument doc;
    try
    {
        doc = NativeAlrDocument.Parse(source);
    }
    catch (Exception ex)
    {
        File.WriteAllText(statusFile, $"REJECT {ex.GetType().Name}\n{ex.Message}");
        continue;
    }

    File.WriteAllText(statusFile, "OK");

    var framesArray = (Array)framesField.GetValue(doc)!;
    int frameCount = doc.Frames.Count;

    // 1. Metadata
    var metaLines = new List<string>
    {
        $"{doc.Version} {doc.LayoutColumns} {doc.LayoutRows} {doc.PaletteVariantCount} {doc.AnchorWidth} {doc.AnchorHeight} {frameCount}"
    };

    var framePayloads = new List<byte[]>();
    var frameRows = new List<uint[]>();
    var sharedIndices = new int[frameCount];

    // Scan source to get exact frame references and offsets
    int parseOffset = 92;
    for (int i = 0; i < frameCount; i++)
    {
        int reference = unchecked((int)BinaryPrimitives.ReadUInt32LittleEndian(source.AsSpan(parseOffset, 4)));
        parseOffset += 4;
        sharedIndices[i] = reference;

        object frameObj = framesArray.GetValue(i)!;
        Type frameType = frameObj.GetType();

        byte[] payload = (byte[])frameType.GetProperty("Payload", BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Public)!.GetValue(frameObj)!;
        uint[] rows = (uint[])frameType.GetProperty("Rows", BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Public)!.GetValue(frameObj)!;

        framePayloads.Add(payload);
        frameRows.Add(rows);

        var info = doc.Frames[i];
        bool isShared = reference >= 0;
        metaLines.Add($"{i} {(isShared ? 1 : 0)} {reference} {info.OffsetX} {info.OffsetY} {info.Width} {info.Height} {info.PaletteColorCount} {info.PackedSize}");

        if (reference < 0)
        {
            parseOffset += 16;
            parseOffset += payload.Length;
            parseOffset += (info.Height + 1) * 4;
        }
    }

    File.WriteAllLines(Path.Combine(casesDir, $"{name}.meta"), metaLines);

    // 2. Palettes
    for (int v = 0; v < doc.PaletteVariantCount; v++)
    {
        if (doc.Frames.Count > 0 && doc.Frames[0].PaletteColorCount > 0)
        {
            try
            {
                IReadOnlyList<uint> pal = doc.ReadRawPalette(v);
                byte[] palBytes = new byte[pal.Count * 4];
                for (int c = 0; c < pal.Count; c++)
                {
                    BinaryPrimitives.WriteUInt32LittleEndian(palBytes.AsSpan(c * 4, 4), pal[c]);
                }
                File.WriteAllBytes(Path.Combine(casesDir, $"{name}.palette_{v}"), palBytes);
            }
            catch (Exception ex)
            {
                File.WriteAllText(Path.Combine(casesDir, $"{name}.palette_err_{v}"), ex.GetType().Name);
            }
        }
    }

    // 3. Indexed Samples & ARGB Pixels
    for (int i = 0; i < frameCount; i++)
    {
        try
        {
            IReadOnlyList<ushort> indexed = doc.ReadIndexedSamples(i);
            byte[] indexedBytes = new byte[indexed.Count * 2];
            for (int p = 0; p < indexed.Count; p++)
            {
                BinaryPrimitives.WriteUInt16LittleEndian(indexedBytes.AsSpan(p * 2, 2), indexed[p]);
            }
            File.WriteAllBytes(Path.Combine(casesDir, $"{name}.indexed_{i}"), indexedBytes);
        }
        catch (Exception ex)
        {
            File.WriteAllText(Path.Combine(casesDir, $"{name}.indexed_err_{i}"), ex.GetType().Name);
        }

        for (int v = 0; v < doc.PaletteVariantCount; v++)
        {
            try
            {
                NativeAlrIndexedFrame decoded = doc.DecodeFrame(i, v);
                byte[] argbBytes = new byte[decoded.ArgbPixels.Count * 4];
                for (int p = 0; p < decoded.ArgbPixels.Count; p++)
                {
                    BinaryPrimitives.WriteUInt32LittleEndian(argbBytes.AsSpan(p * 4, 4), decoded.ArgbPixels[p]);
                }
                File.WriteAllBytes(Path.Combine(casesDir, $"{name}.argb_{i}_{v}"), argbBytes);
            }
            catch (Exception ex)
            {
                File.WriteAllText(Path.Combine(casesDir, $"{name}.argb_err_{i}_{v}"), ex.GetType().Name);
            }
        }
    }

    // 4. Roundtrip serialization
    try
    {
        byte[] roundtrip = SerializeAlr(doc, source, framePayloads, frameRows, sharedIndices);
        File.WriteAllBytes(Path.Combine(casesDir, $"{name}.roundtrip"), roundtrip);
    }
    catch (Exception ex)
    {
        File.WriteAllText(Path.Combine(casesDir, $"{name}.roundtrip_err"), ex.GetType().Name);
    }
}

return 0;

static byte[] SerializeAlr(NativeAlrDocument doc, byte[] source, List<byte[]> payloads, List<uint[]> rowsList, int[] sharedIndices)
{
    using var ms = new MemoryStream();
    using var writer = new BinaryWriter(ms);

    // 23 Header words: take original source[..92] and preserve sentinels
    byte[] header = new byte[92];
    Array.Copy(source, 0, header, 0, Math.Min(92, source.Length));
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(0, 4), 0x41524C41);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(4, 4), doc.Version);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(12, 4), (uint)doc.Frames.Count);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(16, 4), 8);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(24, 4), doc.LayoutColumns);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(28, 4), doc.LayoutRows);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(32, 4), (uint)doc.PaletteVariantCount);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(60, 4), doc.AnchorWidth);
    BinaryPrimitives.WriteUInt32LittleEndian(header.AsSpan(64, 4), doc.AnchorHeight);
    writer.Write(header);

    // Scan source to extract storedPixelBytes for independent frames
    int srcOffset = 92;
    var storedPixelBytesList = new uint[doc.Frames.Count];
    for (int k = 0; k < doc.Frames.Count; k++)
    {
        int reference = unchecked((int)BinaryPrimitives.ReadUInt32LittleEndian(source.AsSpan(srcOffset, 4)));
        srcOffset += 4;
        if (reference >= 0)
        {
            continue;
        }
        // Independent record: tableOffset(4), pos(4), size(4), storedPixelBytes(4)
        uint spb = BinaryPrimitives.ReadUInt32LittleEndian(source.AsSpan(srcOffset + 12, 4));
        storedPixelBytesList[k] = spb;
        srcOffset += 16;
        srcOffset += payloads[k].Length;
        srcOffset += (doc.Frames[k].Height + 1) * 4;
    }

    // Write frame records
    for (int i = 0; i < doc.Frames.Count; i++)
    {
        int sharedFrom = sharedIndices[i];
        if (sharedFrom >= 0)
        {
            writer.Write(sharedFrom);
        }
        else
        {
            writer.Write(-1);
            writer.Write((uint)payloads[i].Length);
            var info = doc.Frames[i];
            uint position = (uint)(info.OffsetX & 0xFFFF) | (uint)((info.OffsetY & 0xFFFF) << 16);
            writer.Write(position);
            writer.Write(info.PackedSize);
            writer.Write(storedPixelBytesList[i]);
            writer.Write(payloads[i]);
            foreach (uint row in rowsList[i])
            {
                writer.Write(row);
            }
        }
    }

    // Append trailing bytes
    if (srcOffset < source.Length)
    {
        writer.Write(source.AsSpan(srcOffset).ToArray());
    }

    return ms.ToArray();
}
