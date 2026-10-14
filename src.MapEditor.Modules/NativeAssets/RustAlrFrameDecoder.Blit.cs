using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    /// <summary>Catalog-bound palette adoption. Explicit raw-buffer overloads
    /// remain available for callers that own their own provenance contract.</summary>
    internal static uint[] BlitPreparedSamples(NativeParticleAssetFrame frame, NativePreparedParticlePalette prepared,
        ReadOnlySpan<uint> destination, int width, int height, int x, int y, uint kernel)
    {
        ArgumentNullException.ThrowIfNull(frame); ArgumentNullException.ThrowIfNull(prepared);
        if (prepared.SourceAlrId != frame.AlrId || prepared.SourceIdentity is null
            || !ReferenceEquals(prepared.SourceIdentity, frame.SourceIdentity)
            || !string.Equals(prepared.SourceAssetName, frame.AssetName, StringComparison.Ordinal))
            throw new InvalidDataException("Prepared palette does not match the owned particle frame asset.");
        return BlitSamples(frame, prepared.Words, destination, width, height, x, y, kernel);
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int BlitFn([In] ushort[] samples, nuint count, uint sw, uint sh,
        [In] uint[] palette, nuint paletteLength, [In] uint[] destination, nuint destinationCount,
        uint dw, uint dh, int x, int y, uint kernel, [Out] uint[] output, nuint capacity);
    private static BlitFn? _blit;

    internal static uint[] BlitSamples(NativeParticleAssetFrame frame, ReadOnlySpan<uint> destination,
        int width, int height, int x, int y, uint kernel)
        => BlitSamples(frame, frame.RawPalette, destination, width, height, x, y, kernel);

    /// <summary>Caller-selected prepared palette; keeps the source frame palette intact.
    /// Kernel IDs are explicit pixel operations, not native mode dispatch.</summary>
    internal static uint[] BlitSamples(NativeParticleAssetFrame frame, IReadOnlyList<uint> preparedPalette,
        ReadOnlySpan<uint> destination, int width, int height, int x, int y, uint kernel)
    {
        ArgumentNullException.ThrowIfNull(frame); ArgumentNullException.ThrowIfNull(preparedPalette);
        BlitFn blit;
        lock (Gate)
        {
            _ = GetDecoder();
            _blit ??= Marshal.GetDelegateForFunctionPointer<BlitFn>(NativeLibrary.GetExport(_library, "ar_particle_sample_blit"));
            blit = _blit;
        }
        var output = new uint[destination.Length];
        int code = blit(frame.IndexedSamples.ToArray(), (nuint)frame.IndexedSamples.Count,
            (uint)frame.Info.Width, (uint)frame.Info.Height, preparedPalette.ToArray(), (nuint)preparedPalette.Count,
            destination.ToArray(), (nuint)destination.Length, (uint)width, (uint)height, x, y, kernel, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust sample blit rejected (code={code}).");
        return output;
    }
}
