using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private void MoveSelectedSceneObject(SceneObjectMoveEventArgs e)
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1 || SelectedSceneDisplay() is not { } source) return;
        if (_sceneList.SelectedItems[0].Tag is MapSceneObject pending && _sceneRemovals.Any(removal =>
            removal.SourceFile.Equals(pending.SourceFile, StringComparison.OrdinalIgnoreCase) && removal.ObjectIndex == pending.ObjectIndex)) return;

        // 拖曳落點是畫面上的有效座標（含暫存聚落平移）；換回以已存檔 refpos 為準的記憶體座標。
        MapSceneObject effectiveSource = WithSettlementOffset(source);
        MapSceneObject effectiveMoved = SceneObjectPositioning.MoveToWorldPosition(effectiveSource, e.WorldX, e.WorldZ);
        MapSceneObject moved = effectiveMoved with
        {
            WorldX = source.WorldX + effectiveMoved.WorldX - effectiveSource.WorldX,
            WorldZ = source.WorldZ + effectiveMoved.WorldZ - effectiveSource.WorldZ,
        };
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            addition.Display = moved;
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int index = _sceneObjects.ToList().FindIndex(item => SceneKey(item) == SceneKey(selected));
            if (index < 0) return;
            MapSceneObject[] objects = _sceneObjects.ToArray();
            objects[index] = moved;
            _sceneObjects = objects;
            _sceneList.SelectedItems[0].Tag = moved;
        }
        else return;

        _sceneX.Value = ClampSceneCoordinate(moved.LocalX, _sceneX);
        _sceneY.Value = ClampSceneCoordinate(moved.LocalY, _sceneY);
        _sceneZ.Value = ClampSceneCoordinate(moved.LocalZ, _sceneZ);
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        PushSceneObjects(effective);
        UpdateEditorState();
        if (e.Completed)
        {
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _status.Text = isEn
                ? $"Object moved to world ({effectiveMoved.WorldX:0}, {effectiveMoved.WorldZ:0}); click Save to write the SDL change."
                : $"物件已移到世界座標 ({effectiveMoved.WorldX:0}, {effectiveMoved.WorldZ:0})；按「儲存」才會寫入 SDL。";
        }
    }

    private void ApplySelectedSceneObjectEdit()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        float localX = (float)_sceneX.Value, localY = (float)_sceneY.Value, localZ = (float)_sceneZ.Value;
        int team = (int)_sceneTeam.Value;
        float angle = (float)_sceneAngle.Value;
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            addition.Display = MoveSceneObject(addition.Display, team, localX, localY, localZ, angle);
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int index = _sceneObjects.ToList().FindIndex(item => SceneKey(item) == SceneKey(selected));
            if (index < 0) return;
            MapSceneObject[] objects = _sceneObjects.ToArray();
            objects[index] = MoveSceneObject(selected, team, localX, localY, localZ, angle);
            _sceneObjects = objects;
        }
        else return;
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    // 原檔沒有 angle 欄位的物件維持 null：不新增遊戲未定義的欄位。
    private static MapSceneObject MoveSceneObject(MapSceneObject source, int team, float localX, float localY, float localZ, float angle) => source with
    {
        Team = team,
        Angle = source.Angle is null ? null : angle,
        WorldX = source.WorldX + localX - source.LocalX,
        WorldY = source.WorldY + localY - source.LocalY,
        WorldZ = source.WorldZ + localZ - source.LocalZ,
        LocalX = localX,
        LocalY = localY,
        LocalZ = localZ
    };

    /// <summary>以選取物件為模板暫存一個複製件（唯一安全的新增路徑：所有欄位沿用原版物件）。</summary>
    private void DuplicateSelectedSceneObject()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        (string templateFile, int templateIndex, MapSceneObject source) = _sceneList.SelectedItems[0].Tag switch
        {
            MapSceneObject item => (item.SourceFile, item.ObjectIndex, item),
            StagedSceneAddition staged => (staged.TemplateFile, staged.TemplateIndex, staged.Display),
            _ => (null!, -1, null!),
        };
        if (source is null) return;
        const float offset = 64f; // 錯開四分之一 tile，避免複製件與原件完全重疊而難以選取。
        int id = _nextSceneAdditionId++;
        var display = source with
        {
            ObjectIndex = -1000 - id, // 負索引：畫布顯示用，絕不寫入檔案；儲存時由 AddObject 重新編號。
            WorldX = source.WorldX + offset,
            WorldZ = source.WorldZ + offset,
            LocalX = source.LocalX + offset,
            LocalZ = source.LocalZ + offset,
        };
        _sceneAdditions.Add(new StagedSceneAddition { Id = id, TemplateFile = templateFile, TemplateIndex = templateIndex, FromCatalog = false, Display = display });
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is StagedSceneAddition added && added.Id == id);
        UpdateEditorState();
    }

    /// <summary>暫存刪除選取物件；再按一次可取消。刪除暫存複製件則直接移除該複製件。</summary>
    private void ToggleDeleteSelectedSceneObject()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            _sceneAdditions.RemoveAll(item => item.Id == addition.Id);
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int existing = _sceneRemovals.FindIndex(item =>
                item.SourceFile.Equals(selected.SourceFile, StringComparison.OrdinalIgnoreCase) && item.ObjectIndex == selected.ObjectIndex);
            if (existing >= 0) _sceneRemovals.RemoveAt(existing);
            else _sceneRemovals.Add(new SdlSceneObjectRemoval(selected.SourceFile, selected.ObjectIndex));
            string key = SceneKey(selected);
            LoadEditingScene(preserveView: true);
            SelectSceneListItem(tag => tag is MapSceneObject item && SceneKey(item) == key);
            UpdateEditorState();
            return;
        }
        else return;
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    /// <summary>
    /// 依物件類型自由新增：從本地圖既有物件挑一個同類型模板（沿用其 def/namedef/nation 等全部欄位），
    /// 放入使用者指定的聚落檔。新物件預設放在聚落原點附近，之後可拖曳或輸入座標調整。
    /// </summary>
    private void AddSceneObjectFromCatalog()
    {
        if (_selected?.IsCustom != true || _sceneObjects.Count == 0 || _settlementOrigins.Count == 0) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        MapSceneObject[] templates = _sceneObjects
            .GroupBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First())
            .OrderBy(item => item.Kind).ThenBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .ToArray();
        string[] settlements = _settlementOrigins.Keys.OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray();
        string defaultSettlement = SelectedSceneDisplay() is { } current && _settlementOrigins.ContainsKey(current.SourceFile) ? current.SourceFile : settlements[0];

        using var dialog = CreateSceneDialog(isEn ? "Add Scene Object" : "新增場景物件", 230);
        var table = (TableLayoutPanel)dialog.Controls[0];
        var typeBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        foreach (MapSceneObject template in templates) typeBox.Items.Add($"[{KindText(template.Kind, isEn)}] {template.Name}");
        var settlementBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        settlementBox.Items.AddRange(settlements);
        var teamBox = new NumericUpDown { Dock = DockStyle.Fill, Minimum = -1, Maximum = 15 };
        AddSceneField(table, 0, isEn ? "Type" : "物件類型", typeBox);
        AddSceneField(table, 1, isEn ? "Settlement" : "目標聚落", settlementBox);
        AddSceneField(table, 2, isEn ? "Team" : "隊伍", teamBox);
        settlementBox.SelectedIndexChanged += (_, _) =>
        {
            // 預設採用目標聚落中最常見的隊伍，讓新物件歸屬該聚落的勢力。
            string file = (string)settlementBox.SelectedItem!;
            int team = _sceneObjects.Where(item => item.SourceFile.Equals(file, StringComparison.OrdinalIgnoreCase))
                .GroupBy(item => item.Team).OrderByDescending(group => group.Count()).Select(group => group.Key).DefaultIfEmpty(-1).First();
            teamBox.Value = Math.Clamp(team, -1, 15);
        };
        typeBox.SelectedIndex = SelectedSceneDisplay() is { } selectedItem
            ? Math.Max(0, Array.FindIndex(templates, item => item.Name.Equals(selectedItem.Name, StringComparison.OrdinalIgnoreCase)))
            : 0;
        settlementBox.SelectedItem = defaultSettlement;
        if (dialog.ShowDialog(this) != DialogResult.OK || typeBox.SelectedIndex < 0 || settlementBox.SelectedItem is not string targetFile) return;

        MapSceneObject source = templates[typeBox.SelectedIndex];
        SdlVector3 origin = _settlementOrigins[targetFile];
        int id = _nextSceneAdditionId++;
        const float offset = 96f; // 避開聚落原點常見的主建築，方便選取。
        const float mapSize = SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize;
        float worldX = Math.Clamp(origin.X + offset, 0, mapSize), worldZ = Math.Clamp(origin.Z + offset, 0, mapSize);
        var display = source with
        {
            ObjectIndex = -1000 - id, // 負索引：畫布顯示用，絕不寫入檔案；儲存時由 AddObject 重新編號。
            SourceFile = targetFile,
            Team = (int)teamBox.Value,
            WorldX = worldX,
            WorldY = origin.Y,
            WorldZ = worldZ,
            LocalX = worldX - origin.X,
            LocalY = 0,
            LocalZ = worldZ - origin.Z,
        };
        _sceneAdditions.Add(new StagedSceneAddition { Id = id, TemplateFile = source.SourceFile, TemplateIndex = source.ObjectIndex, FromCatalog = true, Display = display });
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is StagedSceneAddition added && added.Id == id);
        UpdateEditorState();
        _status.Text = isEn
            ? $"Added {source.Name} to {targetFile}; drag it or type a position, then Save."
            : $"已將 {source.Name} 暫存新增到 {targetFile}；可拖曳或輸入座標，按「儲存」才會寫入。";
    }

    /// <summary>整體平移選取物件所屬的聚落：只改 refpos，整個聚落（含待新增物件）一起移動。</summary>
    private void TranslateSelectedSettlement()
    {
        if (_selected?.IsCustom != true || SelectedSceneDisplay() is not { } selected || !_settlementOrigins.ContainsKey(selected.SourceFile)) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string file = selected.SourceFile;
        _settlementOffsets.TryGetValue(file, out SdlVector3 currentOffset);

        using var dialog = CreateSceneDialog(isEn ? $"Move Settlement - {file}" : $"平移聚落 - {file}", 230);
        var table = (TableLayoutPanel)dialog.Controls[0];
        NumericUpDown dx = SceneCoordinateInput(), dy = SceneCoordinateInput(), dz = SceneCoordinateInput();
        dx.Value = ClampSceneCoordinate(currentOffset.X, dx); dy.Value = ClampSceneCoordinate(currentOffset.Y, dy); dz.Value = ClampSceneCoordinate(currentOffset.Z, dz);
        AddSceneField(table, 0, isEn ? "Offset X" : "平移 X", dx);
        AddSceneField(table, 1, isEn ? "Offset Y" : "平移 Y", dy);
        AddSceneField(table, 2, isEn ? "Offset Z" : "平移 Z", dz);
        if (dialog.ShowDialog(this) != DialogResult.OK) return;

        var offset = new SdlVector3((float)dx.Value, (float)dy.Value, (float)dz.Value);
        const float mapSize = SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize;
        bool outside = _sceneObjects.Concat(_sceneAdditions.Select(item => item.Display))
            .Where(item => item.SourceFile.Equals(file, StringComparison.OrdinalIgnoreCase))
            .Any(item => item.WorldX + offset.X is < 0 or > mapSize || item.WorldZ + offset.Z is < 0 or > mapSize);
        if (outside)
        {
            MessageBox.Show(this, isEn ? "This offset would move part of the settlement outside the map." : "此平移量會讓部分聚落物件超出地圖範圍，已取消。",
                Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        if (offset == default) _settlementOffsets.Remove(file); else _settlementOffsets[file] = offset;
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is MapSceneObject item && SceneKey(item) == SceneKey(selected)
            || tag is StagedSceneAddition added && ReferenceEquals(added.Display, selected));
        UpdateEditorState();
    }

    private Form CreateSceneDialog(string title, int height)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var form = new Form { Text = title, Width = 460, Height = height, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor, FormBorderStyle = FormBorderStyle.FixedDialog, MinimizeBox = false, MaximizeBox = false, ShowInTaskbar = false };
        var table = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12) };
        table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 96)); table.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var ok = new Button { Text = isEn ? "OK" : "確定", DialogResult = DialogResult.OK, Width = 100, Height = 34 };
        var cancel = new Button { Text = isEn ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(ok); buttons.Controls.Add(cancel);
        form.Controls.Add(table); form.Controls.Add(buttons);
        form.AcceptButton = ok; form.CancelButton = cancel;
        WinFormsTheme.Apply(form);
        WinFormsTheme.StylePrimaryButton(ok);
        return form;
    }

    private static string KindText(string kind, bool isEn) => !isEn ? kind : kind switch { "建築" => "Building", "單位" => "Unit", _ => "Other" };

    /// <summary>3D 畫面點選可見物件：選取對應的 SDL 場景清單列（其後拖曳即移動）。</summary>
    private void SelectPickedSceneObject(MapSceneObject picked)
    {
        int placedIndex = -5000 - picked.ObjectIndex;
        if (picked.SourceFile.Equals(SdlPlacedObjectsFile.FileName, StringComparison.OrdinalIgnoreCase)
            && placedIndex >= 0 && placedIndex < _placedObjects.Count)
        {
            _pickedNature = null;
            _sceneList.SelectedItems.Clear();
            _placedList.SelectedItems.Clear();
            if (placedIndex < _placedList.Items.Count) _placedList.Items[placedIndex].Selected = true;
            UpdatePlacedButtonsState();
            return;
        }
        string key = SceneKey(picked);
        bool found = false;
        _pickedNature = null;
        _sceneList.SelectedItems.Clear(); // the list allows multi-select; a pick replaces the selection
        SelectSceneListItem(tag =>
        {
            bool match = tag is MapSceneObject item && SceneKey(item) == key || tag is StagedSceneAddition added && SceneKey(added.Display) == key;
            found |= match;
            return match;
        });
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string name = picked.Name;
        if (found) { _status.Text = isEn ? $"Selected {name}; drag to move it." : $"已選取 {name}；拖曳即可移動。"; return; }
        if (_selected?.IsCustom == true && PickedNatureTarget(picked) is not null)
        {
            _pickedNature = picked;
            _status.Text = isEn ? $"Selected {name}; press Delete to remove it (undo restores it)." : $"已選取 {name}；按 Delete 移除（可復原）。";
            return;
        }
        _status.Text = isEn ? $"{name} is a placed or fixed map object; edit it with the Place tool." : $"{name} 屬於放置物件或地圖固定物件，請用「放置」工具編輯。";
    }

    /// <summary>
    /// Removable nature object behind a 3D pick: an existing landscape DATA slot (ObjectIndex -100000 - slot)
    /// or a pending planted addition (ObjectIndex -200000 - index), as produced by NatureDisplayObjects.
    /// </summary>
    private (int? Slot, AgainstRomeMapEditor.Modules.Nature.NatureAddition? Addition)? PickedNatureTarget(MapSceneObject picked)
    {
        if (!Map3DViewControl.IsLevelDataObject(picked)) return null;
        if (picked.ObjectIndex <= -200000)
        {
            int index = -200000 - picked.ObjectIndex;
            IReadOnlyList<AgainstRomeMapEditor.Modules.Nature.NatureAddition> additions = _natureSession.Additions;
            return index < additions.Count ? (null, additions[index]) : null;
        }
        int slot = -100000 - picked.ObjectIndex;
        LevelWorldObject? item = _levelObjects.FirstOrDefault(entry => entry.Slot == slot);
        return item is not null && IsRemovableNature(item) && !_natureSession.RemovedSlots.Contains(slot) ? (slot, null) : null;
    }

    /// <summary>Remove the nature object picked in 3D as one undoable step; false when nothing removable is picked.</summary>
    private bool DeletePickedNature()
    {
        if (_pickedNature is not { } picked || PickedNatureTarget(picked) is not { } target) { _pickedNature = null; return false; }
        _pickedNature = null;
        bool removed = _natureSession.Remove(target.Slot is { } slot ? [slot] : [], target.Addition is { } addition ? [addition] : []);
        if (!removed) return false;
        _natureSession.CommitStroke();
        RefreshSceneMarkers();
        UpdateEditorState();
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _status.Text = isEn ? $"Removed {picked.Name}; Undo restores it." : $"已移除 {picked.Name}；可用「復原」還原。";
        return true;
    }

    private void SelectSceneListItem(Func<object?, bool> match)
    {
        foreach (ListViewItem row in _sceneList.Items)
        {
            if (!match(row.Tag)) continue;
            row.Selected = true; row.EnsureVisible();
            return;
        }
    }

    private void RestoreOpeningSceneObjects()
    {
        if (_selected?.IsCustom != true) return;
        if (_sceneRemovals.Count == 0 && _sceneAdditions.Count == 0 && _settlementOffsets.Count == 0 && !SdlSceneEditService.HasChanges(_sceneOriginalObjects, _sceneObjects)) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn
            ? "Do you want to restore all unsaved SDL edits (teams, positions, angles, pending additions, deletions and settlement moves) to the state when this map was opened?\nYou still need to click \"Save\" to write them back to the custom map."
            : "要將所有待儲存的 SDL 變更（隊伍、位置、角度、待新增、待刪除與聚落平移）還原到本次開啟地圖時的狀態嗎？\n還原後仍需按「儲存」才會寫回自製地圖。";
        string title = isEn ? "Restore SDL Verification Changes" : "還原 SDL 驗證變更";
        if (MessageBox.Show(this, msg, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        _sceneObjects = _sceneOriginalObjects.ToArray();
        _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    private void UpdateSceneEditButtons()
    {
        bool editable = _selected?.IsCustom == true;
        bool selected = _sceneList.SelectedItems.Count == 1 && SelectedSceneDisplay() is not null;
        _sceneApplyButton.Enabled = editable && selected;
        _sceneDuplicateButton.Enabled = editable && selected;
        _sceneDeleteButton.Enabled = editable && selected;
        _sceneAddButton.Enabled = editable && _sceneObjects.Count > 0 && _settlementOrigins.Count > 0;
        _sceneTranslateButton.Enabled = editable && selected && SelectedSceneDisplay() is { } target && _settlementOrigins.ContainsKey(target.SourceFile);
        _sceneAngle.Enabled = editable && selected && SelectedSceneDisplay()?.Angle is not null;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        bool pendingRemoval = selected && _sceneList.SelectedItems[0].Tag is MapSceneObject item && _sceneRemovals.Any(removal =>
            removal.SourceFile.Equals(item.SourceFile, StringComparison.OrdinalIgnoreCase) && removal.ObjectIndex == item.ObjectIndex);
        bool canMove = editable && selected && !pendingRemoval && _sceneMoveTool.Checked;
        _canvas.SceneMoveEnabled = canMove;
        if (_view3d is not null) { _view3d.SceneMoveEnabled = canMove; _view3d.ScenePickEnabled = editable && _sceneMoveTool.Checked; }
        _sceneDeleteButton.Text = pendingRemoval
            ? (isEn ? "Undo Delete" : "取消刪除")
            : (isEn ? "Delete Object" : "刪除物件");
    }

}
