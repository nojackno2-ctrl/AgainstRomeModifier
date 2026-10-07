using System.Buffers.Binary;
using System.Globalization;
using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length is < 2 or > 4)
{
    Console.Error.WriteLine("Usage: shad-probe <readonly shad.dat> <NEW TEMP directory> [decoded objdef.txt] [decoded cl_shado.txt]");
    return 2;
}
string input = Path.GetFullPath(args[0]), output = Path.GetFullPath(args[1]);
string temp = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Path.GetTempPath())) + Path.DirectorySeparatorChar;
if (!output.StartsWith(temp, StringComparison.OrdinalIgnoreCase) || Directory.Exists(output) || File.Exists(output))
{
    Console.Error.WriteLine("Output must be a NEW child directory of TEMP; existing files are never overwritten.");
    return 2;
}
Directory.CreateDirectory(output);
using var inputStream = new FileStream(input, FileMode.Open, FileAccess.Read, FileShare.Read);
string hash = Convert.ToHexString(SHA256.HashData(inputStream)); inputStream.Position = 0;
using var archive = new ZipArchive(inputStream, ZipArchiveMode.Read);
int documents = 0, frames = 0, directories = 0, otherBmps = 0;
var failures = new List<object>();
var inventory = new List<object>();
var samples = new List<object>();
var shadowEntries = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
foreach (var entry in archive.Entries)
{
    try
    {
        if (entry.FullName.EndsWith('/'))
        {
            if (entry.Length != 0) throw new InvalidDataException("Directory contains data.");
            directories++; inventory.Add(new { entry = entry.FullName, kind = "directory" }); continue;
        }
        using var stream = entry.Open(); using var buffer = new MemoryStream(); stream.CopyTo(buffer);
        byte[] bytes = buffer.ToArray();
        // 每個 ZIP 項目都開啟及核對實際資料；其餘 BMP 並非物件陰影。
        var audit = AuditBmp(bytes);
        bool shadow = entry.FullName.StartsWith("SYSTEM/DATA/SHADOWTEXTURE/", StringComparison.OrdinalIgnoreCase);
        if (!shadow)
        {
            otherBmps++; inventory.Add(new { entry = entry.FullName, kind = "other BMP", audit }); continue;
        }
        var document = NativeShadowDocument.Parse(bytes);
        var frame = document.DecodeFrame();
        documents++; frames++;
        string name = entry.FullName.Split('/')[^1];
        shadowEntries.Add(name, bytes);
        inventory.Add(new { entry = entry.FullName, kind = "shadow", audit,
            document.TrailingByteCount, minStrength = (int)frame.AlphaMask.ToArray().Min(),
            maxStrength = (int)frame.AlphaMask.ToArray().Max() });
        if (name.Equals("GerHau02_shadow.bmp", StringComparison.OrdinalIgnoreCase) ||
            name.Equals("GerNadelbaum_shadow.bmp", StringComparison.OrdinalIgnoreCase) ||
            name.Equals("round_small_shadow.bmp", StringComparison.OrdinalIgnoreCase) ||
            name.Equals("LakaDorn_shadow.bmp", StringComparison.OrdinalIgnoreCase) ||
            name.Equals("error.bmp", StringComparison.OrdinalIgnoreCase))
        {
            string file = $"sample-{samples.Count}.png";
            PngWriter.Write(Path.Combine(output, file), frame.Width, frame.Height, frame.ArgbPixels);
            uint[] ground = frame.AlphaMask.ToArray().Select(v => NativeShadowFrame.DarkenArgb(0xFF94B870, v)).ToArray();
            string groundFile = $"sample-{samples.Count}-ground.png";
            PngWriter.Write(Path.Combine(output, groundFile), frame.Width, frame.Height, ground);
            samples.Add(new { entry = entry.FullName, file, groundFile, document.Width, document.Height });
        }
    }
    catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException)
    {
        failures.Add(new { entry = entry.FullName, error = error.Message });
    }
}
var names = args.Length == 4 ? ReadShadowNames(args[3]) : new Dictionary<int, string>();
var objects = new List<object>();
if (args.Length >= 3)
{
    string[] lines = File.ReadAllLines(args[2]);
    string header = lines.First(line => line.StartsWith(";idx", StringComparison.Ordinal));
    string[] columns = header.Split(',').Select(v => v.Trim().TrimStart(';')).ToArray();
    foreach (string column in new[] { "shidx", "shsiz", "shtyp", "shacx", "shacz" })
        if (Array.IndexOf(columns, column) < 0) throw new InvalidDataException($"Missing objdef column {column}.");
    foreach (string line in lines)
    {
        string[] values = line.Split(',');
        if (!int.TryParse(values[0].Trim(), out int id) || id is not (12 or 42 or 46)) continue;
        int Value(string column) => int.Parse(values[Array.IndexOf(columns, column)].Trim(), CultureInfo.InvariantCulture);
        int shidx = Value("shidx"), shsiz = Value("shsiz"), shtyp = Value("shtyp");
        int shacx = Value("shacx"), shacz = Value("shacz");
        bool hasName = names.TryGetValue(shidx, out string? mapped);
        string candidate = id switch { 42 => "GerHau02_shadow.bmp", 46 => "GerNadelbaum_shadow.bmp", _ => "round_small_shadow.bmp" };
        string selected = hasName ? mapped! : candidate;
        string status = hasName ? "ID from supplied ShadowNames" : "UNVERIFIED candidate; ShadowNames missing";
        bool found = shadowEntries.TryGetValue(selected, out byte[]? source);
        string? preview = null;
        if (found)
        {
            preview = $"object-{id}-ground-assumption.png";
            GroundPreview(Path.Combine(output, preview), NativeShadowDocument.Parse(source!).DecodeFrame(),
                shsiz, shtyp, shacx, shacz);
        }
        objects.Add(new { id, name = values[52].Trim(), shidx, shsiz, shtyp, shacx, shacz,
            correctionScreenX = (shacx - shacz) / 2.0, correctionScreenY = (shacx + shacz) / 4.0,
            mappedEntry = mapped, candidateEntry = hasName ? null : candidate, status, found, preview,
            frame = 0, previewLimit = "Flat ground; assumed UV orientation. No game comparison or object sprite overlay." });
    }
}
var options = new JsonSerializerOptions { WriteIndented = true };
using (var report = new FileStream(Path.Combine(output, "report.json"), FileMode.CreateNew))
    JsonSerializer.Serialize(report, new { hash, inputBytes = inputStream.Length, entries = archive.Entries.Count,
        documents, frames, directories, otherBmps, failures, samples, objects, inventory }, options);
