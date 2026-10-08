using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Packaging;

/// <summary>預檢問題等級。</summary>
public enum PreflightSeverity
{
    Error,    // 阻擋性錯誤：禁止外發
    Warning,  // 潛在問題警告：建議修正
    Info      // 資訊提示或清理提醒
}

/// <summary>預檢問題類別。</summary>
public enum PreflightCategory
{
    StructuralBinary, // 檔案結構與二進位檔案完整性
    Diagnostics,      // 語意檢查（MapDiagnostics 關聯）
    Playability,      // 可玩性與勝負條件
    Sanitization      // 封包衛生與快取清理
}

/// <summary>發布預檢所報告之單一檢查項目。</summary>
public sealed record MapPreflightIssue(
    PreflightSeverity Severity,
    PreflightCategory Category,
    string Code,
    string Chinese,
    string English,
    Guid ObjectId = default,
    int EventIndex = -1,
    float? WorldX = null,
    float? WorldZ = null);

/// <summary>地圖發布前之驗收預檢報告。</summary>
public sealed class MapPreflightReport
{
    public bool CanExport => Issues.All(i => i.Severity != PreflightSeverity.Error);
    public List<MapPreflightIssue> Issues { get; } = [];
    public List<string> FilesToPackage { get; } = [];
    public List<string> FilesToExclude { get; } = [];

    public int ErrorCount => Issues.Count(i => i.Severity == PreflightSeverity.Error);
    public int WarningCount => Issues.Count(i => i.Severity == PreflightSeverity.Warning);
    public int InfoCount => Issues.Count(i => i.Severity == PreflightSeverity.Info);
}

/// <summary>
/// 地圖發布前完整性驗收預檢器 (MapExportPreflightChecker)。
/// 自動執行二進位結構體檢、調用 MapDiagnostics 深度分析、檢查可玩性與勝負條件，
/// 並排查排除可清理的本地快取與殘留暫存檔案。
/// </summary>
public static class MapExportPreflightChecker
{
    // 高度相依且遊戲會在載入時重算的快取檔案
    private static readonly HashSet<string> HeightDependentCaches = new(StringComparer.OrdinalIgnoreCase)
    {
        "skydens.dat", "visible.dat", "cliprect.dat", "shadows.dat"
    };

    /// <summary>執行完整預檢流程。</summary>
    public static MapPreflightReport Check(
        string mapDirectory,
        ScenarioDocument? scenario = null,
        IReadOnlyCollection<string>? knownAliases = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(mapDirectory);
        if (!Directory.Exists(mapDirectory))
            throw new DirectoryNotFoundException($"地圖目錄不存在: {mapDirectory}");

        var report = new MapPreflightReport();

        // 階段一：檔案結構與二進位體檢
        CheckFileStructure(mapDirectory, report);

        // 階段二：調用 MapDiagnostics 進行幾何、事件與通行性診斷
        CheckSemanticDiagnostics(mapDirectory, scenario, knownAliases, report);

        // 階段三：可玩性與開局完整性
        CheckPlayability(mapDirectory, scenario, report);

        // 階段四：封包衛生性排查（清理快取與暫存檔）
        AuditPackageSanitization(mapDirectory, report);

        return report;
    }

