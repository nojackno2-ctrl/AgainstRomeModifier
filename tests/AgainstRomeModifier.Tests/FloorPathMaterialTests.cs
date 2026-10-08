using System.Drawing;
using System.Drawing.Imaging;
using System.IO.Compression;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class FloorPathMaterialTests : IDisposable
{
    private static readonly Color Path = Color.FromArgb(190, 170, 120), Grass = Color.FromArgb(70, 130, 40), Earth = Color.FromArgb(120, 90, 50), Odd = Color.FromArgb(255, 0, 255),
        NearGrass = Color.FromArgb(80, 140, 45), FarGrass = Color.FromArgb(40, 160, 40);
    private readonly string _root = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "ArmPathMaterial_" + Guid.NewGuid().ToString("N"));

    public FloorPathMaterialTests() => Directory.CreateDirectory(_root);
    public void Dispose() { try { Directory.Delete(_root, true); } catch (IOException) { } }

    // 九宮格位置 n 的 A 圖塊：路面在面向區域中心的角落（TL、TR、BR、BL 順序）。
    private static readonly Dictionary<int, bool[]> PathCornersA = new()
    {
        [1] = [false, true, false, false], [2] = [true, true, false, false], [3] = [true, false, false, false],
        [4] = [false, true, true, false], [6] = [true, false, false, true],
        [7] = [false, false, true, false], [8] = [false, false, true, true], [9] = [false, false, false, true],
    };

    [Fact]
    public void Dirt_path_tiles_become_a_paintable_material_with_native_borders()
    {
        string archive = CreateArchive(tiles =>
        {
            tiles["4BB___50"] = Solid(Grass); tiles["4BJ___50"] = Solid(Earth);
            tiles["4BW___50"] = Solid(NearGrass); tiles["4BC___50"] = Solid(FarGrass);
            tiles["PFAD1"] = Solid(Path); tiles["PFAD2"] = Solid(Path);
            foreach ((int shape, bool[] path) in PathCornersA)
            {
                tiles[$"PFAD{shape}A1"] = Quadrants(path, Grass);
                tiles[$"PFAD_Erde{shape}A2"] = Quadrants(path, Earth);
                tiles[$"PFAD{shape}A3"] = Quadrants(path, Odd); // 外側顏色不屬於任何材質：整組不得採用
            }
            foreach (int shape in new[] { 1, 3, 7, 9 }) tiles[$"PFAD{shape}B1"] = Quadrants(PathCornersA[shape].Select(value => !value).ToArray(), Grass);
        });
        using var library = new FloorTextureLibrary(archive);
        var catalog = new FloorMaterialCatalog(library);

        FloorMaterial path = Assert.Single(catalog.Materials, material => material.Id == FloorMaterialCatalog.PathMaterialId);
        Assert.Equal(["PFAD1", "PFAD2"], path.Variants);
        Assert.Equal(("BB", true), (catalog.PathTransitionFits["1"].Outer, catalog.PathTransitionFits["1"].Distance < 1));
        Assert.Equal("BJ", catalog.PathTransitionFits["Erde2"].Outer);
        Assert.Null(catalog.PathTransitionFits["3"].Outer);
        Assert.False(catalog.TryResolveNativeCorners("PFAD8A3", out _));

        Assert.True(catalog.TryResolveNativeCorners("PFAD8A1", out IReadOnlyList<string> top));
        Assert.Equal(["BB", "BB", "PFAD", "PFAD"], top);
        Assert.True(catalog.TryResolveNativeCorners("PFAD1B1", out IReadOnlyList<string> inner));
        Assert.Equal(["PFAD", "BB", "PFAD", "PFAD"], inner);
        Assert.Equal("PFAD2A1", catalog.ResolveNativeTile(["PFAD", "PFAD", "BB", "BB"], 3, 4));
        Assert.Equal("PFAD_Erde4A2", catalog.ResolveNativeTile(["BJ", "PFAD", "PFAD", "BJ"], 0, 0));
        Assert.Equal("PFAD1", catalog.ResolveNativeTile(["PFAD", "PFAD", "PFAD", "PFAD"], 0, 0));
        Assert.True(catalog.HasTransition("PFAD", "BB"));
        Assert.True(catalog.HasTransition("BJ", "PFAD"));
        Assert.Equal(1, catalog.MapSuitability("PFAD", ["BB"]));

        // 顏色相近的草地（BW，色差約 17）可近似銜接同一組邊界；差太多的（BC）不行；匯入判讀仍用主要外側 BB。
        Assert.Contains(catalog.PathApproximateOuters["1"], fit => fit.Outer == "BW");
        Assert.DoesNotContain(catalog.PathApproximateOuters["1"], fit => fit.Outer == "BC");
        Assert.Equal("PFAD8A1", catalog.ResolveNativeTile(["BW", "BW", "PFAD", "PFAD"], 0, 0));
        Assert.Null(catalog.ResolveNativeTile(["BC", "BC", "PFAD", "PFAD"], 0, 0));
        Assert.Equal(1, catalog.MapSuitability("PFAD", ["BW"]));
        Assert.True(catalog.TryResolveNativeCorners("PFAD8A1", out IReadOnlyList<string> primary));
        Assert.Equal(["BB", "BB", "PFAD", "PFAD"], primary);
    }

    [Fact]
    public void Painting_dirt_path_on_grass_bakes_native_border_tiles_and_undoes()
    {
        string archive = CreateArchive(tiles =>
        {
            tiles["4BB___50"] = Solid(Grass); tiles["PFAD1"] = Solid(Path);
            foreach ((int shape, bool[] path) in PathCornersA) tiles[$"PFAD{shape}A1"] = Quadrants(path, Grass);
            foreach (int shape in new[] { 1, 3, 7, 9 }) tiles[$"PFAD{shape}B1"] = Quadrants(PathCornersA[shape].Select(value => !value).ToArray(), Grass);
        });
        using var library = new FloorTextureLibrary(archive);
        var catalog = new FloorMaterialCatalog(library);
        string[] source = Enumerable.Repeat("4BB___50", 64).ToArray();
        var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(8, source, catalog, "BB"), source, catalog);

        TerrainBlendPaintResult paint = session.PaintCircle(4f, 4f, 1.6f, FloorMaterialCatalog.PathMaterialId);
        Assert.True(paint.Succeeded, string.Join("; ", paint.Issues.Select(issue => issue.ToString())));
        Assert.True(session.CommitStroke());
        Assert.Contains("PFAD1", session.CurrentTextures);
        Assert.Contains(session.CurrentTextures, texture => texture.EndsWith("A1", StringComparison.Ordinal));
        Assert.All(session.CurrentTextures, texture => Assert.Contains(texture, library.Names));
        Assert.NotNull(session.Undo());
        Assert.Equal(source, session.CurrentTextures);
    }

    [Fact]
    public void Archive_without_dirt_path_tiles_has_no_path_material()
    {
        string archive = CreateArchive(tiles => { tiles["4BB___50"] = Solid(Grass); tiles["PFAD8A1"] = Quadrants(PathCornersA[8], Grass); });
        using var library = new FloorTextureLibrary(archive);
        var catalog = new FloorMaterialCatalog(library);
        Assert.DoesNotContain(catalog.Materials, material => material.Id == FloorMaterialCatalog.PathMaterialId);
        Assert.False(catalog.TryResolveNativeCorners("PFAD8A1", out _));
    }

    /// <summary>真實素材副本（ARM_COMPARE_GAME，唯讀 TEMP 複製）：名稱推得的路面角落在貼圖上必須比外側更接近純路面顏色。</summary>
    [Fact]
    public void Real_dirt_path_borders_match_their_texture_corners()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_COMPARE_GAME");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(System.IO.Path.Combine(game, "floortex.dat"))) return;
        using var library = new FloorTextureLibrary(System.IO.Path.Combine(game, "floortex.dat"));
        var catalog = new FloorMaterialCatalog(library);
        FloorMaterial path = Assert.Single(catalog.Materials, material => material.Id == FloorMaterialCatalog.PathMaterialId);
        Assert.Contains(catalog.PathTransitionFits.Values, fit => fit.Outer is not null);
        string report = string.Join(Environment.NewLine, catalog.PathTransitionFits.OrderBy(item => item.Key).Select(item => $"{item.Key}\t{item.Value.Outer}\t{item.Value.Distance:F1}"));
        File.WriteAllText(System.IO.Path.Combine(_root, "..", "ArmPathMaterialFits.txt"), report);
        var pathColor = Mean(library.Get(path.RepresentativeTexture)!, 0, 0, 128, 128);
        int checkedTiles = 0;
        foreach (string name in library.Names.Where(name => name.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase)))
        {
            if (!catalog.TryResolveNativeCorners(name, out IReadOnlyList<string> corners) || corners.All(corner => corner == "PFAD")) continue;
            Bitmap bitmap = library.Get(name)!;
            string outer = corners.First(corner => corner != "PFAD");
            var outerColor = Mean(library.Get(catalog.Materials.First(material => material.Id == outer).RepresentativeTexture)!, 0, 0, 128, 128);
            (int X, int Y)[] origins = [(0, 0), (96, 0), (96, 96), (0, 96)];
            for (int corner = 0; corner < 4; corner++)
            {
                var sample = Mean(bitmap, origins[corner].X, origins[corner].Y, 32, 32);
                bool closerToPath = Distance(sample, pathColor) < Distance(sample, outerColor);
                Assert.True(closerToPath == (corners[corner] == "PFAD"), $"{name} corner {corner}: {string.Join(",", corners)}");
            }
            checkedTiles++;
        }
        Assert.True(checkedTiles >= 20, $"only {checkedTiles} registered path border tiles");

        // 在真實地圖副本上以自動過渡畫土路：記錄各地圖成功率（報表），至少要有一筆成功且產生原版邊界圖塊。
        var lines = new List<string>();
        bool anyBorder = false;
        foreach (string mapName in new[] { "ENDL_000", "ENDL_005" })
        {
            string bodenPath = System.IO.Path.Combine(game, mapName, "boden.txt");
            if (!File.Exists(bodenPath)) continue;
            var boden = AgainstRomeModifier.Maps.BodenTexturesDocument.Load(bodenPath);
            string[] source = boden.Textures.ToArray();
            int succeeded = 0, attempts = 0;
            for (int y = 8; y < boden.Dimension - 8; y += 8)
            for (int x = 8; x < boden.Dimension - 8; x += 8)
            {
                var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(boden.Dimension, source, catalog, "BB"), source, catalog);
                attempts++;
                if (!session.PaintCircle(x, y, 2.1f, FloorMaterialCatalog.PathMaterialId, autoBridge: true).Succeeded) continue;
                succeeded++;
                string[] borders = session.CurrentTextures.Where(texture => texture.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase) && texture.Length > 5).ToArray();
                anyBorder |= borders.Length > 0;
                // ENDL_005 全是同一種地表（B8）：同一筆畫不可混用草地邊界與土地（Erde）邊界。
                if (mapName == "ENDL_005") Assert.True(borders.All(texture => texture.Contains("_Erde", StringComparison.OrdinalIgnoreCase)) || borders.All(texture => !texture.Contains("_Erde", StringComparison.OrdinalIgnoreCase)), string.Join(",", borders));
                Assert.All(session.CurrentTextures, texture => Assert.NotNull(library.Get(texture)));
            }
            lines.Add($"{mapName}\t{succeeded}/{attempts}\tsuitability={catalog.MapSuitability("PFAD", source.Select(texture => catalog.FindByTexture(texture)?.Id).OfType<string>().Distinct().ToArray())}");
        }
        File.AppendAllText(System.IO.Path.Combine(_root, "..", "ArmPathMaterialFits.txt"), Environment.NewLine + string.Join(Environment.NewLine, lines));
        if (lines.Count > 0) Assert.True(anyBorder, string.Join("; ", lines));
    }

    private string CreateArchive(Action<Dictionary<string, Bitmap>> fill)
    {
        var tiles = new Dictionary<string, Bitmap>(StringComparer.OrdinalIgnoreCase);
        fill(tiles);
        string archive = System.IO.Path.Combine(_root, "floortex.dat");
        using ZipArchive zip = ZipFile.Open(archive, ZipArchiveMode.Create);
        foreach ((string name, Bitmap bitmap) in tiles)
        {
            using (Stream stream = zip.CreateEntry($"SYSTEM/DATA/FLOORTEXTURE/{name}.bmp").Open()) bitmap.Save(stream, ImageFormat.Bmp);
            bitmap.Dispose();
        }
        return archive;
    }

    private static Bitmap Solid(Color color) => Quadrants([true, true, true, true], color, color);

    private static Bitmap Quadrants(bool[] path, Color outer) => Quadrants(path, outer, Path);

    private static Bitmap Quadrants(bool[] path, Color outer, Color pathColor)
    {
        var bitmap = new Bitmap(128, 128, PixelFormat.Format24bppRgb);
        using Graphics graphics = Graphics.FromImage(bitmap);
        (int X, int Y)[] origins = [(0, 0), (64, 0), (64, 64), (0, 64)];
        for (int corner = 0; corner < 4; corner++)
        {
            using var brush = new SolidBrush(path[corner] ? pathColor : outer);
            graphics.FillRectangle(brush, origins[corner].X, origins[corner].Y, 64, 64);
        }
        return bitmap;
    }

    private static (double R, double G, double B) Mean(Bitmap bitmap, int x, int y, int width, int height)
    {
        double r = 0, g = 0, b = 0; int count = 0;
        for (int sy = y; sy < y + height; sy += 2)
        for (int sx = x; sx < x + width; sx += 2)
        {
            Color color = bitmap.GetPixel(sx, sy); r += color.R; g += color.G; b += color.B; count++;
        }
        return (r / count, g / count, b / count);
    }

    private static double Distance((double R, double G, double B) a, (double R, double G, double B) b)
        => Math.Sqrt(Math.Pow(a.R - b.R, 2) + Math.Pow(a.G - b.G, 2) + Math.Pow(a.B - b.B, 2));
}
