using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripMenuItem _starterKitMenu = new();
    // Retained across Undo/Redo; ScenarioId membership, rather than a separate dirty flag,
    // determines whether this detached kit needs importing. Cleared at load/committed save.
    private BlankMapStarterKit? _pendingStarterKit;
    internal Func<StarterKitDialog, DialogResult> StarterKitDialogRunner { get; set; } = dialog => dialog.ShowDialog();

    private void InitializeStarterKitTools()
    {
        _layoutMenu.DropDownItems.Insert(0, _starterKitMenu);
        _starterKitMenu.Click += (_, _) =>
        {
            try
            {
                using var dialog = new StarterKitDialog((int)_placeTeam.Value);
                if (StarterKitDialogRunner(dialog) == DialogResult.OK)
                    ApplyStarterKit(dialog.Team, dialog.AnchorX, dialog.AnchorZ);
            }
            catch (Exception ex) when (ex is IOException or ArgumentException or InvalidOperationException or UnauthorizedAccessException)
            { _status.Text = ex.Message; }
        };
    }

    private void LocalizeStarterKit(bool english)
    {
        _starterKitMenu.Text = english ? "Starter kit: main house + 4 axemen…" : "起始配置：主屋＋4 名斧兵…";
        _starterKitMenu.ToolTipText = english
            ? "For a custom blank map. Choose team and anchor; staged until Save, with one Undo."
            : "用於自製空白地圖。指定隊伍與錨點；儲存前不寫檔，可單步復原。";
    }

    private string RequireStarterKitBlankMap(LevelObjectStore store, ScenarioDocument previous)
    {
        if (_selected?.IsCustom != true) throw new InvalidOperationException("請選取自製空白地圖。");
        string map = CustomMapAccess.RequireEditableDirectory(_selected.DirectoryPath, _gamePath);
        if (store.Capacity is < 1 or > BlankMapStarterKit.MaximumObjects || store.Objects().Count != 0
            || previous.Spawns.Count != 0 || previous.DataSlots.Count != 0)
            throw new InvalidDataException("起始配置只允許原生 DATA 與 Scenario 放置皆為空的自製地圖。");
        // Empty objects alone does not prove the source map's settlement script is disabled.
        // BlankMapContent creates both these exact idle bootstrap images; never reset a map here.
        byte[] idle = BciImage.CreateIdleLevel().Serialize();
        string scripts = LevelScriptInjector.ScriptDirectory(map);
        foreach (string name in new[] { LevelScriptInjector.ScriptFile, LevelScriptInjector.OriginalBackupFile })
        {
            byte[] bytes = File.ReadAllBytes(Path.Combine(scripts, name));
            byte[] image = bytes.Length >= 64 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8)
                ? GameLZSS.DecompressPfil(bytes) : bytes;
            if (!image.AsSpan().SequenceEqual(idle))
                throw new InvalidDataException("此地圖不是具有閒置開局腳本的空白場景，已拒絕起始配置。");
        }
        return map;
    }

    internal void ApplyStarterKit(int team, float anchorX, float anchorZ)
    {
        if (_selected is null) throw new InvalidOperationException("請選取自製空白地圖。");
        var store = LevelObjectStore.Load(_selected.DirectoryPath, NativeObjectDefinitions.Load(_gamePath), NativeLightDefinitions.Load(_gamePath));
        var previous = ScenarioDocument.Load(_selected.DirectoryPath);
        string map = RequireStarterKitBlankMap(store, previous);
        if (_placementSession.Count != 0 || _sceneAdditions.Count != 0)
            throw new InvalidDataException("請先復原其他待存放置，再加入起始配置。");
        if (_terrainLayers is null || _texturesDocument is null || !float.IsFinite(_nativeHeightMapStep))
            throw new InvalidDataException("地圖缺少可信地形高度資料，無法驗證起始配置。");
        var kit = BlankMapStarterKit.Create(_gamePath, map, team, anchorX, anchorZ, LayoutGroundHeight);
        var candidate = kit.ApplyToCandidate(store, previous, BuildingTemplateFor);
        var ids = kit.Spawns.Select(spawn => spawn.Id).ToHashSet();
        BindStarterKitNativeMetadata(candidate, ids);
        var planned = candidate.Scenario.Spawns.Where(spawn => ids.Contains(spawn.Id)).Select(spawn =>
        {
            var type = _objectCatalog.SingleOrDefault(item => AliasOf(item).Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase))
                ?? throw new InvalidDataException($"起始配置類型 {spawn.Alias} 不在放置目錄內。");
            var binding = candidate.Scenario.DataSlots.Single(b => b.SpawnId == spawn.Id);
            var root = candidate.Store.Objects().Single(item => item.Slot == binding.Slot && item.Uid == binding.Uid);
            // All five are native individual objects; retain raw metadata so the existing
            // placement editor cannot turn axemen into formations or alter native teams/rotation.
            float raw = spawn.RawRotation ?? root.Rotation;
            return new SdlPlacedObject(type, spawn.X, spawn.Y, spawn.Z, spawn.Team,
                ScenarioLevelObjects.RadiansToDisplayAngle(raw), 0) { ScenarioId = spawn.Id, RawRotation = raw };
        }).ToArray();
        if (planned.Length != 5 || ids.Count != 5 || ids.Contains(Guid.Empty))
            throw new InvalidDataException("起始配置身份或數量不符。");
        ValidateStarterKitCandidate(candidate.Store, candidate.Scenario, ids);
        // AddMany prevalidates all identities/bounds before its single history command.
        _placementSession.AddMany(planned);
        _pendingStarterKit = kit;
        SetEditMode(EditMode.PlaceObject);
        _lastActionWasPlacementTool = true;
        _lastActionWasPlacementToolUndone = false;
        RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState();
        _status.Text = Loc.CurrentLanguage == Language.English
            ? "Staged 1 main house + 4 individual axemen; Save to write. Single Undo/Redo available."
            : "已暫存 1 棟主屋＋4 名獨立斧兵；按儲存才寫入，可單步復原／重做。";
    }

    private HashSet<Guid> PendingStarterKitIds()
    {
        if (_pendingStarterKit is null) return [];
        var originals = _pendingStarterKit.Spawns.Select(spawn => spawn.Id).ToHashSet();
        return _placedObjects.Where(item => originals.Contains(item.ScenarioId) || originals.Contains(item.CopySourceScenarioId))
            .Select(item => item.ScenarioId).ToHashSet();
    }

    private LevelObjectTemplate? StarterKitTemplateFor(ScenarioSpawn spawn)
        => _pendingStarterKit?.TemplateFor(spawn) ?? BuildingTemplateFor(spawn);

    private (LevelObjectStore Store, ScenarioDocument Previous) ImportPendingStarterKit(LevelObjectStore store, ScenarioDocument previous)
    {
        if (_pendingStarterKit is null || PendingStarterKitIds().Count == 0) return (store, previous);
        RequireStarterKitBlankMap(store, previous);
        var imported = _pendingStarterKit.ApplyToCandidate(store, previous, BuildingTemplateFor);
        BindStarterKitNativeMetadata(imported, _pendingStarterKit.Spawns.Select(spawn => spawn.Id).ToHashSet());
        // The house becomes a bound native family before the existing Scenario save path;
        // no fake singleton, script fallback or weakened family guard is used.
        return (imported.Store, imported.Scenario);
    }

    private static void BindStarterKitNativeMetadata(ScenarioCandidateResult candidate, HashSet<Guid> ids)
    {
        var objects = candidate.Store.Objects().ToDictionary(item => item.Slot);
        for (int i = 0; i < candidate.Scenario.Spawns.Count; i++)
        {
            var spawn = candidate.Scenario.Spawns[i];
            if (!ids.Contains(spawn.Id)) continue;
            var binding = candidate.Scenario.DataSlots.Single(b => b.SpawnId == spawn.Id);
            if (!objects.TryGetValue(binding.Slot, out var root) || root.Uid != binding.Uid || !float.IsFinite(root.Rotation))
                throw new InvalidDataException("起始配置 DATA binding 或旋轉資料無效。");
            candidate.Scenario.Spawns[i] = spawn with
            { RawRotation = root.Rotation, Angle = ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), Count = 0 };
        }
    }

    private void ValidateStarterKitCandidate(LevelObjectStore store, ScenarioDocument scenario, HashSet<Guid> ids)
    {
        if (ids.Count == 0) return;
        if (_terrainLayers is null || _texturesDocument is null || !float.IsFinite(_nativeHeightMapStep))
            throw new InvalidDataException("地圖缺少可信地形高度資料。");
        if (store.Capacity > BlankMapStarterKit.MaximumObjects || store.Objects().Count > store.Capacity)
            throw new InvalidDataException("起始配置超出原生物件容量。");
        var objects = store.Objects().ToDictionary(item => item.Slot);
        var placements = _placedObjects;
        var ownMarkers = placements.Select((item, index) => (item, index)).Where(pair => ids.Contains(pair.item.ScenarioId))
            .Select(pair => -5000 - pair.index).ToHashSet();
        var obstacles = EffectiveSceneObjects().Concat(NatureDisplayObjects()).ToArray();
        var extents = new List<(float Left, float Right, float Top, float Bottom)>();
        foreach (var spawn in scenario.Spawns.Where(spawn => ids.Contains(spawn.Id)))
        {
            var binding = scenario.DataSlots.Single(b => b.SpawnId == spawn.Id);
            var family = store.FamilyAt(binding.Slot, binding.Uid)
                ?? throw new InvalidDataException("起始配置原生 family 無法安全驗證。");
            var members = family.ObjectSlots.Select(slot => objects[slot]).ToArray();
            bool house = spawn.Alias.Equals(BlankMapStarterKit.MainHouseAlias, StringComparison.OrdinalIgnoreCase);
            float margin = house ? 384 : 64;
            float left = members.Min(item => item.X) - margin, right = members.Max(item => item.X) + margin;
            float top = members.Min(item => item.Z) - margin, bottom = members.Max(item => item.Z) + margin;
            if (!float.IsFinite(left) || !float.IsFinite(right) || !float.IsFinite(top) || !float.IsFinite(bottom)
                || left < 0 || top < 0 || right > 16383 || bottom > 16383
                || members.Any(item => !float.IsFinite(item.Y)))
                throw new InvalidDataException("起始配置的完整 family 範圍超出地圖邊界。");
            if (extents.Any(other => left <= other.Right && right >= other.Left && top <= other.Bottom && bottom >= other.Top))
                throw new InvalidDataException("起始配置物件彼此重疊，請分開放置。");
            extents.Add((left, right, top, bottom));
            float minimum = float.PositiveInfinity, maximum = float.NegativeInfinity;
            int size = _terrainLayers.CollisionSize;
            if (_terrainLayers.Collision is not { } collision || size <= 0 || collision.Count != (long)size * size)
                throw new InvalidDataException("起始配置缺少可信阻擋地形資料。");
            float sampleStep = Math.Min(64f, Math.Min(16384f / size,
                _texturesDocument.Dimension * 256f / (_terrainLayers.VertexSize - 1)));
            if (!float.IsFinite(sampleStep) || sampleStep <= 0)
                throw new InvalidDataException("起始配置地形取樣間距無效。");
            // Include every family member and the entire extent, not just the house root.
            for (float z = top; ; z = Math.Min(bottom, z + sampleStep))
            {
                for (float x = left; ; x = Math.Min(right, x + sampleStep))
                {
                    float ground = LayoutGroundHeight(x, z);
                    if (!float.IsFinite(ground) || ground <= (float)_waterLevel.Value + 1)
                        throw new InvalidDataException("起始配置範圍含水面或無效地形。");
                    int tx = Math.Min(size - 1, (int)(x / 16384f * size)), tz = Math.Min(size - 1, (int)(z / 16384f * size));
                    if (collision[tz * size + tx] > 128)
                        throw new InvalidDataException("起始配置範圍含阻擋地形。");
                    minimum = Math.Min(minimum, ground); maximum = Math.Max(maximum, ground);
                    if (x == right) break;
                }
                if (z == bottom) break;
            }
            if (maximum - minimum > 32)
                throw new InvalidDataException("起始配置範圍地形高差過大（超過 32 世界單位）。");
            if (obstacles.Any(item => item.WorldX >= left && item.WorldX <= right && item.WorldZ >= top && item.WorldZ <= bottom
                && !(item.SourceFile == SdlPlacedObjectsFile.FileName && ownMarkers.Contains(item.ObjectIndex))))
                throw new InvalidDataException("起始配置完整 family 範圍與既有物件重疊，請清出空地。");
        }
    }
}

