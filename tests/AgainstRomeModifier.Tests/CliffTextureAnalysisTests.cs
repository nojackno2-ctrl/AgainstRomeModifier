using System.Drawing;
using System.Drawing.Imaging;
using System.Text.Json;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>Explicitly opt-in, reads only the authorized fixed TEMP copies. No game or map writes.</summary>
public sealed class CliffTextureAnalysisTests
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    [Fact]
    public void Analyze_authorized_temp_cliff_textures_and_original_height_context()
    {
        if (Environment.GetEnvironmentVariable("ARM_CLIFF_ANALYSIS") != "1") return;
        string input = Path.Combine(Path.GetTempPath(), "ArmGameCompare_20261007");
        string output = Path.Combine(Path.GetTempPath(), "ArmCliffAnalysis_20261008");
        Directory.CreateDirectory(output);
        using var library = new FloorTextureLibrary(Path.Combine(input, "floortex.dat"));
        Assert.True(library.IsAvailable);
        var realCatalog = CliffTileCatalog.BuildRealNames(library.Names).FilteredBy(n => library.Get(n) is not null);
        Assert.Equal(4, realCatalog.Entries.Count);
        Assert.All(realCatalog.Entries, entry => Assert.Contains(entry.Texture, library.Names, StringComparer.Ordinal));
        var materials = new FloorMaterialCatalog(library);
        var references = materials.Materials.Select(m => (m.Id, Color: Mean(library.Get(m.RepresentativeTexture)!, 0, 0, 128, 128))).ToArray();
        var dryGround = Mean(library.Get("fels01_35")!, 0, 0, 128, 128);
        var paleRock = Mean(library.Get("ita_fels3")!, 0, 0, 128, 128);
        var grass = Mean(library.Get("Fels_AA_009")!, 96, 96, 32, 32);
        var grayRock = Mean(library.Get("Fels_AA_005")!, 0, 0, 128, 128);
        string[] names = library.Names.Where(n => n.Contains("fels", StringComparison.OrdinalIgnoreCase)).ToArray();
        var tiles = new List<object>();
        var errors = new List<string>();
        using var sheet = new Bitmap(8 * 144, ((names.Length + 7) / 8) * 160);
        using var graphics = Graphics.FromImage(sheet);
        graphics.Clear(Color.White);
        using var font = new Font("Arial", 9);
        for (int i = 0; i < names.Length; i++)
        {
            string name = names[i];
            try
            {
                Bitmap bitmap = library.Get(name)!;
                int w = Math.Max(1, bitmap.Width / 4), h = Math.Max(1, bitmap.Height / 4);
                (int X, int Y)[] origins = [(0, 0), (bitmap.Width-w, 0), (bitmap.Width-w, bitmap.Height-h), (0, bitmap.Height-h)];
                var corners = origins.Select(p => Mean(bitmap, p.X, p.Y, w, h)).ToArray();
                var closest = corners.Select(c => references.OrderBy(r => Distance(c, r.Color)).Take(3).Select(r => new { Material = r.Id, Distance = Distance(c, r.Color) }).ToArray()).ToArray();
                bool aa = name.StartsWith("Fels_AA_", StringComparison.Ordinal);
                double[] ground = aa ? grass : dryGround, rock = aa ? grayRock : paleRock;
                var fits = corners.Select(c => new { GroundDistance = Distance(c, ground), RockDistance = Distance(c, rock),
                    Side = Distance(c, rock) < Distance(c, ground) ? "rock" : "ground", Margin = Math.Abs(Distance(c, rock)-Distance(c, ground)) }).ToArray();
                int rockMask = 0;
                for (int c = 0; c < 4; c++) if (fits[c].Side == "rock") rockMask |= 1 << c;
                var smallCorners = new[] { Mean(bitmap,0,0,16,16), Mean(bitmap,bitmap.Width-16,0,16,16),
                    Mean(bitmap,bitmap.Width-16,bitmap.Height-16,16,16), Mean(bitmap,0,bitmap.Height-16,16,16) };
                int smallMask = 0;
                for (int c = 0; c < 4; c++) if (Distance(smallCorners[c],rock) < Distance(smallCorners[c],ground)) smallMask |= 1 << c;
                bool uniformRock = name.StartsWith("ita_fels", StringComparison.Ordinal) || System.Text.RegularExpressions.Regex.IsMatch(name, "^fels[1-4]$");
                tiles.Add(new { Name = name, bitmap.Width, bitmap.Height, CornersTL_TR_BR_BL = corners, Corners16px = smallCorners,
                    ClosestGroundMaterials = closest, ReferenceGround = ground, ReferenceRock = rock, RockGroundFits = fits,
                    RockMask = uniformRock ? 15 : rockMask, RockMask16px = uniformRock ? 15 : smallMask,
                    RockSideHypothesis = uniformRock ? "Uniform rock; no directional evidence" : Facing(rockMask),
                    AmbiguousCorners = fits.Count(f => f.Margin < 12),
                    Confidence = "Colour and contact-sheet evidence only. Uniform, diagonal, or unstable fits are unresolved; cliff inner/outer geometry and elevation usage unverified." });
                if (new[] { "Fels_AA_002", "Fels_AA_004", "Fels_AA_006", "Fels_AA_008" }.Contains(name))
                {
                    int expected = name switch { "Fels_AA_002" => 12, "Fels_AA_004" => 6, "Fels_AA_006" => 9, _ => 3 };
                    Assert.Equal(expected, rockMask);
                    Assert.Equal(expected, smallMask);
                }
                graphics.DrawImage(bitmap, (i % 8) * 144, (i / 8) * 160, 128, 128);
                graphics.DrawString(name, font, Brushes.Black, (i % 8) * 144, (i / 8) * 160 + 130);
            }
            catch (Exception ex) when (ex is ArgumentException or IOException or System.Runtime.InteropServices.ExternalException)
            { errors.Add(name + ": " + ex.Message); }
        }
        sheet.Save(Path.Combine(output, "fels-contact-sheet.png"), ImageFormat.Png);
        var occurrences = new List<object>();
        var summaries = new List<object>();
        foreach (string map in new[] { "ENDL_000", "ENDL_005" })
        {
            var document = BodenTexturesDocument.Load(Path.Combine(input, map, "boden.txt"));
            var height = TerrainLayerFiles.Read(Path.Combine(input, map, "boden.bmp"))!;
            int step = (height.Width - 1) / document.Dimension;
            Assert.Equal(height.Width - 1, step * document.Dimension);
            var detection = CliffEdgeDetector.DetectFromVertexHeights(height.Width, height.Green, document.Dimension, step).CliffCells.ToDictionary(c => (c.X, c.Y));
            int Height(int x, int y) => height.Green[Math.Clamp(y, 0, height.Height-1) * height.Width + Math.Clamp(x, 0, height.Width-1)];
            for (int y = 0; y < document.Dimension; y++)
            for (int x = 0; x < document.Dimension; x++)
            {
                string name = document.GetTexture(x, y);
                if (!name.Contains("fels", StringComparison.OrdinalIgnoreCase)) continue;
                int[] corners = [Height(x*step,y*step), Height((x+1)*step,y*step), Height((x+1)*step,(y+1)*step), Height(x*step,(y+1)*step)];
                var values = new List<int>();
                for (int vy = y*step; vy <= (y+1)*step; vy++)
                for (int vx = x*step; vx <= (x+1)*step; vx++) values.Add(Height(vx,vy));
                detection.TryGetValue((x,y), out var cell);
                occurrences.Add(new { Map = map, X = x, Y = y, Name = name, ExactLibraryName = library.Names.Contains(name, StringComparer.Ordinal),
                    CornersTL_TR_BR_BL = corners, IntraTileDelta = values.Max()-values.Min(), DetectorFacing = cell?.Facing.ToString(),
                    NeighborCentersN_E_S_W = new[] { Height(x*step+step/2,(y-1)*step+step/2), Height((x+1)*step+step/2,y*step+step/2), Height(x*step+step/2,(y+1)*step+step/2), Height((x-1)*step+step/2,y*step+step/2) } });
            }
            summaries.Add(new { Map = map, Dimension = document.Dimension, VertexSize = height.Width, VertexSamplesPerTile = step,
                DetectedCliffCells = detection.Count, ExactLibraryTileCount = document.Textures.Count(n => library.Names.Contains(n, StringComparer.Ordinal)), Usage = document.Textures.Where(n => n.Contains("fels", StringComparison.OrdinalIgnoreCase)).GroupBy(n => n, StringComparer.Ordinal).ToDictionary(g => g.Key, g => g.Count()) });
        }
        var hashes = new[] { "floortex.dat", "ENDL_000/boden.txt", "ENDL_000/boden.bmp", "ENDL_005/boden.txt", "ENDL_005/boden.bmp" }
            .ToDictionary(p => p, p => Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(Path.Combine(input,p)))));
        var guessed = CliffTileCatalog.CreateDefault().Entries.Where(e => e.Family == "FELS").Select(e => new { e.Texture, Exact = library.Names.Contains(e.Texture, StringComparer.Ordinal), CaseInsensitive = library.Names.Contains(e.Texture, StringComparer.OrdinalIgnoreCase) });
        File.WriteAllText(Path.Combine(output, "analysis.json"), JsonSerializer.Serialize(new { InputSha256 = hashes, SelectedCatalog = realCatalog.Entries, LibraryNameCount = library.Names.Count, CornerOrder = "TL,TR,BR,BL; +X east, +Y south", Names = names, Guessed = guessed, DecodeErrors = errors, Tiles = tiles, Maps = summaries, Occurrences = occurrences }, JsonOptions));
        Assert.NotEmpty(names);
        Assert.Empty(errors);
    }

    internal static string Facing(int rockMask) => rockMask switch
    {
        3 => "North", 6 => "East", 12 => "South", 9 => "West",
        7 => "Rock on N/E; geometric corner unresolved", 11 => "Rock on N/W; geometric corner unresolved", 14 => "Rock on S/E; geometric corner unresolved", 13 => "Rock on S/W; geometric corner unresolved",
        2 => "Rock at NE corner; inner/outer unresolved", 1 => "Rock at NW corner; inner/outer unresolved", 4 => "Rock at SE corner; inner/outer unresolved", 8 => "Rock at SW corner; inner/outer unresolved",
        _ => "Unresolved (uniform or diagonal corners)"
    };

    private static double[] Mean(Bitmap bitmap, int x, int y, int width, int height)
    {
        double[] sum = [0, 0, 0]; int count = 0;
        for (int sy = y; sy < Math.Min(bitmap.Height,y+height); sy += 2)
        for (int sx = x; sx < Math.Min(bitmap.Width,x+width); sx += 2)
        { Color c = bitmap.GetPixel(sx,sy); sum[0] += c.R; sum[1] += c.G; sum[2] += c.B; count++; }
        return sum.Select(c => c/count).ToArray();
    }
    private static double Distance(double[] a, double[] b) => Math.Sqrt(a.Zip(b, (x,y) => (x-y)*(x-y)).Sum());
}