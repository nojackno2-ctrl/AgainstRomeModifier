using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using AgainstRomeMapEditor.Modules.Placement;

namespace AgainstRomeMapEditor;

internal sealed record SavedFamilyParticleFrame(int RootSlot, int EmitterSlot, int OwnerSlot,
    int OwnerType, NativeParticleFrameCandidate Child)
{
    public Guid TargetScenarioId { get; init; }
}

internal sealed partial class MapEditorForm
{
    private IReadOnlyList<SavedFamilyParticleFrame> _savedFamilyParticleFrames = Array.Empty<SavedFamilyParticleFrame>();
    internal IReadOnlyList<SavedFamilyParticleFrame> SavedFamilyParticleFrames => _savedFamilyParticleFrames;
    internal string? SavedFamilyParticleFrameDiagnostic { get; private set; }

    internal bool TryCaptureSavedFamilyParticleFrames(int rootSlot, uint rootUid,
        out IReadOnlyList<SavedFamilyParticleFrame> frames, out string? diagnostic,
        int dx = 0, int dy = 0, int dz = 0)
        => TryCaptureSavedFamilyParticleFramesCore(rootSlot, rootUid, out frames, out diagnostic, dx, dy, dz);

    private bool TryCaptureSavedFamilyParticleFramesCore(int rootSlot, uint rootUid,
        out IReadOnlyList<SavedFamilyParticleFrame> frames, out string? diagnostic,
        int dx, int dy, int dz,
        IReadOnlyDictionary<int, LevelWorldObject>? loaded = null,
        IReadOnlyDictionary<int, LevelWorldObject>? baseline = null)
    {
        frames = Array.Empty<SavedFamilyParticleFrame>(); diagnostic = null;
        try
        {
            if (_selected is null || !IsSavedParticleMap(_selected.DirectoryPath)
                || _familyPreviewStore is null || _savedParticleLoadDiagnostic is not null)
                throw new InvalidDataException(_savedParticleLoadDiagnostic ?? "Saved particle family baseline is unavailable for the current map.");
            var family = _familyPreviewStore.FamilyAt(rootSlot, rootUid)
                ?? throw new InvalidDataException("Saved particle family is no longer closed.");
            var snapshot = _familyPreviewStore.CaptureFamily(rootSlot, rootUid)
                ?? throw new InvalidDataException("Saved particle family snapshot is unavailable.");
            loaded ??= _levelObjects.ToDictionary(item => item.Slot);
            baseline ??= _familyPreviewStore.Objects().ToDictionary(item => item.Slot);
            foreach (int slot in family.ObjectSlots)
                if (!loaded.TryGetValue(slot, out var owner) || !baseline.TryGetValue(slot, out var source)
                    || owner.Uid != source.Uid || owner.TypeId != source.TypeId)
                    throw new InvalidDataException("Saved particle owner identity is stale.");
            if (!family.ParticleSlots.Order().SequenceEqual(snapshot.Particles.Keys.Order()))
                throw new InvalidDataException("Saved particle family membership changed.");
            var result = new List<SavedFamilyParticleFrame>();
            foreach (var (slot, source) in snapshot.Particles)
            {
                var saved = SavedParticleForFamily(slot, source, family);
                var owner = loaded[saved.Owner];
                if (!TryResolveSavedParticleFrameCandidates(owner.TypeId, (uint)slot, saved.RecordBytes(),
                    saved.StateBytes(), out var children, out diagnostic, dx, dy, dz)) return false;
                result.AddRange(children.Select(child => new SavedFamilyParticleFrame(rootSlot, slot,
                    saved.Owner, owner.TypeId, child)));
            }
            frames = result.AsReadOnly(); return true;
        }
        catch (Exception error) when (IsSavedLightPreviewFailure(error))
        { diagnostic = error.Message; return false; }
    }

    private void RefreshSavedFamilyParticleFrames()
    {
        var result = new List<SavedFamilyParticleFrame>();
        SavedFamilyParticleFrameDiagnostic = null;
        if (_familyPreviewStore is not null)
        {
            IReadOnlyDictionary<int, LevelWorldObject>? loaded = null, baseline = null;
            foreach (var root in _levelObjects)
            {
                var family = _familyPreviewStore.FamilyAt(root.Slot, root.Uid);
                if (family is null || family.ParticleSlots.Count == 0) continue;
                loaded ??= _levelObjects.ToDictionary(item => item.Slot);
                baseline ??= _familyPreviewStore.Objects().ToDictionary(item => item.Slot);
                if (TryCaptureSavedFamilyParticleFramesCore(root.Slot, root.Uid, out var frames, out var reason,
                    0, 0, 0, loaded, baseline))
                    result.AddRange(frames);
                else SavedFamilyParticleFrameDiagnostic ??= reason;
            }
        }
        _savedFamilyParticleFrames = result.AsReadOnly();
    }

    private IReadOnlyList<MapSceneObject> ProjectFamilyPreviewWithParticles(IReadOnlyList<MapSceneObject> effective,
        out HashSet<int> suppressed)
    {
        RefreshSavedFamilyParticleFrames();
        var scene = ProjectFamilyPreview(effective, out suppressed);
        var suppressedRoots = suppressed;
        var result = _savedFamilyParticleFrames.Where(frame => !suppressedRoots.Contains(frame.RootSlot)).ToList();
        var store = _familyPreviewStore;
        if (store is not null)
        {
            var current = _placementSession.Capture();
            var owners = _levelObjects.ToDictionary(owner => owner.Slot);
            IReadOnlyDictionary<int, LevelWorldObject>? baseline = null;
            // Only adopt targets already accepted by the existing closed-family scene transaction.
            var accepted = scene.Where(member => member.SourceFile == SdlPlacedObjectsFile.FileName
                && member.HasNativeSavedPosition && member.ObjectIndex <= -5000)
                .Select(member => -5000 - member.ObjectIndex).Distinct();
            foreach (int index in accepted)
            {
                if ((uint)index >= (uint)current.Count) continue;
                var target = current[index];
                try
                {
                    var validation = target.CopySourceScenarioId == Guid.Empty && target.NativeSourceIdentity is null
                        ? target with { CopySourceScenarioId = target.ScenarioId } : target;
                    var requests = PlacementEditSession.BuildFamilyCopyRequests([validation], _familyPreviewScenario,
                        store, ResolveNativeAlias);
                    if (!requests.TryGetValue(target.ScenarioId, out var request)) continue;
                    var owner = owners[request.SourceSlot];
                    int dx = PreviewFixedDelta(owner.X, target.WorldX), dy = PreviewFixedDelta(owner.Y, target.WorldY),
                        dz = PreviewFixedDelta(owner.Z, target.WorldZ);
                    baseline ??= store.Objects().ToDictionary(owner => owner.Slot);
                    if (!TryCaptureSavedFamilyParticleFramesCore(request.SourceSlot, request.SourceUid,
                        out var children, out var reason, dx, dy, dz, owners, baseline))
                    { SavedFamilyParticleFrameDiagnostic ??= reason; continue; }
                    result.AddRange(children.Select(child => child with { TargetScenarioId = target.ScenarioId }));
                }
                catch (Exception error) when (IsSavedLightPreviewFailure(error))
                { SavedFamilyParticleFrameDiagnostic ??= error.Message; }
            }
        }
        _savedFamilyParticleFrames = result.AsReadOnly();
        return scene;
    }
}