internal sealed class StarterKitDialog : Form
{
    private readonly NumericUpDown _team = new() { Minimum = 0, Maximum = 7, Dock = DockStyle.Fill };
    private readonly NumericUpDown _x = new() { Minimum = 0, Maximum = 63.99m, DecimalPlaces = 2, Value = 32, Dock = DockStyle.Fill };
    private readonly NumericUpDown _z = new() { Minimum = 0, Maximum = 63.99m, DecimalPlaces = 2, Value = 32, Dock = DockStyle.Fill };
    internal int Team => (int)_team.Value;
    internal float AnchorX => (float)_x.Value * 256;
    internal float AnchorZ => (float)_z.Value * 256;
    internal void SetInputs(int team, float x, float z) { _team.Value = team; _x.Value = (decimal)x / 256; _z.Value = (decimal)z / 256; }

    internal StarterKitDialog(int team)
    {
        bool en = Loc.CurrentLanguage == Language.English;
        _team.Value = Math.Clamp(team, 0, 7);
        Text = en ? "Blank map starter kit" : "空白地圖起始配置";
        ClientSize = new Size(420, 230); FormBorderStyle = FormBorderStyle.FixedDialog;
        MinimizeBox = MaximizeBox = false; StartPosition = FormStartPosition.CenterParent;
        AutoScaleMode = AutoScaleMode.Dpi; AutoScaleDimensions = new SizeF(96, 96);
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(12), ColumnCount = 2, RowCount = 5 };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        var hint = new Label { Dock = DockStyle.Fill, Text = en
            ? "1 Germanic main house + 4 individual axemen. Anchor uses map tiles. Team 0 is the player. Save writes the kit; Undo removes it as one action."
            : "1 棟日耳曼主屋＋4 名獨立斧兵。錨點使用圖格座標，隊伍 0 為玩家。儲存才寫入，可整批單步復原。" };
        grid.Controls.Add(hint, 0, 0); grid.SetColumnSpan(hint, 2);
        string[] labels = en ? ["Team", "Anchor X", "Anchor Z"] : ["隊伍", "錨點 X", "錨點 Z"];
        Control[] controls = [_team, _x, _z];
        for (int i = 0; i < controls.Length; i++)
        { grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.Controls.Add(new Label { Text = labels[i], AutoSize = true }, 0, i + 1); grid.Controls.Add(controls[i], 1, i + 1); }
        var buttons = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft };
        var apply = new Button { AutoSize = true, Text = en ? "Place" : "放置", DialogResult = DialogResult.OK };
        var cancel = new Button { AutoSize = true, Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel };
        buttons.Controls.AddRange([apply, cancel]); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.Controls.Add(buttons, 0, 4); grid.SetColumnSpan(buttons, 2);
        Controls.Add(grid); AcceptButton = apply; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}
