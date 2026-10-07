using AgainstRomeMapEditor.Modules.Persistence;
using AgainstRomeModifier;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using System.Diagnostics;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private bool SaveMap(bool showSuccess)
    {
        bool saved = TrySaveMap(showSuccess, out Exception? error);
        if (error is not null) ShowError(error);
        return saved;
    }

    // 錯誤呈現與交易分離；可在不顯示模態視窗的情況驗證失敗後的資料與編輯狀態。
    internal bool TrySaveMap(bool showSuccess, out Exception? error)
    {
        error = null;
        if (_selected is null || !_selected.IsCustom) return false;
        try
        {
            string map = CustomMapAccess.RequireEditableDirectory(_selected.DirectoryPath, _gamePath);
            CommitStroke();
            bool natureChanged = NatureDirty();
            bool placedChanged = PlacedDirty();
            bool eventsChanged = EventsDirty();
            ScenarioDocument? scenario = null, previousScenario = null;
            if (placedChanged || eventsChanged)
            {
                previousScenario = ScenarioDocument.Load(map);
                // 建築以官方完工範本寫入 DATA（開局即完工）；人物與部隊由地圖腳本生成。
                scenario = new ScenarioDocument { Events = EventSession.Capture().ToList(), DataSlots = previousScenario.DataSlots.ToList(), Spawns = placedChanged ? _placedObjects.Select(item => new ScenarioSpawn(AliasOf(item.Type), item.WorldX, item.WorldZ, item.Team,
                    item.Type.Category == SdlObjectCategory.Figure ? Math.Max(1, item.UnitCount) : 0, (int)MathF.Round(item.Angle), item.WorldY,
                    Prebuilt: item.Type.Category == SdlObjectCategory.Building && item.Team is >= 0 and <= 8) { Id = item.ScenarioId }).ToList() : previousScenario.Spawns.ToList() };
            }
            if (scenario is not null) ScenarioSavePreflight.Validate(scenario, _objectCatalog.Select(AliasOf).ToArray());
            using var rollback = new FileRollbackScope();
            var put = PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put"));
            put.SetValue("briefing_titel_1", _title.Text.Trim()); put.SetValue("briefing_titel_2", _subtitle.Text.Trim()); put.SetCompositeValue("briefing_text", _briefing.Text);
            for (int index = 0; index < _teamNames.Length; index++) put.SetValue($"briefing_text_teamname{index}", _teamNames[index].Text.Trim());
            put.Save(rollback);
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini")); ini.SetValue("Waterlevel", _waterLevel.Value.ToString()); ini.SetValue("WaterColor", _waterColor.Text.Trim());
            ini.SetValue("WaterWarpShift", _waterWarpShift.Value.ToString()); ini.SetValue("WaterBumpAmplitude", _waterBumpAmplitude.Value.ToString()); ini.SetValue("WaterBumpFrequency", _waterBumpFrequency.Value.ToString()); ini.SetValue("FlashPropability", _flashProbability.Value.ToString());
            ini.SetValue("DayStartTime", _dayStart.Value.ToString()); ini.SetValue("DayEndTime", _dayEnd.Value.ToString()); ini.SetValue("RainDropsOnWater", _rain.Checked ? "1" : "0"); ini.Save(rollback);
            bool sceneStructureChanged = _sceneRemovals.Count > 0 || _sceneAdditions.Count > 0 || _settlementOffsets.Count > 0;
            SdlSceneEditService.SaveChanges(map, _sceneSavedObjects, _sceneObjects, rollback,
                _sceneRemovals, _sceneAdditions.Select(item => item.ToAddition()).ToArray(),
                _settlementOffsets.Select(pair => new SdlSettlementTranslation(pair.Key, pair.Value.X, pair.Value.Y, pair.Value.Z)).ToArray());
            _texturesDocument?.Save(rollback);
            bool heightsChanged = _terrainLayers?.HeightsDirty == true && _bodenLayer is not null;
            bool collisionChanged = _terrainLayers?.CollisionDirty == true && _collisionLayer is not null;
            byte[]? savedEmboss = null;
            if (heightsChanged)
            {
                TerrainLayerFiles.Write(Path.Combine(map, "boden.bmp"), _bodenLayer!, _terrainLayers!.Heights, rollback);
                savedEmboss = _terrainLayers.BuildEmboss();
                if (savedEmboss is not null && _embossLayer is not null) TerrainLayerFiles.Write(Path.Combine(map, "emboss.bmp"), _embossLayer, savedEmboss, rollback);
                // skydens／visible／cliprect／shadows.dat 以高度總和為鍵；刪除後由遊戲在載入時重算。
                TerrainLayerFiles.InvalidateHeightCaches(map, rollback);
            }
            bool prebuiltChanged = placedChanged && scenario is not null && (scenario.Spawns.Any(spawn => spawn.Prebuilt) || previousScenario!.DataSlots.Count > 0);
            if (natureChanged || prebuiltChanged)
            {
                LevelObjectStore store = LevelObjectStore.Load(map);
                foreach (int slot in _natureRemovals) store.Remove(slot);
                foreach (NatureAddition addition in _natureAdditions)
                    if (store.Add(addition.Template, addition.X, addition.Y, addition.Z, addition.Rotation) < 0)
                        throw new InvalidOperationException("地圖的世界物件已達上限（14,000 個），無法再新增。");
                if (prebuiltChanged)
                {
                    IReadOnlyList<ScenarioSpawn> skipped = ScenarioLevelObjects.Apply(store, previousScenario!, scenario!, BuildingTemplateFor);
                    // 原版地圖沒有此建築的範本時改由腳本生成（開局為工地）。
                    foreach (ScenarioSpawn spawn in skipped) scenario!.Spawns[scenario.Spawns.IndexOf(spawn)] = spawn with { Prebuilt = false };
                }
                store.Save(map, rollback);
            }
            if (placedChanged || eventsChanged)
            {
                scenario!.Save(map, rollback);
                LevelScriptInjector.Apply(map, scenario, _objectCatalog.Select(AliasOf).ToArray(), rollback);
                string legacy = Path.Combine(map, SdlPlacedObjectsFile.FileName); // 舊版（SDL onload，遊戲不會生成）實驗檔
                if (File.Exists(legacy)) { rollback.TrackFile(legacy); File.Delete(legacy); }
            }
            bool auxiliaryReset = _resetAuxiliaryLayers;
            if (auxiliaryReset)
            {
                foreach ((string name, byte value) in new[] { ("vertex.bmp", (byte)255), ("smooth.bmp", (byte)0) })
                {
                    string path = Path.Combine(map, name);
                    if (TerrainLayerFiles.Read(path) is not { } layer) continue;
                    TerrainLayerFiles.Write(path, layer, Enumerable.Repeat(value, layer.Width * layer.Height).ToArray(), rollback);
                }
            }
            if (collisionChanged) TerrainLayerFiles.Write(Path.Combine(map, "collision.bmp"), _collisionLayer!, _terrainLayers!.Collision!, rollback);
            // 只有地表確實被繪製過才重生小地圖，避免僅改標題／水面等屬性時用近似圖覆蓋原始 minimap.bmp。
            if (TextureDirty() || heightsChanged)
            {
                byte[]? minimap = _canvas.RenderMinimapBmp();
                if (minimap is not null) AgainstRomeModifier.Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(map, "minimap.bmp"), minimap, rollback);
            }
            rollback.Commit();
            if (auxiliaryReset) _resetAuxiliaryLayers = false;
            if (placedChanged) _placementSession.AcceptChanges();
            if (eventsChanged) _eventsBaseline = _events.ToArray();
            if (natureChanged || prebuiltChanged) LoadLevelObjects(map);
            if (heightsChanged || collisionChanged)
            {
                // 以寫回後的檔案作為下一次保留原像素的基準；光照係數沿用開圖時由原版資料擬合的值。
                if (heightsChanged) { _bodenLayer = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp")); if (_embossLayer is not null) _embossLayer = TerrainLayerFiles.Read(Path.Combine(map, "emboss.bmp")); }
                if (collisionChanged) _collisionLayer = TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"));
                _terrainLayers!.CommitBaseline(savedEmboss);
            }
            _terrainBlendSession?.CommitBaseline(); _savedTextures = _terrainBlendSession?.CurrentTextures.ToArray() ?? _texturesDocument?.Textures.ToArray() ?? Array.Empty<string>(); _propertyDirty = false;
            if (sceneStructureChanged)
            {
                // 複製／刪除已寫回並重新編號，記憶體中的 object 索引不再對應檔案；
                // 從磁碟重讀並重定基準（「還原到本次開啟時」自此以本次儲存後狀態為起點）。
                _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
                _sceneObjects = SdlSceneCatalog.LoadDirectory(map);
                _settlementOrigins = SdlSceneCatalog.LoadSettlementOrigins(map);
                _sceneOriginalObjects = _sceneObjects.ToArray();
                _sceneSavedObjects = _sceneObjects.ToArray();
                LoadEditingScene(preserveView: true);
            }
            else _sceneSavedObjects = _sceneObjects.ToArray();
            if (natureChanged || prebuiltChanged) RefreshSceneMarkers();
            _canvas.CommitBaseline(); // 變更高亮只存在於 2D 檢視，3D 無對應狀態
            RefreshOverview();
            UpdateEditorState();
            if (showSuccess)
            {
                bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
                MessageBox.Show(this, isEn ? "Map saved successfully." : "地圖已安全儲存。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            return true;
        }
        catch (Exception ex) { error = ex; return false; }
    }

    private void PreviewInGame()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected is null) { MessageBox.Show(this, isEn ? "Please select a map to preview." : "請先選擇要預覽的地圖。", Text); return; }
        if (_selected.IsCustom && IsDirty && !SaveMap(showSuccess: false)) return;
        string exePath = Path.Combine(_gamePath, "Against_Rome.exe");
        if (!File.Exists(exePath)) { MessageBox.Show(this, isEn ? "Against_Rome.exe not found in game folder." : "遊戲路徑中找不到 Against_Rome.exe。", Text, MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
        try
        {
            Process.Start(new ProcessStartInfo(exePath) { WorkingDirectory = _gamePath, UseShellExecute = true });
            string msg = isEn
                ? $"Game launched.\n\nTo test the custom map, enter \"Endless Mode\" and select {_selected.Id} ({_selected.DisplayName ?? "Unnamed"}).\nMap editing and offline 3D view do not require launching the game."
                : $"遊戲已啟動。\n\n若要額外測試自製地圖，請進入「無盡模式」並選擇 {_selected.Id}（{_selected.DisplayName ?? "未命名"}）。\n地圖編輯與離線場景顯示不需要啟動遊戲。";
            string title = isEn ? "Optional Game Test" : "選用遊戲測試";
            MessageBox.Show(this, msg, title, MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (Exception ex) { ShowError(ex); }
    }

    private bool ConfirmDiscardOrSave()
    {
        if (!IsDirty) return true;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn
            ? "The current map has unsaved changes.\n\nYes: Save and continue\nNo: Discard changes\nCancel: Stay on current map"
            : "目前地圖有尚未儲存的變更。\n\n是：儲存後繼續\n否：放棄變更\n取消：留在目前地圖";
        string title = isEn ? "Unsaved Changes" : "尚未儲存";
        DialogResult result = MessageBox.Show(this, msg, title, MessageBoxButtons.YesNoCancel, MessageBoxIcon.Warning);
        return result switch { DialogResult.Yes => SaveMap(showSuccess: false), DialogResult.No => true, _ => false };
    }

}
