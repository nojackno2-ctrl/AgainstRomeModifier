using System.IO.Compression;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Packaging;

/// <summary>匯出選項組態。</summary>
public sealed record ModExportOptions
{
    public string PackageId { get; init; } = string.Empty;
    public string? Author { get; init; }
    public string? Version { get; init; }
    public bool CleanCaches { get; init; } = true;
    public bool ForceExportOnErrors { get; init; }
    public bool GenerateThumbnail { get; init; } = true;
    public int ThumbnailResolution { get; init; } = 512;
    public CompressionLevel CompressionLevel { get; init; } = CompressionLevel.Optimal;
}

/// <summary>匯出成果資料模型。</summary>
public sealed record ModExportResult(
    bool Success,
    string OutputZipPath,
    int FileCount,
    long TotalSizeBytes,
    MapPackageManifest Manifest,
    MapPreflightReport PreflightReport);

/// <summary>槽位衝突處理解決策略。</summary>
public enum SlotCollisionStrategy
{
    AutoAllocateNextFree, // 自動尋找下一個未被佔用之自訂槽位 (ENDL_005..999) 並動態重組
    FailIfOccupied,       // 若目標槽位已被佔用則中止報錯
    OverwriteCustom       // 僅允許覆蓋自訂地圖（原廠 0..4 仍嚴格禁止覆蓋）
}

/// <summary>安裝策略。</summary>
public sealed record ModInstallPolicy(
    int? TargetSlot = null,
    SlotCollisionStrategy CollisionStrategy = SlotCollisionStrategy.AutoAllocateNextFree);

/// <summary>安裝成果資料模型。</summary>
public sealed record ModInstallResult(
    int InstalledSlot,
    string DirectoryPath,
    int? RemappedFromSlot,
    MapPackageManifest Manifest,
    int ExtractedFileCount);

/// <summary>
/// 模組發布封裝與匯出管線核心 (ModBundleExporter)。
/// 支援一鍵匯出為標準 ZIP 分發包、嵌入高畫質預覽縮圖與元資料 Manifest、
/// 並提供防覆蓋原廠地圖槽位保護演算法與動態槽位重映射機制 (Dynamic Slot Remapper)。
/// </summary>
public static class ModBundleExporter
{
    public const int MinCustomSlot = 5;
    public const int MaxCustomSlot = 999;
    public const string MapSubdirectoryInZip = "map/";

