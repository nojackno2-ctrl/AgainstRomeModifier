namespace AgainstRomeModifier.Maps;

/// <summary>物件類型的大類（依原版 namedef 前綴）。</summary>
public enum SdlObjectCategory { Building, UnitGroup, Figure, Effect, Other }

/// <summary>
/// 一種可放置的物件類型：namedef、def 編號與一份完整的原版欄位範本。
/// 範本取自原版地圖的 SDL，放置時只覆寫位置、隊伍、角度、數量與 onload，其餘欄位維持原版組合。
/// </summary>
public sealed record SdlObjectType(string NameDef, int Definition, SdlObjectCategory Category, string Tribe, int Occurrences,
    IReadOnlyDictionary<string, string> TemplateFields)
{
    /// <summary>部隊圖示（Ver…Ico）的 objdefn0 指向實際兵種；anzv 第一個值為人數。</summary>
    public bool HasUnitCount => Category == SdlObjectCategory.UnitGroup;
}

/// <summary>
/// 從遊戲目錄所有地圖的 SDL 建立物件目錄。
/// 引擎在載入地圖時會列舉地圖目錄下所有 *.sdl（EXE 字串 "%s%s/*.sdl"、"tcon_buildonload"），
/// 並直接建造 onload=1 的物件（原版 HIST_009/vil.sdl 以此預放領主與部隊）。
/// </summary>
public static class SdlObjectCatalog
{
    public static IReadOnlyList<SdlObjectType> Build(string gamePath)
    {
        string maps = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(maps)) return Array.Empty<SdlObjectType>();
        var templates = new Dictionary<string, (IReadOnlyDictionary<string, string> Fields, int Count, bool PreferredTemplate)>(StringComparer.OrdinalIgnoreCase);
        foreach (string mapDirectory in Directory.GetDirectories(maps))
        {
            if (CustomMapManifest.IsCustomMapDirectory(mapDirectory)) continue; // 只用原版資料當範本
            foreach (string path in Directory.GetFiles(mapDirectory, "*.sdl", SearchOption.TopDirectoryOnly))
            {
                SdlDocument document;
                try { document = SdlDocument.Load(path); }
                catch (Exception ex) when (ex is InvalidDataException or IOException or System.Text.DecoderFallbackException) { continue; }
                foreach (SdlObjectSection section in document.Objects)
                {
                    string? name = section.GetValue("namedef")?.Trim();
                    if (string.IsNullOrEmpty(name) || !int.TryParse(section.GetValue("def"), out int definition) || definition < 0) continue;
                    // onload=1 的範本較接近「預放物件」的寫法（例如部隊圖示的 objdefn0/anzv 已填好），優先採用。
                    bool preferred = section.GetValue("onload")?.Trim() == "1";
                    if (templates.TryGetValue(name, out var existing))
                    {
                        templates[name] = (preferred && !existing.PreferredTemplate ? section.Fields : existing.Fields, existing.Count + 1, existing.PreferredTemplate || preferred);
                    }
                    else templates[name] = (section.Fields, 1, preferred);
                }
            }
        }
        return templates
            .Select(pair => new SdlObjectType(pair.Key, int.Parse(pair.Value.Fields["def"].Trim(), System.Globalization.CultureInfo.InvariantCulture),
                Categorize(pair.Key), Tribe(pair.Key), pair.Value.Count, pair.Value.Fields))
            .OrderBy(type => type.Category).ThenBy(type => type.Tribe).ThenBy(type => type.NameDef, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    public static SdlObjectCategory Categorize(string nameDef) => nameDef switch
    {
        _ when nameDef.StartsWith("Bau", StringComparison.OrdinalIgnoreCase) => SdlObjectCategory.Building,
        _ when nameDef.StartsWith("Ver", StringComparison.OrdinalIgnoreCase) && nameDef.Contains("Ico", StringComparison.OrdinalIgnoreCase) => SdlObjectCategory.UnitGroup,
        _ when nameDef.StartsWith("Fig", StringComparison.OrdinalIgnoreCase) => SdlObjectCategory.Figure,
        _ when nameDef.StartsWith("FX", StringComparison.OrdinalIgnoreCase) => SdlObjectCategory.Effect,
        _ => SdlObjectCategory.Other,
    };

    /// <summary>namedef 第 4–6 字元為部族代碼（Ger／Hun／Kel／Rom），其餘為共通物件。</summary>
    public static string Tribe(string nameDef)
    {
        if (nameDef.Length < 6) return "";
        string code = nameDef.Substring(3, 3);
        return code is "Ger" or "Hun" or "Kel" or "Rom" ? code : "";
    }
}

/// <summary>編輯器暫存的一個預放物件（世界絕對座標）。</summary>
public sealed record SdlPlacedObject(SdlObjectType Type, float WorldX, float WorldY, float WorldZ, int Team, float Angle = 0, int UnitCount = 0)
{
    public Guid ScenarioId { get; init; }
}

/// <summary>
/// 把預放物件寫入地圖目錄中的專用 SDL（refpos 0、絕對座標、onload=1）。
/// 以獨立檔案存放，原版聚落藍圖（AI 建村模板）維持不變；整份檔案每次儲存時重新產生。
/// </summary>
public static class SdlPlacedObjectsFile
{
    public const string FileName = "ARM_Placed.sdl";
    public const string SettlementName = "ARM_Placed";

