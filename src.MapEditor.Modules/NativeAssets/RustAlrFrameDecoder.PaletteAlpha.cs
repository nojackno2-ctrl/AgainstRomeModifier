using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int AlphaLanesFn([In] uint[] source, nuint count, [Out] uint[] output, nuint capacity);
    private static AlphaLanesFn? _alphaLanes;

    /// <summary>Explicit independent DWORD operation; excludes native buffer alignment/pointer effects.</summary>
    internal static IReadOnlyList<uint> PreparePaletteAlphaLanes(IReadOnlyList<uint> source)
    {
        ArgumentNullException.ThrowIfNull(source);
        AlphaLanesFn prepare;
        lock (Gate)
        {
            _ = GetDecoder();
            _alphaLanes ??= Marshal.GetDelegateForFunctionPointer<AlphaLanesFn>(
                NativeLibrary.GetExport(_library, "ar_particle_palette_alpha_lanes"));
            prepare = _alphaLanes;
        }
        var output = new uint[source.Count];
        int code = prepare(source.ToArray(), (nuint)source.Count, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust alpha lanes rejected (code={code}).");
        return Array.AsReadOnly(output);
    }
}
