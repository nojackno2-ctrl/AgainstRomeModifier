using System.Globalization;
using AgainstRomeMapEditor;

// Differential oracle for ThermalErosionSimulator / HydraulicErosionSimulator.
// Input file (argv[0]) is produced by tests/erosion_differential.rs; floats are f32 bit patterns
// in lowercase hex so no text rounding can occur. Brush field: "-" or "cx:cy:r".
//   T|id|size|talus|rate|iters|spacing|brush|scree(0/1)|heights
//   H|id|size|droplets|life|inertia|gravity|capacity|minslope|erosion|deposit|evap|radius|seed|brush|heights
// Output: T|id|heights|scree   or   H|id|heights   or   <kind>|id|ERR:<exception type>
static float F(string s) => BitConverter.Int32BitsToSingle(int.Parse(s, NumberStyles.HexNumber));
static string B(float f) => BitConverter.SingleToInt32Bits(f).ToString("x");
static string Arr(IEnumerable<float> v) => string.Join(',', v.Select(B));
static (float?, float?, float?) Brush(string s)
{
    if (s == "-") return (null, null, null);
    var p = s.Split(':');
    return (F(p[0]), F(p[1]), F(p[2]));
}

foreach (var line in File.ReadLines(args[0]))
{
    if (line.Length == 0) continue;
    var f = line.Split('|');
    string kind = f[0], id = f[1];
    try
    {
        if (kind == "T")
        {
            int size = int.Parse(f[2]);
            var buf = new TerrainHeightWorkBuffer(size);
            var src = f[9].Split(',').Select(F).ToArray();
            Array.Copy(src, buf.RawData, src.Length);
            var p = new ThermalErosionParams { TalusAngleDegrees = F(f[3]), ErosionRate = F(f[4]), Iterations = int.Parse(f[5]), GridSpacing = F(f[6]) };
            var (cx, cy, r) = Brush(f[7]);
            float[]? scree = f[8] == "1" ? new float[size * size] : null;
            ThermalErosionSimulator.Simulate(buf, p, cx, cy, r, scree);
            Console.WriteLine($"T|{id}|{Arr(buf.RawData)}|{(scree == null ? "-" : Arr(scree))}");
        }
        else
        {
            int size = int.Parse(f[2]);
            var buf = new TerrainHeightWorkBuffer(size);
            var src = f[15].Split(',').Select(F).ToArray();
            Array.Copy(src, buf.RawData, src.Length);
            var p = new HydraulicErosionParams
            {
                DropletCount = int.Parse(f[3]), MaxLifetime = int.Parse(f[4]), Inertia = F(f[5]), Gravity = F(f[6]),
                CapacityFactor = F(f[7]), MinSlope = F(f[8]), ErosionRate = F(f[9]), DepositionRate = F(f[10]),
                EvaporationRate = F(f[11]), ErosionRadius = int.Parse(f[12]), RandomSeed = int.Parse(f[13]),
            };
            var (cx, cy, r) = Brush(f[14]);
            HydraulicErosionSimulator.Simulate(buf, p, cx, cy, r);
            Console.WriteLine($"H|{id}|{Arr(buf.RawData)}");
        }
    }
    catch (Exception e) { Console.WriteLine($"{kind}|{id}|ERR:{e.GetType().Name}"); }
}
