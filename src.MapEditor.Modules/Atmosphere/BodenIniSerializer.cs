using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 雙向序列化與解析原生 boden.ini 參數。
/// 確保 100% 與原版遊戲引擎相容，並嚴格遵循無損回寫原則（保留註解、未知區段與原始格式）。
/// </summary>
public static class BodenIniSerializer
{
    /// <summary>
    /// 已知原生 boden.ini 鍵名集合（用於區分標準欄位與保留區段）。
    /// </summary>
    private static readonly HashSet<string> KnownKeys = new(StringComparer.OrdinalIgnoreCase)
    {
        "Waterlevel", "Heightmapstep", "WaterBumpAmplitude", "WaterBumpFrequency",
        "WaterWarpShift", "RainDropsOnWater", "WaterColor", "WasserTexturName",
        "CausticTexturName", "SkyTexturName", "FlashPropability", "FlashObjectDefaultIndex",
        "FlashObjectDefault2Index", "FlashLightDefaultIndex", "SnowAlrIndex", "SnowShadowIndex",
        "SnowShadowSize", "HagelShadowIndex", "HagelShadowSize", "MoveListAmplitude",
        "DayStartTime", "DayEndTime", "ShadowMeshMode", "ShadowMeshAccuracy",
        "ShadowMeshXZsize", "ShadowMeshYsize", "Skydensspread", "Skydensaccuracy",
        "HandleSkyDensMap", "HandleVisibleMap", "HandleClipRectMap", "HandleShadowMeshes",
        "ShowCollisionMesh"
    };

    /// <summary>
    /// 從 <see cref="BodenIniDocument"/> 讀取並填入 <see cref="BodenIniData"/>。
    /// </summary>
    public static BodenIniData Parse(BodenIniDocument document)
    {
        ArgumentNullException.ThrowIfNull(document);
        var data = new BodenIniData();
        PopulateFromDocument(document, data);
        return data;
    }

    /// <summary>
    /// 從原始 INI 文字字串解析為 <see cref="BodenIniData"/>。
    /// </summary>
    public static BodenIniData ParseText(string iniText)
    {
        ArgumentNullException.ThrowIfNull(iniText);
        var data = new BodenIniData();
        var matches = Regex.Matches(iniText, @"(?im)^\s*\[(?<section>[^\]\r\n]+)\][^\r\n]*(?:\r?\n)(?<value>[^\r\n]*)");

        foreach (Match match in matches)
        {
            string section = match.Groups["section"].Value.Trim();
            string rawValue = match.Groups["value"].Value;
            int commentIdx = rawValue.IndexOf(';');
            string value = (commentIdx >= 0 ? rawValue[..commentIdx] : rawValue).Trim();

            if (KnownKeys.Contains(section))
            {
                ApplyValue(data, section, value);
            }
            else
            {
                data.PreservedSections[section] = value;
            }
        }

        return data;
    }

