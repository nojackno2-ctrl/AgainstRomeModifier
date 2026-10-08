namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>柵欄／城牆單件放置結果（世界座標；原版柵欄物件一律角度 0，朝向由變體決定）。</summary>
public sealed record PalisadePiece(string Name, float X, float Z);

/// <summary>
/// 依原版聚落 SDL 的實測規律沿折線排柵欄：每件間距 64 世界單位、角度一律 0；
/// 日耳曼南北向（沿 Z）用 Pal00、東西向（沿 X）用 Pal01，羅馬相反；轉角一律 Pal02，且轉角位於折點。
/// </summary>
public static class PalisadeRunPlanner
{
    public const float Spacing = 64f;

    /// <param name="vertices">折線頂點（世界座標）；非正交線段會拆成先 X 後 Z 兩段。</param>
    public static IReadOnlyList<PalisadePiece> Plan(IReadOnlyList<(float X, float Z)> vertices, string alongZ, string alongX, string corner)
    {
        ArgumentNullException.ThrowIfNull(vertices);
        // 先吸附到 64 網格並展開成正交折線。
        var path = new List<(int X, int Z)>();
        void Add((int X, int Z) point) { if (path.Count == 0 || path[^1] != point) path.Add(point); }
        (int X, int Z)? previous = null;
        foreach (var vertex in vertices)
        {
            var point = ((int)MathF.Round(vertex.X / Spacing), (int)MathF.Round(vertex.Z / Spacing));
            if (previous is { } last && last.X != point.Item1 && last.Z != point.Item2) Add((point.Item1, last.Z));
            Add(point); previous = path[^1];
        }
        if (path.Count < 2) return [];

        var pieces = new List<PalisadePiece>();
        for (int index = 0; index < path.Count; index++)
        {
            var (x, z) = path[index];
            bool isCorner = index > 0 && index < path.Count - 1 && Axis(path[index - 1], path[index]) != Axis(path[index], path[index + 1]);
            if (isCorner) { pieces.Add(new(corner, x * Spacing, z * Spacing)); }
            else
            {
                // 端點與直線中間點：依所在線段的軸向選變體。
                var segment = index < path.Count - 1 ? (path[index], path[index + 1]) : (path[index - 1], path[index]);
                pieces.Add(new(Axis(segment.Item1, segment.Item2) == 'Z' ? alongZ : alongX, x * Spacing, z * Spacing));
            }
            if (index < path.Count - 1)
            {
                var next = path[index + 1];
                int dx = Math.Sign(next.X - x), dz = Math.Sign(next.Z - z), steps = Math.Abs(next.X - x) + Math.Abs(next.Z - z);
                for (int step = 1; step < steps; step++)
                    pieces.Add(new(dz != 0 ? alongZ : alongX, (x + dx * step) * Spacing, (z + dz * step) * Spacing));
            }
        }
        return pieces;
    }

    private static char Axis((int X, int Z) from, (int X, int Z) to) => from.X == to.X ? 'Z' : 'X';
}
