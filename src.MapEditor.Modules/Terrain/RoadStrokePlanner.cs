namespace AgainstRomeMapEditor;

[Flags]
internal enum RoadConnections { None = 0, North = 1, East = 2, South = 4, West = 8 }

internal sealed record RoadTile(string Texture, RoadConnections Connections);
internal sealed record RoadStrokePlan(bool Succeeded, IReadOnlyList<RoadTilePlacement> Tiles, IReadOnlyList<(int X, int Y)> Unsupported);
internal sealed record RoadTilePlacement(int X, int Y, string Texture);

/// <summary>Plans connected native road tiles without changing the map. Missing turns/junctions reject the complete plan.</summary>
internal static class RoadStrokePlanner
{
    private static readonly (int X, int Y, RoadConnections Out, RoadConnections Back)[] Neighbors =
    [
        (0, -1, RoadConnections.North, RoadConnections.South),
        (1, 0, RoadConnections.East, RoadConnections.West),
        (0, 1, RoadConnections.South, RoadConnections.North),
        (-1, 0, RoadConnections.West, RoadConnections.East)
    ];

    public static RoadStrokePlan Plan(int dimension, IReadOnlyList<string> textures,
        IReadOnlyList<(int X, int Y)> path, IReadOnlyList<RoadTile> available, string preferredTexture)
    {
        if (dimension <= 0 || textures.Count != (long)dimension * dimension) throw new ArgumentException("Invalid road map dimensions.");
        if (path.Any(p => p.X < 0 || p.Y < 0 || p.X >= dimension || p.Y >= dimension))
            return new(false, [], path.Where(p => p.X < 0 || p.Y < 0 || p.X >= dimension || p.Y >= dimension).ToArray());
        var byName = available
            .GroupBy(t => t.Texture, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => new RoadTile(g.Key, g.Aggregate(RoadConnections.None, (acc, item) => acc | item.Connections)), StringComparer.OrdinalIgnoreCase);
        var links = new Dictionary<(int X, int Y), RoadConnections>();
        void Add((int X, int Y) p, RoadConnections connection) => links[p] = links.GetValueOrDefault(p) | connection;
        if (path.Count > 0) Add(path[0], RoadConnections.None);
        // Add intended links separately so crossings and backtracking preserve every segment.
        for (int i = 1; i < path.Count; i++)
        {
            var previous = path[i - 1];
            foreach (var next in OrthogonalBetween(previous, path[i]))
            {
                var direction = Neighbors.Single(n => n.X == next.X - previous.X && n.Y == next.Y - previous.Y);
                Add(previous, direction.Out); Add(next, direction.Back);
                previous = next;
            }
        }
        foreach (var p in links.Keys.ToArray())
        {
            if (!byName.TryGetValue(textures[p.Y * dimension + p.X], out var current)) continue;
            foreach (var neighbor in Neighbors)
            {
                int x = p.X + neighbor.X, y = p.Y + neighbor.Y;
                if (x < 0 || y < 0 || x >= dimension || y >= dimension) continue;
                if ((current.Connections & neighbor.Out) != 0 && byName.TryGetValue(textures[y * dimension + x], out var adjacent)
                    && (adjacent.Connections & neighbor.Back) != 0) Add(p, neighbor.Out);
            }
        }
        var placements = new List<RoadTilePlacement>();
        var unsupported = new List<(int X, int Y)>();
        foreach (var (p, required) in links.OrderBy(pair => pair.Key.Y).ThenBy(pair => pair.Key.X))
        {
            // Endpoints may use a straight tile with one open end; turns/junctions require an exact piece.
            var candidates = available.Where(t => t.Connections == required ||
                (IsEndpoint(required) && (t.Connections & required) == required && IsStraight(t.Connections)) ||
                (required == RoadConnections.None && StringComparer.OrdinalIgnoreCase.Equals(t.Texture, preferredTexture)))
                .OrderBy(t => t.Connections == required ? 0 : 1)
                .ThenBy(t => StringComparer.OrdinalIgnoreCase.Equals(t.Texture, preferredTexture) ? 0 : 1)
                .ThenBy(t => t.Texture, StringComparer.OrdinalIgnoreCase).ToArray();
            if (candidates.Length == 0) unsupported.Add(p);
            else placements.Add(new(p.X, p.Y, candidates[0].Texture));
        }
        return unsupported.Count > 0 ? new(false, [], unsupported) : new(true, placements, []);
    }

    private static bool IsEndpoint(RoadConnections connections) => connections is RoadConnections.North or RoadConnections.East or RoadConnections.South or RoadConnections.West;
    private static bool IsStraight(RoadConnections connections) => connections == (RoadConnections.North | RoadConnections.South) || connections == (RoadConnections.East | RoadConnections.West);

    internal static IEnumerable<(int X, int Y)> OrthogonalBetween((int X, int Y) start, (int X, int Y) end)
    {
        var previous = start;
        foreach (var next in TerrainStrokePath.Between(start.X, start.Y, end.X, end.Y))
        {
            if (next.X != previous.X && next.Y != previous.Y) yield return (next.X, previous.Y);
            yield return next;
            previous = next;
        }
    }
}