    /// <summary>
    /// 將 <see cref="BodenIniData"/> 的變更寫入既有的 <see cref="BodenIniDocument"/>。
    /// 僅更新數值行，原樣保留所有既有註解、排列次序與未知鍵值。
    /// </summary>
    public static void UpdateDocument(BodenIniDocument document, BodenIniData data)
    {
        ArgumentNullException.ThrowIfNull(document);
        ArgumentNullException.ThrowIfNull(data);

        // 核心水面與幾何
        SetOrIgnore(document, "Waterlevel", data.Waterlevel.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "Heightmapstep", data.Heightmapstep.ToString(CultureInfo.InvariantCulture));

        // 水面動態
        SetOrIgnore(document, "WaterBumpAmplitude", data.WaterBumpAmplitude.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "WaterBumpFrequency", data.WaterBumpFrequency.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "WaterWarpShift", data.WaterWarpShift.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "RainDropsOnWater", data.RainDropsOnWater ? "1" : "0");
        SetOrIgnore(document, "WaterColor", data.WaterColor.Trim());
        SetOrIgnore(document, "WasserTexturName", data.WasserTexturName.Trim());
        SetOrIgnore(document, "CausticTexturName", data.CausticTexturName.Trim());
        SetOrIgnore(document, "SkyTexturName", data.SkyTexturName.Trim());

        // 天候與雷電 (原生拼寫 FlashPropability)
        SetOrIgnore(document, "FlashPropability", data.FlashPropability.ToString(CultureInfo.InvariantCulture));
        if (data.FlashObjectDefaultIndex >= 0) SetOrIgnore(document, "FlashObjectDefaultIndex", data.FlashObjectDefaultIndex.ToString(CultureInfo.InvariantCulture));
        if (data.FlashObjectDefault2Index >= 0) SetOrIgnore(document, "FlashObjectDefault2Index", data.FlashObjectDefault2Index.ToString(CultureInfo.InvariantCulture));
        if (data.FlashLightDefaultIndex >= 0) SetOrIgnore(document, "FlashLightDefaultIndex", data.FlashLightDefaultIndex.ToString(CultureInfo.InvariantCulture));

        // 冰雪與天候粒子
        if (data.SnowAlrIndex >= 0) SetOrIgnore(document, "SnowAlrIndex", data.SnowAlrIndex.ToString(CultureInfo.InvariantCulture));
        if (data.SnowShadowIndex >= 0) SetOrIgnore(document, "SnowShadowIndex", data.SnowShadowIndex.ToString(CultureInfo.InvariantCulture));
        if (data.SnowShadowSize > 0) SetOrIgnore(document, "SnowShadowSize", data.SnowShadowSize.ToString(CultureInfo.InvariantCulture));
        if (data.HagelShadowIndex >= 0) SetOrIgnore(document, "HagelShadowIndex", data.HagelShadowIndex.ToString(CultureInfo.InvariantCulture));
        if (data.HagelShadowSize > 0) SetOrIgnore(document, "HagelShadowSize", data.HagelShadowSize.ToString(CultureInfo.InvariantCulture));
        if (data.MoveListAmplitude > 0) SetOrIgnore(document, "MoveListAmplitude", data.MoveListAmplitude.ToString(CultureInfo.InvariantCulture));

        // 晝夜與陰影網格
        SetOrIgnore(document, "DayStartTime", data.DayStartTime.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "DayEndTime", data.DayEndTime.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "ShadowMeshMode", data.ShadowMeshMode.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "ShadowMeshAccuracy", data.ShadowMeshAccuracy.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "ShadowMeshXZsize", data.ShadowMeshXZsize.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "ShadowMeshYsize", data.ShadowMeshYsize.ToString(CultureInfo.InvariantCulture));

        // 天空與衍生快取開關
        SetOrIgnore(document, "Skydensspread", data.Skydensspread.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "Skydensaccuracy", data.Skydensaccuracy.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "HandleSkyDensMap", data.HandleSkyDensMap.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "HandleVisibleMap", data.HandleVisibleMap.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "HandleClipRectMap", data.HandleClipRectMap.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "HandleShadowMeshes", data.HandleShadowMeshes.ToString(CultureInfo.InvariantCulture));
        SetOrIgnore(document, "ShowCollisionMesh", data.ShowCollisionMesh.ToString(CultureInfo.InvariantCulture));
    }

