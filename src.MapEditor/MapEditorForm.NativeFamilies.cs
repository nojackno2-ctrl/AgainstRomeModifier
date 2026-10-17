using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly Button _nativeCopyButton = new() { Height = 32, Enabled = false };
    private readonly Label _nativeSourceLabel = new() { Dock = DockStyle.Top, AutoSize = true, Padding = new Padding(4, 4, 4, 4), ForeColor = WinFormsTheme.TextSecondary };
    private NativeFamilySourceInfo? _activeNativeSource;

    internal sealed record NativeFamilySourceInfo(
        int Slot,
        uint Uid,
        int TypeId,
        string Alias,
        int Team,
        float WorldX,
        float WorldY,
        float WorldZ,
        float Rotation,
        SdlObjectType CatalogType);

    internal Button NativeCopyButton => _nativeCopyButton;
    internal Label NativeSourceLabel => _nativeSourceLabel;
    internal NativeFamilySourceInfo? ActiveNativeSource => _activeNativeSource;

    /// <summary>
    /// 可信原生別名解析器：
    /// MUST _objdefNames[typeId] -> known _objectCatalog.NameDef -> canonical AliasOf。
    /// BuildSpawnCatalog.Definition == -1，不能依賴 Definition 欄位。
    /// 當同一 NameDef 有多個 alias 時，以字典序選取 deterministic canonical 項目。
    /// </summary>
    internal string? ResolveNativeAlias(int typeId)
    {
        if (!_objdefNames.TryGetValue(typeId, out string? nameDef) || string.IsNullOrWhiteSpace(nameDef))
            return null;

        var matches = _objectCatalog
            .Where(type => type.NameDef.Equals(nameDef, StringComparison.OrdinalIgnoreCase))
            .ToList();

        if (matches.Count == 0) return null;

        var canonical = matches
            .OrderBy(type => AliasOf(type), StringComparer.OrdinalIgnoreCase)
            .First();

        return AliasOf(canonical);
    }

    private Control BuildNativeFamilyPanel()
    {
        var panel = new Panel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            Padding = new Padding(4, 4, 4, 4),
            BackColor = WinFormsTheme.SurfaceRaised
        };
        _nativeCopyButton.Dock = DockStyle.Top;
        _nativeCopyButton.Click += (_, _) => CopyActiveNativeFamily();
        WinFormsTheme.StyleSecondaryButton(_nativeCopyButton);

        panel.Controls.Add(_nativeCopyButton);
        panel.Controls.Add(_nativeSourceLabel);
        panel.SizeChanged += (_, _) =>
        {
            _nativeSourceLabel.MaximumSize = new Size(
                Math.Max(1, panel.ClientSize.Width - panel.Padding.Horizontal), 0);
        };
        UpdateNativeSourceUi();
        return panel;
    }

    internal void UpdateNativeSourceUi()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _nativeCopyButton.Text = isEn ? "Copy Native Family" : "複製原生群組";

        if (_activeNativeSource is null)
        {
            _nativeCopyButton.Enabled = false;
            _nativeSourceLabel.Text = isEn
                ? "3D Pick: Click an unassigned native building/root in 3D to copy."
                : "3D 選取：點選 3D 場景中的原圖原生群組根物件以啟用複製。";
        }
        else
        {
            _nativeCopyButton.Enabled = _selected?.IsCustom == true;
            string typeName = ObjectDisplayName(_activeNativeSource.CatalogType, isEn);
            _nativeSourceLabel.Text = isEn
                ? $"Native Source: {typeName} (Slot {_activeNativeSource.Slot}, UID {_activeNativeSource.Uid}, Team {_activeNativeSource.Team})"
                : $"原生來源：{typeName} (槽位 {_activeNativeSource.Slot}, UID {_activeNativeSource.Uid}, 隊伍 {_activeNativeSource.Team})";
        }
    }

    /// <summary>
    /// 3D 場景選取 DATA 物件（ObjectIndex == -100000 - slot）挑選原生群組來源。
    /// 若為 child、unsupported、shared、bad UID、未在 catalog 中找到 alias，或已在 Scenario 中 bound，則停用並明確狀態。
    /// 僅限 custom 地圖。
    /// </summary>
    internal void SelectNativeSceneObject(int slot)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? "Original campaign maps cannot be modified directly." : "官方原版地圖無法直接修改。";
            return;
        }

        LevelObjectStore store;
        try
        {
            store = LevelObjectStore.Load(_selected.DirectoryPath,
                NativeObjectDefinitions.Load(_gamePath),
                NativeLightDefinitions.Load(_gamePath));
        }
        catch (Exception ex)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? $"Failed to load DATA store: {ex.Message}" : $"無法載入 DATA 資料：{ex.Message}";
            return;
        }

        var obj = store.Objects().FirstOrDefault(o => o.Slot == slot);
        if (obj is null)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? $"Slot {slot} is inactive or does not exist." : $"槽位 {slot} 為空或不存在。";
            return;
        }

        // 檢查是否已有 scenario 綁定（已 bound 者不能繞過 NativeRequest，必須走 ordinary GUID 複製）
        if (_boundRootSlots.Contains(slot))
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _nativeSourceLabel.Text = isEn
                ? $"Slot {slot} already has a scenario binding; duplicate from the Placed list instead."
                : $"槽位 {slot} 已存在場景綁定；請由「放置物件」清單複製。";
            _status.Text = _nativeSourceLabel.Text;
            return;
        }

        // 檢查是否為獨立支援的 closed family 根節點
        var family = store.FamilyAt(slot, obj.Uid);
        if (family is null)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _nativeSourceLabel.Text = isEn
                ? $"Slot {slot} is not an independent root (child, shared data, or invalid closure)."
                : $"槽位 {slot} 非獨立支援群組根節點（可能是子物件、共用資料或閉包不完整）。";
            _status.Text = _nativeSourceLabel.Text;
            return;
        }

        // 檢查可信 alias 解析
        string? alias = ResolveNativeAlias(obj.TypeId);
        if (string.IsNullOrWhiteSpace(alias))
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _nativeSourceLabel.Text = isEn
                ? $"Slot {slot} (TypeId {obj.TypeId}) has no matching alias in placement catalog."
                : $"槽位 {slot} (TypeId {obj.TypeId}) 在放置目錄中無對應別名，無法複製。";
            _status.Text = _nativeSourceLabel.Text;
            return;
        }

        // 尋找 catalog 中的 SdlObjectType
        var catalogType = _objectCatalog.FirstOrDefault(t => AliasOf(t).Equals(alias, StringComparison.OrdinalIgnoreCase));
        if (catalogType is null)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _nativeSourceLabel.Text = isEn
                ? $"Alias {alias} not found in placement catalog."
                : $"別名 {alias} 未在放置目錄中找到。";
            _status.Text = _nativeSourceLabel.Text;
            return;
        }

        _activeNativeSource = new NativeFamilySourceInfo(
            slot,
            obj.Uid,
            obj.TypeId,
            alias,
            obj.Team,
            obj.X,
            obj.Y,
            obj.Z,
            obj.Rotation,
            catalogType);

        UpdateNativeSourceUi();
        _status.Text = isEn
            ? $"Selected native family root {catalogType.NameDef} (Slot {slot}, UID {obj.Uid}). Click \"Copy Native Family\" to duplicate."
            : $"已選取原生群組根物件 {catalogType.NameDef}（槽位 {slot}，UID {obj.Uid}）；點擊「複製原生群組」即可複製。";
    }

    /// <summary>
    /// 執行原生群組複製：
    /// 重新查驗 store/UID/root closed，防止快取過時狀態；
    /// 建立全新的 Guid、transient slot/UID identity、RawRotation 快照；
    /// fixed translation（預設 256 X/Z）；
    /// 原圖未 binding root 不建立 source Scenario/binding；
    /// 只 staged new copy 入 placement session。
    /// </summary>
    internal void CopyActiveNativeFamily()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true || _activeNativeSource is null) return;

        // Click 時重新查 store / UID / root closed，不 cache stale acceptance
        LevelObjectStore store;
        try
        {
            store = LevelObjectStore.Load(_selected.DirectoryPath,
                NativeObjectDefinitions.Load(_gamePath),
                NativeLightDefinitions.Load(_gamePath));
        }
        catch (Exception ex)
        {
            _status.Text = isEn ? $"Store load failed: {ex.Message}" : $"讀取地圖資料失敗：{ex.Message}";
            return;
        }

        var sourceInfo = _activeNativeSource;
        var currentObj = store.Objects().FirstOrDefault(o => o.Slot == sourceInfo.Slot);
        if (currentObj is null || currentObj.Uid != sourceInfo.Uid)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? "Source object UID changed or object was removed." : "原生來源物件 UID 已變更或物件已不存在。";
            return;
        }

        if (_boundRootSlots.Contains(sourceInfo.Slot))
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? "Source root is already bound in scenario." : "原生來源物件已在場景中綁定。";
            return;
        }

        var family = store.FamilyAt(sourceInfo.Slot, sourceInfo.Uid);
        if (family is null)
        {
            _activeNativeSource = null;
            UpdateNativeSourceUi();
            _status.Text = isEn ? "Source family closure is no longer valid." : "原生來源群組閉包已失效。";
            return;
        }

        const float offsetX = 256f;
        const float offsetZ = 256f;
        float newX = sourceInfo.WorldX + offsetX;
        float newZ = sourceInfo.WorldZ + offsetZ;
        float newY = sourceInfo.WorldY;

        float exactDegrees = float.IsFinite(sourceInfo.Rotation)
            ? (float)((double)sourceInfo.Rotation * (180d / Math.PI))
            : 0f;

        var newGuid = Guid.NewGuid();
        var stagedCopy = new SdlPlacedObject(
            sourceInfo.CatalogType,
            newX, newY, newZ,
            sourceInfo.Team,
            Angle: exactDegrees,
            UnitCount: 0)
        {
            ScenarioId = newGuid,
            CopySourceScenarioId = Guid.Empty, // 原圖未 binding root 不建立 source binding
            NativeSourceIdentity = new NativeObjectIdentity(sourceInfo.Slot, sourceInfo.Uid),
            RawRotation = sourceInfo.Rotation
        };

        try
        {
            _placementSession.Add(stagedCopy);
        }
        catch (Exception ex) when (ex is ArgumentOutOfRangeException or InvalidDataException)
        {
            _status.Text = isEn ? $"Cannot stage copy: {ex.Message}" : $"無法暫存副本：{ex.Message}";
            return;
        }

        RefreshPlacedList();
        PushSceneObjects(EffectiveSceneObjects());
        UpdateEditorState();

        // 選取放置清單中新建立的項目
        if (_placedList.Items.Count > 0)
        {
            _placedList.SelectedItems.Clear();
            _placedList.Items[^1].Selected = true;
            _placedList.EnsureVisible(_placedList.Items.Count - 1);
        }

        _status.Text = isEn
            ? $"Staged native family copy of {sourceInfo.CatalogType.NameDef}; click Save to write it."
            : $"已暫存原生群組副本 {sourceInfo.CatalogType.NameDef}；按「儲存」才會寫入 DATA。";
    }

    internal void ResetNativeSelection()
    {
        _activeNativeSource = null;
        UpdateNativeSourceUi();
    }
}