    public static string Render(IReadOnlyList<SdlPlacedObject> objects)
    {
        var text = new System.Text.StringBuilder();
        text.Append("[settlement]\r\n");
        text.Append("name    =").Append(SettlementName).Append("\r\n");
        text.Append("createdt=").Append(DateTime.Now.ToString("yyyy/MM/dd", System.Globalization.CultureInfo.InvariantCulture)).Append("\r\n");
        text.Append("createtm=").Append(DateTime.Now.ToString("HH:mm:ss", System.Globalization.CultureInfo.InvariantCulture)).Append("\r\n");
        text.Append("teamsett=-1\r\n");
        text.Append("refpos  =0,0,0\r\n");
        for (int index = 0; index < objects.Count; index++)
        {
            SdlPlacedObject item = objects[index];
            if (!float.IsFinite(item.WorldX) || !float.IsFinite(item.WorldY) || !float.IsFinite(item.WorldZ)) throw new InvalidDataException("預放物件座標必須是有限數值。");
            if (item.Team is < -1 or > 15) throw new InvalidDataException("預放物件隊伍必須介於 -1 與 15。");
            var fields = new Dictionary<string, string>(item.Type.TemplateFields, StringComparer.OrdinalIgnoreCase)
            {
                ["name"] = "",
                ["pos"] = new SdlVector3(item.WorldX, item.WorldY, item.WorldZ).ToString("0.00"),
                ["team"] = item.Team.ToString(System.Globalization.CultureInfo.InvariantCulture),
                ["angle"] = item.Angle.ToString("0.00", System.Globalization.CultureInfo.InvariantCulture),
                ["onload"] = "1",
            };
            if (item.Type.HasUnitCount && item.UnitCount > 0)
            {
                string[] counts = (fields.TryGetValue("anzv", out string? anzv) ? anzv : "0,0,0").Split(',');
                counts[0] = Math.Clamp(item.UnitCount, 1, 50).ToString(System.Globalization.CultureInfo.InvariantCulture);
                fields["anzv"] = string.Join(",", counts);
            }
            text.Append("\r\n[object").Append(index.ToString("0000", System.Globalization.CultureInfo.InvariantCulture)).Append("]\r\n");
            foreach ((string key, string value) in fields)
            {
                if (key.IndexOfAny(['=', '\r', '\n']) >= 0 || value.IndexOfAny(['\r', '\n']) >= 0) throw new InvalidDataException("SDL 範本欄位含有無效字元。");
                text.Append(key.PadRight(8)).Append('=').Append(value).Append("\r\n");
            }
        }
        return text.ToString();
    }

    /// <summary>讀回編輯器寫出的預放物件（依目錄比對 namedef）；檔案不存在時回傳空集合。</summary>
    public static IReadOnlyList<SdlPlacedObject> Load(string mapDirectory, IReadOnlyList<SdlObjectType> catalog)
    {
        string path = Path.Combine(mapDirectory, FileName);
        if (!File.Exists(path)) return Array.Empty<SdlPlacedObject>();
        var byName = catalog.ToDictionary(type => type.NameDef, StringComparer.OrdinalIgnoreCase);
        var result = new List<SdlPlacedObject>();
        foreach (SdlObjectSection section in SdlDocument.Load(path).Objects)
        {
            string? name = section.GetValue("namedef")?.Trim();
            SdlObjectType type = name is not null && byName.TryGetValue(name, out SdlObjectType? known)
                ? known
                : new SdlObjectType(name ?? "", int.TryParse(section.GetValue("def"), out int def) ? def : -1, SdlObjectCategory.Other, "", 0, section.Fields);
            SdlVector3 position = SdlVector3.Parse(section.GetValue("pos") ?? "0,0,0");
            _ = int.TryParse(section.GetValue("team"), out int team);
            float.TryParse(section.GetValue("angle"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float angle);
            _ = int.TryParse((section.GetValue("anzv") ?? "0").Split(',')[0], out int count);
            result.Add(new SdlPlacedObject(type, position.X, position.Y, position.Z, team, angle, count));
        }
        return result;
    }

    /// <summary>以 PFIL（沿用同地圖既有 SDL 的 64-byte 標頭）寫入；沒有物件時刪除檔案。全部在呼叫端的交易內。</summary>
    public static void Save(string mapDirectory, IReadOnlyList<SdlPlacedObject> objects, FileRollbackScope rollback)
    {
        string path = Path.Combine(mapDirectory, FileName);
        rollback.TrackFile(path);
        if (objects.Count == 0)
        {
            if (File.Exists(path)) File.Delete(path);
            return;
        }
        string? headerSource = Directory.GetFiles(mapDirectory, "*.sdl").FirstOrDefault(file => !Path.GetFileName(file).Equals(FileName, StringComparison.OrdinalIgnoreCase));
        byte[] plain = MapTextEncoding.Game.GetBytes(Render(objects));
        byte[]? template = headerSource is null ? null : File.ReadAllBytes(headerSource);
        byte[] bytes = template is { Length: >= 64 } && template[0] == 'P' && template[1] == 'F' && template[2] == 'I' && template[3] == 'L'
            ? GameLZSS.CompressPfil(plain, template)
            : plain;
        Core.Services.SafeFileWriter.WriteAllBytes(path, bytes, rollback);
    }
}

/// <summary>地圖文字的 CP1251 編碼（與 MapTextDocument 相同規則）。</summary>
public static class MapTextEncoding
{
    public static System.Text.Encoding Game { get; } = Create();
    private static System.Text.Encoding Create()
    {
        System.Text.Encoding.RegisterProvider(System.Text.CodePagesEncodingProvider.Instance);
        return System.Text.Encoding.GetEncoding(1251, System.Text.EncoderFallback.ExceptionFallback, System.Text.DecoderFallback.ExceptionFallback);
    }
}
