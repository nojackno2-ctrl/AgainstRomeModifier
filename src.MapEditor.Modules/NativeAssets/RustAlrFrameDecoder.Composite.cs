using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int CompositeFn([In] ushort[] samples, nuint count, [In] uint[] palette,
        nuint paletteLength, [In] uint[] destination, uint kernel, [Out] uint[] output, nuint capacity);
    private static CompositeFn? _composite;

    /// <summary>Explicit raw kernel0 replacement/1 half/2 alpha256; not native mode IDs.
    /// Destination and palette are raw words, with no ARGB conversion.</summary>
    internal static uint[] ComposeSamples(IReadOnlyList<ushort> samples, IReadOnlyList<uint> palette,
        ReadOnlySpan<uint> destination, uint kernel)
    {
        if (samples.Count != destination.Length) throw new ArgumentException("Sample/destination lengths differ.", nameof(destination));
        CompositeFn compose;
        lock (Gate)
        {
            _ = GetDecoder();
            _composite ??= Marshal.GetDelegateForFunctionPointer<CompositeFn>(
                NativeLibrary.GetExport(_library, "ar_particle_sample_pixels"));
            compose = _composite;
        }
        var output = new uint[samples.Count];
        int code = compose(samples.ToArray(), (nuint)samples.Count, palette.ToArray(), (nuint)palette.Count,
            destination.ToArray(), kernel, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust sample compositor rejected (code={code}).");
        return output;
    }
}
