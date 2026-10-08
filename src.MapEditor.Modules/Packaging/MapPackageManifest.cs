using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Packaging;

/// <summary>多語系簡報與標題資訊。</summary>
public sealed record MapPackageLocalization(
    string Title,
    string? Subtitle = null,
    string? BriefingText = null,
    IReadOnlyList<string>? TeamNames = null);

/// <summary>推薦玩家配置與人數。</summary>
public sealed record MapRecommendedPlayers(
    int MinPlayers = 1,
    int MaxPlayers = 4,
    int OptimalPlayers = 2,
    string TeamConfigurations = "1v1, 2v2, FFA",
    string FactionRestrictions = "Any");

/// <summary>地圖尺寸、坐標系與高度參數。</summary>
public sealed record MapPackageDimensions(
    int GridWidth = 256,
    int GridHeight = 256,
    float WorldUnitsWidth = 16384f,
    float WorldUnitsHeight = 16384f,
    float HeightStep = 4.0f,
    float WaterLevel = 0f);

/// <summary>模組封包內部檔案分類。</summary>
public enum MapFileCategory
{
    TerrainHeight,
    TerrainEmboss,
    TerrainVertex,
    TerrainSmooth,
    Collision,
    Minimap,
    TextureBlending,
    DataPool,
    Script,
    Localization,
    SettlementSdl,
    ScenarioJson,
    CustomMarker,
    Thumbnail,
    Documentation,
    Auxiliary
}

/// <summary>檔案清單中的單一檔案項目與校驗碼。</summary>
public sealed record MapPackageFileEntry(
    string RelativePath,
    long SizeBytes,
    string Sha256,
    MapFileCategory Category,
    bool IsRequired = true);

/// <summary>槽位相容性與執行策略。</summary>
public sealed record MapCompatibilityPolicy(
    int PreferredSlot = 5,
    bool AllowDynamicSlotRemapping = true,
    bool StandaloneLevel = false,
    string MinGameVersion = "1.0",
    string TargetPatch = "None");

