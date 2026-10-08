namespace AgainstRomeMapEditor;

/// <summary>
/// Catalogs native Against Rome road pieces and maps them to connection masks for RoadStrokePlanner.
/// </summary>
internal static class RoadTileCatalog
{
    // Standard road pieces (Steinweg):
    private static readonly string[] StandardHorizontal = ["H_WEG1", "H_WEG2", "H_WEG3", "H_WEG4", "H_WEG5"];
    private static readonly string[] StandardVertical = ["V_WEG1", "V_WEG2", "V_WEG3"];
    private static readonly string[] StandardJunctions = ["weg1", "weg2", "weg3"];

    // Roman road pieces (Römerstraße):
    private static readonly string[] RomanHorizontal = ["WEG_H1ROM", "WEG_H2ROM"];
    private static readonly string[] RomanVertical = ["WEG_V1ROM", "WEG_V2ROM", "WEG_V3ROM", "WEG_V4ROM"];
    private static readonly string[] RomanJunctions = ["Pflaster_braun1", "Pflaster_braun2", "Pflaster_braun3", "weg1", "weg2", "weg3"];

    public static IReadOnlyList<RoadTile> BuildAvailable(IEnumerable<string>? existingTextureNames, string? preferredTexture = null)
    {
        HashSet<string>? existing = existingTextureNames is null ? null : new HashSet<string>(existingTextureNames, StringComparer.OrdinalIgnoreCase);
        bool IsAvailable(string name) => existing is null || existing.Contains(name);

        bool preferRoman = preferredTexture is not null && (preferredTexture.Contains("ROM", StringComparison.OrdinalIgnoreCase) || preferredTexture.StartsWith("Pflaster", StringComparison.OrdinalIgnoreCase));

        var tiles = new List<RoadTile>();

        bool hasRomanH = RomanHorizontal.Any(IsAvailable);
        bool hasRomanV = RomanVertical.Any(IsAvailable);
        bool hasStdH = StandardHorizontal.Any(IsAvailable);
        bool hasStdV = StandardVertical.Any(IsAvailable);

        string[] horizontal = preferRoman && hasRomanH ? RomanHorizontal
            : (!preferRoman && hasStdH ? StandardHorizontal : StandardHorizontal.Concat(RomanHorizontal).ToArray());
        string[] vertical = preferRoman && hasRomanV ? RomanVertical
            : (!preferRoman && hasStdV ? StandardVertical : StandardVertical.Concat(RomanVertical).ToArray());
        string[] junctions = preferRoman ? RomanJunctions : StandardJunctions.Concat(RomanJunctions).ToArray();

        // 1. Horizontal straights (East | West)
        foreach (string h in horizontal.Where(IsAvailable))
        {
            tiles.Add(new RoadTile(h, RoadConnections.East | RoadConnections.West));
        }

        // 2. Vertical straights (North | South)
        foreach (string v in vertical.Where(IsAvailable))
        {
            tiles.Add(new RoadTile(v, RoadConnections.North | RoadConnections.South));
        }

        // 3. Junctions, corners, T-intersections, 4-ways, endpoints, and single points
        var availableJunctions = junctions.Where(IsAvailable).ToArray();
        if (availableJunctions.Length > 0)
        {
            for (int mask = 0; mask <= 15; mask++)
            {
                var conn = (RoadConnections)mask;
                bool isPureStraight = conn == (RoadConnections.East | RoadConnections.West) || conn == (RoadConnections.North | RoadConnections.South);
                bool isSingleEndpoint = conn is RoadConnections.North or RoadConnections.East or RoadConnections.South or RoadConnections.West;
                if (isPureStraight || isSingleEndpoint)
                {
                    bool hasSpecific = (conn == (RoadConnections.East | RoadConnections.West) || conn is RoadConnections.East or RoadConnections.West)
                        ? horizontal.Any(IsAvailable)
                        : vertical.Any(IsAvailable);
                    if (hasSpecific) continue;
                }

                foreach (string j in availableJunctions)
                {
                    tiles.Add(new RoadTile(j, conn));
                }
            }
        }

        // 4. If preferredTexture is specified and valid, ensure it is registered for single point (RoadConnections.None)
        if (!string.IsNullOrEmpty(preferredTexture) && IsAvailable(preferredTexture) && !tiles.Any(t => t.Connections == RoadConnections.None && StringComparer.OrdinalIgnoreCase.Equals(t.Texture, preferredTexture)))
        {
            tiles.Add(new RoadTile(preferredTexture, RoadConnections.None));
        }

        return tiles;
    }
}
