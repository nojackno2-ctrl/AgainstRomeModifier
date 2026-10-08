namespace AgainstRomeMapEditor.Modules.AI;

/// <summary>AI 與戰役企劃的快照資料結構</summary>
public sealed record CampaignMissionSnapshot(
    CampaignMissionPlan Plan,
    IReadOnlyList<WaypointPath> WaypointPaths)
{
    public static CampaignMissionSnapshot Empty => new(
        new CampaignMissionPlan("Default Mission", "", [], [], [], []),
        Array.Empty<WaypointPath>());
}

/// <summary>AI 與戰役任務編輯工作階段，負責管理路徑點、AI 勢力設定與戰役目標的編輯歷史與狀態追蹤</summary>
public sealed class FactionAiSession : IEditorModule<CampaignMissionSnapshot>
{
    private CampaignMissionPlan _plan;
    private readonly List<WaypointPath> _waypointPaths = new();
    private CampaignMissionSnapshot _baseline;

    public string ModuleId => "campaign_ai";

    public FactionAiSession() : this(CampaignMissionSnapshot.Empty) { }

    public FactionAiSession(CampaignMissionSnapshot initial)
    {
        ArgumentNullException.ThrowIfNull(initial);
        _plan = initial.Plan;
        _waypointPaths.AddRange(initial.WaypointPaths);
        _baseline = Capture();
    }

    public CampaignMissionPlan Plan => _plan;
    public IReadOnlyList<WaypointPath> WaypointPaths => _waypointPaths.ToList();

    public bool IsDirty => !Equals(_plan, _baseline.Plan)
        || _waypointPaths.Count != _baseline.WaypointPaths.Count
        || !_waypointPaths.SequenceEqual(_baseline.WaypointPaths);

    public void Load(CampaignMissionSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        _plan = snapshot.Plan;
        _waypointPaths.Clear();
        _waypointPaths.AddRange(snapshot.WaypointPaths);
        AcceptChanges();
    }

    public CampaignMissionSnapshot Capture()
    {
        return new CampaignMissionSnapshot(_plan, _waypointPaths.ToList());
    }

    public void AcceptChanges()
    {
        _baseline = Capture();
    }

    public void Reset()
    {
        Load(_baseline);
    }

    public void UpdatePlan(CampaignMissionPlan updatedPlan)
    {
        ArgumentNullException.ThrowIfNull(updatedPlan);
        updatedPlan.Validate();
        _plan = updatedPlan;
    }

    public void AddWaypointPath(WaypointPath path)
    {
        ArgumentNullException.ThrowIfNull(path);
        path.Validate();
        if (_waypointPaths.Any(p => p.Id == path.Id))
            throw new InvalidOperationException($"已存在相同 ID 的路徑：{path.Id}");
        _waypointPaths.Add(path);
    }

    public void UpdateWaypointPath(WaypointPath path)
    {
        ArgumentNullException.ThrowIfNull(path);
        path.Validate();
        int index = _waypointPaths.FindIndex(p => p.Id == path.Id);
        if (index < 0) throw new KeyNotFoundException($"找不到要更新的路徑：{path.Id}");
        _waypointPaths[index] = path;
    }

    public bool RemoveWaypointPath(Guid pathId)
    {
        int index = _waypointPaths.FindIndex(p => p.Id == pathId);
        if (index >= 0)
        {
            _waypointPaths.RemoveAt(index);
            return true;
        }
        return false;
    }

    public WaypointPath? FindPath(Guid pathId)
    {
        return _waypointPaths.FirstOrDefault(p => p.Id == pathId);
    }

    public void AddWave(WaveAttackDefinition wave)
    {
        ArgumentNullException.ThrowIfNull(wave);
        wave.Validate();
        var waves = _plan.Waves.ToList();
        if (waves.Any(w => w.WaveIndex == wave.WaveIndex))
            throw new InvalidOperationException($"已存在第 {wave.WaveIndex} 波設定。");
        waves.Add(wave);
        _plan = _plan with { Waves = waves.OrderBy(w => w.WaveIndex).ToList() };
    }

    public void AddObjective(CampaignObjective objective)
    {
        ArgumentNullException.ThrowIfNull(objective);
        objective.Validate();
        var objectives = _plan.Objectives.ToList();
        if (objectives.Any(o => o.Id == objective.Id))
            throw new InvalidOperationException($"已存在相同 ID 的戰役目標：{objective.Id}");
        objectives.Add(objective);
        _plan = _plan with { Objectives = objectives };
    }

    public void AddReinforcement(ReinforcementDefinition reinforcement)
    {
        ArgumentNullException.ThrowIfNull(reinforcement);
        reinforcement.Validate();
        var reinforcements = _plan.Reinforcements.ToList();
        if (reinforcements.Any(r => r.Id == reinforcement.Id))
            throw new InvalidOperationException($"已存在相同 ID 的增援設定：{reinforcement.Id}");
        reinforcements.Add(reinforcement);
        _plan = _plan with { Reinforcements = reinforcements };
    }

    public void SetFactionProfile(FactionAiProfile profile)
    {
        ArgumentNullException.ThrowIfNull(profile);
        profile.Validate();
        var profiles = _plan.FactionProfiles.Where(f => f.Team != profile.Team).ToList();
        profiles.Add(profile);
        _plan = _plan with { FactionProfiles = profiles.OrderBy(f => f.Team).ToList() };
    }
}