/// <summary>
/// 《反抗羅馬》地圖模組封包元資料清單 (Map Package Manifest)。
/// 定義地圖發布所需之中繼資料、多語系、玩家配置、尺寸、相容性策略與檔案雜湊清單。
/// </summary>
public sealed class MapPackageManifest
{
    public const string ManifestFileName = "manifest.json";
    public const string CurrentSchemaVersion = "1.0";

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = { new JsonStringEnumConverter() }
    };

    public string SchemaVersion { get; set; } = CurrentSchemaVersion;
    public string PackageId { get; set; } = string.Empty;
    public string Title { get; set; } = string.Empty;
    public string? Subtitle { get; set; }
    public string Author { get; set; } = "Community Modder";
    public string Version { get; set; } = "1.0.0";
    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.UtcNow;
    public DateTimeOffset ModifiedAt { get; set; } = DateTimeOffset.UtcNow;
    public string Description { get; set; } = string.Empty;

    public Dictionary<string, MapPackageLocalization> Localizations { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    public MapRecommendedPlayers Players { get; set; } = new();
    public MapPackageDimensions Dimensions { get; set; } = new();
    public List<string> GameModes { get; set; } = ["Endless"];
    public List<string> Tags { get; set; } = [];
    public MapCompatibilityPolicy Compatibility { get; set; } = new();
    public List<MapPackageFileEntry> Files { get; set; } = [];
    public string PackageChecksum { get; set; } = string.Empty;

    /// <summary>驗證 Manifest 元資料完整性。</summary>
    public bool Validate(out List<string> validationErrors)
    {
        validationErrors = [];
        if (string.IsNullOrWhiteSpace(PackageId))
            validationErrors.Add("PackageId 不能為空。");
        if (string.IsNullOrWhiteSpace(Title))
            validationErrors.Add("Title 地圖名稱不能為空。");
        if (Compatibility.PreferredSlot is < 5 or > 999)
            validationErrors.Add($"PreferredSlot ({Compatibility.PreferredSlot}) 必須在 5 至 999 範圍內，原廠槽位 0..4 禁止作為發布槽位。");
        if (Players.MinPlayers < 1 || Players.MaxPlayers < Players.MinPlayers)
            validationErrors.Add("玩家人數配置無效 (MinPlayers 需 >= 1 且 MaxPlayers >= MinPlayers)。");
        if (Dimensions.GridWidth <= 0 || Dimensions.GridHeight <= 0)
            validationErrors.Add("地圖網格尺寸必須大於 0。");
        if (Files.Count == 0)
            validationErrors.Add("Files 檔案清單不能為空。");

        return validationErrors.Count == 0;
    }

    /// <summary>計算整個模組檔案清單與關鍵元資料的組合雜湊 (SHA-256 Checksum)。</summary>
    public string ComputePackageChecksum()
    {
        var builder = new StringBuilder();
        builder.Append(PackageId).Append('|')
               .Append(Version).Append('|')
               .Append(Dimensions.GridWidth).Append('x').Append(Dimensions.GridHeight).Append('|')
               .Append(Compatibility.PreferredSlot).Append('|');

        foreach (MapPackageFileEntry file in Files.OrderBy(f => f.RelativePath, StringComparer.OrdinalIgnoreCase))
        {
            builder.Append(file.RelativePath.ToLowerInvariant()).Append(':')
                   .Append(file.SizeBytes).Append(':')
                   .Append(file.Sha256).Append(';');
        }

        byte[] hash = SHA256.HashData(Encoding.UTF8.GetBytes(builder.ToString()));
        return Convert.ToHexString(hash);
    }

    /// <summary>序列化為格式化 JSON 字串。</summary>
    public string ToJson()
    {
        PackageChecksum = ComputePackageChecksum();
        return JsonSerializer.Serialize(this, JsonOptions);
    }

    /// <summary>從 JSON 字串反序列化。</summary>
    public static MapPackageManifest FromJson(string json)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(json);
        return JsonSerializer.Deserialize<MapPackageManifest>(json, JsonOptions)
            ?? throw new InvalidDataException("無法解析 MapPackageManifest JSON。");
    }

    /// <summary>從地圖目錄快速掃描建立基礎 Manifest 模型。</summary>
    public static MapPackageManifest CreateFromDirectory(
        string mapDirectory,
        string packageId,
        string? author = null,
        string? version = null,
        int? preferredSlot = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(mapDirectory);
        if (!Directory.Exists(mapDirectory))
            throw new DirectoryNotFoundException($"地圖目錄不存在: {mapDirectory}");

        var manifest = new MapPackageManifest
        {
            PackageId = string.IsNullOrWhiteSpace(packageId) ? Path.GetFileName(mapDirectory).ToLowerInvariant() : packageId,
            Author = author ?? "Community Modder",
            Version = version ?? "1.0.0",
            CreatedAt = DateTimeOffset.UtcNow,
            ModifiedAt = DateTimeOffset.UtcNow
        };

        // 讀取標題與多語系簡報
        string usBriefing = Path.Combine(mapDirectory, "TEXT", "US", "briefing.put");
        if (File.Exists(usBriefing))
        {
            var put = PutTextDocument.Load(usBriefing);
            manifest.Title = put.GetValue("briefing_titel_1") ?? Path.GetFileName(mapDirectory);
            manifest.Subtitle = put.GetValue("briefing_titel_2");
            manifest.Description = put.GetValue("briefing_text") ?? string.Empty;

            var teamNames = new List<string>();
            for (int i = 0; i < 8; i++)
            {
                string? team = put.GetValue($"briefing_text_teamname{i}");
                if (!string.IsNullOrWhiteSpace(team)) teamNames.Add(team);
            }

            manifest.Localizations["US"] = new MapPackageLocalization(
                manifest.Title,
                manifest.Subtitle,
                manifest.Description,
                teamNames);
        }
        else
        {
            manifest.Title = Path.GetFileName(mapDirectory);
        }

        // 讀取水面與地形配置
        float waterLevel = 0f;
        string bodenIni = Path.Combine(mapDirectory, "boden.ini");
        if (File.Exists(bodenIni))
        {
            var ini = BodenIniDocument.Load(bodenIni);
            if (float.TryParse(ini.GetValue("Waterlevel"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float parsedWater))
                waterLevel = parsedWater;
        }
        manifest.Dimensions = new MapPackageDimensions(WaterLevel: waterLevel);

        // 掃描槽位資訊
        int slot = preferredSlot ?? ExtractSlotFromDirectory(mapDirectory);
        manifest.Compatibility = new MapCompatibilityPolicy(
            PreferredSlot: Math.Clamp(slot, 5, 999),
            StandaloneLevel: CustomMapManifest.HasStandaloneLevel(mapDirectory));

        // 掃描檔案清單
        manifest.ScanFiles(mapDirectory);
        manifest.PackageChecksum = manifest.ComputePackageChecksum();

        return manifest;
    }

    /// <summary>掃描目錄下所有合格的地圖檔案，計算大小與 SHA-256。</summary>
    public void ScanFiles(string mapDirectory)
    {
        Files.Clear();
        var dirInfo = new DirectoryInfo(mapDirectory);
        string rootFullPath = dirInfo.FullName.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;

        foreach (FileInfo file in dirInfo.GetFiles("*", SearchOption.AllDirectories))
        {
            string relative = Path.GetRelativePath(rootFullPath, file.FullName).Replace('\\', '/');
            
            // 排除暫存檔與本地快取檔
            if (ShouldExcludeFromFiles(relative)) continue;

            MapFileCategory category = CategorizeFile(relative);
            using FileStream stream = file.OpenRead();
            string hash = Convert.ToHexString(SHA256.HashData(stream));

            Files.Add(new MapPackageFileEntry(
                RelativePath: relative,
                SizeBytes: file.Length,
                Sha256: hash,
                Category: category,
                IsRequired: IsRequiredFile(category)));
        }
    }

    private static bool ShouldExcludeFromFiles(string relativePath)
    {
        string name = Path.GetFileName(relativePath);
        if (name.EndsWith(".tmp_arm", StringComparison.OrdinalIgnoreCase) ||
            name.EndsWith(".bak", StringComparison.OrdinalIgnoreCase) ||
            name.EndsWith(".log", StringComparison.OrdinalIgnoreCase) ||
            name.Equals("Thumbs.db", StringComparison.OrdinalIgnoreCase) ||
            name.Equals(".DS_Store", StringComparison.OrdinalIgnoreCase))
            return true;

        // 排除舊版實驗性放置檔
        if (name.Equals("sdl_placed_objects.put", StringComparison.OrdinalIgnoreCase))
            return true;

        return false;
    }

    private static MapFileCategory CategorizeFile(string relativePath)
    {
        string normalized = relativePath.Replace('\\', '/').ToLowerInvariant();
        string name = Path.GetFileName(normalized);

        if (name == "boden.bmp") return MapFileCategory.TerrainHeight;
        if (name == "emboss.bmp") return MapFileCategory.TerrainEmboss;
        if (name == "vertex.bmp") return MapFileCategory.TerrainVertex;
        if (name == "smooth.bmp") return MapFileCategory.TerrainSmooth;
        if (name == "collision.bmp") return MapFileCategory.Collision;
        if (name == "minimap.bmp") return MapFileCategory.Minimap;
        if (name == "floortex.dat" || name == "textures.txt") return MapFileCategory.TextureBlending;
        if (normalized.StartsWith("data/", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.DataPool;
        if (normalized.StartsWith("script/", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.Script;
        if (normalized.StartsWith("text/", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.Localization;
        if (name.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.SettlementSdl;
        if (name == "arm_scenario.json") return MapFileCategory.ScenarioJson;
        if (name == CustomMapManifest.MarkerFileName) return MapFileCategory.CustomMarker;
        if (name.StartsWith("thumbnail.", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.Thumbnail;
        if (name.EndsWith(".txt", StringComparison.OrdinalIgnoreCase) || name.EndsWith(".md", StringComparison.OrdinalIgnoreCase)) return MapFileCategory.Documentation;

        return MapFileCategory.Auxiliary;
    }

    private static bool IsRequiredFile(MapFileCategory category) => category switch
    {
        MapFileCategory.TerrainHeight => true,
        MapFileCategory.Collision => true,
        MapFileCategory.DataPool => true,
        MapFileCategory.Script => true,
        MapFileCategory.Localization => true,
        _ => false
    };

    private static int ExtractSlotFromDirectory(string directory)
    {
        string dirName = Path.GetFileName(directory);
        if (dirName.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase) &&
            int.TryParse(dirName[5..], System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out int slot))
        {
            return slot;
        }
        return 5;
    }
}
