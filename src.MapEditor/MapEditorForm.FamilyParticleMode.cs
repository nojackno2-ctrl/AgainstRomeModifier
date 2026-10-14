using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    internal bool TryResolveSavedFamilyParticleMode(int rootSlot, uint rootUid, int ownerSlot,
        out NativeSavedObjectMode? mode, out string? diagnostic)
    {
        mode = null;
        try
        {
            var owner = _levelObjects.SingleOrDefault(item => item.Slot == ownerSlot);
            if (owner is null) { diagnostic = "Particle mode owner is unavailable."; return false; }
            if (!TryResolveParticleDefinition(owner.TypeId, out _, out diagnostic)) return false;
            // An unavailable type mode is only required when the saved gfx mode is99.
            _particleDefinitions!.TryResolvePrimaryMode(owner.TypeId, out var primary, out diagnostic);
            return TryResolveSavedFamilyParticleMode(rootSlot, rootUid, ownerSlot, primary, out mode, out diagnostic);
        }
        catch (Exception error) when (IsSavedLightPreviewFailure(error))
        { diagnostic = error.Message; return false; }
    }
    internal bool TryResolveSavedFamilyParticleMode(int rootSlot, uint rootUid, int ownerSlot,
        int? verifiedPrimaryMode, out NativeSavedObjectMode? mode, out string? diagnostic)
    {
        mode = null; diagnostic = null;
        try
        {
            if (_selected is null || !IsSavedParticleMap(_selected.DirectoryPath) || _familyPreviewStore is null
                || _savedParticleLoadDiagnostic is not null)
            { diagnostic = "Saved particle mode baseline is unavailable for this map."; return false; }
            var family = _familyPreviewStore.CaptureFamily(rootSlot, rootUid);
            if (family is null || !family.Objects.TryGetValue(ownerSlot, out var source)
                || !family.Particles.Values.Any(particle => particle.OwnerObject == ownerSlot))
            { diagnostic = "Particle mode owner is outside the closed family."; return false; }
            var owner = _levelObjects.SingleOrDefault(item => item.Slot == ownerSlot);
            if (owner is null || owner.TypeId != source.TypeId)
            { diagnostic = "Particle mode owner identity is stale."; return false; }
            if (!TryResolveParticleDefinition(owner.TypeId, out _, out diagnostic)) return false;
            var gfx = _familyPreviewStore.CopyGfxTypePayload();
            if (gfx is null)
            { diagnostic = "Saved gfxtype mode payload is unavailable."; return false; }
            return _particleDefinitions!.TryResolveSavedObjectMode(source.Record, gfx, owner.Uid,
                verifiedPrimaryMode, out mode, out diagnostic);
        }
        catch (Exception error) when (IsSavedLightPreviewFailure(error))
        { diagnostic = error.Message; return false; }
    }
}
