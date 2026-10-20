using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier;

// Explicitly run from the repository root; no retail assets or native DLL calls.
if (args.Length != 1) throw new ArgumentException("Expected fixture output path");
static byte[] Header(int type, int width, int height, int depth, int descriptor)
{
    byte[] b = new byte[18]; b[2] = (byte)type;
    BitConverter.GetBytes((ushort)width).CopyTo(b, 12);
    BitConverter.GetBytes((ushort)height).CopyTo(b, 14);
    b[16] = (byte)depth; b[17] = (byte)descriptor; return b;
}
var inputs = new List<byte[]>();
foreach (int depth in new[] {24, 32})
foreach (int desc in new[] {0, 32})
{
    byte[] h = Header(2, 1, 2, depth, desc);
    byte[] pixels = depth == 24 ? new byte[] {1,2,3,4,5,6} : new byte[] {1,2,3,17,4,5,6,99};
    inputs.Add(h.Concat(pixels).ToArray());
}
byte[] palette = Header(1, 3, 1, 8, 32); palette[1] = 1; palette[5] = 2; palette[7] = 24;
inputs.Add(palette.Concat(new byte[] {0,0,0,9,8,7,0,1,9}).ToArray());
byte[] withId = Header(2, 1, 1, 24, 32); withId[0] = 3;
inputs.Add(withId.Concat(new byte[] {65,66,67,1,2,3}).ToArray());
inputs.Add(Header(2, 2, 2, 24, 0)); inputs.Add(Header(3, 1, 1, 8, 0)); inputs.Add(new byte[17]);
var images = inputs.Select(b => {
    var image = TgaDecoder.Decode(b);
    return new { Input = b.Select(x => (int)x).ToArray(), Image = image == null ? null :
        new { image.Width, image.Height, Bgra = image.Bgra.Select(x => (int)x).ToArray() } };
}).ToArray();
double[] values = {100,12.5,40,20,3.2,1500.123456789,500,400,0};
var stats = new Dictionary<string, double[]> { ["A"] = values };
var generated = new DateTime(2026,1,2,3,4,5,DateTimeKind.Utc);
var sources = new[] {"TgaDecoder", "TroopPresetCodec"}.ToDictionary(n => n,
    n => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes($"src.Core/Core/Services/{n}.cs"))));
sources["TroopConfig"] = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes("src.Core/Core/TroopConfig.cs")));
var output = new { Sources = sources, Images = images, UnitOrder = TroopConfig.UnitOrder, BalancedStats = TroopConfig.BalancedUnitStats,
    PresetValues = values, Preset = TroopPresetCodec.Write(stats, generated) };
File.WriteAllText(args[0], JsonSerializer.Serialize(output, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"C# oracle: {images.Length} thumbnails + precise six-column preset -> {args[0]}");
