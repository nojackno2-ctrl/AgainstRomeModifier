using System.Collections.ObjectModel;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier.Maps;

[StructLayout(LayoutKind.Sequential)]
internal struct ArParticlePoolSnapshotNative
{
    public uint StructSize, Slot, ActiveRaw;
    public int CountRaw;
    public short Owner;
    public ushort Reserved;
    public int AgeRaw;
}

/// <summary>A detached immutable saved slot. Metadata is supplied by Rust; the two buffers are opaque.</summary>
public sealed class RustParticleSnapshot
{
    private readonly byte[] _record, _state;
    internal RustParticleSnapshot(in ArParticlePoolSnapshotNative metadata, byte[] record, byte[] state)
    {
        Slot = checked((int)metadata.Slot);
        ActiveRaw = metadata.ActiveRaw; CountRaw = metadata.CountRaw;
        Owner = metadata.Owner; AgeRaw = metadata.AgeRaw;
        _record = (byte[])record.Clone(); _state = (byte[])state.Clone();
    }
    public int Slot { get; }
    public uint ActiveRaw { get; }
    public bool IsActive => ActiveRaw != 0;
    public int CountRaw { get; }
    public short Owner { get; }
    public short OwnerObject => Owner;
    public int AgeRaw { get; }
    public ReadOnlySpan<byte> RawRecord => _record;
    public ReadOnlySpan<byte> RawState => _state;
    public byte[] RecordBytes() => (byte[])_record.Clone();
    public byte[] StateBytes() => (byte[])_state.Clone();
    public RustParticleEmitterRecord CopyEmitterRecord() => RustParticleEmitterRecord.Copy(_record);
}

/// <summary>Native rejection, with the original C ABI operation and code.</summary>
public sealed class RustParticlePoolException : IOException
{
    internal RustParticlePoolException(string operation, int code, string message)
        : base($"{operation}: {message} (code={code}).") { Operation = operation; Code = code; }
    public string Operation { get; }
    public int Code { get; }
}

/// <summary>
/// Thin read-only bridge to Rust's immutable particle.dat pool. Uses RustNativeData's absolute DLL resolver
/// and process-lifetime library. No PFIL/LZSS parsing, particle offsets, simulation or render inference.
/// </summary>
public sealed class RustParticlePool : IDisposable
{
    public const uint ExpectedAbiVersion = 1;
    public const int MetadataSize = 24, RecordSize = 2252, StateSize = 16, ChildSize = 35,
        PayloadLength = 2322444, Capacity = 1024, Children = 64, PfilHeaderLength = 64;
    public const int CodeBufferTooSmall = 2, CodeInactiveSlot = 5, CodeBadRenderableCount = 114;
    private static readonly object ApiGate = new();
    private static ParticleApi? _api;
    private static long _opened, _freed;
    private readonly object _gate = new();
    private readonly ParticleApi _native;
    private readonly NativePoolHandle _handle;

    public static IReadOnlyDictionary<string, int> ExpectedFieldOffsets { get; } =
        new ReadOnlyDictionary<string, int>(new Dictionary<string, int>
        {
            ["struct_size"] = 0, ["slot"] = 4, ["active_raw"] = 8, ["count_raw"] = 12,
            ["owner"] = 16, ["reserved"] = 18, ["age_raw"] = 20,
        });
    public static IReadOnlyDictionary<string, int> ManagedFieldOffsets { get; } = MeasureOffsets();
    public static int ManagedMetadataSize => Marshal.SizeOf<ArParticlePoolSnapshotNative>();
    public static long OpenedHandleCount => Interlocked.Read(ref _opened);
    public static long FreedHandleCount => Interlocked.Read(ref _freed);
    public static uint NativeAbiVersion => GetParticleApi().AbiVersion;
    public static (uint Metadata, nuint Record, nuint State, nuint Child, nuint Payload) NativeSizes
    {
        get { var a = GetParticleApi(); return (a.MetadataSize, a.RecordSize, a.StateSize, a.ChildSize, a.PayloadSize); }
    }

