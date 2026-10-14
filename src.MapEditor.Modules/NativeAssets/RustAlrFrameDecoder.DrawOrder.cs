using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int OrderFn([In] float[] depths, nuint count, [Out] uint[] output, nuint capacity);
    private static OrderFn? _drawOrder;

    internal static IReadOnlyList<uint> OrderSceneDraws(ReadOnlySpan<float> depths)
    {
        OrderFn order;
        lock (Gate)
        {
            _ = GetDecoder();
            _drawOrder ??= Marshal.GetDelegateForFunctionPointer<OrderFn>(NativeLibrary.GetExport(_library, "ar_scene_draw_order"));
            order = _drawOrder;
        }
        var output = new uint[depths.Length];
        int code = order(depths.ToArray(), (nuint)depths.Length, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust draw ordering rejected (code={code}).");
        return Array.AsReadOnly(output);
    }
}
