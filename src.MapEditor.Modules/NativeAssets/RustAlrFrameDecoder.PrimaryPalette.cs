using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int PrimaryPaletteFn([In] uint[] source, nuint count, uint packedGain,
        [Out] uint[] output, nuint capacity);
    private static PrimaryPaletteFn? _primaryPaletteDefault;

    /// <summary>Explicit disk-default40fd50 operation; live endpoint pointer is not inferred.</summary>
    internal static IReadOnlyList<uint> PreparePrimaryPaletteDefault(IReadOnlyList<uint> source, uint packedGain)
    {
        ArgumentNullException.ThrowIfNull(source);
        PrimaryPaletteFn prepare;
        lock (Gate)
        {
            _ = GetDecoder();
            _primaryPaletteDefault ??= Marshal.GetDelegateForFunctionPointer<PrimaryPaletteFn>(
                NativeLibrary.GetExport(_library, "ar_particle_primary_palette_default"));
            prepare = _primaryPaletteDefault;
        }
        var output = new uint[source.Count];
        int code = prepare(source.ToArray(), (nuint)source.Count, packedGain, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust primary default palette rejected (code={code}).");
        return Array.AsReadOnly(output);
    }
}