    /// <summary>
    /// 產生全新原生 boden.ini 格式文字（依標準原版次序與德語註解風格格式化）。
    /// </summary>
    public static string GenerateDefaultIni(BodenIniData data)
    {
        ArgumentNullException.ThrowIfNull(data);
        var sb = new StringBuilder();

        void AppendKey(string key, object value, string? comment = null)
        {
            sb.Append('[').Append(key).Append(']');
            if (!string.IsNullOrEmpty(comment)) sb.Append(" ;").Append(comment);
            sb.AppendLine();
            sb.AppendLine(Convert.ToString(value, CultureInfo.InvariantCulture));
        }

        AppendKey("Waterlevel", data.Waterlevel, "0..");
        AppendKey("Heightmapstep", data.Heightmapstep, "1..");
        AppendKey("ShadowMeshMode", data.ShadowMeshMode, "0=太陽只上下移動, 1=太陽東南西原始移動");
        AppendKey("WasserTexturName", data.WasserTexturName, "水面貼圖基底名 name00.bmp,...");
        AppendKey("CausticTexturName", data.CausticTexturName);
        AppendKey("SkyTexturName", data.SkyTexturName);
        AppendKey("RainDropsOnWater", data.RainDropsOnWater ? "1" : "0", "0/1");
        AppendKey("WaterWarpShift", data.WaterWarpShift, "18(低)..10(高), 14=default");
        AppendKey("WaterBumpAmplitude", data.WaterBumpAmplitude, "0..1024, 256=default");
        AppendKey("WaterBumpFrequency", data.WaterBumpFrequency, "1..16, 4=default");
        AppendKey("FlashPropability", data.FlashPropability, "每秒閃電數 0..1000");
        AppendKey("WaterColor", data.WaterColor, "水色 Hex (bgr), default=0xffbf7f");
        AppendKey("DayStartTime", data.DayStartTime, "白天開始(時)");
        AppendKey("DayEndTime", data.DayEndTime, "夜晚開始(時)");

        if (data.FlashObjectDefaultIndex >= 0) AppendKey("FlashObjectDefaultIndex", data.FlashObjectDefaultIndex);
        if (data.FlashObjectDefault2Index >= 0) AppendKey("FlashObjectDefault2Index", data.FlashObjectDefault2Index);
        if (data.FlashLightDefaultIndex >= 0) AppendKey("FlashLightDefaultIndex", data.FlashLightDefaultIndex);
        if (data.SnowAlrIndex >= 0) AppendKey("SnowAlrIndex", data.SnowAlrIndex);
        if (data.SnowShadowIndex >= 0) AppendKey("SnowShadowIndex", data.SnowShadowIndex);
        if (data.SnowShadowSize > 0) AppendKey("SnowShadowSize", data.SnowShadowSize);
        if (data.HagelShadowIndex >= 0) AppendKey("HagelShadowIndex", data.HagelShadowIndex);
        if (data.HagelShadowSize > 0) AppendKey("HagelShadowSize", data.HagelShadowSize);
        if (data.MoveListAmplitude > 0) AppendKey("MoveListAmplitude", data.MoveListAmplitude);

        AppendKey("ShadowMeshAccuracy", data.ShadowMeshAccuracy);
        AppendKey("ShadowMeshXZsize", data.ShadowMeshXZsize);
        AppendKey("ShadowMeshYsize", data.ShadowMeshYsize);
        AppendKey("Skydensspread", data.Skydensspread);
        AppendKey("Skydensaccuracy", data.Skydensaccuracy);
        AppendKey("HandleSkyDensMap", data.HandleSkyDensMap);
        AppendKey("HandleVisibleMap", data.HandleVisibleMap);
        AppendKey("HandleClipRectMap", data.HandleClipRectMap);
        AppendKey("HandleShadowMeshes", data.HandleShadowMeshes);
        AppendKey("ShowCollisionMesh", data.ShowCollisionMesh);

        // 附加所有保留之未知區段
        foreach (var kvp in data.PreservedSections)
        {
            if (!KnownKeys.Contains(kvp.Key))
            {
                sb.Append('[').Append(kvp.Key).Append(']').AppendLine();
                sb.AppendLine(kvp.Value);
            }
        }

        return sb.ToString();
    }

