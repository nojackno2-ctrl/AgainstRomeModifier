using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length != 2)
{
    Console.Error.WriteLine("Usage: alr-probe <readonly alr.dat> <new output directory>");
    return 2;
}
string input = Path.GetFullPath(args[0]), output = Path.GetFullPath(args[1]);
if (Directory.Exists(output) || File.Exists(output))
{
    Console.Error.WriteLine("Output must be a new directory; existing files are never overwritten.");
    return 2;
}
Directory.CreateDirectory(output);
using var inputStream = new FileStream(input, FileMode.Open, FileAccess.Read, FileShare.Read);
string hash = Convert.ToHexString(SHA256.HashData(inputStream)); inputStream.Position = 0;
using var archive = new ZipArchive(inputStream, ZipArchiveMode.Read);
int documents = 0, frames = 0, variants = 0, blanks = 0;
var failures = new List<object>();
var samples = new List<object>();
foreach (var entry in archive.Entries)
{
    try
    {
        using var stream = entry.Open(); using var buffer = new MemoryStream(); stream.CopyTo(buffer);
        var document = NativeAlrDocument.Parse(buffer.ToArray()); documents++;
        for (int frameIndex = 0; frameIndex < document.Frames.Count; frameIndex++)
        {
            for (int variant = 0; variant < document.PaletteVariantCount; variant++)
            {
                var frame = document.DecodeFrame(frameIndex, variant); variants++;
                if (variant == 0) { frames++; if (frame.Width == 0 || frame.Height == 0) blanks++; }
                // Representative object families; exports use safe generated names, never ZIP paths.
                string name = Path.GetFileName(entry.FullName);
                if (frameIndex == 0 && variant == 0 && frame.Width > 0 && frame.Height > 0 &&
                    (name == "fialgesc00.alr" || name == "lagestgr00.alr" || name == "lagenabust05.alr" || name == "gersch01.alr"))
                {
                    string file = $"sample-{samples.Count}.rgba";
                    using var pixels = File.Create(Path.Combine(output, file));
                    foreach (uint color in frame.ArgbPixels)
                    {
                        pixels.WriteByte((byte)(color >> 16)); pixels.WriteByte((byte)(color >> 8));
                        pixels.WriteByte((byte)color); pixels.WriteByte((byte)(color >> 24));
                    }
                    samples.Add(new { entry = entry.FullName, file, frame.Width, frame.Height,
                        info = document.Frames[0], document.AnchorWidth, document.AnchorHeight });
                }
            }
        }
    }
    catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException)
    {
        failures.Add(new { entry = entry.FullName, error = error.Message });
    }
}
string report = JsonSerializer.Serialize(new { hash, entries = archive.Entries.Count, documents, frames, variants, blanks, failures, samples },
    new JsonSerializerOptions { WriteIndented = true });
File.WriteAllText(Path.Combine(output, "report.json"), report);
Console.WriteLine($"Documents={documents} Frames={frames} PaletteFrames={variants} Blanks={blanks} Failures={failures.Count}");
return failures.Count == 0 ? 0 : 1;
