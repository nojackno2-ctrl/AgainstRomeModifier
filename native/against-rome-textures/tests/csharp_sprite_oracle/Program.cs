// Usage: SpriteCatalogOracle <fixture-dir> <output-file>
// Fixture dir: objdef.txt, alr_list.txt, apt_list.txt, cases.txt and asset files named like the archive entries.
// Case lines: "S name team direction angle|- state|-", "A name team angle|- state|-", "D name".
using System.Globalization;
using System.Text;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

string dir = args[0];
byte[]? Read(string name) { string path = Path.Combine(dir, "assets", name); return File.Exists(path) ? File.ReadAllBytes(path) : null; }
using var catalog = NativeSpriteCatalog.FromText(File.ReadAllText(Path.Combine(dir, "objdef.txt")),
    File.ReadAllText(Path.Combine(dir, "alr_list.txt")), File.ReadAllText(Path.Combine(dir, "apt_list.txt")), Read, Read);
var output = new StringBuilder();
foreach (string line in File.ReadAllLines(Path.Combine(dir, "cases.txt")))
{
    if (line.Length == 0) continue;
    string[] f = line.Split('|');
    output.Append(line).Append(" => ");
    switch (f[0])
    {
        case "S":
            output.Append(Sprite(catalog.GetSprite(f[1], int.Parse(f[2], CultureInfo.InvariantCulture),
                int.Parse(f[3], CultureInfo.InvariantCulture), Angle(f[4]), State(f[5]))));
            break;
        case "A":
            NativeSpriteAnimation? animation = catalog.GetAnimation(f[1], int.Parse(f[2], CultureInfo.InvariantCulture), Angle(f[3]), State(f[4]));
            output.Append(animation is null ? "null" : $"cycle={animation.CycleDurationMs} frames={animation.Frames.Count} " +
                string.Join(" ; ", animation.Frames.Select(Sprite)));
            break;
        case "D":
            output.Append(catalog.TryGetDefinition(f[1], out NativeSpriteDefinition? d)
                ? $"{d.TypeId},{d.Name},{d.AlrId},{d.AptIndex},{d.PaletteType},{d.LayerAlrId},{d.LayerType},{d.AnimationLengthMs},{d.AnimationAdd},{d.AnimationMask:X}" : "null");
            break;
        default: throw new InvalidOperationException(line);
    }
    output.Append('\n');
}
output.Append("count=").Append(catalog.DefinitionCount).Append('\n');
File.WriteAllText(args[1], output.ToString(), new UTF8Encoding(false));

static float? Angle(string s) => s == "-" ? null : float.Parse(s, CultureInfo.InvariantCulture);
static NativeAnimationVisualState? State(string s)
{
    if (s == "-") return null;
    short[] v = s.Split(',').Select(x => short.Parse(x, CultureInfo.InvariantCulture)).ToArray();
    return new NativeAnimationVisualState(v[0], v[1], v[2], v[3], v[4]);
}
static string Sprite(NativeSprite? s) => s is null ? "null"
    : $"{s.Width}x{s.Height}@{s.AnchorX},{s.AnchorY} {s.AssetName} " + string.Join(",", s.ArgbPixels.Select(p => p.ToString("X8", CultureInfo.InvariantCulture)));