    /// <summary>
    /// 檢驗 boden.ini 參數值是否位於遊戲引擎安全合規範圍內。
    /// </summary>
    public static bool Validate(BodenIniData data, out IReadOnlyList<string> validationErrors)
    {
        ArgumentNullException.ThrowIfNull(data);
        var errors = new List<string>();

        if (data.WaterBumpAmplitude is < 0 or > 1024)
            errors.Add($"WaterBumpAmplitude 數值 ({data.WaterBumpAmplitude}) 超出安全範圍 0..1024。");

        if (data.WaterBumpFrequency is < 1 or > 16)
            errors.Add($"WaterBumpFrequency 數值 ({data.WaterBumpFrequency}) 超出安全範圍 1..16。");

        if (data.WaterWarpShift is < 8 or > 20)
            errors.Add($"WaterWarpShift 數值 ({data.WaterWarpShift}) 超出安全範圍 8..20 (標準為 10..18)。");

        if (data.FlashPropability is < 0 or > 1000)
            errors.Add($"FlashPropability 數值 ({data.FlashPropability}) 超出安全範圍 0..1000。");

        if (data.DayStartTime is < 0f or > 24f)
            errors.Add($"DayStartTime 數值 ({data.DayStartTime}) 必須介於 0..24 之間。");

        if (data.DayEndTime is < 0f or > 24f)
            errors.Add($"DayEndTime 數值 ({data.DayEndTime}) 必須介於 0..24 之間。");

        if (data.Heightmapstep <= 0f)
            errors.Add($"Heightmapstep 數值 ({data.Heightmapstep}) 必須大於 0。");

        if (data.ShadowMeshMode is not (0 or 1))
            errors.Add($"ShadowMeshMode 數值 ({data.ShadowMeshMode}) 僅支援 0 或 1。");

        string cleanHex = data.WaterColor.Trim().Replace("0x", "", StringComparison.OrdinalIgnoreCase);
        if (cleanHex.Length != 6 || !uint.TryParse(cleanHex, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out _))
            errors.Add($"WaterColor 格式錯誤 ({data.WaterColor})，必須為 6 碼十六進位 BGR 色碼 (例: 0xffdfbf)。");

        validationErrors = errors;
        return errors.Count == 0;
    }

