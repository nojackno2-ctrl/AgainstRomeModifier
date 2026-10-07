using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.IO.Compression;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 陰影投影形狀類型（對應 objdef shtyp 欄位）。
/// 0 為軸對齊包圍方塊 (Box Quad)，1 為圓形/定向投影视 (Circle)。
/// </summary>
internal enum NativeShadowType
{
    Box = 0,
    Circle = 1,
}

/// <summary>
/// objdef 陰影定義欄位。
/// </summary>
/// <param name="ShadowId">objdef shidx 欄位 (對應 cl_shado.ini 映射 ID)。</param>
/// <param name="ShadowSize">objdef shsiz 欄位 (世界空間半徑 / 半長寬)。</param>
/// <param name="ShadowType">objdef shtyp 欄位 (0=Box, 1=Circle)。</param>
/// <param name="CorrectionX">objdef shacx 欄位 (世界 X 座標中心偏移)。</param>
/// <param name="CorrectionZ">objdef shacz 欄位 (世界 Z 座標中心偏移)。</param>
/// <param name="TextureFileName">從 cl_shado.ini 解析出的 BMP 檔名。</param>
internal sealed record NativeShadowDefinition(
    int ShadowId,
    int ShadowSize,
    NativeShadowType ShadowType,
    int CorrectionX,
    int CorrectionZ,
    string TextureFileName);

/// <summary>
/// 2D/3D 世界空間平面四邊形投影幾何。
/// </summary>
/// <param name="CenterX">世界空間陰影中心 X (物件錨點 X + CorrectionX)。</param>
/// <param name="CenterZ">世界空間陰影中心 Z (物件錨點 Z + CorrectionZ)。</param>
/// <param name="HalfExtent">世界空間半寬高 (ShadowSize)。</param>
/// <param name="CornerNW_X">西北角 (-HalfExtent, -HalfExtent) 世界 X。</param>
/// <param name="CornerNW_Z">西北角 (-HalfExtent, -HalfExtent) 世界 Z。</param>
/// <param name="CornerNE_X">東北角 (+HalfExtent, -HalfExtent) 世界 X。</param>
/// <param name="CornerNE_Z">東北角 (+HalfExtent, -HalfExtent) 世界 Z。</param>
/// <param name="CornerSE_X">東南角 (+HalfExtent, +HalfExtent) 世界 X。</param>
/// <param name="CornerSE_Z">東南角 (+HalfExtent, +HalfExtent) 世界 Z。</param>
/// <param name="CornerSW_X">西南角 (-HalfExtent, +HalfExtent) 世界 X。</param>
/// <param name="CornerSW_Z">西南角 (-HalfExtent, +HalfExtent) 世界 Z。</param>
internal readonly record struct NativeShadowWorldQuad(
    double CenterX, double CenterZ, double HalfExtent,
    double CornerNW_X, double CornerNW_Z,
    double CornerNE_X, double CornerNE_Z,
    double CornerSE_X, double CornerSE_Z,
    double CornerSW_X, double CornerSW_Z)
{
    public static NativeShadowWorldQuad FromCenterAndSize(double centerX, double centerZ, double halfExtent)
    {
        return new NativeShadowWorldQuad(
            centerX, centerZ, halfExtent,
            centerX - halfExtent, centerZ - halfExtent,
            centerX + halfExtent, centerZ - halfExtent,
            centerX + halfExtent, centerZ + halfExtent,
            centerX - halfExtent, centerZ + halfExtent);
    }
}

