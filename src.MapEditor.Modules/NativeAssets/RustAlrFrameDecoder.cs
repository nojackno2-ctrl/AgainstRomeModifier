using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>Thin caller-buffer bridge to Rust's ALR scanline decoder; no frame selection.</summary>
internal static partial class RustAlrFrameDecoder
{
    private static readonly object Gate = new();
    private static DecodeFn? _decode;
    private static IntPtr _library;

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate uint VersionFn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int DecodeFn(uint width, uint height, [In] uint[] rows, nuint rowCount,
        [In] byte[] data, nuint dataLength, [In] uint[] palette, nuint paletteLength,
        [In, Out] uint[] output, nuint outputCapacity);

    internal static uint[] Decode(int width, int height, ReadOnlySpan<uint> rows,
        ReadOnlySpan<byte> data, ReadOnlySpan<uint> palette)
    {
        var output = new uint[checked(width * height)];
        int code = GetDecoder()((uint)width, (uint)height, rows.ToArray(), (nuint)rows.Length,
            data.ToArray(), (nuint)data.Length, palette.ToArray(), (nuint)palette.Length,
            output, (nuint)output.Length);
        if (code != 0) throw new InvalidDataException($"Rust ALR scanline decode failed (code={code}).");
        return output;
    }

    private static DecodeFn GetDecoder()
    {
        lock (Gate)
        {
            if (_decode is not null) return _decode;
            string? configured = Environment.GetEnvironmentVariable("ARM_NATIVE_DATA_DLL");
            if (!string.IsNullOrWhiteSpace(configured) && !Path.IsPathFullyQualified(configured))
                throw new InvalidDataException("ARM_NATIVE_DATA_DLL must be an absolute path.");
            string path = string.IsNullOrWhiteSpace(configured)
                ? Path.Combine(AppContext.BaseDirectory, "against_rome_data.dll") : Path.GetFullPath(configured);
            IntPtr library = IntPtr.Zero;
            try
            {
                library = NativeLibrary.Load(path);
                var version = Marshal.GetDelegateForFunctionPointer<VersionFn>(
                    NativeLibrary.GetExport(library, "ar_alr_scanline_abi_version"));
                if (version() != 1) throw new InvalidDataException("Rust ALR scanline ABI version mismatch.");
                _decode = Marshal.GetDelegateForFunctionPointer<DecodeFn>(
                    NativeLibrary.GetExport(library, "ar_alr_decode_argb"));
                _library = library;
                // Successful binding owns one process-lifetime library reference, as do the saved-pool bridges.
                return _decode;
            }
            catch (Exception error) when (error is DllNotFoundException or BadImageFormatException
                or EntryPointNotFoundException or InvalidDataException)
            {
                if (library != IntPtr.Zero) NativeLibrary.Free(library);
                throw new InvalidDataException($"Rust ALR decoder unavailable at {path}: {error.Message}", error);
            }
        }
    }
}
