namespace AgainstRomeMapEditor;

internal static class TerrainRegionPlanner
{
    public static IReadOnlyCollection<(int X, int Y)> Rectangle(int dimension, (int X, int Y) a, (int X, int Y) b)
    {
        Validate(dimension, a); Validate(dimension, b);
        var cells = new List<(int X, int Y)>();
        for (int y = Math.Min(a.Y, b.Y); y <= Math.Max(a.Y, b.Y); y++)
            for (int x = Math.Min(a.X, b.X); x <= Math.Max(a.X, b.X); x++) cells.Add((x, y));
        return cells;
    }

    public static IReadOnlyCollection<(int X, int Y)> Connected(int dimension, IReadOnlyList<string> materials, (int X, int Y) seed)
    {
        Validate(dimension, seed);
        if (materials.Count != dimension * dimension) throw new ArgumentException("Grid dimensions do not match.", nameof(materials));
        var result = new List<(int X, int Y)>();
        var visited = new bool[materials.Count];
        var queue = new Queue<(int X, int Y)>();
        string material = materials[seed.Y * dimension + seed.X];
        queue.Enqueue(seed); visited[seed.Y * dimension + seed.X] = true;
        while (queue.TryDequeue(out var cell))
        {
            result.Add(cell);
            Visit(cell.X - 1, cell.Y); Visit(cell.X + 1, cell.Y); Visit(cell.X, cell.Y - 1); Visit(cell.X, cell.Y + 1);
        }
        return result;
        void Visit(int x, int y)
        {
            if (x < 0 || y < 0 || x >= dimension || y >= dimension) return;
            int index = y * dimension + x;
            if (visited[index] || !StringComparer.OrdinalIgnoreCase.Equals(materials[index], material)) return;
            visited[index] = true; queue.Enqueue((x, y));
        }
    }

    public static IReadOnlyCollection<(int X, int Y)> Polyline(int dimension, IReadOnlyList<(int X, int Y)> vertices, int width)
    {
        if (vertices.Count < 2) throw new ArgumentException("At least two vertices are required.", nameof(vertices));
        if (width < 1 || width > dimension) throw new ArgumentOutOfRangeException(nameof(width));
        foreach (var vertex in vertices) Validate(dimension, vertex);
        var cells = new HashSet<(int X, int Y)>();
        for (int i = 1; i < vertices.Count; i++)
        {
            var a = vertices[i - 1]; var b = vertices[i];
            int x = a.X, y = a.Y, dx = Math.Abs(b.X - x), dy = Math.Abs(b.Y - y);
            int sx = Math.Sign(b.X - x), sy = Math.Sign(b.Y - y), error = dx - dy;
            while (true)
            {
                Stamp(x, y);
                if (x == b.X && y == b.Y) break;
                int twice = 2 * error;
                if (twice > -dy) { error -= dy; x += sx; Stamp(x, y); }
                if (twice < dx) { error += dx; y += sy; }
            }
        }
        return cells.OrderBy(cell => cell.Y).ThenBy(cell => cell.X).ToArray();
        void Stamp(int x, int y)
        {
            int lower = (width - 1) / 2, upper = width / 2;
            for (int yy = Math.Max(0, y - lower); yy <= Math.Min(dimension - 1, y + upper); yy++)
                for (int xx = Math.Max(0, x - lower); xx <= Math.Min(dimension - 1, x + upper); xx++) cells.Add((xx, yy));
        }
    }

    private static void Validate(int dimension, (int X, int Y) point)
    {
        if (dimension <= 0 || point.X < 0 || point.Y < 0 || point.X >= dimension || point.Y >= dimension)
            throw new ArgumentOutOfRangeException(nameof(point));
    }
}