    public static bool TryEnsureLoaded(out string? error)
    {
        try { _ = GetParticleApi(); error = null; return true; }
        catch (Exception ex) when (ex is IOException or InvalidOperationException or DllNotFoundException
            or BadImageFormatException or EntryPointNotFoundException or ArgumentException)
        {
            error = $"Rust particle pool ABI1 unavailable: {ex.Message}"; return false;
        }
    }

    internal static ParticleApi GetParticleApi()
    {
        lock (ApiGate) return _api ??= new ParticleApi(RustNativeData.GetApi());
    }

    private static IReadOnlyDictionary<string, int> MeasureOffsets()
    {
        string[] names = ["StructSize", "Slot", "ActiveRaw", "CountRaw", "Owner", "Reserved", "AgeRaw"];
        string[] headers = ["struct_size", "slot", "active_raw", "count_raw", "owner", "reserved", "age_raw"];
        var offsets = new Dictionary<string, int>();
        for (int i = 0; i < names.Length; i++)
            offsets.Add(headers[i], checked((int)Marshal.OffsetOf<ArParticlePoolSnapshotNative>(names[i])));
        return new ReadOnlyDictionary<string, int>(offsets);
    }

    private RustParticlePool(NativePoolHandle handle, ParticleApi native) { _handle = handle; _native = native; }
    public static RustParticlePool Load(string path) => Open(File.ReadAllBytes(path));

    /// <summary>Rust copies the input image; accepts raw payload or PFIL. Never writes to the source.</summary>
    public static RustParticlePool Open(ReadOnlySpan<byte> fileImage)
    {
        var api = GetParticleApi();
        byte[] input = fileImage.ToArray();
        int code = api.Open(input, (nuint)input.Length, out IntPtr raw);
        if (code != 0)
        {
            if (raw != IntPtr.Zero) api.Free(raw);
            throw Fail(api, "ar_particle_pool_open", code);
        }
        if (raw == IntPtr.Zero) throw new IOException("ar_particle_pool_open succeeded with a null handle.");
        var handle = new NativePoolHandle(raw, api);
        Interlocked.Increment(ref _opened);
        return new RustParticlePool(handle, api);
    }

    public bool IsDisposed => _handle.IsClosed;
    public int ActiveCount => WithHandle(h =>
    {
        int code = _native.ActiveCount(h, out uint count);
        Check(code, "ar_particle_pool_active_count");
        if (count > Capacity) throw new IOException($"Rust particle active count exceeds {Capacity}: {count}.");
        return checked((int)count);
    });

    /// <summary>Returns null only for an inactive emitter; native bounds errors throw.</summary>
    public RustParticleSnapshot? Capture(int slot) => WithHandle(h => ReadSnapshot(h, slot, false));
    /// <summary>Includes inactive slots with all saved bytes preserved.</summary>
    public RustParticleSnapshot CaptureAny(int slot) => WithHandle(h => ReadSnapshot(h, slot, true))!;

    private RustParticleSnapshot? ReadSnapshot(IntPtr h, int slot, bool any)
    {
        ValidateSlot(slot);
        var metadata = new ArParticlePoolSnapshotNative { StructSize = MetadataSize };
        byte[] record = new byte[RecordSize], state = new byte[StateSize];
        string operation = any ? "ar_particle_pool_snapshot_any" : "ar_particle_pool_snapshot";
        int code = (any ? _native.SnapshotAny : _native.Snapshot)(h, (uint)slot,
            ref metadata, record, (nuint)record.Length, state, (nuint)state.Length);
        if (!any && code == CodeInactiveSlot) return null;
        Check(code, operation);
        if (metadata.StructSize != MetadataSize || metadata.Slot != (uint)slot || metadata.Reserved != 0
            || metadata.ActiveRaw > ushort.MaxValue || (!any && metadata.ActiveRaw == 0))
            throw new IOException($"{operation}: invalid ABI1 metadata returned for slot {slot}.");
        return new RustParticleSnapshot(metadata, record, state);
    }

