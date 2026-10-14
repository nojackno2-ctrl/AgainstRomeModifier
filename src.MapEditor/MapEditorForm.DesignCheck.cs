using System.IO;
using System.Linq;
using System.Text;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripMenuItem _designCheckMenuItem = new("設計可玩性檢查…");

    /// <summary>
    /// 自訂儲存前設計警告確認委派（供自動化單元測試攔截或斷言，避免彈出模態視窗）。
    /// </summary>
    internal Func<IReadOnlyList<MapDesignIssue>, bool>? DesignWarningPrompt { get; set; }

    private void InitializeDesignCheck(ToolStrip tools)
    {
        _designCheckMenuItem.Click += (_, _) => RunDesignCheck(showSummaryDialog: true);
        _layoutMenu.DropDownItems.Insert(0, _designCheckMenuItem);
        _layoutMenu.DropDownItems.Insert(1, new ToolStripSeparator());
    }

    private void LocalizeDesignCheck(bool en)
    {
        _designCheckMenuItem.Text = en ? "Design Playability Check…" : "設計可玩性檢查…";
        _designCheckMenuItem.ToolTipText = en
            ? "Check team completeness, placed objects on water/block, out-of-bounds objects, and victory/defeat events."
            : "檢查勢力主堡、起始部隊、水下/阻擋放置、越界物件及勝利/失敗事件。";
    }

    /// <summary>
    /// 執行設計層級的可玩性檢查（針對當前編輯器記憶體狀態）。
    /// </summary>
    internal IReadOnlyList<MapDesignIssue> RunDesignCheck(bool showSummaryDialog = false)
    {
        var objects = new List<ScenarioSpawn>();
        foreach (var item in _placedObjects)
        {
            bool building = item.Type.Category == SdlObjectCategory.Building;
            bool isPrebuilt = (building && item.Team is >= 0 and <= 8) || item.RawRotation.HasValue;
            var spawn = new ScenarioSpawn(
                AliasOf(item.Type),
                item.WorldX,
                item.WorldZ,
                item.Team,
                item.Type.Category == SdlObjectCategory.Figure && !item.Type.IsAnimal && !item.RawRotation.HasValue ? Math.Max(1, item.UnitCount) : 0,
                item.RawRotation is { } raw ? ScenarioLevelObjects.RadiansToDisplayAngle(raw) : (int)MathF.Round(item.Angle),
                item.WorldY,
                Prebuilt: isPrebuilt)
            {
                Id = item.ScenarioId,
                RawRotation = item.RawRotation
            };
            objects.Add(spawn);
        }

        if (!PlacedDirty() && _selected is not null)
        {
            try
            {
                var previous = ScenarioDocument.Load(_selected.DirectoryPath);
                var visibleIds = objects.Select(s => s.Id).ToHashSet();
                foreach (ScenarioSpawn saved in previous.Spawns.Where(s => !visibleIds.Contains(s.Id)))
                {
                    objects.Add(saved);
                }
            }
            catch (Exception ex) when (ex is IOException or InvalidDataException)
            {
                // 忽略檔案讀取異常，以當前放置清單為準
            }
        }

        if (_terrainLayers is null && _selected is not null)
        {
            try
            {
                InitializeTerrainLayers(_selected.DirectoryPath);
            }
            catch (Exception ex) when (ex is IOException or InvalidDataException)
            {
                // 忽略圖層讀取異常
            }
        }

        var events = EventSession.Capture();
        var request = new MapDesignValidationRequest(
            Objects: objects,
            Events: events,
            Collision: _terrainLayers?.Collision,
            CollisionSize: _terrainLayers?.Collision is not null ? _terrainLayers.CollisionSize : 0,
            Heights: _terrainLayers?.Heights,
            HeightSize: _terrainLayers?.Heights is not null ? _terrainLayers.VertexSize : 0,
            HeightStep: _heightMapStep > 0 ? _heightMapStep : 4.0f,
            WaterLevel: (float)_waterLevel.Value);

        var issues = MapDesignValidator.Validate(request);

        if (showSummaryDialog)
        {
            bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            if (issues.Count == 0)
            {
                string msg = en
                    ? "Design check passed! All teams have a main house and units, all objects are within bounds and on passable ground, and scenario victory/defeat conditions are configured."
                    : "設計檢查通過！所有參戰勢力均具備主堡與起始部隊，所有物件均在地圖範圍與可通行陸地內，且已設定劇本勝負判定事件。";
                MessageBox.Show(this, msg, en ? "Design Check" : "設計檢查", MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            else
            {
                var sb = new StringBuilder();
                sb.AppendLine(en
                    ? $"Design check found {issues.Count} playability issue(s):"
                    : $"設計檢查發現 {issues.Count} 項可玩性問題：");
                sb.AppendLine();
                foreach (var issue in issues.Take(12))
                {
                    sb.AppendLine("• " + (en ? issue.English : issue.Chinese));
                }
                if (issues.Count > 12)
                {
                    sb.AppendLine(en ? $"... and {issues.Count - 12} more issue(s)." : $"... 以及其他 {issues.Count - 12} 項問題。");
                }
                sb.AppendLine();
                sb.AppendLine(en
                    ? "Would you like to switch to the Map Check tab to inspect and locate them?"
                    : "是否要切換至「地圖檢查」分頁進行檢視與座標定位？");

                var dialogResult = MessageBox.Show(this, sb.ToString(), en ? "Design Check Warnings" : "設計檢查警告",
                    MessageBoxButtons.YesNo, MessageBoxIcon.Warning);

                if (dialogResult == DialogResult.Yes && _mapCheckTab is not null)
                {
                    RefreshMapDiagnostics();
                    _inspectorTabs.SelectedTab = _mapCheckTab;
                }
            }
        }

        return issues;
    }

    /// <summary>
    /// 在儲存前執行設計檢查；若有潛在問題且為使用者互動儲存，顯示警告對話框由設計師決定是否繼續。
    /// </summary>
    internal bool ConfirmDesignCheckBeforeSave(bool interactive)
    {
        if (_selected is null || !_selected.IsCustom) return true;

        var issues = RunDesignCheck();
        if (issues.Count == 0) return true;

        if (DesignWarningPrompt is not null)
            return DesignWarningPrompt(issues);

        if (!interactive) return true;

        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var sb = new StringBuilder();
        sb.AppendLine(en
            ? $"The map has {issues.Count} design-level warning(s) that may affect playability:"
            : $"地圖目前存在 {issues.Count} 項設計層級的問題（可能影響可玩性或遊戲體驗）：");
        sb.AppendLine();
        foreach (var issue in issues.Take(10))
        {
            sb.AppendLine("• " + (en ? issue.English : issue.Chinese));
        }
        if (issues.Count > 10)
        {
            sb.AppendLine(en ? $"... and {issues.Count - 10} more warning(s)." : $"... 以及其他 {issues.Count - 10} 項警告。");
        }
        sb.AppendLine();
        sb.AppendLine(en
            ? "Do you want to continue saving the map anyway?"
            : "是否仍要繼續儲存地圖？\n（選「否」可取消儲存並前往「地圖檢查」分頁定位修復）");

        string title = en ? "Confirm Save (Design Warnings)" : "確認儲存（設計檢查警告）";
        var result = MessageBox.Show(this, sb.ToString(), title, MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
        if (result != DialogResult.Yes)
        {
            if (_mapCheckTab is not null)
            {
                RefreshMapDiagnostics();
                _inspectorTabs.SelectedTab = _mapCheckTab;
            }
            return false;
        }

        return true;
    }
}