/// <summary>
/// 物件已解析的陰影資源包裝，包含陰影定義、遮罩圖像文件與世界四邊形產生器。
/// </summary>
internal sealed record NativeObjectShadow(
    string ObjectName,
    NativeShadowDefinition Definition,
    NativeShadowDocument Document)
{
    /// <summary>
    /// 依據指定之物件世界錨點 (anchorX, anchorZ)，計算該陰影在世界座標中的四邊形頂點。
    /// </summary>
    public NativeShadowWorldQuad ComputeWorldQuad(double anchorX, double anchorZ)
    {
        double cx = anchorX + Definition.CorrectionX;
        double cz = anchorZ + Definition.CorrectionZ;
        return NativeShadowWorldQuad.FromCenterAndSize(cx, cz, Definition.ShadowSize);
    }

    /// <summary>
    /// 計算在 2:1 正交等角視角 (isometric) 下，陰影中心相對於物件錨點的螢幕像素偏移。
    /// 依據引擎反組譯: ScreenDeltaX = (shacx - shacz) / 2.0, ScreenDeltaY = (shacx + shacz) / 4.0。
    /// </summary>
    public (double DeltaX, double DeltaY) ComputeScreenOffset()
    {
        return ((Definition.CorrectionX - Definition.CorrectionZ) / 2.0,
                (Definition.CorrectionX + Definition.CorrectionZ) / 4.0);
    }
}

/// <summary>
/// 針對 Against Rome 原生陰影資源的目錄管理器。
/// 嚴格解析 objdef.txt (欄位 shidx, shsiz, shtyp, shacx, shacz)、cl_shado.ini 映射表，
/// 以及 shad.dat (ZIP) 內 128x128 8-bit BMP 陰影遮罩。
/// </summary>
internal sealed class NativeShadowCatalog : IDisposable
{
    private readonly Dictionary<string, NativeShadowDefinition> _definitions;
    private readonly IReadOnlyDictionary<int, string> _shadowNames;
    private readonly Func<string, byte[]?> _readShadowTexture;
    private readonly IDisposable? _owner;
    private readonly Dictionary<string, NativeShadowDocument?> _documents = new(StringComparer.OrdinalIgnoreCase);
    private readonly object _gate = new();

    public NativeShadowCatalog(
        Dictionary<string, NativeShadowDefinition> definitions,
        IReadOnlyDictionary<int, string> shadowNames,
        Func<string, byte[]?> readShadowTexture,
        IDisposable? owner = null)
    {
        _definitions = definitions ?? throw new ArgumentNullException(nameof(definitions));
        _shadowNames = shadowNames ?? throw new ArgumentNullException(nameof(shadowNames));
        _readShadowTexture = readShadowTexture ?? throw new ArgumentNullException(nameof(readShadowTexture));
        _owner = owner;
    }

    public int DefinitionCount => _definitions.Count;
    public int ShadowNameMappingCount => _shadowNames.Count;

    /// <summary>
    /// 從純文字字串與自訂讀取委派建構 NativeShadowCatalog。
    /// </summary>
    public static NativeShadowCatalog FromText(
        string objdefText,
        string shadowNamesIniText,
        Func<string, byte[]?> readShadowTexture,
        IDisposable? owner = null)
    {
        var shadowNames = ParseShadowNames(shadowNamesIniText);
        var definitions = ParseObjdefShadows(objdefText, shadowNames);
        return new NativeShadowCatalog(definitions, shadowNames, readShadowTexture, owner);
    }

