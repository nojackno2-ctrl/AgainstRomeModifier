using System.IO.Compression;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;

// 明確傳入授權 apt.dat 副本；結果送 stdout，既不搜尋安裝目錄也不寫素材副本。
if (args.Length == 1 && args[0] == "validate")
{
    using var input = JsonDocument.Parse(Console.In.ReadToEnd());
    foreach (var item in input.RootElement.EnumerateArray())
    {
        byte[] bytes = Convert.FromBase64String(item.GetProperty("bytes").GetString()!);
        string name = item.GetProperty("name").GetString()!;
        if (name.EndsWith("shadows.dat", StringComparison.Ordinal))
        {
            var map = MapLightingShadowMap.ParseDecoded(bytes);
            Console.WriteLine($"VALID {name} decoded={bytes.Length} slices={MapLightingShadowMap.SliceCount} header={string.Join(',', map.Header)}");
        }
        else if (name.EndsWith("daynight.bmp", StringComparison.Ordinal))
        {
            var table = MapLightingDayNight.Parse(bytes);
            Console.WriteLine($"VALID {name} ambient17:55={table.AmbientAt(17, 55)} row4hour12={table.RawColor(12, 4)}");
        }
        else
        {
            var image = MapLightingBitmap.Parse(bytes);
            Console.WriteLine($"VALID {name} {image.Width}x{image.Height} rgb0={image.ColorAt(0, 0)}");
        }
    }
    return 0;
}
if (args.Length != 1) { Console.Error.WriteLine("Usage: lighting-probe <readonly apt.dat copy>"); return 2; }
using var archive = ZipFile.OpenRead(args[0]);
var samples = new List<object>();
foreach (string name in new[] { "gerhau00", "gerwoh00" })
{
    var entry = archive.Entries.Single(e => e.FullName.EndsWith("/" + name + ".apt", StringComparison.OrdinalIgnoreCase));
    using var stream = entry.Open(); using var buffer = new MemoryStream(); stream.CopyTo(buffer);
    var document = NativeAptDocument.Parse(buffer.ToArray());
    int frame = checked((int)((document.Layout[0] - 1) * document.Layout[1] * document.Layout[2] * document.Layout[3]));
    var image = document.DecodeFrame(frame);
    samples.Add(new { name, frame, image.Width, image.Height, document.AnchorX, document.AnchorY, pixels = image.ArgbPixels });
}
Console.WriteLine(JsonSerializer.Serialize(samples));
return 0;
