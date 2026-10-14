namespace AgainstRomeMapEditor.NativeAssets;

internal sealed partial class NativeSpriteCatalog
{
    internal bool TryPrepareParticlePalette4Ca340Default(int alrId, int first, int second, int weight,
        uint key, uint low, uint middle, uint high, uint tint, int interpolationCount,
        out NativePreparedParticlePalette? prepared, out string? diagnostic)
    {
        prepared = null;
        lock (_gate)
        {
            if (_particleAssetsDisposed)
            { diagnostic = "Particle asset catalog is disposed."; return false; }
            if (!_alrNames.TryGetValue(alrId, out string? name))
            { diagnostic = "Particle ALR ID is absent from the asset name list."; return false; }
            NativeAlrDocument? document = LoadAlr(name);
            if (document is null)
            { diagnostic = "Particle ALR asset is missing or unsupported."; return false; }
            try
            {
                prepared = NativeParticlePalettePreparation.Prepare4Ca340Default(document,
                    first, second, weight, key, low, middle, high, tint, interpolationCount)
                    with { SourceAlrId = alrId, SourceAssetName = name, SourceIdentity = document.IdentityToken };
                diagnostic = null; return true;
            }
            catch (Exception error) when (error is InvalidDataException or ArgumentException or NotSupportedException)
            { diagnostic = $"Particle palette preparation unavailable: {error.Message}"; return false; }
        }
    }
}
