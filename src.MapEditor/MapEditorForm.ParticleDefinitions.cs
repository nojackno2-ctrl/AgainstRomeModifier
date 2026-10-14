using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private RustParticleBaseDefinitions? _particleDefinitions;
    private string? _particleDefinitionsMap;
    private string? _particleDefinitionDiagnostic;
    internal string? ParticleDefinitionDiagnostic => _particleDefinitionDiagnostic;

    private static string ParticleDefinitionMapKey(string map)
        => Path.TrimEndingDirectorySeparator(Path.GetFullPath(map));

    private void ClearParticleDefinitions()
    {
        _savedFamilyParticleFrames = Array.Empty<SavedFamilyParticleFrame>();
        SavedFamilyParticleFrameDiagnostic = null;
        _particleDefinitions?.Dispose();
        _particleDefinitions = null;
        _particleDefinitionsMap = _particleDefinitionDiagnostic = null;
    }

    private void LoadParticleDefinitionsForMap(string map)
    {
        string key = ParticleDefinitionMapKey(map);
        if (!string.Equals(key, _particleDefinitionsMap, StringComparison.OrdinalIgnoreCase)) ClearParticleDefinitions();
        RustParticleBaseDefinitions? candidate = null;
        try
        {
            candidate = RustParticleBaseDefinitions.LoadGameSources(_gamePath);
            _particleDefinitions?.Dispose();
            _particleDefinitions = candidate;
            candidate = null;
            _particleDefinitionsMap = key;
            _particleDefinitionDiagnostic = null;
        }
        catch (Exception error) when (error is IOException or InvalidDataException or ArgumentException or InvalidOperationException
            or DllNotFoundException or BadImageFormatException or EntryPointNotFoundException or UnauthorizedAccessException)
        {
            _particleDefinitionDiagnostic = $"Particle definitions unavailable: {error.Message}";
        }
        finally { candidate?.Dispose(); }
    }

    internal bool TryResolveParticleDefinition(int ownerType, out NativeParticleRenderDefinition? definition, out string? diagnostic)
    {
        definition = null;
        diagnostic = _particleDefinitionDiagnostic;
        if (diagnostic is not null) return false;
        if (_particleDefinitions is null || _selected is null
            || !string.Equals(_particleDefinitionsMap, ParticleDefinitionMapKey(_selected.DirectoryPath), StringComparison.OrdinalIgnoreCase))
        { diagnostic = "Particle definitions are not loaded for the current map."; return false; }
        return _particleDefinitions.TryResolve(ownerType, out definition, out diagnostic);
    }

    internal bool TryResolveParticleLifetimeFrame(int ownerType, int childLifetime, int alrId,
        int columns, int directions, uint frameCount, out uint? frame, out string? diagnostic)
    {
        frame = null;
        if (!TryResolveParticleDefinition(ownerType, out _, out diagnostic)) return false;
        return _particleDefinitions!.TryResolveLifetimeFrame(ownerType, childLifetime, alrId, columns,
            directions, frameCount, out frame, out diagnostic);
    }

    internal bool TryDecodeParticleLifetimeFrame(int ownerType, int childLifetime, int paletteVariant,
        out NativeParticleAssetFrame? frame, out string? diagnostic)
    {
        frame = null;
        if (!TryResolveParticleDefinition(ownerType, out var definition, out diagnostic)) return false;
        if (_spriteCatalog is null)
        { diagnostic = "Particle asset catalog is unavailable."; return false; }
        bool Resolve(int alrId, int columns, int directions, uint count, out uint? index, out string? reason)
            => TryResolveParticleLifetimeFrame(ownerType, childLifetime, alrId, columns, directions,
                count, out index, out reason);
        return _spriteCatalog.TryDecodeParticleLifetimeFrame(definition!.AlrId, paletteVariant,
            Resolve, out frame, out diagnostic);
    }

    internal bool TryResolveSavedParticleFrameCandidates(int ownerType, uint slot, byte[] record, byte[] state,
        out IReadOnlyList<NativeParticleFrameCandidate> candidates, out string? diagnostic,
        int dx = 0, int dy = 0, int dz = 0)
    {
        candidates = Array.Empty<NativeParticleFrameCandidate>();
        if (!TryResolveParticleDefinition(ownerType, out var definition, out diagnostic)) return false;
        if (_spriteCatalog is null)
        { diagnostic = "Particle asset catalog is unavailable."; return false; }
        bool Resolve(int id, int columns, int directions, uint count,
            out IReadOnlyList<NativeParticleFrameCandidate> children, out string? reason)
            => _particleDefinitions!.TryResolveSavedFrames(ownerType, slot, record, state, id, columns,
                directions, count, out children, out reason, dx, dy, dz);
        return _spriteCatalog.TryResolveParticleSavedFrames(definition!.AlrId, Resolve, out candidates, out diagnostic);
    }
}
