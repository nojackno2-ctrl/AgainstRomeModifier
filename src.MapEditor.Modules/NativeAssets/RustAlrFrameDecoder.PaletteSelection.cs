using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativePaletteSelection(bool IsEndpoint, int FirstVariant, int SecondVariant, int Weight);

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int SelectionFn(int variants, int first, int second, int weight, [Out] int[] output);
    private static SelectionFn? _paletteSelection;

    internal static NativePaletteSelection SelectPalette(int variants, int first, int second, int weight)
    {
        SelectionFn select;
        lock (Gate)
        {
            _ = GetDecoder();
            _paletteSelection ??= Marshal.GetDelegateForFunctionPointer<SelectionFn>(
                NativeLibrary.GetExport(_library, "ar_particle_palette_selection"));
            select = _paletteSelection;
        }
        var output = new int[4];
        int code = select(variants, first, second, weight, output);
        if (code != 0) throw new InvalidDataException($"Rust palette selection rejected (code={code}).");
        return new(output[0] == 0, output[1], output[2], output[3]);
    }
}
