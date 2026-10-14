namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativePreparedParticlePalette(NativePaletteSelection Selection, IReadOnlyList<uint> Words)
{
    public int? SourceAlrId { get; init; }
    public string? SourceAssetName { get; init; }
    public object? SourceIdentity { get; init; }
}

internal static class NativeParticlePalettePreparation
{
    internal static NativePreparedParticlePalette Prepare4Ca340Default(NativeAlrDocument document,
        int first, int second, int weight, uint key, uint low, uint middle, uint high, uint tint,
        int interpolationCount)
    {
        ArgumentNullException.ThrowIfNull(document);
        return Prepare4Ca340Default(new AlrVariants(document), first, second, weight,
            key, low, middle, high, tint, interpolationCount);
    }

    // Read only selected variants, not every serialized palette in an asset.
    private sealed class AlrVariants(NativeAlrDocument document) : IReadOnlyList<IReadOnlyList<uint>>
    {
        public int Count => document.PaletteVariantCount;
        public IReadOnlyList<uint> this[int index] => document.ReadRawPalette(index);
        public IEnumerator<IReadOnlyList<uint>> GetEnumerator()
        {
            for (int index = 0; index < Count; index++) yield return this[index];
        }
        System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => GetEnumerator();
    }

    /// <summary>Explicit4CA340 disk-default profile. Caller supplies verified raw
    /// variant buffers/count/key/gains; no map state or live pointer is inferred.</summary>
    internal static NativePreparedParticlePalette Prepare4Ca340Default(
        IReadOnlyList<IReadOnlyList<uint>> variants, int first, int second, int weight,
        uint key, uint low, uint middle, uint high, uint tint, int interpolationCount)
    {
        ArgumentNullException.ThrowIfNull(variants);
        var selection = RustAlrFrameDecoder.SelectPalette(variants.Count, first, second, weight);
        var a = variants[selection.FirstVariant];
        ArgumentNullException.ThrowIfNull(a);
        IReadOnlyList<uint> words;
        if (selection.IsEndpoint)
        {
            //4CA340 passes count256 to40E4D0, unlike its40E5C0 count argument.
            if (a.Count < 256) throw new InvalidDataException("Native endpoint requires256 verified palette words.");
            words = RustAlrFrameDecoder.PrepareEndpointDefault(Array.AsReadOnly(a.Take(256).ToArray()),
                key, low, middle, high, tint, 0, null);
        }
        else
        {
            var b = variants[selection.SecondVariant];
            ArgumentNullException.ThrowIfNull(b);
            words = RustAlrFrameDecoder.PrepareRawPalette(a, b, interpolationCount,
                new(selection.Weight, low, middle, high, tint, 0));
        }
        return new(selection, words);
    }
}