    /// <summary>
    /// 一鍵打包匯出地圖為乾淨相容的獨立 ZIP 壓縮包。
    /// </summary>
    public static ModExportResult ExportToZip(
        string mapDirectory,
        string outputZipPath,
        ModExportOptions? options = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(mapDirectory);
        ArgumentException.ThrowIfNullOrWhiteSpace(outputZipPath);

        options ??= new ModExportOptions();

        // 步驟 1：執行發布前預檢驗收
        MapPreflightReport preflight = MapExportPreflightChecker.Check(mapDirectory);
        if (!preflight.CanExport && !options.ForceExportOnErrors)
        {
            var firstError = preflight.Issues.First(i => i.Severity == PreflightSeverity.Error);
            throw new InvalidOperationException($"地圖未通過發布預檢: [{firstError.Code}] {firstError.Chinese}");
        }

        // 步驟 2：建立並填寫 Manifest
        string pkgId = string.IsNullOrWhiteSpace(options.PackageId)
            ? Path.GetFileName(mapDirectory).ToLowerInvariant()
            : options.PackageId;

        MapPackageManifest manifest = MapPackageManifest.CreateFromDirectory(
            mapDirectory,
            packageId: pkgId,
            author: options.Author,
            version: options.Version);

        // 步驟 3：產生高質感發布縮圖
        byte[]? thumbnailBmp = null;
        if (options.GenerateThumbnail)
        {
            thumbnailBmp = TryGenerateThumbnail(mapDirectory, options.ThumbnailResolution, manifest);
        }

        // 步驟 4：建立暫存檔案並執行 ZIP 打包
        string outputDir = Path.GetDirectoryName(Path.GetFullPath(outputZipPath))!;
        Directory.CreateDirectory(outputDir);

        string tempZip = outputZipPath + ".tmp_" + Guid.NewGuid().ToString("N");
        int fileCount = 0;
        long totalBytes = 0;

        try
        {
            using (var zipStream = new FileStream(tempZip, FileMode.Create, FileAccess.Write, FileShare.None))
            using (var archive = new ZipArchive(zipStream, ZipArchiveMode.Create, leaveOpen: false))
            {
                // 寫入 thumbnail.bmp
                if (thumbnailBmp != null)
                {
                    ZipArchiveEntry thumbEntry = archive.CreateEntry("thumbnail.bmp", options.CompressionLevel);
                    using var thumbStream = thumbEntry.Open();
                    thumbStream.Write(thumbnailBmp);
                }

                // 寫入 README.txt 供玩家直接閱覽
                ZipArchiveEntry readmeEntry = archive.CreateEntry("README.txt", options.CompressionLevel);
                using (var writer = new StreamWriter(readmeEntry.Open(), Encoding.UTF8))
                {
                    writer.WriteLine($"=======================================================");
                    writer.WriteLine($" Against Rome Custom Map: {manifest.Title}");
                    writer.WriteLine($" Author: {manifest.Author} | Version: {manifest.Version}");
                    writer.WriteLine($" Preferred Slot: ENDL_{manifest.Compatibility.PreferredSlot:000}");
                    writer.WriteLine($"=======================================================");
                    writer.WriteLine();
                    writer.WriteLine(manifest.Description);
                    writer.WriteLine();
                    writer.WriteLine($"[安裝方式]");
                    writer.WriteLine($"1. 使用 Against Rome Modifier 模組管理器一鍵安裝（自動防覆蓋槽位）。");
                    writer.WriteLine($"2. 或手動將 map/ 內所有檔案複製至遊戲 MAPS/ENDL_XXX 資料夾中。");
                }

                // 寫入地圖主體檔案 (純淨白名單)
                var filesToInclude = options.CleanCaches
                    ? preflight.FilesToPackage
                    : Directory.GetFiles(mapDirectory, "*", SearchOption.AllDirectories)
                               .Select(f => Path.GetRelativePath(mapDirectory, f).Replace('\\', '/'))
                               .Where(r => !r.EndsWith(".tmp_arm", StringComparison.OrdinalIgnoreCase))
                               .ToList();

                foreach (string relative in filesToInclude)
                {
                    string sourceFilePath = Path.Combine(mapDirectory, relative);
                    if (!File.Exists(sourceFilePath)) continue;

                    string zipEntryName = MapSubdirectoryInZip + relative;
                    ZipArchiveEntry entry = archive.CreateEntry(zipEntryName, options.CompressionLevel);

                    using (var sourceStream = File.OpenRead(sourceFilePath))
                    using (var entryStream = entry.Open())
                    {
                        sourceStream.CopyTo(entryStream);
                    }

                    fileCount++;
                    totalBytes += new FileInfo(sourceFilePath).Length;
                }

                // 寫入 manifest.json
                ZipArchiveEntry manifestEntry = archive.CreateEntry(MapPackageManifest.ManifestFileName, options.CompressionLevel);
                using (var manifestStream = manifestEntry.Open())
                {
                    byte[] manifestBytes = Encoding.UTF8.GetBytes(manifest.ToJson());
                    manifestStream.Write(manifestBytes);
                }
            }

            if (File.Exists(outputZipPath)) File.Delete(outputZipPath);
            File.Move(tempZip, outputZipPath);

            return new ModExportResult(
                Success: true,
                OutputZipPath: outputZipPath,
                FileCount: fileCount,
                TotalSizeBytes: totalBytes,
                Manifest: manifest,
                PreflightReport: preflight);
        }
        finally
        {
            if (File.Exists(tempZip)) File.Delete(tempZip);
        }
    }