    /// <summary>Copies one opaque 35-byte child through Rust, regardless of flags or CountRaw.</summary>
    public byte[] ChildAny(int slot, int ordinal) => WithHandle(h =>
    {
        ValidateSlot(slot);
        if (ordinal is < 0 or >= Children) throw new ArgumentOutOfRangeException(nameof(ordinal));
        byte[] bytes = new byte[ChildSize];
        Check(_native.ChildAny(h, (uint)slot, (uint)ordinal, bytes, (nuint)bytes.Length), "ar_particle_pool_child_any");
        return bytes;
    });

    /// <summary>Optional Rust count diagnostic. Does not establish sprite/frame/render eligibility.</summary>
    public uint ValidateRenderableCount(int slot) => WithHandle(h =>
    {
        ValidateSlot(slot);
        Check(_native.ValidateRenderableCount(h, (uint)slot, out uint count), "ar_particle_pool_validate_renderable_count");
        return count;
    });

    public byte[] CopyPayload() => WithHandle(h => CopyBytes(h, _native.CopyPayload, "ar_particle_pool_copy_payload", PayloadLength, false));
    public byte[] CopyPfilHeader() => WithHandle(h => CopyBytes(h, _native.CopyHeader, "ar_particle_pool_copy_pfil_header", PfilHeaderLength, true));
    public byte[] CopyInputSha256() => WithHandle(h => CopyBytes(h, _native.CopySha256, "ar_particle_pool_copy_input_sha256", 32, false));

    private byte[] CopyBytes(IntPtr h, CopyFn copy, string operation, int expected, bool allowEmpty)
    {
        int code = copy(h, null, 0, out nuint length);
        if (code != 0 && code != CodeBufferTooSmall) throw Fail(_native, operation, code);
        if (length != (nuint)expected && !(allowEmpty && length == 0))
            throw new IOException($"{operation}: invalid required length {length}; expected {expected}{(allowEmpty ? " or 0" : "")}.");
        if (length == 0)
        {
            Check(code, operation);
            return [];
        }
        if (code != CodeBufferTooSmall) throw new IOException($"{operation}: null-buffer size query unexpectedly succeeded.");
        byte[] bytes = new byte[checked((int)length)];
        Check(copy(h, bytes, (nuint)bytes.Length, out nuint written), operation);
        if (written != length) throw new IOException($"{operation}: required length changed from {length} to {written}.");
        return bytes;
    }

