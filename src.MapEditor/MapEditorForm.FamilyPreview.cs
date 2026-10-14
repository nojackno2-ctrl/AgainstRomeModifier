using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    // Private, same-map baseline: never mutated by preview or a save candidate.
    private LevelObjectStore? _familyPreviewStore;
    private ScenarioDocument _familyPreviewScenario = new();
    private IReadOnlyList<SdlPlacedObject> _familyPreviewPlacements = Array.Empty<SdlPlacedObject>();

    private void ClearFamilyPreview()
    {
        // LoadLevelObjects clears the family before opening its replacement. Keep only
        // the same-map immutable particle baseline across that boundary on failure.
        if (IsSavedParticleMap(_selected?.DirectoryPath)) ClearSavedLightPreview();
        else ClearSavedLights();
        _familyPreviewStore = null;
        _familyPreviewScenario = new();
        _familyPreviewPlacements = Array.Empty<SdlPlacedObject>();
    }

    private void LoadFamilyPreview(LevelObjectStore store, string map)
    {
        // Read only at load/committed-save boundaries, never during rendering.
        try
        {
            _familyPreviewScenario = ScenarioDocument.Load(map);
            _familyPreviewPlacements = _placementSession.Capture();
            _familyPreviewStore = store;
            LoadSavedParticles(map);
            LoadSavedLights(map);
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or System.Text.Json.JsonException)
        {
            ClearFamilyPreview();
        }
    }

    private static int PreviewFixedDelta(float source, float target)
    {
        double value = ((double)target - source) * 65536d;
        if (!double.IsFinite(value) || value < int.MinValue || value > int.MaxValue || value != Math.Truncate(value))
            throw new InvalidDataException("群組預覽位移超出儲存支援範圍。");
        return (int)value;
    }

    private IReadOnlyList<MapSceneObject> ProjectFamilyPreview(IReadOnlyList<MapSceneObject> effective,
        out HashSet<int> suppressed)
    {
        var suppressedSlots = new HashSet<int>();
        suppressed = suppressedSlots;
        var projected = new List<MapSceneObject>();
        var adoptedLights = new List<AgainstRomeMapEditor.NativeAssets.NativeLightInstance>();
        var removedLights = new HashSet<int>();
        var lightKeys = BaselineLightKeys();
        _savedLightProjectionDiagnostic = null;
        var replaced = new HashSet<int>();
        var store = _familyPreviewStore;
        if (store is null) return ExpandTroopsFor3D(effective).ToArray();
        var objects = _levelObjects.ToDictionary(item => item.Slot);
        var baselineObjects = store.Objects().ToDictionary(item => item.Slot);
        var current = _placementSession.Capture();

        LevelObjectFamily? ValidFamily(int slot, uint uid, Guid owner)
        {
            if (!objects.TryGetValue(slot, out var root) || root.Uid != uid) return null;
            var family = store.FamilyAt(slot, uid);
            if (family is null || family.ObjectSlots.Any(member => !objects.TryGetValue(member, out var loaded)
                || !baselineObjects.TryGetValue(member, out var saved) || loaded.Uid != saved.Uid)) return null;
            // No other binding may own a root or descendant, even with a stale UID.
            if (_familyPreviewScenario.DataSlots.Any(binding => family.ObjectSlots.Contains(binding.Slot)
                && (owner == Guid.Empty || binding.SpawnId != owner))) return null;
            if (owner != Guid.Empty && _familyPreviewScenario.DataSlots.Count(binding => binding.SpawnId == owner) != 1)
                return null;
            return family;
        }

        (List<MapSceneObject> Members, IReadOnlyList<AgainstRomeMapEditor.NativeAssets.NativeLightInstance> Lights) Project(LevelObjectFamily family, SdlPlacedObject target, int index)
        {
            var root = objects[family.RootSlot];
            int dx = PreviewFixedDelta(root.X, target.WorldX), dy = PreviewFixedDelta(root.Y, target.WorldY),
                dz = PreviewFixedDelta(root.Z, target.WorldZ);
            var members = new List<MapSceneObject>();
            foreach (int slot in family.ObjectSlots)
            {
                var member = objects[slot];
                float x = (float)(member.X + dx / 65536d), y = (float)(member.Y + dy / 65536d),
                    z = (float)(member.Z + dz / 65536d);
                if (!float.IsFinite(x) || !float.IsFinite(y) || !float.IsFinite(z))
                    throw new InvalidDataException("群組預覽座標不是有限 float32。");
                members.Add(new MapSceneObject(_objdefNames.GetValueOrDefault(member.TypeId) ?? "", x, y, z,
                    member.Team, SdlPlacedObjectsFile.FileName, -5000 - index,
                    Angle: float.IsFinite(member.Rotation) ? (float)((double)member.Rotation * (180d / Math.PI)) : null)
                { NativeVisualState = LoadedVisualState(member), HasNativeSavedPosition = true });
            }
            // Complete both Rust effect batches before adopting members or suppression.
            var lights = ProjectSavedFamilyLights(family, target.ScenarioId, dx, dy, dz);
            return (members, lights);
        }

        for (int index = 0; index < current.Count; index++)
        {
            var item = current[index];
            if (item.CopySourceScenarioId == Guid.Empty && item.NativeSourceIdentity is null) continue;
            try
            {
                var requests = PlacementEditSession.BuildFamilyCopyRequests([item], _familyPreviewScenario, store, ResolveNativeAlias);
                if (!requests.TryGetValue(item.ScenarioId, out var request)) continue;
                Guid owner = item.NativeSourceIdentity.HasValue ? Guid.Empty : item.CopySourceScenarioId;
                var family = ValidFamily(request.SourceSlot, request.SourceUid, owner);
                if (family is null) continue;
                var candidate = Project(family, item, index);
                projected.AddRange(candidate.Members);
                adoptedLights.AddRange(candidate.Lights);
                if (family.LightSlots.Count > 0) lightKeys.Add(PlacedLightKey(index));
                replaced.Add(index);
            }
            catch (Exception ex) when (IsSavedLightPreviewFailure(ex)) { _savedLightProjectionDiagnostic = ex.Message; }
        }

        foreach (var binding in _familyPreviewScenario.DataSlots.Where(binding => binding.SpawnId != Guid.Empty))
        {
            try
            {
                var baseline = _familyPreviewPlacements.Where(item => item.ScenarioId == binding.SpawnId).ToArray();
                var matches = current.Select((item, index) => (item, index)).Where(pair => pair.item.ScenarioId == binding.SpawnId).ToArray();
                if (baseline.Length != 1 || matches.Length > 1) continue;
                var target = matches.Length == 0 ? baseline[0] : matches[0].item;
                var family = ValidFamily(binding.Slot, binding.Uid, binding.SpawnId);
                if (family is null) continue;
                var old = _familyPreviewScenario.Spawns.SingleOrDefault(spawn => spawn.Id == binding.SpawnId && spawn.Prebuilt);
                if (old is null) continue;
                // Ordinary independently owned single objects still use save's template path and
                // the legacy marker/formation renderer (including supported team/angle edits).
                if (family.LightSlots.Count == 0 && family.ParticleSlots.Count == 0
                    && family.ObjectSlots.Count == 1 && old.RawRotation is null && target.RawRotation is null
                    && old.Alias.Equals(AliasOf(target.Type), StringComparison.OrdinalIgnoreCase)
                    && target.Type.Category == SdlObjectCategory.Building && target.Team is >= 0 and <= 8
                    && store.OwnedTemplate(binding.Slot, binding.Uid) is not null)
                {
                    suppressedSlots.UnionWith(family.ObjectSlots);
                    continue;
                }
                if (target.RawRotation is { } raw && ScenarioLevelObjects.NormalizeAngle((int)MathF.Round(target.Angle))
                    != ScenarioLevelObjects.RadiansToDisplayAngle(raw)) continue;
                // Reuse the same source GUID/alias/prebuilt/team/exact rotation contracts as save copies.
                var validation = target with { CopySourceScenarioId = binding.SpawnId, NativeSourceIdentity = null };
                var requests = PlacementEditSession.BuildFamilyCopyRequests([validation], _familyPreviewScenario, store, ResolveNativeAlias);
                if (!requests.ContainsKey(target.ScenarioId)) continue;
                // Deletion also validates every effect before removing the baseline.
                var candidate = Project(family, target, matches.Length == 0 ? 0 : matches[0].index);
                // Accept suppression only after the entire projection has succeeded.
                suppressedSlots.UnionWith(family.ObjectSlots);
                removedLights.UnionWith(family.LightSlots);
                if (matches.Length != 0)
                {
                    replaced.Add(matches[0].index);
                    projected.AddRange(candidate.Members);
                    adoptedLights.AddRange(candidate.Lights);
                    if (family.LightSlots.Count > 0) lightKeys.Add(PlacedLightKey(matches[0].index));
                }
            }
            catch (Exception ex) when (IsSavedLightPreviewFailure(ex)) { _savedLightProjectionDiagnostic = ex.Message; }
        }

        foreach (int slot in _natureSession.RemovedSlots)
        {
            if (!objects.TryGetValue(slot, out var root)) continue;
            var family = ValidFamily(slot, root.Uid, Guid.Empty);
            if (family is null) continue;
            try
            {
                _ = ProjectSavedFamilyLights(family, Guid.Empty, 0, 0, 0);
                removedLights.UnionWith(family.LightSlots);
                suppressedSlots.UnionWith(family.ObjectSlots);
            }
            catch (Exception ex) when (IsSavedLightPreviewFailure(ex)) { _savedLightProjectionDiagnostic = ex.Message; }
        }
        _savedSceneLights = new AgainstRomeMapEditor.NativeAssets.SavedSceneLightSet(
            _savedLightBaseline.Where(light => !removedLights.Contains(light.SavedSlot)).Concat(adoptedLights), lightKeys);

        bool Keep(MapSceneObject item)
        {
            if (item.SourceFile.Equals(SdlPlacedObjectsFile.FileName, StringComparison.OrdinalIgnoreCase))
                return !replaced.Contains(-5000 - item.ObjectIndex);
            return !item.SourceFile.Equals("DATA/objects.dat", StringComparison.OrdinalIgnoreCase)
                || !suppressedSlots.Contains(-100000 - item.ObjectIndex);
        }
        // Formation expansion applies only to legacy markers, never native projected members.
        return ExpandTroopsFor3D(effective.Where(Keep).ToArray()).Concat(projected).ToArray();
    }
}
