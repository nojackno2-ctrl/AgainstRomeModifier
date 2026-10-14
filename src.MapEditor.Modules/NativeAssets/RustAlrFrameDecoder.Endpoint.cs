using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal static partial class RustAlrFrameDecoder
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int EndpointFn([In] uint[] source, nuint count, uint key, uint low, uint middle,
        uint high, uint tint, uint alpha, uint mod8, [Out] uint[] output, nuint capacity);
    private static EndpointFn? _endpointDefault;

    internal static IReadOnlyList<uint> PrepareEndpointDefault(IReadOnlyList<uint> source, uint key,
        uint low, uint middle, uint high, uint tint, uint alpha, uint? verifiedBufferMod8)
    {
        ArgumentNullException.ThrowIfNull(source);
        if (verifiedBufferMod8 is > 7) throw new ArgumentOutOfRangeException(nameof(verifiedBufferMod8));
        EndpointFn prepare;
        lock (Gate)
        {
            _ = GetDecoder();
            _endpointDefault ??= Marshal.GetDelegateForFunctionPointer<EndpointFn>(
                NativeLibrary.GetExport(_library, "ar_particle_primary_endpoint_default"));
            prepare = _endpointDefault;
        }
        var output = new uint[source.Count];
        int code = prepare(source.ToArray(), (nuint)source.Count, key, low, middle, high, tint, alpha,
            verifiedBufferMod8 ?? 8, output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust endpoint default rejected (code={code}).");
        return Array.AsReadOnly(output);
    }
}