    /// <summary>
    /// 從 ZIP 模組包安裝自訂地圖，實作原生槽位絕對保護與動態槽位重映射 (Slot Protection &amp; Remapping)。
    /// </summary>
    public static ModInstallResult InstallFromZip(
        string zipPath,
        string gamePath,
        ModInstallPolicy? policy = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(zipPath);
        ArgumentException.ThrowIfNullOrWhiteSpace(gamePath);
        if (!File.Exists(zipPath)) throw new FileNotFoundException("找不到模組 ZIP 檔案: " + zipPath);

        string normalizedGamePath = EndlessMapCatalog.ValidateGamePath(gamePath);
        string mapsRoot = Path.Combine(normalizedGamePath, "MAPS");
        Directory.CreateDirectory(mapsRoot);

        policy ??= new ModInstallPolicy();

        using var zipStream = File.OpenRead(zipPath);
        using var archive = new ZipArchive(zipStream, ZipArchiveMode.Read);

        // 讀取 manifest.json
        ZipArchiveEntry? manifestEntry = archive.GetEntry(MapPackageManifest.ManifestFileName);
        MapPackageManifest manifest;
        if (manifestEntry != null)
        {
            using var reader = new StreamReader(manifestEntry.Open(), Encoding.UTF8);
            manifest = MapPackageManifest.FromJson(reader.ReadToEnd());
        }
        else
        {
            manifest = new MapPackageManifest
            {
                PackageId = Path.GetFileNameWithoutExtension(zipPath).ToLowerInvariant(),
                Title = Path.GetFileNameWithoutExtension(zipPath)
            };
        }

        int requestedSlot = policy.TargetSlot ?? manifest.Compatibility.PreferredSlot;

        // 核心保護規則：嚴格禁止覆蓋原廠槽位 0..4
        if (requestedSlot < MinCustomSlot)
        {
            throw new InvalidOperationException($"目標槽位 ENDL_{requestedSlot:000} 屬於原廠官方地圖 (ENDL_000 - ENDL_004)，系統嚴格保護，禁止覆蓋！");
        }

        // 槽位衝突檢測與分配策略
        int sourceSlot = manifest.Compatibility.PreferredSlot;
        int targetSlot = requestedSlot;
        string targetDir = Path.Combine(mapsRoot, $"ENDL_{targetSlot:000}");

        if (Directory.Exists(targetDir))
        {
            switch (policy.CollisionStrategy)
            {
                case SlotCollisionStrategy.FailIfOccupied:
                    throw new IOException($"目標地圖槽位 ENDL_{targetSlot:000} 已被佔用。");

                case SlotCollisionStrategy.OverwriteCustom:
                    if (!CustomMapManifest.IsCustomMapDirectory(targetDir))
                    {
                        throw new InvalidOperationException($"目錄 ENDL_{targetSlot:000} 不是自訂地圖，禁止覆蓋原生地圖！");
                    }
                    break;

                case SlotCollisionStrategy.AutoAllocateNextFree:
                default:
                    targetSlot = AllocateNextFreeSlot(normalizedGamePath);
                    targetDir = Path.Combine(mapsRoot, $"ENDL_{targetSlot:000}");
                    break;
            }
        }

        // 解壓縮與動態重映射 (使用交易回滾保障)
        using var rollback = new FileRollbackScope();
        string tempStaging = targetDir + ".tmp_arm_" + Guid.NewGuid().ToString("N");
        Directory.CreateDirectory(tempStaging);

        int extractedFiles = 0;
        try
        {
            foreach (ZipArchiveEntry entry in archive.Entries)
            {
                if (entry.FullName.EndsWith('/') || entry.FullName.EndsWith('\\')) continue;
                if (!entry.FullName.StartsWith(MapSubdirectoryInZip, StringComparison.OrdinalIgnoreCase)) continue;

                string relativeInMap = entry.FullName[MapSubdirectoryInZip.Length..];
                string destFile = Path.Combine(tempStaging, relativeInMap);
                Directory.CreateDirectory(Path.GetDirectoryName(destFile)!);

                entry.ExtractToFile(destFile, overwrite: true);
                extractedFiles++;
            }

            // 動態槽位重映射：若目標槽位與來源槽位不同，重寫 SDL 內部路徑
            int? remappedFrom = null;
            if (targetSlot != sourceSlot)
            {
                remappedFrom = sourceSlot;
                RemapInternalSdlPaths(tempStaging, sourceSlot, targetSlot);
            }

            // 寫入/更新 .arm_custom_map 標記
            var marker = new CustomMapEntry(
                Slot: targetSlot,
                SourceSlot: sourceSlot,
                CreatedAt: DateTimeOffset.UtcNow,
                ToolVersion: "AgainstRomeModifier.Packaging")
            {
                StandaloneLevel = manifest.Compatibility.StandaloneLevel
            };

            string markerPath = Path.Combine(tempStaging, CustomMapManifest.MarkerFileName);
            SafeFileWriter.WriteAllBytes(markerPath, JsonSerializer.SerializeToUtf8Bytes(marker, JsonDefaults.Indented), rollback);

            // 若目標資料夾已存在（例如 OverwriteCustom），先移除舊目錄
            if (Directory.Exists(targetDir))
            {
                rollback.TrackDirectory(targetDir);
                Directory.Delete(targetDir, recursive: true);
            }

            Directory.Move(tempStaging, targetDir);

            // 登記至 arm_custom_maps.json 清單
            CustomMapManifest customManifest = CustomMapManifest.Load(normalizedGamePath);
            customManifest.Register(marker);
            customManifest.Save(normalizedGamePath, rollback);

            rollback.Commit();

            return new ModInstallResult(
                InstalledSlot: targetSlot,
                DirectoryPath: targetDir,
                RemappedFromSlot: remappedFrom,
                Manifest: manifest,
                ExtractedFileCount: extractedFiles);
        }
        catch
        {
            if (Directory.Exists(tempStaging))
            {
                try { Directory.Delete(tempStaging, recursive: true); } catch { }
            }
            throw;
        }
    }

