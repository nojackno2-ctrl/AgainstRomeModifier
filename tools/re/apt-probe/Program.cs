using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length is < 2 or > 3 || (args.Length == 3 && args[2] != "--attachments-only"))
{
    Console.Error.WriteLine("Usage: apt-probe <readonly apt.dat> <new output directory> [--attachments-only]"); return 2;
}
string input = Path.GetFullPath(args[0]), output = Path.GetFullPath(args[1]);
bool attachmentsOnly = args.Length == 3;
if (output.StartsWith(Path.GetDirectoryName(input) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
{
    Console.Error.WriteLine("Output must be outside the source archive directory."); return 2;
}
if (Directory.Exists(output) || File.Exists(output))
{
    Console.Error.WriteLine("Output must be a new directory; existing files are never overwritten."); return 2;
}
Directory.CreateDirectory(output);
using var inputStream = new FileStream(input, FileMode.Open, FileAccess.Read, FileShare.Read);
string hash = Convert.ToHexString(SHA256.HashData(inputStream)); inputStream.Position = 0;
using var archive = new ZipArchive(inputStream, ZipArchiveMode.Read);
int documents = 0, tiles = 0, frames = 0;
var failures = new List<object>(); var samples = new List<object>(); var attachments = new List<object>();
foreach (var entry in archive.Entries)
{
    try
    {
        using var stream = entry.Open(); using var buffer = new MemoryStream(); stream.CopyTo(buffer);
        var document = NativeAptDocument.Parse(buffer.ToArray()); documents++;
        int selections = 0, lightPoints = 0, smokePoints = 0, markers = 0;
        for (uint a = 0; a < document.Layout[0]; a++)
            for (uint c = 0; c < document.Layout[2]; c++)
                for (uint b = 0; b < document.Layout[1]; b++)
                {
                    var selected = document.GetAttachments(a, b, c); selections++;
                    lightPoints += selected.Lights.Count; smokePoints += selected.Smoke.Count;
                    if (selected.Marker.HasValue) markers++;
                    foreach (var point in selected.Lights.Concat(selected.Smoke))
                        _ = NativeAptAttachmentGeometry.Place(document, point, 0, 0, 0);
                    if (selected.Marker is { } marker) _ = NativeAptAttachmentGeometry.Place(document, marker, 0, 0, 0);
                }
        attachments.Add(new { entry = entry.FullName, document.Version, document.Layout,
            document.ProjectionOrigin, anchor = new { X = document.AnchorX, Y = document.AnchorY },
            groups = document.AttachmentGroups.Count, selections, lightPoints, smokePoints, markers });
        if (attachmentsOnly) continue;
        for (int variant = 0; variant < document.PaletteVariantCount; variant++)
        {
            for (int i = 0; i < document.Tiles.Count; i++) { _ = document.DecodeTile(i, variant); tiles++; }
            for (int i = 0; i < document.Frames.Count; i++)
            {
                var frame = document.DecodeFrame(i, variant); frames++;
                if (entry.FullName.EndsWith("/gerhau02.apt", StringComparison.OrdinalIgnoreCase) && variant == 0 && (i == 0 || i == 1000))
                {
                    string file = $"sample-{samples.Count}.rgba";
                    using var pixels = File.Create(Path.Combine(output, file));
                    foreach (uint color in frame.ArgbPixels)
                    {
                        pixels.WriteByte((byte)(color >> 16)); pixels.WriteByte((byte)(color >> 8));
                        pixels.WriteByte((byte)color); pixels.WriteByte((byte)(color >> 24));
                    }
                    samples.Add(new { entry = entry.FullName, frameIndex = i, file, frame.Width, frame.Height,
                        document.AnchorX, document.AnchorY });
                }
            }
        }
    }
    catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException)
    {
        failures.Add(new { entry = entry.FullName, error = error.Message });
        Console.WriteLine($"FAIL {entry.FullName}: {error.Message}");
    }
}
inputStream.Position = 0;
string hashAfter = Convert.ToHexString(SHA256.HashData(inputStream));
if (hash != hashAfter) throw new InvalidDataException("Source archive changed during audit.");
File.WriteAllText(Path.Combine(output, "report.json"), JsonSerializer.Serialize(new
{ hash, hashAfter, attachmentsOnly, entries = archive.Entries.Count, documents, tiles, frames, failures, samples, attachments }, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"Documents={documents} PaletteTiles={tiles} PaletteFrames={frames} Failures={failures.Count}");
return failures.Count == 0 ? 0 : 1;
