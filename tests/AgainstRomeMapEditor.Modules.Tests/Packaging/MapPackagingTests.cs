using System.IO.Compression;
using System.Text;
using AgainstRomeMapEditor.Modules.Packaging;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Packaging;

public sealed class MapPackagingTests : IDisposable
{
    private readonly string _tempDir;

    public MapPackagingTests()
    {
        _tempDir = Path.Combine(Path.GetTempPath(), "ArmPackagingTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_tempDir);
    }

    public void Dispose()
    {
        if (Directory.Exists(_tempDir))
        {
            try { Directory.Delete(_tempDir, true); } catch { }
        }
    }

    [Fact]
    public void Manifest_RoundTripSerialization_PreservesAllMetadataAndChecksum()
    {
        var manifest = new MapPackageManifest
        {
            PackageId = "mod_teutoburg_forest",
            Title = "條頓堡之圍",
            Subtitle = "羅馬第十九軍團的殞落",
            Author = "TeutonModder",
            Version = "1.2.0",
            Description = "在深邃的條頓堡日耳曼密林中伏擊瓦盧斯的大軍。",
            Dimensions = new MapPackageDimensions(GridWidth: 256, GridHeight: 256, WaterLevel: 75.5f),
            Players = new MapRecommendedPlayers(MinPlayers: 1, MaxPlayers: 4, OptimalPlayers: 2),
            GameModes = ["Endless", "Survival"],
            Tags = ["Forest", "Ambush", "Historical"],
            Compatibility = new MapCompatibilityPolicy(PreferredSlot: 8, StandaloneLevel: true)
        };

        manifest.Localizations["US"] = new MapPackageLocalization("Battle of Teutoburg", "Fall of Varus");
        manifest.Localizations["DE"] = new MapPackageLocalization("Varusschlacht", "Teutoburger Wald");

        manifest.Files.Add(new MapPackageFileEntry("boden.bmp", 198458, "ABCDEF123456", MapFileCategory.TerrainHeight));
        manifest.Files.Add(new MapPackageFileEntry("collision.bmp", 196662, "7890ABCDEF12", MapFileCategory.Collision));

        string json = manifest.ToJson();
        Assert.NotEmpty(manifest.PackageChecksum);

        var loaded = MapPackageManifest.FromJson(json);
        Assert.Equal(manifest.PackageId, loaded.PackageId);
        Assert.Equal(manifest.Title, loaded.Title);
        Assert.Equal(manifest.Subtitle, loaded.Subtitle);
        Assert.Equal(manifest.Author, loaded.Author);
        Assert.Equal(manifest.Version, loaded.Version);
        Assert.Equal(75.5f, loaded.Dimensions.WaterLevel);
        Assert.True(loaded.Compatibility.StandaloneLevel);
        Assert.Equal(8, loaded.Compatibility.PreferredSlot);
        Assert.Equal(2, loaded.Files.Count);
        Assert.Equal(manifest.PackageChecksum, loaded.PackageChecksum);

        Assert.True(loaded.Validate(out var errors));
        Assert.Empty(errors);
    }

    [Fact]
    public void Manifest_Validation_RejectsForbiddenNativeSlots()
    {
        var manifest = new MapPackageManifest
        {
            PackageId = "illegal_mod",
            Title = "覆蓋官方地圖",
            Compatibility = new MapCompatibilityPolicy(PreferredSlot: 2) // 原廠 0..4 槽位
        };

        bool valid = manifest.Validate(out var errors);
        Assert.False(valid);
        Assert.Contains(errors, e => e.Contains("PreferredSlot (2)"));
    }

    [Fact]
    public void MinimapThumbnailRenderer_GeneratesValid24BitBmpWithReliefAndWater()
    {
        int size = 257;
        byte[] heights = new byte[size * size];

        // 模擬中心丘陵與周圍低窪湖泊
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            float dx = x - 128f;
            float dy = y - 128f;
            float dist = MathF.Sqrt(dx * dx + dy * dy);
            // 中心高，邊緣低
            float h = Math.Clamp(150f - dist, 10f, 220f);
            heights[y * size + x] = (byte)h;
        }

        var options = new ThumbnailRenderOptions
        {
            Width = 256,
            Height = 256,
            WaterLevel = 100f, // 低於 25 的高度會被浸沒
            HeightStep = 4.0f,
            ShowWater = true,
            ShowShorelines = true,
            ShowSpawnBanners = true
        };

        var spawns = new List<MapThumbnailSpawnPoint>
        {
            new(8192f, 8192f, Team: 0, Label: "P1", IsPlayer: true),
            new(3000f, 3000f, Team: 1, Label: "P2", IsPlayer: true)
        };

        byte[] bmpBytes = MinimapThumbnailRenderer.RenderToBmp(
            heightGrid: heights,
            heightSize: 257,
            options: options,
            spawnPoints: spawns);

        Assert.NotNull(bmpBytes);
        Assert.True(bmpBytes.Length > 54);
        Assert.Equal((byte)'B', bmpBytes[0]);
        Assert.Equal((byte)'M', bmpBytes[1]);

        int width = BitConverter.ToInt32(bmpBytes.AsSpan(18, 4));
        int height = BitConverter.ToInt32(bmpBytes.AsSpan(22, 4));
        short bpp = BitConverter.ToInt16(bmpBytes.AsSpan(28, 2));

        Assert.Equal(256, width);
        Assert.Equal(256, height);
        Assert.Equal((short)24, bpp);

        int stride = (256 * 3 + 3) & ~3;
        Assert.Equal(54 + stride * 256, bmpBytes.Length);
    }

