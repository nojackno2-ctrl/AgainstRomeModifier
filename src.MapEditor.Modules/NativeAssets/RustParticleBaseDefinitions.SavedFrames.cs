using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeParticleFrameCandidate(uint Ordinal, int X, int Y, int Z,
    int VelocityX, int VelocityY, int VelocityZ, int LifetimeRaw, uint? Frame, int Status);

internal sealed partial class RustParticleBaseDefinitions
{
    [StructLayout(LayoutKind.Sequential)]
    private struct NativeCandidate
    {
        public uint Ordinal;
        public int X, Y, Z, VelocityX, VelocityY, VelocityZ, LifetimeRaw;
        public uint Frame;
        public int Status;
    }
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int BatchFn(IntPtr handle, int ownerType, uint slot,
        [In] byte[] record, nuint recordLength, [In] byte[] state, nuint stateLength,
        int alrId, int columns, int directions, uint frameCount,
        int dx, int dy, int dz,
        [Out] NativeCandidate[] output, nuint capacity, out uint count);
    private BatchFn? _batch;

    internal bool TryResolveSavedFrames(int ownerType, uint slot, ReadOnlySpan<byte> record,
        ReadOnlySpan<byte> state, int alrId, int columns, int directions, uint frameCount,
        out IReadOnlyList<NativeParticleFrameCandidate> candidates, out string? diagnostic,
        int dx = 0, int dy = 0, int dz = 0)
    {
        candidates = Array.Empty<NativeParticleFrameCandidate>(); diagnostic = null;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            if (_batch is null)
            {
                var size = Marshal.GetDelegateForFunctionPointer<SizeFn>(NativeLibrary.GetExport(_library,
                    "ar_particle_frame_candidate_struct_size"));
                if (size() != 40 || Marshal.SizeOf<NativeCandidate>() != 40)
                    throw new InvalidDataException("Rust particle candidate ABI size mismatch.");
                _batch = Marshal.GetDelegateForFunctionPointer<BatchFn>(NativeLibrary.GetExport(_library,
                    "ar_particle_definition_saved_frames_translated"));
            }
            byte[] recordCopy = record.ToArray(), stateCopy = state.ToArray();
            var output = new NativeCandidate[64];
            int code = _batch(_handle.DangerousGetHandle(), ownerType, slot, recordCopy, (nuint)recordCopy.Length,
                stateCopy, (nuint)stateCopy.Length, alrId, columns, directions, frameCount, dx, dy, dz, output, 64, out uint count);
            if (code != 0)
            { diagnostic = $"Rust saved particle frame query rejected (code={code})."; return false; }
            if (count > output.Length) throw new InvalidDataException("Rust particle candidate count exceeds capacity.");
            candidates = Array.AsReadOnly(output.Take((int)count).Select(c => new NativeParticleFrameCandidate(
                c.Ordinal, c.X, c.Y, c.Z, c.VelocityX, c.VelocityY, c.VelocityZ, c.LifetimeRaw,
                c.Status == 0 ? c.Frame : null, c.Status)).ToArray());
            return true;
        }
    }
}
