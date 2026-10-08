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
    int ExtractedFileCount)
{
    public MapPreflightReport? PreflightReport { get; init; }
}

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
        MapBundleContract.RejectLinks(mapDirectory);

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

        // One authoritative list for both the archive and its manifest; never export the local marker.
        manifest.Files = manifest.Files.Where(f => MapBundleContract.IsPayloadPath(f.RelativePath) &&
            (!options.CleanCaches || preflight.FilesToPackage.Contains(f.RelativePath))).ToList();
        if (!manifest.Validate(out var errors)) throw new InvalidDataException(string.Join("; ", errors));

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
                    writer.WriteLine("1. 先關閉遊戲，使用 tools/ArmMapPackage 安裝 CLI（詳見 docs/map-package-install.md）。");
                    writer.WriteLine("2. map/ 是相對於槽位的內容；不可直接解壓到任意 ENDL_XXX。安裝器會選連續空位、重寫 SDL 並建立 marker。");
                }

                // 寫入地圖主體檔案 (純淨白名單)
                var filesToInclude = manifest.Files.Select(f => f.RelativePath);

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

        // Schema 1.1 is deliberately strict: legacy bundles must be re-exported.
        var entries = new Dictionary<string, ZipArchiveEntry>(StringComparer.OrdinalIgnoreCase);
        foreach (var entry in archive.Entries)
        {
            if (!entries.TryAdd(entry.FullName, entry)) throw new InvalidDataException("Duplicate ZIP entry.");
            if (entry.FullName is "manifest.json" or "README.txt" or "thumbnail.bmp") continue;
            if (!entry.FullName.StartsWith(MapSubdirectoryInZip, StringComparison.Ordinal) ||
                !MapBundleContract.IsPayloadPath(entry.FullName[MapSubdirectoryInZip.Length..]))
                throw new InvalidDataException("Unsupported ZIP path: " + entry.FullName);
        }
        if (!entries.TryGetValue(MapPackageManifest.ManifestFileName, out var manifestEntry) || manifestEntry.Length > 1024 * 1024)
            throw new InvalidDataException("Missing or oversized manifest.json.");
        MapPackageManifest manifest;
        using (var reader = new StreamReader(manifestEntry.Open(), Encoding.UTF8))
            manifest = MapPackageManifest.FromJson(reader.ReadToEnd());
        if (manifest.Compatibility is null || manifest.Files is null || manifest.Players is null || manifest.Dimensions is null ||
            !manifest.Validate(out _) || manifest.SchemaVersion != MapPackageManifest.CurrentSchemaVersion ||
            manifest.PayloadLayout != "map-relative" || manifest.SourceMapId != $"ENDL_{manifest.Compatibility.PreferredSlot:000}" ||
            manifest.PackageChecksum != manifest.ComputePackageChecksum())
            throw new InvalidDataException("Invalid or unsupported package manifest; re-export with schema 1.1.");
        var files = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        long totalSize = 0;
        foreach (var file in manifest.Files)
        {
            if (!MapBundleContract.IsPayloadPath(file.RelativePath) || !files.Add(file.RelativePath) ||
                !entries.TryGetValue(MapSubdirectoryInZip + file.RelativePath, out var entry) ||
                file.SizeBytes != entry.Length || file.SizeBytes < 0 || file.SizeBytes > 128 * 1024 * 1024)
                throw new InvalidDataException("Invalid payload inventory.");
            totalSize += file.SizeBytes;
        }
        if (totalSize > 512L * 1024 * 1024 || entries.Keys.Count(k => k.StartsWith(MapSubdirectoryInZip, StringComparison.Ordinal)) != files.Count)
            throw new InvalidDataException("Payload inventory mismatch or size limit exceeded.");

        int sourceSlot = manifest.Compatibility.PreferredSlot;
        if (policy.TargetSlot is < MinCustomSlot)
            throw new InvalidOperationException("目標槽位屬於原廠官方地圖，禁止覆蓋！");
        if (policy.TargetSlot is > MaxCustomSlot) throw new ArgumentOutOfRangeException(nameof(policy));
        if (policy.CollisionStrategy == SlotCollisionStrategy.OverwriteCustom)
            throw new NotSupportedException("Install never overwrites maps. Install into the first free slot instead.");
        MapBundleContract.RejectLinks(mapsRoot);
        int nextFree = AllocateNextFreeSlot(normalizedGamePath);
        int targetSlot = policy.CollisionStrategy == SlotCollisionStrategy.AutoAllocateNextFree
            ? nextFree : policy.TargetSlot ?? nextFree;
        if (targetSlot != nextFree) throw new IOException("Target must be the first contiguous free slot: " + nextFree);
        if (targetSlot != sourceSlot && !manifest.Compatibility.AllowDynamicSlotRemapping)
            throw new InvalidOperationException("Package disallows slot remapping.");
        string targetDir = Path.Combine(mapsRoot, $"ENDL_{targetSlot:000}");
        if (File.Exists(targetDir) || Directory.Exists(targetDir)) throw new IOException("Slot is occupied.");
        // Parse the registry before publishing anything. FileRollbackScope protects its update.
        CustomMapManifest customManifest = CustomMapManifest.Load(normalizedGamePath);
        using var rollback = new FileRollbackScope();
        string tempStaging = Path.Combine(mapsRoot, ".arm_install_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempStaging);
        bool moved = false;
        try
        {
            foreach (var file in manifest.Files)
            {
                string destFile = Path.Combine(tempStaging, file.RelativePath);
                Directory.CreateDirectory(Path.GetDirectoryName(destFile)!);
                entries[MapSubdirectoryInZip + file.RelativePath].ExtractToFile(destFile, overwrite: false);
                using var stream = File.OpenRead(destFile);
                if (!Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(stream)).Equals(file.Sha256, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("Payload checksum mismatch: " + file.RelativePath);
            }
            MapPreflightReport installedPreflight = MapExportPreflightChecker.Check(tempStaging);
            if (installedPreflight.Issues.Any(i => i.Severity == PreflightSeverity.Error && i.Category == PreflightCategory.StructuralBinary))
                throw new InvalidDataException("Installed payload failed map preflight.");
            EndlessMapCloner.RewriteKnownFiles(tempStaging, manifest.SourceMapId, $"ENDL_{targetSlot:000}", manifest.Title);
            var marker = new CustomMapEntry(targetSlot, sourceSlot, DateTimeOffset.UtcNow, "AgainstRomeModifier.Packaging")
            { StandaloneLevel = manifest.Compatibility.StandaloneLevel };
            SafeFileWriter.WriteAllBytes(Path.Combine(tempStaging, CustomMapManifest.MarkerFileName),
                JsonSerializer.SerializeToUtf8Bytes(marker, JsonDefaults.Indented));
            // Move cannot replace an occupied slot; a concurrent installer safely fails here.
            Directory.Move(tempStaging, targetDir);
            moved = true;
            customManifest.Register(marker);
            customManifest.Save(normalizedGamePath, rollback);
            rollback.Commit();
            return new ModInstallResult(targetSlot, targetDir, targetSlot == sourceSlot ? null : sourceSlot, manifest, files.Count) { PreflightReport = installedPreflight };
        }
        catch
        {
            if (moved && Directory.Exists(targetDir)) Directory.Delete(targetDir, recursive: true);
            throw;
        }
        finally
        {
            if (Directory.Exists(tempStaging)) Directory.Delete(tempStaging, recursive: true);
        }
    }

    /// <summary>The first gap from ENDL_005, shared with the editor catalog.</summary>
    public static int AllocateNextFreeSlot(string gamePath) => new EndlessMapCatalog().GetNextFreeSlot(gamePath);
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
