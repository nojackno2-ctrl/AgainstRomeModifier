using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Fortification;

// Actual C# planner/catalog sources are linked by the project, never reimplemented.
var available = Enumerable.Range(1, 15).Select(c => new RoadTile("road" + c, (RoadConnections)c)).ToArray();
for (int a = 0; a < 36; a++)
for (int b = 0; b < 36; b++)
{
    var textures = Enumerable.Repeat("grass", 36).ToArray();
    for (int x = 0; x < 6; x++) textures[2 * 6 + x] = "road10";
    var plan = RoadStrokePlanner.Plan(6, textures, [(a % 6, a / 6), (b % 6, b / 6), (2, 2)], available, "road10");
    if (args.Length == 0 || args[0] == "road") Console.WriteLine($"{a},{b}|{plan.Succeeded}|" + string.Join(';', plan.Tiles.Select(t => $"{t.X},{t.Y},{t.Texture}")));
}

if (args.Length > 0 && args[0] == "river")
for (int mode = 0; mode < 3; mode++)
for (int a = 0; a < 16; a++)
for (int b = 0; b < 16; b++)
{
    var textures = Enumerable.Repeat("grass", 16).ToArray();
    textures[3] = "SEE1";
    var heights = Enumerable.Range(0, 81).Select(i => (byte)(50 + (i * 7 + a * 3 + b) % 150)).ToArray();
    var options = new RiverPlannerOptions(ElevationMode: (RiverElevationMode)mode,
        AutoDetectDownhillFlow: a % 2 == 0, WaterLevel: b % 2 == 0 ? 160 : null);
    var p = RiverFlowPlanner.Plan(4, textures, heights, 9, [(a%4,a/4),(b%4,b/4),(2,2)], RiverTileCatalog.BuildAvailable(null), options);
    Console.WriteLine($"R|{mode},{a},{b}|{p.Succeeded}|" +
        string.Join(';', p.WaterTiles.Select(t => $"{t.X},{t.Y},{t.Texture},{(byte)t.Connections},{t.Kind},{(byte)t.Flow}")) + "|" +
        string.Join(';', p.BankTiles.Select(t => $"{t.X},{t.Y},{t.Texture},{(byte)t.Side}")) + "|" +
        string.Join(';', p.HeightAdjustments.Select(h => $"{h.Index},{h.VertexX},{h.VertexY},{h.Before},{h.After}")) + "|" +
        string.Join(';', p.Warnings.Select(w => $"{w.X},{w.Y},{w.CurrentElevation},{w.TargetElevation}")));
}

if (args.Length > 0 && args[0] == "landscape")
{
    var cliffCatalog = CliffTileCatalog.CreateDefault();
    for (int seed = 0; seed < 64; seed++)
    {
        var heights = Enumerable.Range(0, 36).Select(i => (float)((i*17 + seed*13) % 97)).ToArray();
        var d = CliffEdgeDetector.DetectFromTileHeights(6, heights);
        Console.WriteLine($"C|{seed}|" + string.Join(';', d.CliffCells.Select(c => $"{c.X},{c.Y},{c.Facing},{c.ElevationDelta},{cliffCatalog.PickTile(c.Facing,c.X,c.Y,seed,"FELS")}")) + "|" +
            string.Join(';', d.Contours.Select(c => $"{c.Id},{c.IsClosedLoop}:" + string.Join('/',c.Cells.Select(p => $"{p.X},{p.Y}")))));
    }
    var catalog = new WallTileCatalog();
    for (int style = 0; style < 4; style++)
    for (int a = 0; a < 16; a++)
    {
        int b=15-a;
        var options = new WallStrokeOptions { Style=(FortificationStyle)style, Team=3, AutoCornerTowers=a%2==0,
            ForcedGateLocations=[(1,1)], DefaultGateState=a%3==0 ? GatePassabilityState.Closed : GatePassabilityState.Open };
        var p = WallStrokePlanner.Plan(4, [(a%4,a/4),(b%4,b/4),(1,1)], options, catalog,
            getHeightAtTile:(x,z)=>x+z, isRoadAtTile:(x,z)=>x==2 && z%2==0);
        var collision=new byte[256];
        var changes=FortificationPassabilitySync.SynchronizePassability(collision,16,4,p);
        Console.WriteLine($"F|{style},{a}|{p.Succeeded}|" + string.Join(';',p.Placements.Select(t=>$"{t.NameDef},{t.Kind},{t.TileX},{t.TileZ},{t.WorldX},{t.WorldY},{t.WorldZ},{t.AngleDeg},{t.Team},{t.GateState},{t.FoundationTexture}")) + "|" +
            Convert.ToHexString(collision) + "|" + string.Join(';',changes.Select(c=>$"{c.Index},{c.Before},{c.After}")));
    }
}

namespace AgainstRomeMapEditor
{
    // Only needed by the unused TerrainRiverPaintResult return record in RiverTypes.
    internal sealed record TerrainTextureChange;
    internal sealed record TerrainSampleChange;
    // Verbatim standalone helper from TerrainHeightEditSession.cs (the original file also
    // contains the unrelated height session). Kept here to compile the linked road planner.
    internal static class TerrainStrokePath
    {
        public static IEnumerable<(int X, int Y)> Between(int fromX, int fromY, int toX, int toY)
        {
            int dx = Math.Abs(toX - fromX), dy = -Math.Abs(toY - fromY);
            int sx = fromX < toX ? 1 : -1, sy = fromY < toY ? 1 : -1, error = dx + dy;
            int x = fromX, y = fromY;
            while (x != toX || y != toY)
            {
                int twice = 2 * error;
                if (twice >= dy) { error += dy; x += sx; }
                if (twice <= dx) { error += dx; y += sy; }
                yield return (x, y);
            }
        }
    }
}