    private static void CheckFileStructure(string mapDirectory, MapPreflightReport report)
    {
        // 核心地形與高度檔
        string bodenBmp = Path.Combine(mapDirectory, "boden.bmp");
        if (!File.Exists(bodenBmp))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-boden",
                "缺少地圖核心高度檔案 boden.bmp。", "Missing core terrain height file boden.bmp."));
        }
        else
        {
            ValidateBmpHeader(bodenBmp, 257, 257, "boden.bmp", report);
        }

        // 碰撞通行層
        string collisionBmp = Path.Combine(mapDirectory, "collision.bmp");
        if (!File.Exists(collisionBmp))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-collision",
                "缺少通行阻擋層 collision.bmp。", "Missing collision layer collision.bmp."));
        }
        else
        {
            ValidateBmpHeader(collisionBmp, 256, 256, "collision.bmp", report);
        }

        // 地形設定檔
        string bodenIni = Path.Combine(mapDirectory, "boden.ini");
        if (!File.Exists(bodenIni))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-boden-ini",
                "缺少地形設定檔案 boden.ini。", "Missing terrain configuration boden.ini."));
        }

        // 簡報與語系
        string usBriefing = Path.Combine(mapDirectory, "TEXT", "US", "briefing.put");
        if (!File.Exists(usBriefing))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-briefing",
                "缺少主要語系簡報檔 TEXT/US/briefing.put。", "Missing main localization file TEXT/US/briefing.put."));
        }

        // 場景物件資料庫
        string objectsDat = Path.Combine(mapDirectory, "DATA", "objects.dat");
        if (!File.Exists(objectsDat))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-objects-dat",
                "缺少世界物件定義檔 DATA/objects.dat。", "Missing world objects definition DATA/objects.dat."));
        }

        // 關卡執行腳本
        string levelBci = Path.Combine(mapDirectory, "SCRIPT", "ak_level.bci");
        if (!File.Exists(levelBci))
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-ak-level",
                "缺少關卡主腳本 SCRIPT/ak_level.bci。", "Missing level script SCRIPT/ak_level.bci."));
        }

        // SDL 場景聚落定義檔
        var sdlFiles = Directory.GetFiles(mapDirectory, "*.sdl", SearchOption.TopDirectoryOnly);
        if (sdlFiles.Length == 0)
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "missing-sdl",
                "地圖目錄未找到任何聚落或場景 SDL 檔案。", "No settlement or scene SDL files found."));
        }
    }

    private static void ValidateBmpHeader(string filePath, int expectedWidth, int expectedHeight, string label, MapPreflightReport report)
    {
        try
        {
            using var stream = File.OpenRead(filePath);
            if (stream.Length < 54)
            {
                report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "corrupt-bmp-header",
                    $"{label} 檔案長度小於 54 位元組，BMP 標頭損壞。", $"{label} file length is less than 54 bytes; BMP header corrupt."));
                return;
            }

            Span<byte> header = stackalloc byte[54];
            stream.ReadExactly(header);

            if (header[0] != (byte)'B' || header[1] != (byte)'M')
            {
                report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "invalid-bmp-magic",
                    $"{label} 不是合法的 BMP 格式（缺少 BM 標記）。", $"{label} is not a valid BMP format (missing BM signature)."));
                return;
            }

            int width = BitConverter.ToInt32(header[18..22]);
            int height = BitConverter.ToInt32(header[22..26]);

            if (width != expectedWidth || height != expectedHeight)
            {
                report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "invalid-bmp-dimensions",
                    $"{label} 尺寸為 {width}x{height}，與標準要求 {expectedWidth}x{expectedHeight} 不符。",
                    $"{label} dimensions are {width}x{height}; expected {expectedWidth}x{expectedHeight}."));
            }
        }
        catch (Exception ex)
        {
            report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.StructuralBinary, "bmp-read-error",
                $"讀取 {label} 失敗: {ex.Message}", $"Failed reading {label}: {ex.Message}"));
        }
    }

    private static void CheckSemanticDiagnostics(
        string mapDirectory,
        ScenarioDocument? scenario,
        IReadOnlyCollection<string>? knownAliases,
        MapPreflightReport report)
    {
        // 嘗試載入或補齊 ScenarioDocument
        string scenarioPath = Path.Combine(mapDirectory, "arm_scenario.json");
        if (scenario == null && File.Exists(scenarioPath))
        {
            try { scenario = ScenarioDocument.Load(mapDirectory); }
            catch (Exception ex)
            {
                report.Issues.Add(new(PreflightSeverity.Error, PreflightCategory.Diagnostics, "corrupted-scenario-json",
                    $"無法解析場景設定檔 arm_scenario.json: {ex.Message}", $"Corrupted arm_scenario.json: {ex.Message}"));
            }
        }

        // 讀取 collision 與 heights 圖層供 MapDiagnostics 分析
        byte[]? collision = null;
        string colPath = Path.Combine(mapDirectory, "collision.bmp");
        if (File.Exists(colPath))
        {
            collision = ReadBmpGreenChannel(colPath, 256, 256);
        }

        byte[]? heights = null;
        float waterLevel = 0f;
        string bodenPath = Path.Combine(mapDirectory, "boden.bmp");
        if (File.Exists(bodenPath))
        {
            heights = ReadBmpGreenChannel(bodenPath, 257, 257);
        }

        string bodenIni = Path.Combine(mapDirectory, "boden.ini");
        if (File.Exists(bodenIni))
        {
            var ini = BodenIniDocument.Load(bodenIni);
            if (float.TryParse(ini.GetValue("Waterlevel"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float wl))
                waterLevel = wl;
        }

        var aliases = knownAliases ?? new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var objects = new List<MapCheckObject>();
        var events = scenario?.Events ?? (IReadOnlyList<ScenarioEvent>)Array.Empty<ScenarioEvent>();

        if (scenario != null)
        {
            foreach (var spawn in scenario.Spawns)
            {
                bool isBuilding = spawn.Alias.StartsWith("Bau", StringComparison.OrdinalIgnoreCase);
                objects.Add(new MapCheckObject(spawn, isBuilding, HasCompletedTemplate: true));
            }
        }

        var snapshot = new MapCheckSnapshot(
            Objects: objects,
            Events: events,
            Aliases: aliases,
            CollisionSize: 256,
            Collision: collision,
            HeightSize: 257,
            Heights: heights,
            HeightStep: 4.0f,
            WaterLevel: waterLevel);

        var diagnosticIssues = MapDiagnostics.Check(snapshot);

        foreach (var issue in diagnosticIssues)
        {
            // 將關鍵阻擋性問題提升為 Error
            PreflightSeverity severity = issue.Code switch
            {
                "identity" or "coordinates" or "team-count" or "event-validation" or "event-target" => PreflightSeverity.Error,
                "blocked-start" or "blocked-building" => PreflightSeverity.Error, // 核心單位陷於水下或岩石屬於阻擋發布的嚴重錯誤
                "no-collision" => PreflightSeverity.Error,
                _ => issue.Severity == MapIssueSeverity.Error ? PreflightSeverity.Error : PreflightSeverity.Warning
            };

            // Without a recipient asset catalog, an empty alias set is not evidence of a missing asset.
            if (issue.Code == "alias" && knownAliases is null) severity = PreflightSeverity.Warning;
            report.Issues.Add(new MapPreflightIssue(
                Severity: severity,
                Category: PreflightCategory.Diagnostics,
                Code: issue.Code,
                Chinese: issue.Chinese,
                English: issue.English,
                ObjectId: issue.ObjectId,
                EventIndex: issue.EventIndex,
                WorldX: issue.WorldX,
                WorldZ: issue.WorldZ));
        }
    }

    private static void CheckPlayability(string mapDirectory, ScenarioDocument? scenario, MapPreflightReport report)
    {
        // 檢查玩家 Team 0 是否具備初始單位或主屋
        bool hasPlayerStart = false;
        if (scenario != null)
        {
            hasPlayerStart = scenario.Spawns.Any(s => s.Team == 0 && (s.Count > 0 || s.Alias.StartsWith("Bau", StringComparison.OrdinalIgnoreCase)));
        }

        if (!hasPlayerStart)
        {
            // 若 scenario 沒有，檢驗 SDL 是否存在 Team 0 物件
            try
            {
                var sceneObjs = SdlSceneCatalog.LoadDirectory(mapDirectory);
                hasPlayerStart = sceneObjs.Any(o => o.Team == 0);
            }
            catch { }
        }

        if (!hasPlayerStart)
        {
            report.Issues.Add(new(PreflightSeverity.Warning, PreflightCategory.Playability, "no-player-presence",
                "地圖未檢測到 Team 0（人類玩家）的起始部隊或聚落主基地；玩家可能在進入地圖時無單位可操作。",
                "No starting unit or town hall detected for Team 0 (human player); player may have no units to control."));
        }

        // 檢查有事件的地圖是否具備勝負終止條件
        if (scenario != null && scenario.Events.Count > 0)
        {
            bool hasTerminalAction = scenario.Events
                .Where(e => e.Enabled)
                .SelectMany(e => e.Actions)
                .Any(a => a.Kind is ScenarioActionKind.Victory or ScenarioActionKind.Defeat);

            if (!hasTerminalAction)
            {
                report.Issues.Add(new(PreflightSeverity.Warning, PreflightCategory.Playability, "no-terminal-event",
                    "地圖設定了腳本事件，但未包含任何勝利 (Victory) 或失敗 (Defeat) 結算行動；戰役可能無法正常通關結算。",
                    "Scenario events are defined but lack Victory or Defeat terminal actions; scenario might never conclude."));
            }
        }
    }

    private static void AuditPackageSanitization(string mapDirectory, MapPreflightReport report)
    {
        var dirInfo = new DirectoryInfo(mapDirectory);
        string root = dirInfo.FullName.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;

        foreach (FileInfo file in dirInfo.GetFiles("*", SearchOption.AllDirectories))
        {
            string relative = Path.GetRelativePath(root, file.FullName).Replace('\\', '/');
            string fileName = file.Name;

            // 檢查高度相依快取
            if (HeightDependentCaches.Contains(fileName))
            {
                report.FilesToExclude.Add(relative);
                report.Issues.Add(new(PreflightSeverity.Info, PreflightCategory.Sanitization, "height-cache-excluded",
                    $"偵測到引擎動態重算快取檔 {fileName}；已標記為排除，減少模組包體積並防止快取錯亂。",
                    $"Detected engine runtime cache {fileName}; marked for exclusion to reduce size and prevent cache mismatch."));
                continue;
            }

            // 檢查暫存或編輯備份檔案
            if (fileName.EndsWith(".tmp_arm", StringComparison.OrdinalIgnoreCase) ||
                fileName.EndsWith(".bak", StringComparison.OrdinalIgnoreCase) ||
                fileName.EndsWith(".log", StringComparison.OrdinalIgnoreCase) ||
                fileName.Equals("Thumbs.db", StringComparison.OrdinalIgnoreCase) ||
                fileName.Equals(".DS_Store", StringComparison.OrdinalIgnoreCase) ||
                fileName.Equals("sdl_placed_objects.put", StringComparison.OrdinalIgnoreCase))
            {
                report.FilesToExclude.Add(relative);
                report.Issues.Add(new(PreflightSeverity.Info, PreflightCategory.Sanitization, "temporary-file-excluded",
                    $"偵測到本地暫存或無用檔案 {relative}；發布時將自動排除。",
                    $"Detected temporary/junk file {relative}; automatically excluded from package."));
                continue;
            }

            if (!MapBundleContract.IsPayloadPath(relative))
            {
                report.FilesToExclude.Add(relative);
                report.Issues.Add(new(PreflightSeverity.Info, PreflightCategory.Sanitization, "non-payload-excluded",
                    $"排除槽位外素材、marker 或不支援檔案：{relative}", $"Excluded non-payload file: {relative}"));
                continue;
            }
            report.FilesToPackage.Add(relative);
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
                // BMP 由底向上
                int rowIdx = height - 1 - y;
                fs.Seek(54 + (long)rowIdx * stride, SeekOrigin.Begin);
                fs.ReadExactly(row);

                for (int x = 0; x < width; x++)
                {
                    green[y * width + x] = row[x * 3 + 1]; // Green channel
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
