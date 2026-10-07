using System.IO.Compression;
using System.Numerics;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 管理遊戲場景光照之全域與局部資料模型。
/// 支援載入日夜色表 (daynight.bmp)、光源定義 (lightdef.dau)、建築與物件光照 (objdef.dau + apt.dat)。
/// fail-soft 容錯設計：缺少檔案時優雅降級為無光照或中性光照。
/// </summary>
internal sealed class SceneLightingContext : IDisposable
{
    private readonly MapLightingDayNight? _dayNight;
    private readonly NativeLightCatalog? _lightCatalog;
    private readonly NativeObjectLightingCatalog? _objectLightingCatalog;
    private readonly IReadOnlyDictionary<int, string> _aptNames;
    private readonly Func<string, byte[]?>? _readApt;
    private readonly IDisposable? _archiveOwner;

    // 成功及失敗均快取，場景更新時不重新讀取 APT。
    private readonly Dictionary<string, IReadOnlyList<Vector3>> _aptLightPointCache = new(StringComparer.OrdinalIgnoreCase);

    public bool IsDayNightAvailable => _dayNight != null;
    public bool IsLightCatalogAvailable => _lightCatalog != null;

    public SceneLightingContext(
        MapLightingDayNight? dayNight,
        NativeLightCatalog? lightCatalog,
        NativeObjectLightingCatalog? objectLightingCatalog,
        IReadOnlyDictionary<int, string>? aptNames,
        Func<string, byte[]?>? readApt,
        IDisposable? archiveOwner = null)
    {
        _dayNight = dayNight;
        _lightCatalog = lightCatalog;
        _objectLightingCatalog = objectLightingCatalog;
        _aptNames = aptNames ?? new Dictionary<int, string>();
        _readApt = readApt;
        _archiveOwner = archiveOwner;
    }

    /// <summary>
    /// 計算給定遊戲時鐘小時 (0..24 float) 之環境光顏色 (row0 native floor 插值)。
    /// 若無日夜色表，回傳中性白光 (1, 1, 1)。
    /// </summary>
    public Vector3 GetAmbientColor(float gameHour)
    {
        if (_dayNight == null) return Vector3.One;

        float clamped = float.IsFinite(gameHour) ? Math.Clamp(gameHour, 0f, 24f) % 24f : 12f;
        int hour = (int)MathF.Floor(clamped);
        float fraction = clamped - hour;
        int minute = Math.Clamp((int)MathF.Floor(fraction * 60f), 0, 59);

        return _dayNight.AmbientAt(hour, minute);
    }

    /// <summary>
    /// 從場景物件中收集所有有效局部光源實例 (建築 APT 光源 + lidef 物件光源)。
    /// 坐標單位為 World Units (tile = 256 world units)。
    /// </summary>
    public List<NativeLightInstance> CollectSceneLights(
        IReadOnlyList<MapSceneObject> objects,
        Func<float, float, float> sampleWorldGroundHeight)
    {
        var result = new List<NativeLightInstance>();
        if (_lightCatalog == null || _objectLightingCatalog == null) return result;

        foreach (var obj in objects)
        {
            if (!_objectLightingCatalog.TryGetDefinition(obj.Name, out var def))
                continue;

            // 1. 建築物 APT 光源 (aptli >= 0 且 aptix >= 0)
            if (def.AptLightDefIndex >= 0 && def.AptIndex >= 0 && _readApt != null)
            {
                if (TryGetActiveLight(def.AptLightDefIndex, out var lightDef))
                {
                    if (_aptNames.TryGetValue(def.AptIndex, out string? aptName))
                    {
                        var lightPoints = GetAptLightPoints(aptName);
                        if (lightPoints != null && lightPoints.Count > 0)
                        {
                            foreach (var delta in lightPoints)
                            {
                                float x = obj.WorldX + delta.X, z = obj.WorldZ + delta.Z;
                                Vector3 worldPos = new Vector3(
                                    x, sampleWorldGroundHeight(x, z) + def.AptLightHeightOffset, z);

                                result.Add(new NativeLightInstance(worldPos, lightDef));
                            }
                        }
                    }
                }
            }

            // 2. 物件獨立光源 (lidef >= 0)
            if (def.LightDefIndex >= 0)
            {
                if (TryGetActiveLight(def.LightDefIndex, out var lightDef))
                {
                    Vector3 worldPos = new Vector3(
                        obj.WorldX,
                        obj.WorldY + def.LightHeightOffset,
                        obj.WorldZ);

                    result.Add(new NativeLightInstance(worldPos, lightDef));
                }
            }
        }

        return result;
    }

