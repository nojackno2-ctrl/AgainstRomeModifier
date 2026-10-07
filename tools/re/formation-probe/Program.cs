using System.IO.Compression;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;
if (args.Length is < 2 or > 3) throw new ArgumentException("Usage: <offline alr.dat> <NEW output directory> [ALR asset name]");
if (Directory.Exists(args[1])) throw new IOException("Output must be new");
Directory.CreateDirectory(args[1]);
using var zip = ZipFile.OpenRead(args[0]);
using var stream = zip.GetEntry("SYSTEM/DATA/ALR/" + (args.Length == 3 ? args[2] : "gerinf01.alr"))!.Open();
using var buffer = new MemoryStream(); stream.CopyTo(buffer);
var doc = NativeAlrDocument.Parse(buffer.ToArray());
var info = new List<object>();
for (int frame=0; frame<Math.Min(24,doc.LayoutColumns); frame++) {
 int index=(doc.LayoutRows > 14 ? 14*(int)doc.LayoutColumns : 0)+frame; var f=doc.DecodeFrame(index,0); var meta=doc.Frames[index];
 using var output=File.Create(Path.Combine(args[1],$"soldier-{frame}.rgba"));
 foreach (uint pixel in f.ArgbPixels) { output.WriteByte((byte)(pixel>>16)); output.WriteByte((byte)(pixel>>8)); output.WriteByte((byte)pixel); output.WriteByte((byte)(pixel>>24)); }
 info.Add(new { frame, f.Width, f.Height, AnchorX=(int)doc.AnchorWidth/2-meta.OffsetX, AnchorY=(int)doc.AnchorHeight/2-meta.OffsetY });
}
File.WriteAllText(Path.Combine(args[1],"frames.json"),JsonSerializer.Serialize(info));
Console.WriteLine("Exported frames");