    private static void PopulateFromDocument(BodenIniDocument doc, BodenIniData data)
    {
        if (doc.GetValue("Waterlevel") is { } wl && float.TryParse(wl, NumberStyles.Float, CultureInfo.InvariantCulture, out float waterLevel)) data.Waterlevel = waterLevel;
        if (doc.GetValue("Heightmapstep") is { } hms && float.TryParse(hms, NumberStyles.Float, CultureInfo.InvariantCulture, out float step)) data.Heightmapstep = step;
        if (doc.GetValue("WaterBumpAmplitude") is { } wba && int.TryParse(wba, NumberStyles.Integer, CultureInfo.InvariantCulture, out int bumpAmp)) data.WaterBumpAmplitude = bumpAmp;
        if (doc.GetValue("WaterBumpFrequency") is { } wbf && int.TryParse(wbf, NumberStyles.Integer, CultureInfo.InvariantCulture, out int bumpFreq)) data.WaterBumpFrequency = bumpFreq;
        if (doc.GetValue("WaterWarpShift") is { } wws && int.TryParse(wws, NumberStyles.Integer, CultureInfo.InvariantCulture, out int warpShift)) data.WaterWarpShift = warpShift;
        if (doc.GetValue("RainDropsOnWater") is { } rdw) data.RainDropsOnWater = rdw.Trim() == "1";
        if (doc.GetValue("WaterColor") is { } wc && !string.IsNullOrWhiteSpace(wc)) data.WaterColor = wc.Trim();
        if (doc.GetValue("WasserTexturName") is { } wtn && !string.IsNullOrWhiteSpace(wtn)) data.WasserTexturName = wtn.Trim();
        if (doc.GetValue("CausticTexturName") is { } ctn && !string.IsNullOrWhiteSpace(ctn)) data.CausticTexturName = ctn.Trim();
        if (doc.GetValue("SkyTexturName") is { } stn && !string.IsNullOrWhiteSpace(stn)) data.SkyTexturName = stn.Trim();

        // 閃電機率
        if (doc.GetValue("FlashPropability") is { } fp && int.TryParse(fp, NumberStyles.Integer, CultureInfo.InvariantCulture, out int flashProb)) data.FlashPropability = flashProb;
        if (doc.GetValue("FlashObjectDefaultIndex") is { } fo && int.TryParse(fo, NumberStyles.Integer, CultureInfo.InvariantCulture, out int foIdx)) data.FlashObjectDefaultIndex = foIdx;
        if (doc.GetValue("FlashObjectDefault2Index") is { } fo2 && int.TryParse(fo2, NumberStyles.Integer, CultureInfo.InvariantCulture, out int fo2Idx)) data.FlashObjectDefault2Index = fo2Idx;
        if (doc.GetValue("FlashLightDefaultIndex") is { } fl && int.TryParse(fl, NumberStyles.Integer, CultureInfo.InvariantCulture, out int flIdx)) data.FlashLightDefaultIndex = flIdx;

        // 冰雪
        if (doc.GetValue("SnowAlrIndex") is { } sa && int.TryParse(sa, NumberStyles.Integer, CultureInfo.InvariantCulture, out int saIdx)) data.SnowAlrIndex = saIdx;
        if (doc.GetValue("SnowShadowIndex") is { } ss && int.TryParse(ss, NumberStyles.Integer, CultureInfo.InvariantCulture, out int ssIdx)) data.SnowShadowIndex = ssIdx;
        if (doc.GetValue("SnowShadowSize") is { } sss && int.TryParse(sss, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sssVal)) data.SnowShadowSize = sssVal;
        if (doc.GetValue("HagelShadowIndex") is { } hs && int.TryParse(hs, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hsIdx)) data.HagelShadowIndex = hsIdx;
        if (doc.GetValue("HagelShadowSize") is { } hss && int.TryParse(hss, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hssVal)) data.HagelShadowSize = hssVal;
        if (doc.GetValue("MoveListAmplitude") is { } mla && int.TryParse(mla, NumberStyles.Integer, CultureInfo.InvariantCulture, out int mlaVal)) data.MoveListAmplitude = mlaVal;

        // 晝夜與陰影
        if (doc.GetValue("DayStartTime") is { } dst && float.TryParse(dst, NumberStyles.Float, CultureInfo.InvariantCulture, out float dayStart)) data.DayStartTime = dayStart;
        if (doc.GetValue("DayEndTime") is { } det && float.TryParse(det, NumberStyles.Float, CultureInfo.InvariantCulture, out float dayEnd)) data.DayEndTime = dayEnd;
        if (doc.GetValue("ShadowMeshMode") is { } smm && int.TryParse(smm, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smMode)) data.ShadowMeshMode = smMode;
        if (doc.GetValue("ShadowMeshAccuracy") is { } sma && int.TryParse(sma, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smAcc)) data.ShadowMeshAccuracy = smAcc;
        if (doc.GetValue("ShadowMeshXZsize") is { } smxz && int.TryParse(smxz, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smXZ)) data.ShadowMeshXZsize = smXZ;
        if (doc.GetValue("ShadowMeshYsize") is { } smy && int.TryParse(smy, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smY)) data.ShadowMeshYsize = smY;

        // 快取開關
        if (doc.GetValue("Skydensspread") is { } sds && float.TryParse(sds, NumberStyles.Float, CultureInfo.InvariantCulture, out float skySpread)) data.Skydensspread = skySpread;
        if (doc.GetValue("Skydensaccuracy") is { } sda && int.TryParse(sda, NumberStyles.Integer, CultureInfo.InvariantCulture, out int skyAcc)) data.Skydensaccuracy = skyAcc;
        if (doc.GetValue("HandleSkyDensMap") is { } hsd && int.TryParse(hsd, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hSky)) data.HandleSkyDensMap = hSky;
        if (doc.GetValue("HandleVisibleMap") is { } hvm && int.TryParse(hvm, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hVis)) data.HandleVisibleMap = hVis;
        if (doc.GetValue("HandleClipRectMap") is { } hcr && int.TryParse(hcr, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hClip)) data.HandleClipRectMap = hClip;
        if (doc.GetValue("HandleShadowMeshes") is { } hsm && int.TryParse(hsm, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hShad)) data.HandleShadowMeshes = hShad;
        if (doc.GetValue("ShowCollisionMesh") is { } scm && int.TryParse(scm, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sColl)) data.ShowCollisionMesh = sColl;
    }

