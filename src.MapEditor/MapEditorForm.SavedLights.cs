using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    // Owned only by this Form. Open at load/committed-save boundaries, never per frame.
    private RustLightPool? _savedLightPool;
    private IReadOnlyList<NativeLightInstance> _savedLightBaseline = Array.Empty<NativeLightInstance>();
    private SavedSceneLightSet _savedSceneLights = SavedSceneLightSet.Empty;
    private HashSet<SceneLightSuppressionKey> _savedBaselineLightKeys = new();
    private string? _savedLightLoadDiagnostic;
    private string? _savedLightProjectionDiagnostic;
    internal string? SavedLightPreviewDiagnostic => _savedLightProjectionDiagnostic ?? _savedLightLoadDiagnostic
        ?? _savedParticleLoadDiagnostic;
    internal SavedSceneLightSet SavedLightPreview => _savedSceneLights;

    private static bool IsSavedLightPreviewFailure(Exception ex) => ex is IOException or InvalidDataException or ArgumentException
        or OverflowException or InvalidOperationException or DllNotFoundException or BadImageFormatException
        or EntryPointNotFoundException or UnauthorizedAccessException;

    private void ClearSavedLights()
    {
        ClearSavedParticles();
        ClearSavedLightPreview();
    }

    private void ClearSavedLightPreview()
    {
        _savedLightPool?.Dispose();
        _savedLightPool = null;
        _savedLightBaseline = Array.Empty<NativeLightInstance>();
        _savedSceneLights = SavedSceneLightSet.Empty;
        _savedBaselineLightKeys.Clear();
        _savedLightLoadDiagnostic = _savedLightProjectionDiagnostic = null;
        _view3d?.UpdateSavedSceneLights(null);
    }

    private static NativeLightInstance SavedInstance(RustLightSnapshot snapshot) => NativeLightInstance.CreateSaved(
        snapshot.Slot, snapshot.Position, snapshot.Color, snapshot.EffectiveRadius, snapshot.BaselineRadius, snapshot.IsActive);

    private void LoadSavedLights(string map)
    {
        RustLightPool? candidate = null;
        try
        {
            candidate = RustLightPool.Load(Path.Combine(map, "DATA", "light.dat"));
            var instances = new List<NativeLightInstance>();
            for (int slot = 0; slot < RustLightPool.Capacity; slot++)
                if (candidate.Capture(slot) is { } snapshot) instances.Add(SavedInstance(snapshot));
            var keys = BuildBaselineLightKeys();
            _savedLightPool?.Dispose();
            _savedLightPool = candidate;
            candidate = null;
            _savedLightBaseline = instances.ToArray();
            _savedSceneLights = new SavedSceneLightSet(_savedLightBaseline, null);
            _savedBaselineLightKeys = keys;
            _savedLightLoadDiagnostic = null;
        }
        catch (Exception ex) when (IsSavedLightPreviewFailure(ex))
        {
            // Do not discard an already adopted preview if a candidate fails.
            _savedLightLoadDiagnostic = $"Saved light preview unavailable: {ex.Message}";
        }
        finally { candidate?.Dispose(); }
    }

    private static SceneLightSuppressionKey PlacedLightKey(int index) => new(SdlPlacedObjectsFile.FileName, -5000 - index);

    private HashSet<SceneLightSuppressionKey> BaselineLightKeys() => new(_savedBaselineLightKeys);

    private HashSet<SceneLightSuppressionKey> BuildBaselineLightKeys()
    {
        var keys = new HashSet<SceneLightSuppressionKey>();
        if (_familyPreviewStore is null) return keys;
        // A successfully decoded complete saved pool also represents absence
        // of lights. Never synthesize definition lights for saved DATA objects,
        // including objects whose family cannot be edited independently.
        foreach (var item in _levelObjects)
            keys.Add(new SceneLightSuppressionKey("DATA/objects.dat", -100000 - item.Slot));
        return keys;
    }

    private IReadOnlyList<NativeLightInstance> ProjectSavedFamilyLights(LevelObjectFamily family, Guid target, int dx, int dy, int dz)
    {
        if (family.LightSlots.Count == 0 && family.ParticleSlots.Count == 0) return Array.Empty<NativeLightInstance>();
        var snapshot = _familyPreviewStore!.CaptureFamily(family.RootSlot, family.RootUid)
            ?? throw new InvalidDataException("Saved light family is no longer closed.");
        var particles = new List<RustParticleLightRequest>();
        var particleSlots = new HashSet<int>();
        foreach (var (slot, source) in snapshot.Particles)
        {
            var emitter = SavedParticleForFamily(slot, source, family);
            // Only describe links/ordinals here. Full opaque record and all numeric work go to Rust.
            var record = emitter.CopyEmitterRecord();
            for (int ordinal = 0; ordinal < RustParticleLightProjection.ChildCount; ordinal++)
            {
                int link = BinaryPrimitives.ReadInt16LittleEndian(emitter.RawRecord.Slice(12 + ordinal * 35 + 33));
                if (link == -1) continue;
                if (!family.LightSlots.Contains(link) || !particleSlots.Add(link))
                    throw new InvalidDataException("Saved particle light link is outside the closed family or duplicated.");
                particles.Add(new RustParticleLightRequest(target, record, ordinal, link, dx, dy, dz));
            }
        }
        if (family.LightSlots.Count == 0) return Array.Empty<NativeLightInstance>();
        var pool = _savedLightPool ?? throw new InvalidDataException(
            _savedLightLoadDiagnostic ?? "Saved light pool has not been loaded.");
        var direct = pool.ProjectBatch(family.LightSlots.Where(slot => !particleSlots.Contains(slot))
            .Select(slot => new RustLightProjectionRequest(target, slot, dx, dy, dz)));
        var children = pool.ProjectParticleBatch(particles);
        // Neither batch result is observable before both batches succeed.
        return direct.Items.Select(item => SavedInstance(item.Snapshot))
            .Concat(children.Items.Select(item => SavedInstance(item.Snapshot))).ToArray();
    }
}
