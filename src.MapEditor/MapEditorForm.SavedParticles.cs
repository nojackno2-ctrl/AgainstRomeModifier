using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private RustParticlePool? _savedParticlePool;
    private IReadOnlyDictionary<int, RustParticleSnapshot> _savedParticleBaseline =
        new Dictionary<int, RustParticleSnapshot>();
    private string? _savedParticleMap;
    private string? _savedParticleLoadDiagnostic;

    private bool IsSavedParticleMap(string? map) => map is not null && _savedParticleMap is not null
        && Path.GetFullPath(map).Equals(_savedParticleMap, StringComparison.OrdinalIgnoreCase);

    private void ClearSavedParticles()
    {
        _savedParticlePool?.Dispose();
        _savedParticlePool = null;
        _savedParticleBaseline = new Dictionary<int, RustParticleSnapshot>();
        _savedParticleMap = _savedParticleLoadDiagnostic = null;
    }

    private void LoadSavedParticles(string map)
    {
        if (!IsSavedParticleMap(map)) ClearSavedParticles();
        RustParticlePool? candidate = null;
        try
        {
            candidate = RustParticlePool.Load(Path.Combine(map, "DATA", "particle.dat"));
            var snapshots = new Dictionary<int, RustParticleSnapshot>();
            for (int slot = 0; slot < RustParticlePool.Capacity; slot++)
                if (candidate.Capture(slot) is { } snapshot) snapshots.Add(slot, snapshot);
            _savedParticlePool?.Dispose();
            _savedParticlePool = candidate;
            candidate = null;
            _savedParticleBaseline = snapshots;
            _savedParticleMap = Path.GetFullPath(map);
            _savedParticleLoadDiagnostic = null;
        }
        catch (Exception ex) when (IsSavedLightPreviewFailure(ex))
        {
            // Keep the adopted same-map handle and detached bytes; never mix them unchecked.
            _savedParticleLoadDiagnostic = $"Saved particle baseline unavailable: {ex.Message}";
        }
        finally { candidate?.Dispose(); }
    }

    private RustParticleSnapshot SavedParticleForFamily(int slot, NativeParticleSnapshot source,
        LevelObjectFamily family)
    {
        if (_savedParticlePool is null || !_savedParticleBaseline.TryGetValue(slot, out var saved))
            throw new InvalidDataException(_savedParticleLoadDiagnostic
                ?? $"Saved particle baseline missing for slot {slot}.");
        byte[] record = source.RecordBytes();
        int[] words = source.StateWords();
        Span<byte> state = stackalloc byte[RustParticlePool.StateSize];
        for (int column = 0; column < words.Length; column++)
            BinaryPrimitives.WriteInt32LittleEndian(state[(column * 4)..], words[column]);
        if (saved.Slot != slot || !saved.IsActive || saved.ActiveRaw != BinaryPrimitives.ReadUInt16LittleEndian(record)
            || saved.Owner != source.OwnerObject || !family.ObjectSlots.Contains(saved.Owner)
            || !saved.RawRecord.SequenceEqual(record) || !saved.RawState.SequenceEqual(state))
            throw new InvalidDataException($"Saved particle baseline does not match closed family source at slot {slot}.");
        return saved;
    }
}