    private static void ApplyValue(BodenIniData data, string key, string value)
    {
        switch (key)
        {
            case "Waterlevel" when float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out float wl): data.Waterlevel = wl; break;
            case "Heightmapstep" when float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out float hms): data.Heightmapstep = hms; break;
            case "WaterBumpAmplitude" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int wba): data.WaterBumpAmplitude = wba; break;
            case "WaterBumpFrequency" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int wbf): data.WaterBumpFrequency = wbf; break;
            case "WaterWarpShift" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int wws): data.WaterWarpShift = wws; break;
            case "RainDropsOnWater": data.RainDropsOnWater = value == "1"; break;
            case "WaterColor": data.WaterColor = value; break;
            case "WasserTexturName": data.WasserTexturName = value; break;
            case "CausticTexturName": data.CausticTexturName = value; break;
            case "SkyTexturName": data.SkyTexturName = value; break;
            case "FlashPropability" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int fp): data.FlashPropability = fp; break;
            case "FlashObjectDefaultIndex" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int fo): data.FlashObjectDefaultIndex = fo; break;
            case "FlashObjectDefault2Index" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int fo2): data.FlashObjectDefault2Index = fo2; break;
            case "FlashLightDefaultIndex" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int fl): data.FlashLightDefaultIndex = fl; break;
            case "SnowAlrIndex" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sa): data.SnowAlrIndex = sa; break;
            case "SnowShadowIndex" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int ss): data.SnowShadowIndex = ss; break;
            case "SnowShadowSize" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sss): data.SnowShadowSize = sss; break;
            case "HagelShadowIndex" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hs): data.HagelShadowIndex = hs; break;
            case "HagelShadowSize" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hss): data.HagelShadowSize = hss; break;
            case "MoveListAmplitude" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int mla): data.MoveListAmplitude = mla; break;
            case "DayStartTime" when float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out float dst): data.DayStartTime = dst; break;
            case "DayEndTime" when float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out float det): data.DayEndTime = det; break;
            case "ShadowMeshMode" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smm): data.ShadowMeshMode = smm; break;
            case "ShadowMeshAccuracy" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sma): data.ShadowMeshAccuracy = sma; break;
            case "ShadowMeshXZsize" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smxz): data.ShadowMeshXZsize = smxz; break;
            case "ShadowMeshYsize" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int smy): data.ShadowMeshYsize = smy; break;
            case "Skydensspread" when float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out float sds): data.Skydensspread = sds; break;
            case "Skydensaccuracy" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int sda): data.Skydensaccuracy = sda; break;
            case "HandleSkyDensMap" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hsd): data.HandleSkyDensMap = hsd; break;
            case "HandleVisibleMap" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hvm): data.HandleVisibleMap = hvm; break;
            case "HandleClipRectMap" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hcr): data.HandleClipRectMap = hcr; break;
            case "HandleShadowMeshes" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hsm): data.HandleShadowMeshes = hsm; break;
            case "ShowCollisionMesh" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out int scm): data.ShowCollisionMesh = scm; break;
        }
    }

    private static void SetOrIgnore(BodenIniDocument doc, string section, string value)
    {
        try
        {
            doc.SetValue(section, value);
        }
        catch (KeyNotFoundException)
        {
            // 若原檔未含此選擇性欄位，安全略過，維持原生檔案最小侵入原則。
        }
    }
}