    private bool TryGetActiveLight(int index, out NativeLightDefinition definition)
    {
        return _lightCatalog!.TryGetDefinition(index, out definition) && definition.IsActive &&
            float.IsFinite(definition.Radius) && definition.Radius > 0 &&
            float.IsFinite(definition.Color.X) && float.IsFinite(definition.Color.Y) && float.IsFinite(definition.Color.Z);
    }

    private IReadOnlyList<Vector3>? GetAptLightPoints(string aptName)
    {
        if (_aptLightPointCache.TryGetValue(aptName, out var cached))
            return cached;

        if (_readApt == null) return null;

        try
        {
            byte[]? bytes = _readApt(aptName);
            if (bytes != null && bytes.Length >= 112)
            {
                var points = NativeAptLightPoints.GetBuildingWorldLightPositions(bytes, 0, 0, 0, 0);
                _aptLightPointCache[aptName] = points;
                return points;
            }
        }
        catch
        {
            // Fail-soft
        }

        _aptLightPointCache[aptName] = Array.Empty<Vector3>();
        return _aptLightPointCache[aptName];
    }

    /// <summary>
    /// 從地圖資料夾及遊戲安裝根目錄建立 SceneLightingContext。
    /// 若路徑不存在或缺少檔案，將以可用部分建立或安全降級。
    /// </summary>
    public static SceneLightingContext? TryCreate(string? mapDirectory, string? gamePath)
    {
        MapLightingDayNight? dayNight = null;
        if (!string.IsNullOrEmpty(mapDirectory))
        {
            string dayNightPath = Path.Combine(mapDirectory, "daynight.bmp");
            if (File.Exists(dayNightPath))
            {
                try
                {
                    byte[] bytes = File.ReadAllBytes(dayNightPath);
                    dayNight = MapLightingDayNight.Parse(bytes);
                }
                catch { }
            }
        }

        NativeLightCatalog? lightCatalog = null;
        NativeObjectLightingCatalog? objectCatalog = null;
        Dictionary<int, string> aptNames = new();
        Func<string, byte[]?>? readApt = null;
        IDisposable? archiveOwner = null;

        if (!string.IsNullOrEmpty(gamePath) && Directory.Exists(gamePath))
        {
            string lightDefPath = Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS", "lightdef.dau");
            if (File.Exists(lightDefPath))
            {
                try
                {
                    lightCatalog = NativeLightCatalog.Open(lightDefPath);
                }
                catch { }
            }

            objectCatalog = NativeObjectLightingCatalog.Open(gamePath);

            string clAptPath = Path.Combine(gamePath, "SYSTEM", "cl_apt.ini");
            if (!File.Exists(clAptPath)) clAptPath = Path.Combine(gamePath, "SYSTEM", "cl_apt.txt");
            if (File.Exists(clAptPath))
            {
                try
                {
                    byte[] bytes = File.ReadAllBytes(clAptPath);
                    if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8))
                        bytes = AgainstRomeModifier.GameLZSS.DecompressPfil(bytes);
                    string text = MapTextEncoding.Game.GetString(bytes);
                    aptNames = NativeSpriteCatalog.ParseNameList(text);
                }
                catch { }
            }

            string aptDatPath = Path.Combine(gamePath, "apt.dat");
            if (File.Exists(aptDatPath))
            {
                try
                {
                    var file = new FileStream(aptDatPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    ZipArchive zip;
                    try { zip = new ZipArchive(file, ZipArchiveMode.Read); }
                    catch { file.Dispose(); throw; }
                    archiveOwner = zip;
                    var entries = new Dictionary<string, ZipArchiveEntry>(StringComparer.OrdinalIgnoreCase);
                    foreach (var entry in zip.Entries) entries.TryAdd(entry.FullName.Replace('\\', '/'), entry);
                    readApt = name =>
                    {
                        if (!entries.TryGetValue("SYSTEM/DATA/APT/" + name, out var entry) && !entries.TryGetValue(name, out entry)) return null;
                        using var s = entry.Open();
                        using var ms = new MemoryStream();
                        s.CopyTo(ms);
                        return ms.ToArray();
                    };
                }
                catch { }
            }
        }

        if (dayNight == null && lightCatalog == null && objectCatalog == null)
            return null;

        return new SceneLightingContext(dayNight, lightCatalog, objectCatalog, aptNames, readApt, archiveOwner);
    }

    public void Dispose()
    {
        _archiveOwner?.Dispose();
        _aptLightPointCache.Clear();
    }
}
