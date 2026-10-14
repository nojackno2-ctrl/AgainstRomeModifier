using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using Current = AgainstRomeMapEditor.NativeAssets.NativeAlrDocument;
using Frozen = AlrScanlineDifferential.Frozen.NativeAlrDocument;

if (args.Length != 3) throw new ArgumentException("Usage: <game-root> <particle-assets-inventory.json> <new-output.json>");
string archivePath = Path.Combine(Path.GetFullPath(args[0]), "alr.dat");
string outputPath = Path.GetFullPath(args[2]);
if (File.Exists(outputPath)) throw new IOException("Output already exists.");
string workspace = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../../re_workspace"));
if (!Path.GetRelativePath(workspace, outputPath).StartsWith("..", StringComparison.Ordinal)
    && Path.GetExtension(outputPath) == ".json") { }
else throw new IOException("Output must be a new JSON under re_workspace.");
static string Hash(string path) {
    using var input = File.OpenRead(path); return Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant();
}
string before = Hash(archivePath);
var beforeInfo = new FileInfo(archivePath);
long length = beforeInfo.Length; DateTime modified = beforeInfo.LastWriteTimeUtc;
using var inventory = JsonDocument.Parse(File.ReadAllBytes(args[1]));
using var archive = ZipFile.OpenRead(archivePath);
var records = new List<object>();
long frames = 0, decodes = 0, pixels = 0;
foreach (var asset in inventory.RootElement.GetProperty("target_alr_assets").EnumerateObject()) {
    var matches = archive.Entries.Where(e => e.FullName.Equals(asset.Name, StringComparison.OrdinalIgnoreCase)).ToArray();
    if (matches.Length != 1) throw new InvalidDataException($"Missing/ambiguous archive member: {asset.Name}");
    using var stream = matches[0].Open(); using var memory = new MemoryStream(); stream.CopyTo(memory);
    byte[] source = memory.ToArray();
    string memberHash = Convert.ToHexString(SHA256.HashData(source)).ToLowerInvariant();
    if (memberHash != asset.Value.GetProperty("sha256").GetString())
        throw new InvalidDataException($"Inventory identity changed: {asset.Name}");
    var current = Current.Parse(source); var frozen = Frozen.Parse(source);
    if (current.Frames.Count != frozen.Frames.Count || current.PaletteVariantCount != frozen.PaletteVariantCount)
        throw new InvalidDataException("Container mismatch.");
    for (int frame = 0; frame < current.Frames.Count; frame++)
        for (int variant = 0; variant < current.PaletteVariantCount; variant++) {
            var actual = current.DecodeFrame(frame, variant); var expected = frozen.DecodeFrame(frame, variant);
            if (actual.Width != expected.Width || actual.Height != expected.Height
                || !actual.ArgbPixels.SequenceEqual(expected.ArgbPixels))
                throw new InvalidDataException($"Pixel mismatch: {asset.Name}, frame={frame}, palette={variant}");
            decodes++; pixels += actual.ArgbPixels.Count;
        }
    frames += current.Frames.Count;
    records.Add(new { member = asset.Name, sha256 = memberHash, frames = current.Frames.Count, variants = current.PaletteVariantCount });
}
if (records.Count != 68 || frames != 11345) throw new InvalidDataException("Incomplete particle inventory scope.");
var afterInfo = new FileInfo(archivePath);
if (before != Hash(archivePath) || length != afterInfo.Length || modified != afterInfo.LastWriteTimeUtc)
    throw new IOException("Source archive changed during validation.");
string dll = Environment.GetEnvironmentVariable("ARM_NATIVE_DATA_DLL")
    ?? Path.Combine(AppContext.BaseDirectory, "against_rome_data.dll");
var result = new { scope = "Every frame and palette variant of the 68 identity-matched particle-linked ALRs",
    oracle = "Frozen C# editor decoder from e3a21d9; not original game runtime",
    members = records.Count, frames, decodes, pixels, failures = 0, source_unchanged = true,
    archive_sha256 = before, native_dll_sha256 = Hash(dll), records };
using (var output = new FileStream(outputPath, FileMode.CreateNew, FileAccess.Write))
    JsonSerializer.Serialize(output, result);
Console.WriteLine($"members={records.Count} frames={frames} decodes={decodes} pixels={pixels} failures=0 source_unchanged=true output={outputPath}");
