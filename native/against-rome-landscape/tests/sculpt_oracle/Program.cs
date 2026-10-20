using System.Globalization;
using AgainstRomeMapEditor;

// Differential oracle for TerrainSculptFilter.{Terrace,SharpenRidge,CarveValley} and
// TerrainSculptPipeline.{Terrace,SharpenRidge}. Input is produced by tests/sculpt_differential.rs;
// floats are f32 bit patterns in lowercase hex.
//   A|id|size|cx|cy|r|step|flat|edge|str|base|heights(f32 bits)       terrace
//   B|id|size|cx|cy|r|gain|ridgeOnly|iters|heights                    sharpen ridge
//   C|id|size|cx|cy|r|depth|str|heights                               carve valley
//   P|id|size|cx|cy|r|step|flat|edge|str|base|bytes(decimal)          pipeline terrace
//   Q|id|size|cx|cy|r|gain|ridgeOnly|iters|bytes(decimal)             pipeline sharpen
// Output: A/B/C -> kind|id|heights ; P/Q -> kind|id|idx:before:after;... ; any -> kind|id|ERR:<type>
static float F(string s) => BitConverter.Int32BitsToSingle(int.Parse(s, NumberStyles.HexNumber));
static string B(float f) => BitConverter.SingleToInt32Bits(f).ToString("x");
static string Arr(IEnumerable<float> v) => string.Join(',', v.Select(B));
static TerrainHeightWorkBuffer Load(int size, string bits)
{
    var buf = new TerrainHeightWorkBuffer(size);
    var src = bits.Split(',').Select(F).ToArray();
    Array.Copy(src, buf.RawData, src.Length);
    return buf;
}
static string Changes(IEnumerable<TerrainSampleChange> c) => string.Join(';', c.Select(x => $"{x.Index}:{x.Before}:{x.After}"));

foreach (var line in File.ReadLines(args[0]))
{
    if (line.Length == 0) continue;
    var f = line.Split('|');
    string kind = f[0], id = f[1];
    try
    {
        int size = int.Parse(f[2]);
        float cx = F(f[3]), cy = F(f[4]), r = F(f[5]);
        switch (kind)
        {
            case "A":
            {
                var buf = Load(size, f[11]);
                TerrainSculptFilter.Terrace(buf, cx, cy, r, F(f[6]), F(f[7]), F(f[8]), F(f[9]), F(f[10]));
                Console.WriteLine($"A|{id}|{Arr(buf.RawData)}");
                break;
            }
            case "B":
            {
                var buf = Load(size, f[9]);
                TerrainSculptFilter.SharpenRidge(buf, cx, cy, r, F(f[6]), f[7] == "1", int.Parse(f[8]));
                Console.WriteLine($"B|{id}|{Arr(buf.RawData)}");
                break;
            }
            case "C":
            {
                var buf = Load(size, f[8]);
                TerrainSculptFilter.CarveValley(buf, cx, cy, r, F(f[6]), F(f[7]));
                Console.WriteLine($"C|{id}|{Arr(buf.RawData)}");
                break;
            }
            default:
            {
                var bytes = f[kind == "P" ? 11 : 9].Split(',').Select(byte.Parse).ToArray();
                var session = new TerrainHeightEditSession(size, bytes, null, 1, null);
                var changes = kind == "P"
                    ? TerrainSculptPipeline.Terrace(session, cx, cy, r, F(f[6]), F(f[7]), F(f[8]), F(f[9]), F(f[10]))
                    : TerrainSculptPipeline.SharpenRidge(session, cx, cy, r, F(f[6]), f[7] == "1", int.Parse(f[8]));
                Console.WriteLine($"{kind}|{id}|{Changes(changes)}");
                break;
            }
        }
    }
    catch (Exception e) { Console.WriteLine($"{kind}|{id}|ERR:{e.GetType().Name}"); }
}
