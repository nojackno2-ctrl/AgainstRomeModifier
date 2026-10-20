using AgainstRomeMapEditor;

// Differential oracle for TerrainRampTool.CalculateRamp. The case matrix is mirrored
// verbatim by tests/ramp_differential.rs; every input is exactly representable in f32.
// Output: R|<case id>|<index>,<before>,<after>;...   or   R|<case id>|ERR
static byte[] Grid(int size, int pattern) =>
    Enumerable.Range(0, size * size).Select(i => pattern == 0
        ? (byte)((i * 17 + i / size * 13) % 256)
        : (byte)(i % size * 255 / (size - 1))).ToArray();

var cases = new List<(int Size, int Pattern, float[] Pts, float Width, float Falloff, float? H0, float? H1, TerrainFalloffType Type, bool Caps)>();
foreach (int size in new[] { 9, 33 })
foreach (int pattern in new[] { 0, 1 })
foreach (TerrainFalloffType type in Enum.GetValues<TerrainFalloffType>())
foreach (bool caps in new[] { false, true })
{
    float m = size - 1;
    cases.Add((size, pattern, new[] { 1.5f, 2.25f, m - 1.75f, m - 3.5f }, 3.5f, 2.75f, null, null, type, caps));
    cases.Add((size, pattern, new[] { 2f, m / 2, m - 2f, m / 2 }, 2f, 3f, 12.5f, 241.5f, type, caps));
    cases.Add((size, pattern, new[] { -3f, -3f, m + 4f, m + 4f }, 0f, 1.5f, -50f, 900f, type, caps));
    cases.Add((size, pattern, new[] { 4.5f, 4.5f, 4.5f, 4.5f }, 4f, 2f, null, null, type, caps));
    cases.Add((size, pattern, new[] { 3f, 6f, 3.25f, 2.5f }, 1f, 0f, 100f, 200f, type, caps));
}
int n = 0;
foreach (var c in cases)
{
    string id = n++.ToString();
    try
    {
        var h = Grid(c.Size, c.Pattern);
        var ch = TerrainRampTool.CalculateRamp(h, c.Size, c.Pts[0], c.Pts[1], c.Pts[2], c.Pts[3], new TerrainRampParameters
        { Width = c.Width, Falloff = c.Falloff, StartHeight = c.H0, EndHeight = c.H1, FalloffType = c.Type, RoundCaps = c.Caps });
        Console.WriteLine($"R|{id}|" + string.Join(';', ch.Select(x => $"{x.Index},{x.Before},{x.After}")));
    }
    catch (ArgumentException) { Console.WriteLine($"R|{id}|ERR"); }
}
// Error cases (ids continue): negative width, wrong grid length.
foreach (var (tag, act) in new (string, Action)[] {
    ("neg", () => TerrainRampTool.CalculateRamp(Grid(9, 0), 9, 0, 0, 1, 1, new TerrainRampParameters { Width = -1f })),
    ("len", () => TerrainRampTool.CalculateRamp(new byte[80], 9, 0, 0, 1, 1)) })
{
    try { act(); Console.WriteLine($"E|{tag}|OK"); } catch (ArgumentException) { Console.WriteLine($"E|{tag}|ERR"); }
}
