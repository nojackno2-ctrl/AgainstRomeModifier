namespace AgainstRomeMapEditor.NativeAssets;

internal delegate bool NativeParticleLifetimeFrameResolver(int alrId, int columns, int directions,
    uint frameCount, out uint? frame, out string? diagnostic);
internal delegate bool NativeParticleSavedFrameResolver(int alrId, int columns, int directions,
    uint frameCount, out IReadOnlyList<NativeParticleFrameCandidate> candidates, out string? diagnostic);

/// <summary>Uncropped decoded frame; preserves stored offsets and anchor canvas.
/// No direction fallback, palette selection, positioning or blending is inferred.</summary>
internal sealed record NativeParticleAssetFrame(int AlrId, string AssetName, uint FrameIndex,
    uint AnchorWidth, uint AnchorHeight, NativeAlrFrameInfo Info, NativeAlrIndexedFrame Image)
{
    public IReadOnlyList<uint> RawPalette { get; init; } = Array.Empty<uint>();
    public int PaletteVariant { get; init; }
    public IReadOnlyList<ushort> IndexedSamples { get; init; } = Array.Empty<ushort>();
    public object? SourceIdentity { get; init; }
}

internal sealed partial class NativeSpriteCatalog
{
    private readonly Dictionary<(int Alr, uint Frame, int Palette), NativeParticleAssetFrame> _particleAssetFrames = new();
    private bool _particleAssetsDisposed;
    internal bool TryDecodeParticleFrameIndex(int alrId, uint index, int paletteVariant,
        out NativeParticleAssetFrame? frame, out string? diagnostic)
    {
        bool Resolve(int id, int columns, int directions, uint count, out uint? selected, out string? reason)
        { selected = index; reason = null; return true; }
        return TryDecodeParticleLifetimeFrame(alrId, paletteVariant, Resolve, out frame, out diagnostic);
    }

    internal bool TryResolveParticleSavedFrames(int alrId, NativeParticleSavedFrameResolver resolve,
        out IReadOnlyList<NativeParticleFrameCandidate> candidates, out string? diagnostic)
    {
        ArgumentNullException.ThrowIfNull(resolve);
        candidates = Array.Empty<NativeParticleFrameCandidate>();
        lock (_gate)
        {
            if (_particleAssetsDisposed)
            { diagnostic = "Particle asset catalog is disposed."; return false; }
            if (!_alrNames.TryGetValue(alrId, out string? name))
            { diagnostic = "Particle ALR ID is absent from the asset name list."; return false; }
            NativeAlrDocument? document = LoadAlr(name);
            if (document is null)
            { diagnostic = "Particle ALR asset is missing or unsupported."; return false; }
            if (document.LayoutColumns is 0 or > int.MaxValue || document.LayoutRows is 0 or > int.MaxValue)
            { diagnostic = "Particle ALR layout is unsupported without runtime normalization."; return false; }
            return resolve(alrId, (int)document.LayoutColumns, (int)document.LayoutRows,
                (uint)document.Frames.Count, out candidates, out diagnostic);
        }
    }

    internal bool TryDecodeParticleLifetimeFrame(int alrId, int paletteVariant,
        NativeParticleLifetimeFrameResolver resolve, out NativeParticleAssetFrame? frame,
        out string? diagnostic)
    {
        ArgumentNullException.ThrowIfNull(resolve);
        frame = null;
        lock (_gate)
        {
            if (_particleAssetsDisposed)
            { diagnostic = "Particle asset catalog is disposed."; return false; }
            if (!_alrNames.TryGetValue(alrId, out string? name))
            { diagnostic = "Particle ALR ID is absent from the asset name list."; return false; }
            NativeAlrDocument? document = LoadAlr(name);
            if (document is null)
            { diagnostic = "Particle ALR asset is missing or unsupported."; return false; }
            if (document.LayoutColumns is 0 or > int.MaxValue || document.LayoutRows is 0 or > int.MaxValue)
            { diagnostic = "Particle ALR layout is unsupported without runtime normalization."; return false; }
            if ((uint)paletteVariant >= (uint)document.PaletteVariantCount)
            { diagnostic = "Particle palette variant is outside the asset."; return false; }
            if (!resolve(alrId, (int)document.LayoutColumns, (int)document.LayoutRows,
                (uint)document.Frames.Count, out uint? index, out diagnostic)) return false;
            if (index is null || index.Value >= (uint)document.Frames.Count)
            { diagnostic = "Particle resolver returned no valid frame."; return false; }
            try
            {
                var key = (alrId, index.Value, paletteVariant);
                if (_particleAssetFrames.TryGetValue(key, out frame))
                { diagnostic = null; return true; }
                NativeAlrIndexedFrame image = document.DecodeFrame((int)index.Value, paletteVariant);
                frame = new(alrId, name, index.Value, document.AnchorWidth, document.AnchorHeight,
                    document.Frames[(int)index.Value], image)
                { RawPalette = document.ReadRawPalette(paletteVariant), PaletteVariant = paletteVariant,
                    IndexedSamples = document.ReadIndexedSamples((int)index.Value), SourceIdentity = document.IdentityToken };
                _particleAssetFrames.Add(key, frame);
                diagnostic = null;
                return true;
            }
            catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException)
            { diagnostic = $"Particle frame decode failed: {error.Message}"; return false; }
        }
    }
}
