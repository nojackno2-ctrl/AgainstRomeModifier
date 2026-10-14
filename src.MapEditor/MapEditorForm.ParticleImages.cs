using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor;

internal sealed record SavedParticlePreviewImage(SavedFamilyParticleFrame Candidate,
    NativeParticleAssetFrame? Image, string? Diagnostic)
{
    public NativeSavedObjectMode? Mode { get; init; }
    public string? ModeDiagnostic { get; init; }
    public NativeParticleSpriteRect? Rect { get; init; }
    public string? GeometryDiagnostic { get; init; }
}

internal sealed partial class MapEditorForm
{
    internal ParticleSpriteBatch CaptureSavedParticleRenderBatch(int paletteVariant,
        TerrainHeightField heights, System.Numerics.Matrix4x4 view)
        => SceneParticleRenderer.Build(DecodeSavedParticlePreviewImages(paletteVariant), heights, view);

    /// <summary>Explicit palette selection. Refresh candidates at the same scene boundary
    /// before decoding, retaining unavailable branches as diagnostics without images.</summary>
    internal IReadOnlyList<SavedParticlePreviewImage> DecodeSavedParticlePreviewImages(int paletteVariant)
    {
        _ = SceneObjectsFor3D(EffectiveSceneObjects());
        var result = new List<SavedParticlePreviewImage>();
        var roots = _levelObjects.ToDictionary(owner => owner.Slot);
        var modes = new Dictionary<(int Root, int Owner), (NativeSavedObjectMode? Mode, string? Reason)>();
        // Detached definitions are shared only within this scene decode. A later
        // invocation must revalidate the current map and adopted definition catalog.
        var definitions = new Dictionary<int, (NativeParticleRenderDefinition? Definition, string? Reason)>();
        foreach (var candidate in _savedFamilyParticleFrames)
        {
            var key = (candidate.RootSlot, candidate.OwnerSlot);
            if (!modes.TryGetValue(key, out var context))
            {
                NativeSavedObjectMode? mode = null; string? reason = "Particle mode root is unavailable.";
                if (roots.TryGetValue(candidate.RootSlot, out var root))
                    TryResolveSavedFamilyParticleMode(root.Slot, root.Uid, candidate.OwnerSlot, out mode, out reason);
                if (mode is not null && mode.LeafAddress is null) reason = "Particle native draw leaf is unknown.";
                context = (mode, reason); modes.Add(key, context);
            }
            void Add(NativeParticleAssetFrame? image, string? diagnostic)
            {
                NativeParticleSpriteRect? rect = null; string? geometryReason = null;
                if (image is not null && _particleDefinitions is not null)
                    _particleDefinitions.TryResolveSpriteRect(image, out rect, out geometryReason);
                result.Add(new(candidate, image, diagnostic) { Mode = context.Mode, ModeDiagnostic = context.Reason,
                    Rect = rect, GeometryDiagnostic = geometryReason });
            }
            if (candidate.Child.Status != 0 || candidate.Child.Frame is null)
            {
                Add(null, $"Particle frame unavailable (code={candidate.Child.Status}).");
                continue;
            }
            if (!definitions.TryGetValue(candidate.OwnerType, out var resolved))
            {
                bool available = TryResolveParticleDefinition(candidate.OwnerType, out var definition, out var reason);
                resolved = (available ? definition : null, reason);
                definitions.Add(candidate.OwnerType, resolved);
            }
            if (resolved.Definition is null)
            { Add(null, resolved.Reason); continue; }
            if (_spriteCatalog is null)
            { Add(null, "Particle asset catalog is unavailable."); continue; }
            _spriteCatalog.TryDecodeParticleFrameIndex(resolved.Definition.AlrId, candidate.Child.Frame.Value,
                paletteVariant, out var image, out var diagnostic);
            Add(image, diagnostic);
        }
        return result.AsReadOnly();
    }
}