    [Fact]
    public void PreflightChecker_DetectsMissingFilesAndSanitizesHeightCaches()
    {
        string mapDir = Path.Combine(_tempDir, "ENDL_005");
        Directory.CreateDirectory(mapDir);

        // 僅建立部分檔案
        File.WriteAllText(Path.Combine(mapDir, "boden.ini"), "Waterlevel = 0\n");

        var report = MapExportPreflightChecker.Check(mapDir);
        Assert.False(report.CanExport);
        Assert.True(report.ErrorCount >= 4); // 缺 boden.bmp, collision.bmp, briefing.put, objects.dat 等
        Assert.Contains(report.Issues, i => i.Code == "missing-boden");
        Assert.Contains(report.Issues, i => i.Code == "missing-collision");

        // 加入高度相依快取檔案
        File.WriteAllText(Path.Combine(mapDir, "skydens.dat"), "cache");
        File.WriteAllText(Path.Combine(mapDir, "shadows.dat"), "cache");

        var report2 = MapExportPreflightChecker.Check(mapDir);
        Assert.Contains("skydens.dat", report2.FilesToExclude);
        Assert.Contains("shadows.dat", report2.FilesToExclude);
        Assert.Contains(report2.Issues, i => i.Code == "height-cache-excluded");
    }

    [Fact]
    public void ModBundleExporter_FullExportAndSlotRemappedInstall_Succeeds()
    {
        string sourceMap = Path.Combine(_tempDir, "SourceMap_ENDL_005");
        CreateValidMockMap(sourceMap, slot: 5, mapTitle: "條頓之森");

        string zipOutput = Path.Combine(_tempDir, "TeutonForest_1.0.armpack");

        // 1. 執行匯出
        var exportOptions = new ModExportOptions
        {
            PackageId = "teuton_forest",
            Author = "ArmDev",
            Version = "1.0.0",
            GenerateThumbnail = true,
            ThumbnailResolution = 256
        };

        var exportResult = ModBundleExporter.ExportToZip(sourceMap, zipOutput, exportOptions);
        Assert.True(exportResult.Success);
        Assert.True(File.Exists(zipOutput));
        Assert.True(exportResult.FileCount > 0);

        // 驗證 ZIP 內部結構
        using (var zip = ZipFile.OpenRead(zipOutput))
        {
            Assert.NotNull(zip.GetEntry("manifest.json"));
            Assert.NotNull(zip.GetEntry("thumbnail.bmp"));
            Assert.NotNull(zip.GetEntry("README.txt"));
            Assert.NotNull(zip.GetEntry("map/boden.bmp"));
            Assert.NotNull(zip.GetEntry("map/boden.ini"));
            Assert.NotNull(zip.GetEntry("map/TEXT/US/briefing.put"));
        }

        // 2. 模擬目標遊戲目錄安裝（目標槽位 5 已被其他地圖佔用，觸發自動重映射到 6）
        string mockGamePath = Path.Combine(_tempDir, "MockGame");
        string mockMaps = Path.Combine(mockGamePath, "MAPS");
        Directory.CreateDirectory(mockMaps);

        // 佔用 ENDL_005
        string occupiedDir = Path.Combine(mockMaps, "ENDL_005");
        Directory.CreateDirectory(occupiedDir);
        File.WriteAllText(Path.Combine(occupiedDir, ".arm_custom_map"), "{\"Slot\":5}");

        var installResult = ModBundleExporter.InstallFromZip(
            zipOutput,
            mockGamePath,
            new ModInstallPolicy(TargetSlot: 5, CollisionStrategy: SlotCollisionStrategy.AutoAllocateNextFree));

        // 應自動重分配到槽位 6
        Assert.Equal(6, installResult.InstalledSlot);
        Assert.Equal(5, installResult.RemappedFromSlot);
        Assert.True(Directory.Exists(Path.Combine(mockMaps, "ENDL_006")));

        // 驗證 SDL 檔案內部路徑重寫為 ENDL_006
        string installedSdl = Path.Combine(mockMaps, "ENDL_006", "Endlos_005_Siedlung1.sdl");
        Assert.True(File.Exists(installedSdl));
        string sdlContent = File.ReadAllText(installedSdl);
        Assert.Contains("MAPS/ENDL_006/", sdlContent);
        Assert.DoesNotContain("MAPS/ENDL_005/", sdlContent);

        // 驗證 arm_custom_maps.json 已登錄
        var manifest = CustomMapManifest.Load(mockGamePath);
        Assert.Contains(manifest.Entries, e => e.Slot == 6);
    }