    private static void ValidateSlot(int slot)
    {
        if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slot));
    }
    private void Check(int code, string operation) { if (code != 0) throw Fail(_native, operation, code); }
    private static RustParticlePoolException Fail(ParticleApi api, string operation, int code)
        => new(operation, code, Marshal.PtrToStringAnsi(api.Message(code)) ?? "unknown native error");

    private T WithHandle<T>(Func<IntPtr, T> call)
    {
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            bool added = false;
            try
            {
                _handle.DangerousAddRef(ref added);
                return call(_handle.DangerousGetHandle());
            }
            finally { if (added) _handle.DangerousRelease(); }
        }
    }
    public void Dispose()
    {
        lock (_gate) _handle.Dispose();
        GC.SuppressFinalize(this);
    }

    private sealed class NativePoolHandle : SafeHandle
    {
        private readonly ParticleApi _api;
        internal NativePoolHandle(IntPtr value, ParticleApi api) : base(IntPtr.Zero, true) { _api = api; SetHandle(value); }
        public override bool IsInvalid => handle == IntPtr.Zero;
        protected override bool ReleaseHandle()
        {
            _api.Free(handle);
            Interlocked.Increment(ref _freed);
            return true;
        }
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate uint U32Fn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate nuint SizeFn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate IntPtr MessageFn(int code);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int OpenFn([In] byte[] input, nuint length, out IntPtr pool);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate void FreeFn(IntPtr pool);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int CountFn(IntPtr pool, out uint count);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int SnapshotFn(IntPtr pool, uint slot,
        [In, Out] ref ArParticlePoolSnapshotNative metadata, [In, Out] byte[] record, nuint recordCap, [In, Out] byte[] state, nuint stateCap);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int ChildFn(IntPtr pool, uint slot, uint ordinal, [In, Out] byte[] bytes, nuint cap);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int ValidateCountFn(IntPtr pool, uint slot, out uint count);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int CopyFn(IntPtr pool, [In, Out] byte[]? bytes, nuint cap, out nuint length);

    internal sealed class ParticleApi
    {
        internal uint AbiVersion { get; }
        internal uint MetadataSize { get; }
        internal nuint RecordSize { get; }
        internal nuint StateSize { get; }
        internal nuint ChildSize { get; }
        internal nuint PayloadSize { get; }
        internal OpenFn Open { get; }
        internal FreeFn Free { get; }
        internal MessageFn Message { get; }
        internal CountFn ActiveCount { get; }
        internal SnapshotFn Snapshot { get; }
        internal SnapshotFn SnapshotAny { get; }
        internal ChildFn ChildAny { get; }
        internal ValidateCountFn ValidateRenderableCount { get; }
        internal CopyFn CopyPayload { get; }
        internal CopyFn CopyHeader { get; }
        internal CopyFn CopySha256 { get; }
        internal ParticleApi(RustNativeData.Api api)
        {
            AbiVersion = api.BindExport<U32Fn>("ar_particle_pool_abi_version")();
            MetadataSize = api.BindExport<U32Fn>("ar_particle_pool_snapshot_struct_size")();
            RecordSize = api.BindExport<SizeFn>("ar_particle_pool_record_size")();
            StateSize = api.BindExport<SizeFn>("ar_particle_pool_state_size")();
            ChildSize = api.BindExport<SizeFn>("ar_particle_pool_child_size")();
            PayloadSize = api.BindExport<SizeFn>("ar_particle_pool_payload_size")();
            if (AbiVersion != ExpectedAbiVersion || MetadataSize != RustParticlePool.MetadataSize
                || ManagedMetadataSize != RustParticlePool.MetadataSize || RecordSize != RustParticlePool.RecordSize
                || StateSize != RustParticlePool.StateSize || ChildSize != RustParticlePool.ChildSize || PayloadSize != PayloadLength)
                throw new InvalidOperationException($"Rust particle ABI mismatch: version={AbiVersion} (expected 1); sizes="
                    + $"{MetadataSize}/{RecordSize}/{StateSize}/{ChildSize}/{PayloadSize} (expected 24/2252/16/35/2322444); managed={ManagedMetadataSize}.");
            foreach (var offset in ExpectedFieldOffsets)
                if (ManagedFieldOffsets[offset.Key] != offset.Value)
                    throw new InvalidOperationException($"ArParticlePoolSnapshot.{offset.Key} offset mismatch.");
            Message = api.BindExport<MessageFn>("ar_particle_pool_error_message");
            Open = api.BindExport<OpenFn>("ar_particle_pool_open");
            Free = api.BindExport<FreeFn>("ar_particle_pool_free");
            ActiveCount = api.BindExport<CountFn>("ar_particle_pool_active_count");
            Snapshot = api.BindExport<SnapshotFn>("ar_particle_pool_snapshot");
            SnapshotAny = api.BindExport<SnapshotFn>("ar_particle_pool_snapshot_any");
            ChildAny = api.BindExport<ChildFn>("ar_particle_pool_child_any");
            ValidateRenderableCount = api.BindExport<ValidateCountFn>("ar_particle_pool_validate_renderable_count");
            CopyPayload = api.BindExport<CopyFn>("ar_particle_pool_copy_payload");
            CopyHeader = api.BindExport<CopyFn>("ar_particle_pool_copy_pfil_header");
            CopySha256 = api.BindExport<CopyFn>("ar_particle_pool_copy_input_sha256");
        }
    }
}