Console.WriteLine($"Entries={archive.Entries.Count} Documents={documents} Frames={frames} Directories={directories} OtherBMPs={otherBmps} Failures={failures.Count}");
Console.WriteLine($"Report: {Path.Combine(output, "report.json")}");
return failures.Count == 0 ? 0 : 1;

static object AuditBmp(byte[] bytes)
{
    if (bytes.Length < 54 || BinaryPrimitives.ReadUInt16LittleEndian(bytes) != 0x4D42)
        throw new InvalidDataException("Missing/truncated BMP.");
    uint Word(int offset) => BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(offset));
    int width = unchecked((int)Word(18)), height = unchecked((int)Word(22));
    int depth = BinaryPrimitives.ReadUInt16LittleEndian(bytes.AsSpan(28));
    long stride = (((long)width * depth + 31) / 32) * 4, extent = stride * Math.Abs((long)height);
    uint colors = Word(46), offset = Word(10);
    if (depth == 8 && colors == 0) colors = 256;
    if (Word(2) != bytes.Length || Word(6) != 0 || Word(14) != 40 ||
        width <= 0 || height is 0 or int.MinValue || depth is not (8 or 24) || Word(30) != 0 ||
        BinaryPrimitives.ReadUInt16LittleEndian(bytes.AsSpan(26)) != 1 ||
        (depth == 8 && colors > 256) || offset < 54L + colors * 4L ||
        offset + extent > bytes.Length)
        throw new InvalidDataException("Invalid BMP structural extent/layout.");
    if (depth == 8)
        for (int y = 0; y < Math.Abs(height); y++)
            for (int x = 0; x < width; x++)
                if (bytes[(int)(offset + y * stride + x)] >= colors)
                    throw new InvalidDataException("BMP palette index out of range.");
    return new { width, height, depth, stride, declaredImageBytes = Word(34), actualImageBytes = extent,
        imageSizeMatches = Word(34) == 0 || Word(34) == extent, trailingBytes = bytes.Length - offset - extent };
}

