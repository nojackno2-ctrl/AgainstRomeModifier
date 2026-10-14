using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int IndexedFn(uint width, uint height, [In] uint[] rows, nuint rowCount,
        [In] byte[] data, nuint dataLength, nuint paletteLength, [Out] ushort[] output, nuint capacity);
    private static IndexedFn? _indexed;

    internal static IReadOnlyList<ushort> DecodeIndexed(int width, int height, uint[] rows,
        ReadOnlySpan<byte> data, int paletteLength)
    {
        IndexedFn decode;
        lock (Gate)
        {
            _ = GetDecoder();
            _indexed ??= Marshal.GetDelegateForFunctionPointer<IndexedFn>(
                NativeLibrary.GetExport(_library, "ar_alr_decode_indexed"));
            decode = _indexed;
        }
        var output = new ushort[checked(width * height)];
        int code = decode((uint)width, (uint)height, rows, (nuint)rows.Length,
            data.ToArray(), (nuint)data.Length, (nuint)paletteLength, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust indexed ALR decode failed (code={code}).");
        return Array.AsReadOnly(output);
    }
}
