using System.Globalization;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly FormationLayoutDefinition _formationDefinition;
    private IReadOnlyList<SdlObjectType>? _troopSdlTemplates;

    private static FormationLayoutDefinition LoadFormationDefinition(string gamePath)
    {
        string path = Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS", "formdef.dau");
        if (!File.Exists(path)) return FormationLayoutDefinition.NativeDefault;
        try
        {
            byte[] bytes = File.ReadAllBytes(path);
            if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8)) bytes = GameLZSS.DecompressPfil(bytes);
            return FormationLayoutDefinition.FromFormDefText(MapTextEncoding.Game.GetString(bytes));
        }
        // 只影響部隊的 3D 顯示：讀不到或格式不符時退回原生預設隊形，不阻擋開啟編輯器。
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException or FormatException
            or ArgumentException or System.Text.DecoderFallbackException) { return FormationLayoutDefinition.NativeDefault; }
    }

    /// <summary>僅產生 3D 顯示物件；所有成員沿用部隊來源與索引，不加入 placement／SDL／DATA。</summary>
    private IEnumerable<MapSceneObject> ExpandTroopsFor3D(IReadOnlyList<MapSceneObject> effective)
    {
        IReadOnlyList<SdlPlacedObject> placed = _placedObjects;
        foreach (MapSceneObject marker in effective)
        {
            int index = -5000 - marker.ObjectIndex;
            if (!marker.SourceFile.Equals(SdlPlacedObjectsFile.FileName, StringComparison.OrdinalIgnoreCase)
                || index < 0 || index >= placed.Count)
            { yield return marker; continue; }
            SdlPlacedObject spawn = placed[index];
            string? soldier = SoldierName(spawn.Type);
            if (soldier is null || spawn.UnitCount <= 0)
            { yield return marker; continue; }

            string? banner = BannerName(spawn.Type, soldier);
            // 未能解析旗幟時仍保留單一 spawn marker（同原 2D／選取行為）。
            yield return marker with { Name = banner ?? $"TroopSpawn:{AliasOf(spawn.Type)}" };
            foreach (var offset in FormationLayout.Create(Math.Min(spawn.UnitCount, FormationLayout.MaximumScenarioMembers), spawn.Angle, _formationDefinition))
                yield return marker with { Name = soldier, WorldX = spawn.WorldX + offset.X, WorldZ = spawn.WorldZ + offset.Y };
        }
    }

    private string? SoldierName(SdlObjectType type)
    {
        if (type.Category == SdlObjectCategory.Figure) return type.NameDef;
        if (!type.HasUnitCount) return null;
        if (!type.TemplateFields.TryGetValue("objdefn0", out string? member))
        {
            _troopSdlTemplates ??= SdlObjectCatalog.Build(_gamePath);
            SdlObjectType? template = _troopSdlTemplates.FirstOrDefault(item => item.NameDef.Equals(type.NameDef, StringComparison.OrdinalIgnoreCase));
            if (template is null || !template.TemplateFields.TryGetValue("objdefn0", out member)) return null;
        }
        member = member.Trim();
        if (int.TryParse(member, NumberStyles.Integer, CultureInfo.InvariantCulture, out int id))
            return _objdefNames.GetValueOrDefault(id);
        return _objectCatalog.FirstOrDefault(item => AliasOf(item).Equals(member, StringComparison.OrdinalIgnoreCase))?.NameDef
            ?? (member.StartsWith("Fig", StringComparison.OrdinalIgnoreCase) ? member : null);
    }

    private string? BannerName(SdlObjectType type, string soldier)
    {
        if (type.HasUnitCount) return type.NameDef;
        _troopSdlTemplates ??= SdlObjectCatalog.Build(_gamePath);
        SdlObjectType? template = _troopSdlTemplates.FirstOrDefault(item =>
            item.HasUnitCount && SoldierName(item)?.Equals(soldier, StringComparison.OrdinalIgnoreCase) == true);
        if (template is not null) return template.NameDef;
        // 缺少 SDL 部隊範本的副本：僅使用已存在的原版軍事旗幟定義。
        // team 的民族／旗幟變體依遊戲狀態而異，這裡尚未模擬。
        string name = $"Ver{SdlObjectCatalog.Tribe(soldier)}KamIco00_Kampf_Icon";
        return _objdefNames.Values.Contains(name, StringComparer.OrdinalIgnoreCase)
            || _spriteCatalog?.TryGetDefinition(name, out _) == true ? name : null;
    }
}
