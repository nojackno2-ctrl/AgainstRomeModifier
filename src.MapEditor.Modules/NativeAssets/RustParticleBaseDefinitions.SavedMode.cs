using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeSavedObjectMode(uint ObjectUid, uint GfxSlot, int RawMode,
    int ResolvedMode, uint? LeafAddress);

internal sealed partial class RustParticleBaseDefinitions
{
    [StructLayout(LayoutKind.Sequential)]
    private struct NativeMode
    {
        public uint StructSize, ObjectUid, GfxSlot;
        public int RawMode, ResolvedMode;
        public uint LeafAddress;
    }
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int ModeFn([In] byte[] record, nuint recordLength, [In] byte[] gfx,
        nuint gfxLength, uint expectedUid, int primaryMode, uint hasPrimary, ref NativeMode output);
    private ModeFn? _savedMode;
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int PrimaryModeFn(IntPtr handle, int ownerType, ref int output);
    private PrimaryModeFn? _primaryMode;

    internal bool TryResolvePrimaryMode(int ownerType, out int? mode, out string? diagnostic)
    {
        mode = null; diagnostic = null;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            _primaryMode ??= Marshal.GetDelegateForFunctionPointer<PrimaryModeFn>(NativeLibrary.GetExport(_library,
                "ar_particle_definition_primary_mode"));
            int result = int.MaxValue;
            int code = _primaryMode(_handle.DangerousGetHandle(), ownerType, ref result);
            if (code == 0) { mode = result; return true; }
            diagnostic = code switch
            {
                224 => "Particle primary definition chain is unavailable.",
                225 => "Particle grafik declares no primary object.",
                249 => "Particle primary type mode is not established.",
                _ => throw new InvalidDataException($"Rust primary type mode rejected (code={code}).")
            };
            return false;
        }
    }

    internal bool TryResolveSavedObjectMode(byte[] record, byte[] decodedGfxPayload,
        uint expectedUid, int? primaryMode, out NativeSavedObjectMode? mode, out string? diagnostic)
    {
        ArgumentNullException.ThrowIfNull(record); ArgumentNullException.ThrowIfNull(decodedGfxPayload);
        mode = null; diagnostic = null;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            if (_savedMode is null)
            {
                var size = Marshal.GetDelegateForFunctionPointer<SizeFn>(NativeLibrary.GetExport(_library,
                    "ar_particle_saved_object_mode_struct_size"));
                if (size() != 24 || Marshal.SizeOf<NativeMode>() != 24)
                    throw new InvalidDataException("Rust saved object mode ABI size mismatch.");
                _savedMode = Marshal.GetDelegateForFunctionPointer<ModeFn>(NativeLibrary.GetExport(_library,
                    "ar_particle_saved_object_mode"));
            }
            var output = new NativeMode { StructSize = 24 };
            int code = _savedMode(record, (nuint)record.Length, decodedGfxPayload, (nuint)decodedGfxPayload.Length,
                expectedUid, primaryMode ?? 0, primaryMode.HasValue ? 1u : 0u, ref output);
            if (code == 0)
            {
                mode = new(output.ObjectUid, output.GfxSlot, output.RawMode, output.ResolvedMode,
                    output.LeafAddress == 0 ? null : output.LeafAddress);
                return true;
            }
            diagnostic = code switch
            {
                249 => "Particle mode requires verified primary grafik type context.",
                250 => "Particle mode context is outside the supported signed16 range.",
                251 => "Saved particle owner UID does not match.",
                252 => "Saved object or gfxtype mode data is unresolved.",
                _ => throw new InvalidDataException($"Rust saved object mode rejected (code={code}).")
            };
            return false;
        }
    }
}
