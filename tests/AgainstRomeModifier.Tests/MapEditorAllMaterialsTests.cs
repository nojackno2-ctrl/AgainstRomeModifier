using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>
    /// 真實素材庫：從 ARM_GAME_PATH 唯讀複製 floortex.dat 與 MAPS/ENDL_000 到 TEMP，對每一種地表材質以 1／5／15 格筆刷
    /// 在地圖中央塗佈。成功的筆畫必須只產生素材庫內存在的原生 tile 且中心為該材質；被拒絕的筆畫必須列出問題並完整回滾。
    /// 結果表輸出到 ARM_MATERIAL_REPORT（預設 TEMP），未設定 ARM_GAME_PATH 時直接返回。
    /// </summary>
    [Fact]
    public void Every_real_floor_material_paints_or_rejects_cleanly_at_each_brush_size()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        string source = Path.Combine(game, "MAPS", Environment.GetEnvironmentVariable("ARM_MATERIAL_SOURCE_MAP") ?? "ENDL_000");
        string map = Path.Combine(_root, "MAPS", "ENDL_005");
        foreach (string file in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
        {
            string target = Path.Combine(map, Path.GetRelativePath(source, file));
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            File.Copy(file, target);
        }
        File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        File.Copy(Path.Combine(game, "floortex.dat"), Path.Combine(_root, "floortex.dat"));
        string report = Environment.GetEnvironmentVariable("ARM_MATERIAL_REPORT") ?? Path.Combine(Path.GetTempPath(), "ArmMaterialReport.tsv");
        var lines = new List<string> { "material\tcategory\tname\tsize\tresult\ttiles\tissues\tcenter" };
        var failures = new List<string>();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Materials", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var catalog = GetField<FloorMaterialCatalog>(form, "_floorMaterials");
            var library = GetField<FloorTextureLibrary>(form, "_floorTextures");
            var session = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string[] original = document.Textures.ToArray();
            int dimension = document.Dimension;
            Assert.NotEmpty(catalog.Materials);
            foreach (FloorMaterial material in catalog.Materials)
            foreach (int size in new[] { 1, 5, 15 })
            {
                TerrainBlendPaintResult result = session.PaintCircle(32.5f, 32.5f, size / 2f + .26f, material.Id);
                string center = document.Textures[32 * dimension + 32];
                if (result.Succeeded)
                {
                    session.CommitStroke();
                    var unknown = result.TextureChanges.Select(change => change.After).Where(texture => library.Get(texture) is null).Distinct().ToArray();
                    if (unknown.Length > 0) failures.Add($"{material.Id} {size}: 產生素材庫沒有的 tile {string.Join(",", unknown)}");
                    if (!string.Equals(catalog.FindByTexture(center)?.Id, material.Id, StringComparison.OrdinalIgnoreCase))
                        failures.Add($"{material.Id} {size}: 中心 tile {center} 不屬於該材質");
                    session.Undo();
                }
                else
                {
                    if (result.Issues.Count == 0) failures.Add($"{material.Id} {size}: 拒絕但沒有列出問題");
                    session.CommitStroke();
                }
                if (!document.Textures.SequenceEqual(original)) failures.Add($"{material.Id} {size}: 復原／回滾後地表與原圖不同");
                lines.Add($"{material.Id}\t{material.Category}\t{material.DisplayName}\t{size}\t{(result.Succeeded ? "ok" : "rejected")}\t{result.TextureChanges.Count}\t{result.Issues.Count}\t{center}");
            }
        }, TimeSpan.FromMinutes(10));
        File.WriteAllLines(report, lines);
        Assert.True(failures.Count == 0, string.Join("\n", failures.Take(40)));
    }

    /// <summary>
    /// 材質配對矩陣：以真實素材庫把整張圖鋪成底材質 A，再以 5 格筆刷塗 B；記錄每一組 A→B 是否有原版過渡 tile 可用。
    /// 只輸出報表並檢查成功／拒絕都乾淨；未設定 ARM_GAME_PATH 時直接返回。
    /// </summary>
    [Fact]
    public void Real_floor_material_pair_matrix_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        string map = CopyRealMap(game, "ENDL_000");
        string report = Environment.GetEnvironmentVariable("ARM_MATERIAL_MATRIX") ?? Path.Combine(Path.GetTempPath(), "ArmMaterialMatrix.tsv");
        var lines = new List<string>(); var failures = new List<string>();
        RunInSta(() =>
        {
            string[] ids;
            using (var probe = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Probe", "Test")))
            {
                _ = probe.Handle; Invoke(probe, "LoadSelectedMap");
                ids = GetField<FloorMaterialCatalog>(probe, "_floorMaterials").Materials.Select(item => item.Id).ToArray();
            }
            lines.Add("base/paint\t" + string.Join("\t", ids));
            var representative = new Dictionary<string, string>();
            using (var probe2 = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Probe", "Test")))
            {
                _ = probe2.Handle; Invoke(probe2, "LoadSelectedMap");
                foreach (var item in GetField<FloorMaterialCatalog>(probe2, "_floorMaterials").Materials) representative[item.Id] = item.RepresentativeTexture;
            }
            string boden = Path.Combine(map, "boden.txt");
            byte[] originalBoden = File.ReadAllBytes(boden);
            foreach (string baseId in ids)
            {
                // 直接把 boden.txt 寫成單一底材質，避免「鋪底」本身受過渡限制。
                File.WriteAllBytes(boden, originalBoden);
                var uniformDocument = BodenTexturesDocument.Load(boden);
                uniformDocument.SetTextures(Enumerable.Repeat(representative[baseId], 64 * 64).ToArray());
                uniformDocument.Save();
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Matrix", "Test"));
                _ = form.Handle; Invoke(form, "LoadSelectedMap");
                var catalog = GetField<FloorMaterialCatalog>(form, "_floorMaterials");
                var session = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
                var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
                string center = document.Textures[32 * 64 + 32];
                bool uniform = string.Equals(catalog.FindByTexture(center)?.Id, baseId, StringComparison.OrdinalIgnoreCase);
                string[] before = document.Textures.ToArray();
                var row = new List<string> { baseId + (uniform ? "" : "?") };
                foreach (string paintId in ids)
                {
                    if (paintId == baseId) { row.Add("-"); continue; }
                    TerrainBlendPaintResult result = session.PaintCircle(32.5f, 32.5f, 2.76f, paintId);
                    session.CommitStroke();
                    if (result.Succeeded) session.Undo();
                    else if (result.Issues.Count == 0) failures.Add($"{baseId}->{paintId}: 拒絕但沒有問題清單");
                    if (!document.Textures.SequenceEqual(before)) failures.Add($"{baseId}->{paintId}: 未完整復原");
                    row.Add(result.Succeeded ? "ok" : "x");
                }
                lines.Add(string.Join("\t", row));
            }
        }, TimeSpan.FromMinutes(20));
        File.WriteAllLines(report, lines);
        Assert.True(failures.Count == 0, string.Join("\n", failures.Take(40)));
    }

    /// <summary>真實地圖隨機位置塗常用材質的成功率（1 與 5 格），報表輸出到 ARM_MATERIAL_SPOTS。</summary>
    [Fact]
    public void Real_map_random_spot_paint_acceptance_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        string sourceMap = Environment.GetEnvironmentVariable("ARM_MATERIAL_SOURCE_MAP") ?? "ENDL_000";
        string map = CopyRealMap(game, sourceMap);
        string report = Environment.GetEnvironmentVariable("ARM_MATERIAL_SPOTS") ?? Path.Combine(Path.GetTempPath(), "ArmMaterialSpots.tsv");
        var lines = new List<string> { "material\tsize\tmode\tok\trejected" };
        var failures = new List<string>();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Spots", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var catalog = GetField<FloorMaterialCatalog>(form, "_floorMaterials");
            var session = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            // 原圖最常見的五種材質最能代表實際使用。
            string[] common = document.Textures.Select(texture => catalog.FindByTexture(texture)?.Id).OfType<string>()
                .GroupBy(id => id).OrderByDescending(group => group.Count()).Take(5).Select(group => group.Key).ToArray();
            int understood = document.Textures.Count(texture => catalog.TryResolveNativeCorners(texture, out _));
            lines.Add($"coverage\t{sourceMap}\t{understood}\t{document.Textures.Count}");
            var random = new Random(1234);
            var spots = Enumerable.Range(0, 200).Select(_ => (X: random.Next(2, 62), Y: random.Next(2, 62))).ToArray();
            var library = GetField<FloorTextureLibrary>(form, "_floorTextures");
            string[] original = document.Textures.ToArray();
            foreach (string id in common)
            foreach (int size in new[] { 1, 5 })
            foreach (bool bridge in new[] { false, true })
            {
                int ok = 0, rejected = 0;
                foreach (var (x, y) in spots)
                {
                    TerrainBlendPaintResult result = session.PaintCircle(x + .5f, y + .5f, size / 2f + .26f, id, autoBridge: bridge);
                    session.CommitStroke();
                    if (result.Succeeded)
                    {
                        ok++;
                        if (result.TextureChanges.Any(change => library.Get(change.After) is null)) failures.Add($"{id} {size} {bridge}: 產生素材庫沒有的 tile");
                        session.Undo();
                    }
                    else rejected++;
                    if (!document.Textures.SequenceEqual(original)) { failures.Add($"{id} {size} {bridge} ({x},{y}): 復原後地表不同"); break; }
                }
                lines.Add($"{id}\t{size}\t{(bridge ? "bridge" : "direct")}\t{ok}\t{rejected}");
            }
        }, TimeSpan.FromMinutes(10));
        File.WriteAllLines(report, lines);
        Assert.True(failures.Count == 0, string.Join("\n", failures.Take(20)));
    }

    /// <summary>
    /// 原版地圖證據：統計相鄰兩格（上下左右）都是基礎 tile 但材質不同的「硬邊」次數，以及每種材質出現在硬邊的次數。
    /// 用來判斷原版是否直接以硬邊相接（編輯器目前一律拒絕）。報表輸出到 ARM_MATERIAL_EDGES。
    /// </summary>
    [Fact]
    public void Original_maps_hard_material_edge_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        string map = CopyRealMap(game, "ENDL_000");
        string report = Environment.GetEnvironmentVariable("ARM_MATERIAL_EDGES") ?? Path.Combine(Path.GetTempPath(), "ArmMaterialEdges.tsv");
        var lines = new List<string> { "map\tbaseTiles\ttransitionTiles\tbaseAdjacencies\thardEdges\thardEdgePairs" };
        var pairTotals = new Dictionary<string, int>();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Edges", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            var catalog = GetField<FloorMaterialCatalog>(form, "_floorMaterials");
            foreach (string directory in Directory.GetDirectories(Path.Combine(game, "MAPS")).Order())
            {
                string path = Path.Combine(directory, "boden.txt");
                if (!File.Exists(path) || File.Exists(Path.Combine(directory, CustomMapManifest.MarkerFileName))) continue;
                var document = BodenTexturesDocument.Load(path);
                int dimension = document.Dimension; var textures = document.Textures;
                string? Base(int x, int y)
                {
                    string texture = textures[y * dimension + x];
                    return texture.StartsWith("4B", StringComparison.OrdinalIgnoreCase) ? catalog.FindByTexture(texture)?.Id ?? "?" + texture : null;
                }
                int baseTiles = 0, transitions = 0, adjacencies = 0, hard = 0; var pairs = new Dictionary<string, int>();
                for (int y = 0; y < dimension; y++)
                for (int x = 0; x < dimension; x++)
                {
                    string? here = Base(x, y);
                    if (here is null) { transitions++; continue; }
                    baseTiles++;
                    foreach ((int nx, int ny) in new[] { (x + 1, y), (x, y + 1) })
                    {
                        if (nx >= dimension || ny >= dimension) continue;
                        string? there = Base(nx, ny);
                        if (there is null) continue;
                        adjacencies++;
                        if (string.Equals(here, there, StringComparison.OrdinalIgnoreCase)) continue;
                        hard++;
                        string key = string.CompareOrdinal(here, there) < 0 ? here + "|" + there : there + "|" + here;
                        pairs[key] = pairs.GetValueOrDefault(key) + 1; pairTotals[key] = pairTotals.GetValueOrDefault(key) + 1;
                    }
                }
                lines.Add($"{Path.GetFileName(directory)}\t{baseTiles}\t{transitions}\t{adjacencies}\t{hard}\t{string.Join(",", pairs.OrderByDescending(item => item.Value).Take(6).Select(item => item.Key + "=" + item.Value))}");
            }
        }, TimeSpan.FromMinutes(5));
        lines.Add("TOTAL\t\t\t\t" + pairTotals.Values.Sum() + "\t" + string.Join(",", pairTotals.OrderByDescending(item => item.Value).Take(30).Select(item => item.Key + "=" + item.Value)));
        File.WriteAllLines(report, lines);
    }

    /// <summary>原版地圖貼圖名稱前兩字元（系列）統計與範例，報表輸出到 ARM_TEXTURE_FAMILIES。</summary>
    [Fact]
    public void Original_maps_texture_family_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game)) return;
        var families = new Dictionary<string, (int Count, HashSet<string> Maps, SortedSet<string> Samples)>();
        foreach (string directory in Directory.GetDirectories(Path.Combine(game, "MAPS")).Order())
        {
            string path = Path.Combine(directory, "boden.txt");
            if (!File.Exists(path) || File.Exists(Path.Combine(directory, CustomMapManifest.MarkerFileName))) continue;
            foreach (string texture in BodenTexturesDocument.Load(path).Textures)
            {
                string family = texture.Length >= 2 ? texture[..2].ToUpperInvariant() : texture;
                if (!families.TryGetValue(family, out var entry)) families[family] = entry = (0, new HashSet<string>(), new SortedSet<string>(StringComparer.OrdinalIgnoreCase));
                entry.Maps.Add(Path.GetFileName(directory));
                if (entry.Samples.Count < 12) entry.Samples.Add(texture);
                families[family] = (entry.Count + 1, entry.Maps, entry.Samples);
            }
        }
        string report = Environment.GetEnvironmentVariable("ARM_TEXTURE_FAMILIES") ?? Path.Combine(Path.GetTempPath(), "ArmTextureFamilies.tsv");
        File.WriteAllLines(report, families.OrderByDescending(item => item.Value.Count)
            .Select(item => $"{item.Key}\t{item.Value.Count}\t{item.Value.Maps.Count}\t{string.Join(",", item.Value.Samples)}"));
    }

    /// <summary>L 系列過渡推斷報表：每個過渡編號推得的材質配對與平均角落色差，輸出到 ARM_REGIONAL_FITS。</summary>
    [Fact]
    public void Regional_transition_fit_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        using var library = new FloorTextureLibrary(Path.Combine(game, "floortex.dat"));
        var catalog = new FloorMaterialCatalog(library);
        var lines = new List<string> { "transition\tfirst\tsecond\tdistance" };
        lines.AddRange(catalog.RegionalTransitionFits.OrderBy(item => item.Key, StringComparer.Ordinal)
            .Select(item => $"{item.Key}\t{item.Value.First ?? "-"}\t{item.Value.Second ?? "-"}\t{item.Value.Distance:0.0}"));
        lines.Add($"regional materials\t{catalog.Materials.Count(material => material.Id.StartsWith('L'))}\tall materials\t{catalog.Materials.Count}");
        File.WriteAllLines(Environment.GetEnvironmentVariable("ARM_REGIONAL_FITS") ?? Path.Combine(Path.GetTempPath(), "ArmRegionalFits.tsv"), lines);
        Assert.Contains(catalog.RegionalTransitionFits.Values, fit => fit.First is not null);
    }

    /// <summary>真實地圖調色盤排序報表：每張無盡地圖各等級（已使用／可直接銜接／可自動過渡／難以銜接）的材質，輸出到 ARM_PALETTE_REPORT。</summary>
    [Fact]
    public void Real_map_palette_suitability_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        using var library = new FloorTextureLibrary(Path.Combine(game, "floortex.dat"));
        var catalog = new FloorMaterialCatalog(library);
        var lines = new List<string>();
        foreach (string mapId in new[] { "ENDL_000", "ENDL_001", "ENDL_002", "ENDL_003", "ENDL_004" })
        {
            var textures = BodenTexturesDocument.Load(Path.Combine(game, "MAPS", mapId, "boden.txt")).Textures;
            var used = textures.SelectMany(texture => catalog.TryResolveNativeCorners(texture, out var corners) ? corners : Array.Empty<string>())
                .ToHashSet(StringComparer.OrdinalIgnoreCase);
            var tiers = catalog.Materials.GroupBy(material => catalog.MapSuitability(material.Id, used)).OrderBy(group => group.Key);
            lines.Add($"{mapId}: " + string.Join(" | ", tiers.Select(group => $"T{group.Key}({group.Count()}): {string.Join(",", group.Select(material => material.Id))}")));
        }
        File.WriteAllLines(Environment.GetEnvironmentVariable("ARM_PALETTE_REPORT") ?? Path.Combine(Path.GetTempPath(), "ArmPaletteReport.txt"), lines);
    }

    /// <summary>真實地圖的地景地區判斷報表（自然物件清單依此篩選），輸出到 ARM_REGION_REPORT。</summary>
    [Fact]
    public void Real_map_nature_region_report()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game) || !File.Exists(Path.Combine(game, "floortex.dat"))) return;
        var lines = new List<string>();
        foreach (string mapId in new[] { "ENDL_000", "ENDL_001", "ENDL_002", "ENDL_003", "ENDL_004" })
        {
            string map = CopyRealMap(game, mapId);
            RunInSta(() =>
            {
                using var form = new MapEditorForm(game, new GameMapInfo(mapId, Path.Combine(game, "MAPS", mapId), false, mapId, "Test"));
                _ = form.Handle;
                Invoke(form, "LoadSelectedMap");
                var regions = (IReadOnlySet<string>)Invoke(form, "MapNatureRegions")!;
                lines.Add($"{mapId}: {string.Join(",", regions.Order())}");
            });
        }
        File.WriteAllLines(Environment.GetEnvironmentVariable("ARM_REGION_REPORT") ?? Path.Combine(Path.GetTempPath(), "ArmRegionReport.txt"), lines);
    }

    private string CopyRealMap(string game, string mapId)
    {
        string source = Path.Combine(game, "MAPS", mapId), map = Path.Combine(_root, "MAPS", "ENDL_005");
        foreach (string file in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
        {
            string target = Path.Combine(map, Path.GetRelativePath(source, file));
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            File.Copy(file, target, overwrite: true);
        }
        File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        string floortex = Path.Combine(_root, "floortex.dat");
        if (!File.Exists(floortex)) File.Copy(Path.Combine(game, "floortex.dat"), floortex);
        return map;
    }
}