    [Fact]
    public void ModBundleExporter_Install_RejectsNativeSlotsStrictly()
    {
        string sourceMap = Path.Combine(_tempDir, "SourceMap_ENDL_005");
        CreateValidMockMap(sourceMap, slot: 5, mapTitle: "原廠覆蓋測試");

        string zipOutput = Path.Combine(_tempDir, "NativeAttack.zip");
        ModBundleExporter.ExportToZip(sourceMap, zipOutput, new ModExportOptions { ForceExportOnErrors = true });

        string mockGamePath = Path.Combine(_tempDir, "MockGameNative");
        Directory.CreateDirectory(Path.Combine(mockGamePath, "MAPS"));

        // 嘗試安裝至原廠官方槽位 0..4
        for (int illegalSlot = 0; illegalSlot < 5; illegalSlot++)
        {
            var ex = Assert.Throws<InvalidOperationException>(() =>
            {
                ModBundleExporter.InstallFromZip(
                    zipOutput,
                    mockGamePath,
                    new ModInstallPolicy(TargetSlot: illegalSlot, CollisionStrategy: SlotCollisionStrategy.FailIfOccupied));
            });
            Assert.Contains("原廠官方地圖", ex.Message);
        }
    }

    private static void CreateValidMockMap(string directory, int slot, string mapTitle)
    {
        Directory.CreateDirectory(directory);
        Directory.CreateDirectory(Path.Combine(directory, "TEXT", "US"));
        Directory.CreateDirectory(Path.Combine(directory, "DATA"));
        Directory.CreateDirectory(Path.Combine(directory, "SCRIPT"));

        // 1. boden.bmp (257x257 24bpp)
        WriteSolidBmp(Path.Combine(directory, "boden.bmp"), 257, 257, 128);

        // 2. collision.bmp (256x256 24bpp, 0 = 通行)
        WriteSolidBmp(Path.Combine(directory, "collision.bmp"), 256, 256, 0);

        // 3. boden.ini
        File.WriteAllText(Path.Combine(directory, "boden.ini"), "Waterlevel = 50.0\nWaterColor = 30 100 180\n");

        // 4. briefing.put
        File.WriteAllText(Path.Combine(directory, "TEXT", "US", "briefing.put"),
            $"[texts]\nbriefing_titel_1 = {mapTitle}\nbriefing_titel_2 = 示範副標題\nbriefing_text = 測試簡報內容\n");

        // 5. DATA/objects.dat, objdata.dat, pos.dat
        File.WriteAllBytes(Path.Combine(directory, "DATA", "objects.dat"), new byte[64]);
        File.WriteAllBytes(Path.Combine(directory, "DATA", "objdata.dat"), new byte[64]);
        File.WriteAllBytes(Path.Combine(directory, "DATA", "pos.dat"), new byte[64]);

        // 6. SCRIPT/ak_level.bci
        File.WriteAllBytes(Path.Combine(directory, "SCRIPT", "ak_level.bci"), new byte[] { (byte)'P', (byte)'F', (byte)'I', (byte)'L', 0, 0, 0, 0 });

        // 7. Endlos_*.sdl
        string sdlText = $"""
            [settlement]
            name = MAPS/ENDL_{slot:000}/Endlos_005_Siedlung1.sdl
            refpos = 8192, 100, 8192

            [object0]
            namedef = BauGerHau00
            pos = 0, 0, 0
            team = 0
            """;
        File.WriteAllText(Path.Combine(directory, "Endlos_005_Siedlung1.sdl"), sdlText);

        // 8. .arm_custom_map marker
        File.WriteAllText(Path.Combine(directory, ".arm_custom_map"), $"{{\"Slot\":{slot},\"SourceSlot\":{slot},\"CreatedAt\":\"2026-10-08T00:00:00Z\",\"ToolVersion\":\"1.0\"}}");
    }

    private static void WriteSolidBmp(string filePath, int width, int height, byte fillValue)
    {
        int stride = (width * 3 + 3) & ~3;
        int imageSize = stride * height;
        int fileSize = 54 + imageSize;
        byte[] bytes = new byte[fileSize];

        bytes[0] = (byte)'B';
        bytes[1] = (byte)'M';
        BitConverter.TryWriteBytes(bytes.AsSpan(2), fileSize);
        BitConverter.TryWriteBytes(bytes.AsSpan(10), 54);
        BitConverter.TryWriteBytes(bytes.AsSpan(14), 40);
        BitConverter.TryWriteBytes(bytes.AsSpan(18), width);
        BitConverter.TryWriteBytes(bytes.AsSpan(22), height);
        BitConverter.TryWriteBytes(bytes.AsSpan(26), (short)1);
        BitConverter.TryWriteBytes(bytes.AsSpan(28), (short)24);
        BitConverter.TryWriteBytes(bytes.AsSpan(34), imageSize);

        for (int y = 0; y < height; y++)
        {
            int row = 54 + y * stride;
            for (int x = 0; x < width; x++)
            {
                int offset = row + x * 3;
                bytes[offset] = fillValue;
                bytes[offset + 1] = fillValue;
                bytes[offset + 2] = fillValue;
            }
        }

        File.WriteAllBytes(filePath, bytes);
    }
}
