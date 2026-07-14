using System.Drawing;
using System.Text.RegularExpressions;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using Xunit.Abstractions;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorNativeBlendEvidenceTests(ITestOutputHelper output)
{
    private static readonly Regex TransitionName = new("^4U(?<first>[0-9A-Z])(?<second>[0-9A-Z])__(?<shape>[1-46-9])(?<variant>[0-9A-Z])$", RegexOptions.Compiled | RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly Regex BaseName = new("^4B(?<material>[0-9A-Z])___5[0-9A-Z]$", RegexOptions.Compiled | RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly Regex ThreeMaterialName = new("^4T(?<first>[0-9A-Z])[0-9A-Z]{2}_[2468][0-9A-Z]$", RegexOptions.Compiled | RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly (string Name, int Dx, int Dy)[] Directions =
    [
        ("NW", -1, -1), ("N", 0, -1), ("NE", 1, -1),
        ("W", -1, 0),                     ("E", 1, 0),
        ("SW", -1, 1),  ("S", 0, 1),   ("SE", 1, 1)
    ];

    [Fact]
    public void Original_maps_expose_directional_evidence_for_4u_pair_order()
    {
        string gamePath = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
        string mapsPath = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(mapsPath)) return;
        var counts = new Dictionary<(int Shape, string Direction), EvidenceCount>();
        var ownerCounts = new Dictionary<(int Shape, string Direction), EvidenceCount>();
        var pairCounts = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        int transitionCount = 0;
        foreach (string mapPath in Directory.GetDirectories(mapsPath))
        {
            string path = Path.Combine(mapPath, "boden.txt");
            if (!File.Exists(path)) continue;
            BodenTexturesDocument document = BodenTexturesDocument.Load(path);
            int dimension = document.Dimension;
            IReadOnlyList<string> textures = document.Textures;
            for (int y = 0; y < dimension; y++)
            {
                for (int x = 0; x < dimension; x++)
                {
                    Match transition = TransitionName.Match(textures[y * dimension + x]);
                    if (!transition.Success) continue;
                    transitionCount++;
                    string first = transition.Groups["first"].Value;
                    string second = transition.Groups["second"].Value;
                    string pair = first + second;
                    pairCounts[pair] = pairCounts.GetValueOrDefault(pair) + 1;
                    int shape = int.Parse(transition.Groups["shape"].Value);
                    foreach ((string direction, int dx, int dy) in Directions)
                    {
                        var key = (shape, direction);
                        int neighborX = x + dx, neighborY = y + dy;
                        if (neighborX >= 0 && neighborY >= 0 && neighborX < dimension && neighborY < dimension)
                        {
                            string? owner = LogicalOwner(textures[neighborY * dimension + neighborX]);
                            if (owner is not null)
                            {
                                ownerCounts.TryGetValue(key, out EvidenceCount ownerCurrent);
                                ownerCounts[key] = owner.Equals(first, StringComparison.OrdinalIgnoreCase) ? ownerCurrent with { First = ownerCurrent.First + 1 }
                                    : owner.Equals(second, StringComparison.OrdinalIgnoreCase) ? ownerCurrent with { Second = ownerCurrent.Second + 1 }
                                    : ownerCurrent with { Other = ownerCurrent.Other + 1 };
                            }
                        }
                        string? nearby = FindNearestBase(textures, dimension, x, y, dx, dy, maxDistance: 3);
                        if (nearby is null) continue;
                        counts.TryGetValue(key, out EvidenceCount current);
                        counts[key] = nearby.Equals(first, StringComparison.OrdinalIgnoreCase) ? current with { First = current.First + 1 }
                            : nearby.Equals(second, StringComparison.OrdinalIgnoreCase) ? current with { Second = current.Second + 1 }
                            : current with { Other = current.Other + 1 };
                    }
                }
            }
        }
        output.WriteLine("Immediate logical-owner evidence:");
        foreach (int shape in new[] { 1, 2, 3, 4, 6, 7, 8, 9 })
        {
            string summary = string.Join("  ", Directions.Select(direction =>
            {
                ownerCounts.TryGetValue((shape, direction.Name), out EvidenceCount value);
                return $"{direction.Name}:F{value.First}/S{value.Second}/O{value.Other}";
            }));
            output.WriteLine($"shape {shape}: {summary}");
        }
        output.WriteLine($"4U transitions: {transitionCount}");
        foreach (int shape in new[] { 1, 2, 3, 4, 6, 7, 8, 9 })
        {
            string summary = string.Join("  ", Directions.Select(direction =>
            {
                counts.TryGetValue((shape, direction.Name), out EvidenceCount value);
                return $"{direction.Name}:F{value.First}/S{value.Second}/O{value.Other}";
            }));
            output.WriteLine($"shape {shape}: {summary}");
        }
        int firstOwnerEvidence = ownerCounts.Values.Sum(value => value.First);
        int secondOwnerEvidence = ownerCounts.Values.Sum(value => value.Second);
        output.WriteLine("Most common pairs: " + string.Join(", ", pairCounts.OrderByDescending(item => item.Value).Take(20).Select(item => $"{item.Key}={item.Value}")));
        Assert.True(transitionCount > 10_000, $"原版地圖 fixture 的 4U 樣本不足：{transitionCount}");
        Assert.True(firstOwnerEvidence > secondOwnerEvidence * 1.5, $"4U 第一材質不是穩定 owner：F={firstOwnerEvidence}, S={secondOwnerEvidence}");
    }

    [Fact]
    public void Render_original_4u89_neighborhoods_when_evidence_mode_is_enabled()
    {
        if (Environment.GetEnvironmentVariable("ARM_BLEND_EVIDENCE") != "1") return;
        string gamePath = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
        string mapsPath = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(mapsPath) || !File.Exists(Path.Combine(gamePath, "floortex.dat"))) return;
        const int tilePixels = 56, patchTiles = 7, header = 28;
        int panelWidth = tilePixels * patchTiles, panelHeight = header + tilePixels * patchTiles;
        using var library = new FloorTextureLibrary(Path.Combine(gamePath, "floortex.dat"));
        using var montage = new Bitmap(panelWidth * 4, panelHeight * 2);
        using Graphics graphics = Graphics.FromImage(montage);
        graphics.Clear(Color.FromArgb(24, 24, 24));
        using var font = new Font("Microsoft JhengHei UI", 9F, FontStyle.Bold);
        int[] shapes = [1, 2, 3, 4, 6, 7, 8, 9];
        foreach ((int shape, int panelIndex) in shapes.Select((shape, index) => (shape, index)))
        {
            (string MapId, int X, int Y, IReadOnlyList<string> Textures, int Dimension)? sample = FindSample(mapsPath, shape);
            Assert.NotNull(sample);
            var value = sample.Value;
            int originX = panelIndex % 4 * panelWidth, originY = panelIndex / 4 * panelHeight;
            graphics.DrawString($"shape {shape} — {value.MapId} ({value.X},{value.Y})", font, Brushes.White, originX + 4, originY + 4);
            for (int patchY = -3; patchY <= 3; patchY++)
            {
                for (int patchX = -3; patchX <= 3; patchX++)
                {
                    string texture = value.Textures[(value.Y + patchY) * value.Dimension + value.X + patchX];
                    Bitmap? image = library.Get(texture);
                    var target = new Rectangle(originX + (patchX + 3) * tilePixels, originY + header + (patchY + 3) * tilePixels, tilePixels, tilePixels);
                    if (image is not null) graphics.DrawImage(image, target);
                }
            }
            using var pen = new Pen(Color.Red, 3);
            graphics.DrawRectangle(pen, originX + 3 * tilePixels + 1, originY + header + 3 * tilePixels + 1, tilePixels - 2, tilePixels - 2);
        }
        string outputDirectory = Path.Combine(Path.GetTempPath(), "arm-floortex-analysis");
        Directory.CreateDirectory(outputDirectory);
        string outputPath = Path.Combine(outputDirectory, "original-4U89-neighborhoods.png");
        montage.Save(outputPath, System.Drawing.Imaging.ImageFormat.Png);
        output.WriteLine(outputPath);
        (double R, double G, double B) firstMean = MeanColor(library.Get("4B8___50")!);
        (double R, double G, double B) secondMean = MeanColor(library.Get("4B9___50")!);
        foreach (string name in library.Names.Where(name => name.StartsWith("4U89__", StringComparison.OrdinalIgnoreCase)).OrderBy(name => name, StringComparer.OrdinalIgnoreCase))
        {
            (double R, double G, double B) transitionMean = MeanColor(library.Get(name)!);
            double coverage = ProjectCoverage(firstMean, secondMean, transitionMean);
            output.WriteLine($"{name} second-coverage={coverage:F3}");
        }

        var coverageByVariant = new Dictionary<string, List<double>>(StringComparer.OrdinalIgnoreCase);
        foreach (string name in library.Names)
        {
            Match match = TransitionName.Match(name);
            if (!match.Success) continue;
            Bitmap? first = library.Get($"4B{match.Groups["first"].Value}___50");
            Bitmap? second = library.Get($"4B{match.Groups["second"].Value}___50");
            Bitmap? transition = library.Get(name);
            if (first is null || second is null || transition is null) continue;
            var firstColor = MeanColor(first);
            var secondColor = MeanColor(second);
            double colorDistanceSquared = Math.Pow(secondColor.R - firstColor.R, 2) + Math.Pow(secondColor.G - firstColor.G, 2) + Math.Pow(secondColor.B - firstColor.B, 2);
            if (colorDistanceSquared < 400) continue;
            string variant = match.Groups["variant"].Value;
            if (!coverageByVariant.TryGetValue(variant, out List<double>? values)) coverageByVariant[variant] = values = [];
            values.Add(ProjectCoverage(firstColor, secondColor, MeanColor(transition)));
        }
        foreach ((string variant, List<double> values) in coverageByVariant.OrderBy(item => item.Key, StringComparer.OrdinalIgnoreCase))
        {
            double[] plausible = values.Where(value => value >= -0.25 && value <= 1.25).OrderBy(value => value).ToArray();
            if (plausible.Length == 0) continue;
            output.WriteLine($"all-pairs variant {variant}: n={plausible.Length}, median={plausible[plausible.Length / 2]:F3}, mean={plausible.Average():F3}");
        }

        var neighborhoodByTile = new Dictionary<string, (int Samples, int First, int Second)>(StringComparer.OrdinalIgnoreCase);
        foreach (string mapPath in Directory.GetDirectories(mapsPath))
        {
            string path = Path.Combine(mapPath, "boden.txt");
            if (!File.Exists(path)) continue;
            BodenTexturesDocument document = BodenTexturesDocument.Load(path);
            int dimension = document.Dimension;
            for (int y = 3; y < dimension - 3; y++)
            {
                for (int x = 3; x < dimension - 3; x++)
                {
                    string tile = document.Textures[y * dimension + x];
                    if (!tile.StartsWith("4U89__", StringComparison.OrdinalIgnoreCase)) continue;
                    neighborhoodByTile.TryGetValue(tile, out var current);
                    int first = 0, second = 0;
                    for (int offsetY = -3; offsetY <= 3; offsetY++)
                    {
                        for (int offsetX = -3; offsetX <= 3; offsetX++)
                        {
                            if (offsetX == 0 && offsetY == 0) continue;
                            string? owner = LogicalOwner(document.Textures[(y + offsetY) * dimension + x + offsetX]);
                            if (owner?.Equals("8", StringComparison.OrdinalIgnoreCase) == true) first++;
                            else if (owner?.Equals("9", StringComparison.OrdinalIgnoreCase) == true) second++;
                        }
                    }
                    neighborhoodByTile[tile] = (current.Samples + 1, current.First + first, current.Second + second);
                }
            }
        }
        foreach ((string tile, var neighborhood) in neighborhoodByTile.OrderBy(item => item.Key, StringComparer.OrdinalIgnoreCase))
        {
            int comparable = neighborhood.First + neighborhood.Second;
            double secondShare = comparable == 0 ? 0 : neighborhood.Second / (double)comparable;
            output.WriteLine($"map-use {tile}: n={neighborhood.Samples}, nearby-second={secondShare:F3} ({neighborhood.Second}/{comparable})");
        }
    }

    private static (double R, double G, double B) MeanColor(Bitmap bitmap)
    {
        long red = 0, green = 0, blue = 0, count = 0;
        for (int y = 0; y < bitmap.Height; y += 2)
        {
            for (int x = 0; x < bitmap.Width; x += 2)
            {
                Color color = bitmap.GetPixel(x, y);
                red += color.R; green += color.G; blue += color.B; count++;
            }
        }
        return (red / (double)count, green / (double)count, blue / (double)count);
    }

    private static double ProjectCoverage((double R, double G, double B) first, (double R, double G, double B) second, (double R, double G, double B) value)
    {
        double dr = second.R - first.R, dg = second.G - first.G, db = second.B - first.B;
        double denominator = dr * dr + dg * dg + db * db;
        return denominator <= double.Epsilon ? 0 : ((value.R - first.R) * dr + (value.G - first.G) * dg + (value.B - first.B) * db) / denominator;
    }

    private static (string MapId, int X, int Y, IReadOnlyList<string> Textures, int Dimension)? FindSample(string mapsPath, int shape)
    {
        foreach (string mapPath in Directory.GetDirectories(mapsPath))
        {
            string path = Path.Combine(mapPath, "boden.txt");
            if (!File.Exists(path)) continue;
            BodenTexturesDocument document = BodenTexturesDocument.Load(path);
            int dimension = document.Dimension;
            IReadOnlyList<string> textures = document.Textures;
            for (int y = 3; y < dimension - 3; y++)
            {
                for (int x = 3; x < dimension - 3; x++)
                {
                    if (textures[y * dimension + x].StartsWith($"4U89__{shape}", StringComparison.OrdinalIgnoreCase))
                        return (Path.GetFileName(mapPath), x, y, textures, dimension);
                }
            }
        }
        return null;
    }

    private static string? LogicalOwner(string texture)
    {
        Match @base = BaseName.Match(texture);
        if (@base.Success) return @base.Groups["material"].Value;
        Match transition = TransitionName.Match(texture);
        if (transition.Success) return transition.Groups["first"].Value;
        Match threeMaterial = ThreeMaterialName.Match(texture);
        return threeMaterial.Success ? threeMaterial.Groups["first"].Value : null;
    }

    private static string? FindNearestBase(IReadOnlyList<string> textures, int dimension, int x, int y, int dx, int dy, int maxDistance)
    {
        for (int distance = 1; distance <= maxDistance; distance++)
        {
            int sampleX = x + dx * distance, sampleY = y + dy * distance;
            if (sampleX < 0 || sampleY < 0 || sampleX >= dimension || sampleY >= dimension) return null;
            Match match = BaseName.Match(textures[sampleY * dimension + sampleX]);
            if (match.Success) return match.Groups["material"].Value;
        }
        return null;
    }

    private readonly record struct EvidenceCount(int First, int Second, int Other);
}
