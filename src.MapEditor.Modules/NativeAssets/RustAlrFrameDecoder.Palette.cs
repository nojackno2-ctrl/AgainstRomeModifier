using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativePaletteKernelArguments(int Weight, uint GainLow, uint GainMiddle,
    uint GainHigh, uint Tint, uint AlphaFlag);

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int PaletteFn([In] uint[] a, nuint aLength, [In] uint[] b, nuint bLength,
        nuint count, int weight, uint low, uint middle, uint high, uint tint, uint alphaFlag,
        [Out] uint[] output, nuint capacity);
    private static PaletteFn? _paletteKernel;

    internal static IReadOnlyList<uint> PrepareRawPalette(IReadOnlyList<uint> a, IReadOnlyList<uint> b,
        int count, NativePaletteKernelArguments args)
    {
        ArgumentNullException.ThrowIfNull(a); ArgumentNullException.ThrowIfNull(b); ArgumentNullException.ThrowIfNull(args);
        if (count is < 1 or > 256) throw new ArgumentOutOfRangeException(nameof(count));
        PaletteFn kernel;
        lock (Gate)
        {
            _ = GetDecoder();
            _paletteKernel ??= Marshal.GetDelegateForFunctionPointer<PaletteFn>(NativeLibrary.GetExport(_library, "ar_particle_palette_kernel"));
            kernel = _paletteKernel;
        }
        var output = new uint[count];
        int code = kernel(a.ToArray(), (nuint)a.Count, b.ToArray(), (nuint)b.Count, (nuint)count,
            args.Weight, args.GainLow, args.GainMiddle, args.GainHigh, args.Tint, args.AlphaFlag, output, (nuint)count);
        if (code != 0) throw new InvalidDataException($"Rust raw palette kernel rejected (code={code}).");
        return Array.AsReadOnly(output);
    }
}
