using System.Reflection;
using System.Runtime.ExceptionServices;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 遊戲內驗收用：以真正的 MapEditorForm 與存檔交易，把指定案例寫入已安裝遊戲的測試地圖。
/// 僅在同時設定 ARM_GAME_PATH、ARM_INGAME_MAP（例如 ENDL_005）與 ARM_INGAME_SCENARIO 時執行；
/// 會修改該自製地圖，執行前須先備份。案例：
/// victory＝3 秒訊息＋第一支 team 0 部隊進入其東方矩形後訊息與勝利；
/// defeat＝3 秒訊息＋45 秒後訊息與失敗（用於存讀檔後事件是否延續）；
/// terrain＝出生點附近示範區（不同材質自動過渡、粗糙化丘陵、5×5 小湖、茂密混合森林、印章道路）＋3 秒訊息；
/// angles＝出生部隊東側一列 8 名同型單兵，角度 0、45…315，供遊戲內比對角度與 ALR 方向列。
/// </summary>
public sealed class InGameAcceptanceScenarioTests
{
    [Fact]
    public void Write_in_game_acceptance_scenario()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        string? mapId = Environment.GetEnvironmentVariable("ARM_INGAME_MAP");
        string? scenario = Environment.GetEnvironmentVariable("ARM_INGAME_SCENARIO");
        if (string.IsNullOrWhiteSpace(game) || string.IsNullOrWhiteSpace(mapId) || string.IsNullOrWhiteSpace(scenario)) return;
        GameMapInfo map = new GameMapCatalog().Require(game, mapId);
        Assert.True(map.IsCustom, "只允許寫入自製地圖。");
        string report = "";
        RunInSta(() =>
        {
            using var form = new MapEditorForm(game, map);
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var unit = form.PlacementSession.Capture().First(item => item.Type.Category == SdlObjectCategory.Figure && item.Team == 0);
            var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            events.RemoveAll(item => item.Name.StartsWith("ARM ", StringComparison.Ordinal));
            events.Add(new("ARM start", 3) { Actions = [new(ScenarioActionKind.Message, scenario == "victory"
                ? "ARM test: move the soldiers east into the marked area."
                : "ARM test: defeat in 45 seconds. Save and load now.")] });
            if (scenario == "victory")
            {
                int x1 = (int)unit.WorldX + 1000, x2 = x1 + 1000, z1 = (int)unit.WorldZ - 600, z2 = (int)unit.WorldZ + 600;
                events.Add(new("ARM area win", 1)
                {
                    Conditions = [new(ScenarioConditionKind.ObjectInArea, unit.ScenarioId, x1, z1, x2, z2)],
                    Actions = [new(ScenarioActionKind.Message, "ARM test: area reached - victory."), new(ScenarioActionKind.Victory)]
                });
                report = $"unit {unit.Type.NameDef} id={unit.ScenarioId} at ({unit.WorldX},{unit.WorldZ}); area X {x1}-{x2}, Z {z1}-{z2}";
            }
            else if (scenario == "defeat")
            {
                events.Add(new("ARM timed defeat", 45) { Actions = [new(ScenarioActionKind.Message, "ARM test: timer elapsed - defeat."), new(ScenarioActionKind.Defeat)] });
                report = "timed defeat at 45 s";
            }
            else if (scenario == "angles")
            {
                // Eight single soldiers of the start unit's type in a row along +X (screen right-down), angles 0..315,
                // so in-game sprite matching can map scenario angles to ALR direction rows.
                var placed = new List<string>();
                for (int index = 0; index < 8; index++)
                {
                    float x = unit.WorldX + 700 + index * 250, z = unit.WorldZ - 300;
                    form.PlacementSession.Add(new SdlPlacedObject(unit.Type, x, unit.WorldY, z, 0, index * 45f, 1) { ScenarioId = Guid.NewGuid() });
                    placed.Add($"{index * 45}deg@({x},{z})");
                }
                Invoke(form, "RefreshPlacedList");
                events.Add(new("ARM angles", 3) { Actions = [new(ScenarioActionKind.Message, "ARM test: eight soldiers east of the start face angles 0..315.")] });
                report = $"type {unit.Type.NameDef}: " + string.Join(", ", placed);
            }
            else if (scenario == "terrain")
            {
                report = TerrainShowcase(form);
                events.Add(new("ARM terrain", 3) { Actions = [new(ScenarioActionKind.Message, "ARM test: terrain showcase north of the start (materials, hills, forest, road).")] });
            }
            else if (scenario == "waves")
            {
                // Campaign wave planner path: two timed waves of enemy squads east of the start (team 1), merged through the real form API.
                var aliases = (string[])typeof(MapEditorForm).GetMethod("CampaignAliases", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, null)!;
                using var dialog = new CampaignWaveDialog(aliases, (ScenarioDocument)typeof(MapEditorForm).GetMethod("CampaignScenario", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, null)!)
                { WaveCount = 2, FirstDelaySeconds = 15, IntervalSeconds = 20, SquadCount = 3, SpawnX = (int)unit.WorldX + 1500, SpawnZ = (int)unit.WorldZ };
                var result = form.ApplyCampaignWaves(dialog.Plan);
                Assert.True(result.Success, string.Join("; ", result.Diagnostics));
                events.Add(new("ARM waves", 3) { Actions = [new(ScenarioActionKind.Message, "ARM test: enemy waves arrive at 15 s and 35 s east of the start.")] });
                report = $"waves at ({unit.WorldX + 1500},{unit.WorldZ}): " + string.Join(", ", result.CompiledEvents.Select(e => $"{e.Name}@{e.DelaySeconds}s"));
            }
            else throw new ArgumentException("未知案例：" + scenario);
            Invoke(form, "RefreshEventList", 0);
            Invoke(form, "UpdateEditorState");
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
        });
        File.WriteAllText(Path.Combine(Path.GetTempPath(), "ArmInGameScenario.txt"), $"{DateTime.Now:O} {mapId} {scenario}: {report}");
    }

    /// <summary>以真正的表單工具在出生點附近做示範區：自動過渡材質、粗糙化丘陵、茂密混合森林、印章道路。回傳每一步結果。</summary>
    private static string TerrainShowcase(MapEditorForm form)
    {
        var log = new List<string>();
        T Field<T>(string name) => (T)typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
        void Set(string name, object? value) => typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, value);
        Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
        void Mode(string name) => Invoke(form, "SetEditMode", Enum.Parse(mode, name));
        void Paint(int x, int y) => Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, y, "", ""));
        var brush = Field<System.Windows.Forms.ComboBox>("_brushSize");
        var document = Field<BodenTexturesDocument>("_texturesDocument");

        // 1. 自動過渡材質：取地圖最常見的三種材質之外、能與周圍銜接的材質，各塗一塊 5×5。
        Mode("Texture");
        Field<System.Windows.Forms.CheckBox>("_autoBridge").Checked = true;
        brush.SelectedIndex = 2;
        var catalog = Field<FloorMaterialCatalog>("_floorMaterials");
        var session = Field<TerrainBlendEditSession>("_terrainBlendSession");
        int painted = 0;
        var used = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach ((int x, int y) in new[] { (33, 29), (39, 26), (45, 29) })
        {
            string? here = catalog.FindByTexture(document.GetTexture(x, y))?.Id;
            foreach (FloorMaterial material in catalog.Materials.Where(item => !used.Contains(item.Id) && !string.Equals(item.Id, here, StringComparison.OrdinalIgnoreCase)))
            {
                string textureBefore = document.GetTexture(x, y);
                Set("_activeMaterial", material);
                Paint(x, y); Invoke(form, "CommitStroke");
                if (!string.Equals(document.GetTexture(x, y), textureBefore, StringComparison.OrdinalIgnoreCase)
                    && string.Equals(catalog.FindByTexture(document.GetTexture(x, y))?.Id, material.Id, StringComparison.OrdinalIgnoreCase))
                {
                    log.Add($"material {material.Id} ({material.DisplayName}) over {here} at ({x},{y})"); used.Add(material.Id); painted++; break;
                }
            }
        }
        // 2. 粗糙化丘陵：9×9、強。
        Mode("Height");
        brush.SelectedIndex = 3;
        Field<System.Windows.Forms.ToolStripComboBox>("_terrainOperation").SelectedIndex = (int)TerrainHeightOperation.Roughen;
        Field<System.Windows.Forms.ToolStripComboBox>("_terrainStrength").SelectedIndex = 2;
        var layers = Field<TerrainHeightEditSession>("_terrainLayers");
        byte[] heightsBefore = layers.Heights.ToArray();
        Paint(39, 20); Paint(39, 20); Invoke(form, "CommitStroke");
        log.Add($"roughen changed {heightsBefore.Where((value, index) => value != layers.Heights[index]).Count()} vertices at (39,20)");
        // 水域：5×5 小湖，多次塗抹直到挖到水面下。
        brush.SelectedIndex = 2;
        Field<System.Windows.Forms.ToolStripComboBox>("_terrainOperation").SelectedIndex = (int)TerrainHeightOperation.Roughen + 1;
        for (int pass = 0; pass < 12; pass++) { Paint(32, 37); Invoke(form, "CommitStroke"); }
        int lakeVertex = (int)(37.5f * (layers.VertexSize - 1) / 64) * layers.VertexSize + (int)(32.5f * (layers.VertexSize - 1) / 64);
        log.Add($"lake at (32,37): center height {layers.Heights[lakeVertex]} (water level {Field<System.Windows.Forms.NumericUpDown>("_waterLevel").Value})");
        // 3. 茂密混合森林：9×9，樹木類。
        Mode("Nature");
        var catalogTask = (Task)typeof(MapEditorForm).GetField("_natureCatalogTask", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
        var deadline = DateTime.UtcNow.AddMinutes(2);
        var types = Field<System.Windows.Forms.ListBox>("_natureTypes");
        while ((!catalogTask.IsCompleted || types.Items.Count == 0 || types.Items[0] is string) && DateTime.UtcNow < deadline)
        { System.Windows.Forms.Application.DoEvents(); Thread.Sleep(50); }
        var category = Field<System.Windows.Forms.ComboBox>("_natureCategory");
        for (int index = 0; index < category.Items.Count; index++)
            if (category.Items[index]!.GetType().GetProperty("Value")!.GetValue(category.Items[index]) as string == "tree") { category.SelectedIndex = index; break; }
        Field<System.Windows.Forms.ComboBox>("_natureDensity").SelectedIndex = 2;
        Field<System.Windows.Forms.CheckBox>("_natureMix").Checked = true;
        brush.SelectedIndex = 3;
        var nature = Field<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>("_natureSession");
        int before = nature.Additions.Count;
        Paint(29, 43); Invoke(form, "CommitStroke");
        log.Add($"forest planted {nature.Additions.Count - before} ({string.Join(",", nature.Additions.Skip(before).Select(item => item.Name).Distinct())}) at (29,43)");
        // 4. 印章道路：tile y=33、x 34–44。
        Mode("Texture");
        Field<System.Windows.Forms.CheckBox>("_stampMode").Checked = true;
        string? road = Field<FloorTextureLibrary>("_floorTextures").Names.FirstOrDefault(name => name.Equals("weg1", StringComparison.OrdinalIgnoreCase));
        if (road is not null)
        {
            Set("_stampTexture", road);
            for (int x = 34; x <= 44; x++) Paint(x, 33);
            Invoke(form, "CommitStroke");
            log.Add($"road {road} x34-44 y33: {Enumerable.Range(34, 11).Count(x => document.GetTexture(x, 33) == road)} tiles");
        }
        Field<System.Windows.Forms.CheckBox>("_stampMode").Checked = false;
        log.Add($"materials painted {painted}/3");
        return string.Join("; ", log);
    }

    private static object? Invoke(MapEditorForm form, string name, params object[] args)
    {
        MethodInfo method = typeof(MapEditorForm).GetMethods(BindingFlags.Instance | BindingFlags.NonPublic)
            .Single(item => item.Name == name && item.GetParameters().Length == args.Length);
        try { return method.Invoke(form, args); }
        catch (TargetInvocationException ex) when (ex.InnerException is not null) { throw ex.InnerException; }
    }

    private static void RunInSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() => { try { action(); } catch (Exception ex) { failure = ex; } });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromMinutes(3)), "STA 逾時（可能彈出模態對話框）。");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }
}