    /// <summary>
    /// 唯讀方式從遊戲素材路徑 (或暫存目錄) 開啟 NativeShadowCatalog。
    /// 若必要的檔案 (objdef.dau / objdef.txt, cl_shado.ini, shad.dat) 不存在則回傳 null。
    /// 同時接受扁平的素材副本目錄與遊戲安裝目錄結構（SYSTEM/cl_shado.ini、SYSTEM/DATA_MP/DEFAULTS/objdef.dau）。
    /// </summary>
    public static NativeShadowCatalog? Open(string assetsPath)
    {
        ArgumentNullException.ThrowIfNull(assetsPath);
        if (!Directory.Exists(assetsPath)) return null;

        string shadPath = Path.Combine(assetsPath, "shad.dat");
        if (!File.Exists(shadPath)) return null;

        string? clShadoPath = FindFile(assetsPath, "cl_shado.ini", "cl_shado.txt", Path.Combine("SYSTEM", "cl_shado.ini"));
        if (clShadoPath is null) return null;

        string? objdefPath = FindFile(assetsPath, "objdef.dau", "objdef.txt", Path.Combine("SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"));
        if (objdefPath is null) return null;

        ZipArchive? zipArchive = null;
        try
        {
            string clShadoText = ReadGameText(clShadoPath);
            string objdefText = ReadGameText(objdefPath);

            var shadowNames = ParseShadowNames(clShadoText);
            var definitions = ParseObjdefShadows(objdefText, shadowNames);

            var fileStream = new FileStream(shadPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            zipArchive = new ZipArchive(fileStream, ZipArchiveMode.Read);

            // 建立 ZIP 檔名快取（大小寫無關）
            var zipEntries = new Dictionary<string, ZipArchiveEntry>(StringComparer.OrdinalIgnoreCase);
            foreach (var entry in zipArchive.Entries)
            {
                zipEntries.TryAdd(entry.FullName, entry);
                string leafName = Path.GetFileName(entry.FullName);
                if (!string.IsNullOrEmpty(leafName))
                {
                    zipEntries.TryAdd(leafName, entry);
                }
            }

            byte[]? ReadTexture(string textureFileName)
            {
                // 先嘗試完整路徑 SYSTEM/DATA/SHADOWTEXTURE/<filename>
                string fullPath = "SYSTEM/DATA/SHADOWTEXTURE/" + textureFileName;
                if (!zipEntries.TryGetValue(fullPath, out var entry))
                {
                    if (!zipEntries.TryGetValue(textureFileName, out entry))
                    {
                        return null;
                    }
                }

                using var stream = entry.Open();
                using var memory = new MemoryStream();
                stream.CopyTo(memory);
                return memory.ToArray();
            }

            return new NativeShadowCatalog(definitions, shadowNames, ReadTexture, zipArchive);
        }
        catch
        {
            zipArchive?.Dispose();
            throw;
        }
    }

    /// <summary>
    /// 依據物件名稱取得其陰影定義與載入後的遮罩 Document。
    /// 若物件無陰影 (shidx &lt; 0)、未定義或素材遺失則回傳 null。
    /// </summary>
    public NativeObjectShadow? GetShadow(string objectName)
    {
        ArgumentNullException.ThrowIfNull(objectName);

        if (!_definitions.TryGetValue(objectName, out var def))
            return null;

        var doc = GetDocument(def.TextureFileName);
        if (doc is null) return null;

        return new NativeObjectShadow(objectName, def, doc);
    }

    /// <summary>
    /// 嘗試取得指定物件之陰影定義。
    /// </summary>
    public bool TryGetDefinition(string objectName, [NotNullWhen(true)] out NativeShadowDefinition? definition)
    {
        return _definitions.TryGetValue(objectName, out definition);
    }

    /// <summary>
    /// 載入並快取特定遮罩 BMP 的 NativeShadowDocument。
    /// </summary>
    public NativeShadowDocument? GetDocument(string textureFileName)
    {
        ArgumentNullException.ThrowIfNull(textureFileName);
        lock (_gate)
        {
            if (_documents.TryGetValue(textureFileName, out var cached))
                return cached;

            byte[]? data = _readShadowTexture(textureFileName);
            if (data is null || data.Length == 0)
            {
                _documents[textureFileName] = null;
                return null;
            }

            try
            {
                var doc = NativeShadowDocument.Parse(data);
                _documents[textureFileName] = doc;
                return doc;
            }
            catch (Exception ex) when (ex is InvalidDataException or NotSupportedException)
            {
                _documents[textureFileName] = null;
                return null;
            }
        }
    }

    /// <summary>
    /// 解析 [ShadowNames] 設定文字 (cl_shado.ini)。
    /// 格式為 "0000,small_round_shadow.bmp"。
    /// </summary>
    public static Dictionary<int, string> ParseShadowNames(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        var result = new Dictionary<int, string>();
        bool inSection = false;

        foreach (string rawLine in text.Split('\n'))
        {
            string line = rawLine.Trim();
            if (line.Length == 0 || line.StartsWith(';')) continue;

            if (line.StartsWith('['))
            {
                inSection = line.Equals("[ShadowNames]", StringComparison.OrdinalIgnoreCase);
                continue;
            }

            if (!inSection) continue;

            string[] parts = line.Split(',', 2);
            if (parts.Length < 2) continue;

            if (int.TryParse(parts[0].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int id)
                && id is >= 0 and < 2000)
            {
                string fileName = parts[1].Trim();
                if (fileName.Length > 0 && !fileName.Contains('/') && !fileName.Contains('\\'))
                {
                    result.TryAdd(id, fileName);
                }
            }
        }

        return result;
    }

    /// <summary>
    /// 解析 objdef 文字中的陰影欄位，結合 shadowNames 映射表。
    /// 欄位：shidx col 11, shsiz 12, shtyp 13, shacx 15, shacz 16, name 52。
    /// </summary>
    public static Dictionary<string, NativeShadowDefinition> ParseObjdefShadows(
        string text,
        IReadOnlyDictionary<int, string> shadowNames)
    {
        ArgumentNullException.ThrowIfNull(text);
        ArgumentNullException.ThrowIfNull(shadowNames);

        var result = new Dictionary<string, NativeShadowDefinition>(StringComparer.OrdinalIgnoreCase);
        int colShidx = 11, colShsiz = 12, colShtyp = 13, colShacx = 15, colShacz = 16, colName = 52;

        foreach (string rawLine in text.Split('\n'))
        {
            string line = rawLine.Trim();
            if (line.Length == 0 || line.StartsWith('[')) continue;

            if (line.StartsWith(";idx", StringComparison.OrdinalIgnoreCase))
            {
                var headers = line.Split(',').Select(h => h.Trim().TrimStart(';')).ToArray();
                int idxShidx = Array.IndexOf(headers, "shidx");
                int idxShsiz = Array.IndexOf(headers, "shsiz");
                int idxShtyp = Array.IndexOf(headers, "shtyp");
                int idxShacx = Array.IndexOf(headers, "shacx");
                int idxShacz = Array.IndexOf(headers, "shacz");
                if (idxShidx >= 0) colShidx = idxShidx;
                if (idxShsiz >= 0) colShsiz = idxShsiz;
                if (idxShtyp >= 0) colShtyp = idxShtyp;
                if (idxShacx >= 0) colShacx = idxShacx;
                if (idxShacz >= 0) colShacz = idxShacz;
                continue;
            }

            if (line.StartsWith(';')) continue;

            string[] cols = line.Split(',');
            int minCols = Math.Max(colName, Math.Max(colShidx, Math.Max(colShsiz, Math.Max(colShtyp, Math.Max(colShacx, colShacz)))));
            if (cols.Length <= minCols) continue;

            string name = cols[colName].Trim();
            if (string.IsNullOrEmpty(name)) continue;

            if (!TryParseInt(cols[colShidx], out int shidx) || shidx < 0) continue;
            if (!TryParseInt(cols[colShsiz], out int shsiz)) continue;
            if (!TryParseInt(cols[colShtyp], out int shtypRaw)) continue;
            if (!TryParseInt(cols[colShacx], out int shacx)) continue;
            if (!TryParseInt(cols[colShacz], out int shacz)) continue;

            if (!shadowNames.TryGetValue(shidx, out string? textureFile) || string.IsNullOrEmpty(textureFile))
                continue;

            var shadowType = shtypRaw == 1 ? NativeShadowType.Circle : NativeShadowType.Box;
            var def = new NativeShadowDefinition(shidx, shsiz, shadowType, shacx, shacz, textureFile);
            result.TryAdd(name, def);
        }

        return result;
    }

    private static bool TryParseInt(string value, out int result)
        => int.TryParse(value.Trim(), NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture, out result);

    private static string? FindFile(string directory, params string[] candidates)
    {
        foreach (var candidate in candidates)
        {
            string path = Path.Combine(directory, candidate);
            if (File.Exists(path)) return path;
        }
        return null;
    }

    private static string ReadGameText(string path)
    {
        byte[] bytes = File.ReadAllBytes(path);
        if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8))
            bytes = GameLZSS.DecompressPfil(bytes);
        return MapTextEncoding.Game.GetString(bytes);
    }

    public void Dispose()
    {
        lock (_gate)
        {
            _owner?.Dispose();
            _documents.Clear();
        }
    }
}
