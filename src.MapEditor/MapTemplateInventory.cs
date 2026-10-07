using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed record MapTemplateInventory(int SdlFiles, int SdlObjects, int OnloadObjects, int Scripts, int Placements, int Events,
    int? RemovableNature, int? ProtectedNative, IReadOnlyList<string> Unknown)
{
    internal static MapTemplateInventory Read(string gamePath, string map)
    {
        int files = 0, objects = 0, onload = 0, placements = 0, events = 0;
        var unknown = new List<string>();
        foreach (string path in Directory.GetFiles(map, "*.sdl"))
        {
            files++;
            try
            {
                var document = SdlDocument.Load(path); objects += document.Objects.Count;
                onload += document.Objects.Count(item => item.GetValue("onload")?.Trim() == "1");
            }
            catch (Exception ex) when (ex is IOException or InvalidDataException or ArgumentException) { unknown.Add(Path.GetFileName(path)); }
        }
        try { var scenario = ScenarioDocument.Load(map); placements = scenario.Spawns.Count; events = scenario.Events.Count; }
        catch (Exception ex) when (ex is IOException or InvalidDataException or System.Text.Json.JsonException) { unknown.Add("arm_scenario.json"); }
        int? removable = null, protectedNative = null;
        try
        {
            var names = ObjDefNames.Load(gamePath);
            var native = LevelObjectStore.Load(map).Objects();
            removable = native.Count(item => !item.Linked && ObjDefNames.IsLandscape(names.GetValueOrDefault(item.TypeId)));
            protectedNative = native.Count - removable;
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or ArgumentException) { unknown.Add("DATA native objects"); }
        string scripts = Path.Combine(map, "SCRIPT");
        return new(files, objects, onload, Directory.Exists(scripts) ? Directory.GetFiles(scripts, "*.bci", SearchOption.AllDirectories).Length : 0,
            placements, events, removable, protectedNative, unknown);
    }

    internal string Describe(string sourceId, bool flat, bool en)
    {
        string unknown = en ? "unknown" : "未知";
        string native = ProtectedNative?.ToString() ?? unknown, scenery = RemovableNature?.ToString() ?? unknown;
        string text = en
            ? $"Source: {sourceId}\r\nKept: {SdlFiles} settlement blueprints / {SdlObjects} object definitions ({OnloadObjects} placed at startup), {Scripts} startup scripts, {Placements} editor placements, {Events} editor events.\r\nObjects kept: {native} buildings, markers, linked or other objects.\r\n"
            : $"來源：{sourceId}\r\n保留：{SdlFiles} 份聚落藍圖／{SdlObjects} 個物件定義（{OnloadObjects} 個開局放置）、{Scripts} 份啟動腳本、{Placements} 個編輯器放置物件、{Events} 個編輯器事件。\r\n保留物件：{native} 個建築、標記、連結或其他物件。\r\n";
        text += flat
            ? en ? $"Flattened: terrain, material, collision and auxiliary layers. Cleared: {scenery} removable scenery objects. Settlements and scripts remain; this is a flat template. Terrain changes are saved from the editor."
                : $"整平：地形、材質、阻擋及輔助層。清除：{scenery} 個可移除地景。聚落與腳本保留；此為平坦範本。地形變更須在編輯器儲存。"
            : en ? $"Terrain and {scenery} removable scenery objects are also kept." : $"地形與 {scenery} 個可移除地景也保留。";
        if (Unknown.Count > 0) text += (en ? "\r\nSome counts are incomplete: " : "\r\n部分數量未能完整讀取：") + string.Join(", ", Unknown);
        return text;
    }
}
