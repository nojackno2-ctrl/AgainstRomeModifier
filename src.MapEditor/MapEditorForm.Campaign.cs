using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripMenuItem _campaignWaves = new("AI 戰役波次企劃…") { Enabled = false };
    private readonly ToolStripMenuItem _objectiveStudio = new("任務目標設計…") { Enabled = false };
    private readonly ToolStripMenuItem _wildLairs = new("野外巢穴守衛波次…") { Enabled = false };
    internal Func<ObjectiveStudioDialog, DialogResult> ObjectiveStudioDialogRunner { get; set; } = dialog => dialog.ShowDialog();
    internal Func<WildLairDialog, DialogResult> WildLairDialogRunner { get; set; } = dialog => dialog.ShowDialog();
    internal Func<CampaignWaveDialog, DialogResult> CampaignWaveDialogRunner { get; set; } = dialog => dialog.ShowDialog();

    private string[] CampaignAliases() => _objectCatalog.Where(t => t.Category == SdlObjectCategory.Figure).Select(AliasOf).ToArray();
    private ScenarioDocument CampaignScenario()
    {
        var previous = ScenarioDocument.Load(_selected!.DirectoryPath);
        return new ScenarioDocument
        {
            Events = EventSession.Capture().ToList(),
            DataSlots = previous.DataSlots.ToList(),
            Spawns = PlacedDirty() ? _placedObjects.Select(item => new ScenarioSpawn(AliasOf(item.Type), item.WorldX, item.WorldZ, item.Team,
                item.Type.Category == SdlObjectCategory.Figure && !item.Type.IsAnimal ? Math.Max(1, item.UnitCount) : 0,
                (int)MathF.Round(item.Angle), item.WorldY, Prebuilt: item.Type.Category == SdlObjectCategory.Building && item.Team is >= 0 and <= 8)
                { Id = item.ScenarioId }).ToList() : previous.Spawns.ToList()
        };
    }

    internal void RunCampaignWaves()
    {
        if (_selected?.IsCustom != true || _eventGraphDirty || EventSession.Count >= 256) return;
        using var dialog = new CampaignWaveDialog(CampaignAliases(), CampaignScenario());
        if (CampaignWaveDialogRunner(dialog) != DialogResult.OK) return;
        // Recompile against current state; even a custom runner cannot bypass validation.
        var result = ApplyCampaignWaves(dialog.Plan);
        if (result.Diagnostics.Count > 0) _status.Text = string.Join(Environment.NewLine, result.Diagnostics);
    }

    internal void RunObjectiveStudio()
    {
        if (!CanMergeScenarioEvents()) return;
        using var dialog = new ObjectiveStudioDialog(CampaignScenario(), CampaignAliases());
        if (ObjectiveStudioDialogRunner(dialog) != DialogResult.OK || !dialog.CanApply) return;
        MergeScenarioEvents(dialog.CompiledEvents);
    }

    internal void RunWildLairs()
    {
        if (!CanMergeScenarioEvents()) return;
        using var dialog = new WildLairDialog(CampaignAliases(), CampaignScenario());
        if (WildLairDialogRunner(dialog) != DialogResult.OK || !dialog.CanApply) return;
        MergeScenarioEvents(dialog.ResultingEvents);
    }

    private bool CanMergeScenarioEvents() => _selected?.IsCustom == true && !_eventGraphDirty && EventSession.Count < 256;

    /// <summary>Appends already-validated events as one undoable step; refuses to exceed the 256-event cap.</summary>
    internal bool MergeScenarioEvents(IReadOnlyList<ScenarioEvent> events)
    {
        if (!CanMergeScenarioEvents() || events.Count == 0 || EventSession.Count + events.Count > 256) return false;
        int index = EventSession.Count;
        EventSession.AddRange(UniqueEventNames(events));
        _inspectorTabs.SelectedIndex = 5;
        _eventViews.SelectedIndex = 0;
        RefreshEventList(index);
        UpdateEditorState();
        return true;
    }

    /// <summary>Renames incoming events that collide with existing names (e.g. a second Wave_01) by appending _2, _3...</summary>
    private List<ScenarioEvent> UniqueEventNames(IReadOnlyList<ScenarioEvent> events)
    {
        var used = new HashSet<string>(EventSession.Capture().Select(e => e.Name), StringComparer.OrdinalIgnoreCase);
        var result = new List<ScenarioEvent>(events.Count);
        foreach (var item in events)
        {
            string name = item.Name;
            for (int n = 2; !used.Add(name); n++) name = $"{item.Name}_{n}";
            result.Add(name == item.Name ? item : item with { Name = name });
        }
        return result;
    }

    internal CampaignCompilationResult ApplyCampaignWaves(CampaignMissionPlan plan)
    {
        if (_selected?.IsCustom != true || _eventGraphDirty)
            return new(false, [], [Loc.CurrentLanguage == Language.English
                ? "Select a custom map and apply or discard graph edits first."
                : "請先選擇自製地圖，並套用或放棄事件圖編輯。"]);
        // Reject planning fields the compiler currently ignores instead of silently dropping them.
        if (plan.FactionProfiles.Count > 0 || plan.Waves.Any(w => w.AssignedPathId.HasValue || w.TargetObjectId.HasValue || w.Squads.Any(s => s.RelativeDelaySeconds != 0))
            || plan.Reinforcements.Any(r => r.AssignedPathId.HasValue || r.Squads.Any(s => s.RelativeDelaySeconds != 0)))
            return new(false, [], [Loc.CurrentLanguage == Language.English ? "Faction AI, paths, attack targets and relative squad delays are unsupported." : "尚不支援勢力 AI、路徑、攻擊目標與部隊相對延遲。"]);
        var result = CampaignWaveDialog.CompileForMerge(plan, CampaignAliases(), CampaignScenario());
        if (!result.Success || result.CompiledEvents.Count == 0) return result;
        return MergeScenarioEvents(result.CompiledEvents) ? result : new(false, [], [Loc.CurrentLanguage == Language.English ? "Merging would exceed the 256-event limit." : "合併後會超過 256 個事件上限。"]);
    }

    private bool EventHistoryActive => _inspectorTabs.SelectedIndex == 5;
    private void UpdateEventHistoryButtons()
    {
        if (!EventHistoryActive) return;
        bool editable = _selected?.IsCustom == true && !_eventGraphDirty;
        _undoButton.Enabled = editable && EventSession.CanUndo;
        _redoButton.Enabled = editable && EventSession.CanRedo;
    }
}