    /// <summary>尋找大於等於 5 且尚未被佔用的最小槽位。</summary>
    public static int AllocateNextFreeSlot(string gamePath)
    {
        string mapsRoot = Path.Combine(NormalizeGamePath(gamePath), "MAPS");
        var occupied = new HashSet<int>();

        if (Directory.Exists(mapsRoot))
        {
            foreach (string dir in Directory.GetDirectories(mapsRoot))
            {
                string name = Path.GetFileName(dir);
                if (name.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase) &&
                    int.TryParse(name[5..], System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out int s))
                {
                    occupied.Add(s);
                }
            }
        }

        for (int slot = MinCustomSlot; slot <= MaxCustomSlot; slot++)
        {
            if (!occupied.Contains(slot)) return slot;
        }

        throw new InvalidOperationException($"沒有可用的自訂地圖槽位 (ENDL_{MinCustomSlot:000} - ENDL_{MaxCustomSlot:000})。");
    }

    private static string NormalizeGamePath(string gamePath)
    {
        if (string.IsNullOrWhiteSpace(gamePath)) throw new ArgumentException("未提供遊戲路徑。", nameof(gamePath));
        string fullPath = Path.GetFullPath(gamePath);
        if (!Directory.Exists(fullPath)) throw new DirectoryNotFoundException("找不到遊戲路徑: " + fullPath);
        return fullPath;
    }

    private static void RemapInternalSdlPaths(string mapDirectory, int oldSlot, int newSlot)
    {
        string oldMapId = $"ENDL_{oldSlot:000}";
        string newMapId = $"ENDL_{newSlot:000}";

        foreach (string sdlPath in Directory.GetFiles(mapDirectory, "*.sdl", SearchOption.TopDirectoryOnly))
        {
            try
            {
                var sdl = SdlDocument.Load(sdlPath);
                sdl.RewriteMapPath(oldMapId, newMapId);
                sdl.Save();
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"重寫 SDL 路徑失敗 ({sdlPath}): {ex.Message}");
            }
        }
    }

    private static byte[]? TryGenerateThumbnail(string mapDirectory, int resolution, MapPackageManifest manifest)
    {
        try
        {
            string bodenPath = Path.Combine(mapDirectory, "boden.bmp");
            if (!File.Exists(bodenPath)) return null;

            byte[]? heights = ReadBmpGreenChannel(bodenPath, 257, 257);
            if (heights == null) return null;

            var options = new ThumbnailRenderOptions
            {
                Width = resolution,
                Height = resolution,
                WaterLevel = manifest.Dimensions.WaterLevel,
                HeightStep = manifest.Dimensions.HeightStep,
                ShowWater = true,
                ShowSpawnBanners = true
            };

            // 讀取可能的出發點
            var spawns = new List<MapThumbnailSpawnPoint>();
            string scenarioPath = Path.Combine(mapDirectory, "arm_scenario.json");
            if (File.Exists(scenarioPath))
            {
                try
                {
                    var scenario = ScenarioDocument.Load(mapDirectory);
                    foreach (var s in scenario.Spawns.Where(sp => sp.Team >= 0))
                    {
                        spawns.Add(new MapThumbnailSpawnPoint(s.X, s.Z, s.Team, s.Alias, s.Team == 0));
                    }
                }
                catch { }
            }

            return MinimapThumbnailRenderer.RenderToBmp(
                heightGrid: heights,
                heightSize: 257,
                options: options,
                spawnPoints: spawns);
        }
        catch
        {
            return null;
        }
    }

    private static byte[]? ReadBmpGreenChannel(string path, int expectedWidth, int expectedHeight)
    {
        try
        {
            using var fs = File.OpenRead(path);
            if (fs.Length < 54) return null;
            byte[] header = new byte[54];
            fs.ReadExactly(header);

            int width = BitConverter.ToInt32(header.AsSpan(18, 4));
            int height = BitConverter.ToInt32(header.AsSpan(22, 4));
            if (width != expectedWidth || height != expectedHeight) return null;

            int stride = (width * 3 + 3) & ~3;
            byte[] green = new byte[width * height];
            byte[] row = new byte[stride];

            for (int y = 0; y < height; y++)
            {
                int rowIdx = height - 1 - y;
                fs.Seek(54 + (long)rowIdx * stride, SeekOrigin.Begin);
                fs.ReadExactly(row);

                for (int x = 0; x < width; x++)
                {
                    green[y * width + x] = row[x * 3 + 1];
                }
            }

            return green;
        }
        catch
        {
            return null;
        }
    }
}