static Dictionary<int, string> ReadShadowNames(string path)
{
    var result = new Dictionary<int, string>(); bool section = false;
    foreach (string raw in File.ReadLines(path))
    {
        string line = raw.Trim();
        if (line.StartsWith('[')) { section = line.Equals("[ShadowNames]", StringComparison.OrdinalIgnoreCase); continue; }
        if (!section || line.Length == 0 || line.StartsWith(';')) continue;
        string[] fields = line.Split(',', 2);
        if (fields.Length != 2 || !int.TryParse(fields[0], out int id) || id is < 0 or >= 2000)
            throw new InvalidDataException("Invalid decoded ShadowNames row.");
        string name = fields[1].Trim();
        if (name.Length == 0 || name.Contains('/') || name.Contains('\\') || !result.TryAdd(id, name))
            throw new InvalidDataException("Invalid/duplicate shadow name.");
    }
    if (result.Count == 0) throw new InvalidDataException("Missing/empty ShadowNames section; supply decoded text, not PFIL.");
    return result;
}

static void GroundPreview(string path, NativeShadowFrame frame, int radius, int type, int correctionX, int correctionZ)
{
    // 僅供錨點研究：UV 軸向尚未與遊戲比對，不能當 renderer 驗收。
    const int width = 720, height = 400;
    var pixels = Enumerable.Repeat(0xFF94B870u, width * height).ToArray();
    if (radius > 0)
        for (int y = 0; y < height; y++)
            for (int x = 0; x < width; x++)
            {
                double dx = x - width / 2.0, dy = y - height / 2.0;
                double worldX = dx + 2 * dy - correctionX, worldZ = 2 * dy - dx - correctionZ;
                double u = worldX / radius, v = worldZ / radius;
                if (Math.Abs(u) >= 1 || Math.Abs(v) >= 1 || (type == 1 && u * u + v * v >= 1)) continue;
                int tx = Math.Clamp((int)((u + 1) * frame.Width / 2), 0, frame.Width - 1);
                int ty = Math.Clamp((int)((v + 1) * frame.Height / 2), 0, frame.Height - 1);
                pixels[y * width + x] = NativeShadowFrame.DarkenArgb(pixels[y * width + x], frame.AlphaMask[ty * frame.Width + tx]);
            }
    Cross(width / 2, height / 2, 0xFFFF4040);
    Cross((int)Math.Round(width / 2.0 + (correctionX - correctionZ) / 2.0),
        (int)Math.Round(height / 2.0 + (correctionX + correctionZ) / 4.0), 0xFF20FFFF);
    PngWriter.Write(path, width, height, pixels);
    void Cross(int cx, int cy, uint color)
    {
        for (int delta = -5; delta <= 5; delta++)
        {
            if ((uint)(cx + delta) < width && (uint)cy < height) pixels[cy * width + cx + delta] = color;
            if ((uint)cx < width && (uint)(cy + delta) < height) pixels[(cy + delta) * width + cx] = color;
        }
    }
}