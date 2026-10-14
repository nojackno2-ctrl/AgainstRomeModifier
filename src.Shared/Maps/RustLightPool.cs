using System.Buffers.Binary;
using System.Numerics;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier.Maps;

/// <summary>Managed mirror of <c>ArLightSnapshot</c> (native/against-rome-data/include/against_rome_data.h, ABI 1).</summary>
[StructLayout(LayoutKind.Sequential)]
internal struct ArLightSnapshotNative
{
    public uint StructSize;
    public uint Slot;
    public uint ActiveRaw;
    public float PositionX, PositionY, PositionZ;
    public float ColorR, ColorG, ColorB;
    public float EffectiveRadius;
    public float BaselineRadius;
    public float TypeParameter;
    public uint AgeRaw;
    public uint SpecialFxRaw;
    public ushort TypeRaw;
    public short Owner;
    public short Parent;
    public short Definition;
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 56)]
    public byte[] RawFields;
}

/// <summary>
/// Locates, loads and ABI-checks <c>against_rome_data.dll</c>. The DLL is taken from
/// <c>ARM_NATIVE_DATA_DLL</c> (absolute path, test/development override) or else from the fixed
/// <c>AppContext.BaseDirectory</c>; never from the current directory or PATH. Once loaded the library is kept
/// for the process lifetime so every handle's free function stays valid.
/// </summary>
public static class RustNativeData
{
    public const string EnvironmentVariable = "ARM_NATIVE_DATA_DLL";
    public const string DefaultFileName = "against_rome_data.dll";
    public const uint ExpectedAbiVersion = 1;
    public const int ExpectedSnapshotStructSize = 120;

    /// <summary>C layout offsets from the header (natural alignment), keyed by header field name.</summary>
    public static IReadOnlyDictionary<string, int> ExpectedFieldOffsets { get; } = new Dictionary<string, int>
    {
        ["struct_size"] = 0, ["slot"] = 4, ["active_raw"] = 8, ["position"] = 12, ["color"] = 24,
        ["effective_radius"] = 36, ["baseline_radius"] = 40, ["type_parameter"] = 44, ["age_raw"] = 48,
        ["special_fx_raw"] = 52, ["type_raw"] = 56, ["owner"] = 58, ["parent"] = 60, ["definition"] = 62,
        ["raw_fields"] = 64,
    };

    /// <summary>Offsets of the managed marshalling struct, measured with <see cref="Marshal.OffsetOf"/>.</summary>
    public static IReadOnlyDictionary<string, int> ManagedFieldOffsets { get; } = MeasureManagedOffsets();
    public static int ManagedSnapshotStructSize => Marshal.SizeOf<ArLightSnapshotNative>();

    private static readonly object Gate = new();
    private static Api? _api;

    public static string ResolveLibraryPath()
    {
        string? env = Environment.GetEnvironmentVariable(EnvironmentVariable);
        string path;
        if (!string.IsNullOrWhiteSpace(env))
        {
            if (!Path.IsPathFullyQualified(env))
                throw new InvalidOperationException($"{EnvironmentVariable} 必須是絕對路徑：{env}");
            path = Path.GetFullPath(env);
        }
        else path = Path.Combine(AppContext.BaseDirectory, DefaultFileName);
        if (!File.Exists(path)) throw new FileNotFoundException("找不到 against_rome_data 原生庫。", path);
        return path;
    }

    /// <summary>True when the library resolves, loads and passes the ABI check; otherwise explains why.</summary>
    public static bool TryEnsureLoaded(out string? error)
    {
        try { _ = GetApi(); error = null; return true; }
        catch (Exception ex) when (ex is IOException or InvalidOperationException or DllNotFoundException
            or BadImageFormatException or EntryPointNotFoundException or ArgumentException)
        {
            error = ex.Message; return false;
        }
    }

    /// <summary>True when the appended projection exports bind and report the expected struct sizes.</summary>
    public static bool TryEnsureProjectionLoaded(out string? error)
    {
        try { _ = GetApi().Projection; error = null; return true; }
        catch (Exception ex) when (ex is IOException or InvalidOperationException or DllNotFoundException
            or BadImageFormatException or EntryPointNotFoundException or ArgumentException)
        {
            error = ex.Message; return false;
        }
    }

    /// <summary>Native-reported <c>ar_light_projection_request_struct_size()</c> / <c>ar_projected_light_struct_size()</c>.</summary>
    public static (uint Request, uint Projected) NativeProjectionStructSizes
        => (GetApi().Projection.NativeRequestSize, GetApi().Projection.NativeProjectedSize);

    public static string LoadedLibraryPath => GetApi().Path;
    public static uint NativeAbiVersion => GetApi().AbiVersion;
    public static uint NativeSnapshotStructSize => GetApi().StructSize;

    internal static Api GetApi()
    {
        lock (Gate)
        {
            // Failures are not cached so a corrected environment can be retried.
            return _api ??= Load(ResolveLibraryPath());
        }
    }

    private static Dictionary<string, int> MeasureManagedOffsets()
    {
        var t = typeof(ArLightSnapshotNative);
        (string Header, string Field)[] map =
        [
            ("struct_size", nameof(ArLightSnapshotNative.StructSize)), ("slot", nameof(ArLightSnapshotNative.Slot)),
            ("active_raw", nameof(ArLightSnapshotNative.ActiveRaw)), ("position", nameof(ArLightSnapshotNative.PositionX)),
            ("color", nameof(ArLightSnapshotNative.ColorR)), ("effective_radius", nameof(ArLightSnapshotNative.EffectiveRadius)),
            ("baseline_radius", nameof(ArLightSnapshotNative.BaselineRadius)), ("type_parameter", nameof(ArLightSnapshotNative.TypeParameter)),
            ("age_raw", nameof(ArLightSnapshotNative.AgeRaw)), ("special_fx_raw", nameof(ArLightSnapshotNative.SpecialFxRaw)),
            ("type_raw", nameof(ArLightSnapshotNative.TypeRaw)), ("owner", nameof(ArLightSnapshotNative.Owner)),
            ("parent", nameof(ArLightSnapshotNative.Parent)), ("definition", nameof(ArLightSnapshotNative.Definition)),
            ("raw_fields", nameof(ArLightSnapshotNative.RawFields)),
        ];
        var result = new Dictionary<string, int>();
        foreach (var (header, field) in map) result[header] = (int)Marshal.OffsetOf(t, field);
        return result;
    }

    private static Api Load(string path)
    {
        IntPtr library = NativeLibrary.Load(path);
        try
        {
            var api = new Api(path, library);
            if (api.AbiVersion != ExpectedAbiVersion)
                throw new InvalidOperationException($"against_rome_data ABI 版本 {api.AbiVersion}，預期 {ExpectedAbiVersion}。");
            int managed = ManagedSnapshotStructSize;
            if (api.StructSize != (uint)managed || managed != ExpectedSnapshotStructSize)
                throw new InvalidOperationException(
                    $"ArLightSnapshot 大小不符：native={api.StructSize}，managed={managed}，預期={ExpectedSnapshotStructSize}。");
            foreach (var pair in ExpectedFieldOffsets)
                if (ManagedFieldOffsets[pair.Key] != pair.Value)
                    throw new InvalidOperationException(
                        $"ArLightSnapshot.{pair.Key} offset 不符：managed={ManagedFieldOffsets[pair.Key]}，預期={pair.Value}。");
            return api;
        }
        catch
        {
            NativeLibrary.Free(library);
            throw;
        }
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate uint U32Fn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate IntPtr MessageFn(int code);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int OpenFn(byte[] data, nuint len, out IntPtr pool);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate void FreeFn(IntPtr pool);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int CountFn(IntPtr pool, out uint count);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int SnapshotFn(IntPtr pool, uint slot, ref ArLightSnapshotNative snapshot);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] internal delegate int CopyFn(IntPtr pool, byte[]? buffer, nuint capacity, out nuint length);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate int ProjectBatchFn(IntPtr pool, byte[]? requests, nuint count, [In, Out] byte[]? output, nuint outCapacity,
        out nuint outCount, out uint failedIndex);

    /// <summary>
    /// Saved-light projection ABI (request 36 bytes / output 140 bytes). Bound lazily so a DLL that predates the
    /// appended exports still serves the base ABI. A future sibling export family (e.g. particle-linked lights)
    /// should follow this shape: <see cref="Api.BindExport{T}"/> + size checks, cached on first successful bind.
    /// </summary>
    internal sealed class ProjectionApi
    {
        public const int RequestSize = 36, ProjectedSize = 140;
        public ProjectBatchFn ProjectBatch { get; }
        public uint NativeRequestSize { get; }
        public uint NativeProjectedSize { get; }

        internal ProjectionApi(Api api)
        {
            uint request = api.BindExport<U32Fn>("ar_light_projection_request_struct_size")();
            uint projected = api.BindExport<U32Fn>("ar_projected_light_struct_size")();
            if (request != RequestSize || projected != ProjectedSize)
                throw new InvalidOperationException(
                    $"投影 ABI 大小不符：request native={request}/預期={RequestSize}，projected native={projected}/預期={ProjectedSize}。");
            NativeRequestSize = request; NativeProjectedSize = projected;
            ProjectBatch = api.BindExport<ProjectBatchFn>("ar_light_pool_project_batch");
        }
    }

    internal sealed class Api
    {
        private readonly MessageFn _message;
        private readonly IntPtr _library;
        private ProjectionApi? _projection;
        public string Path { get; }
        public uint AbiVersion { get; }
        public uint StructSize { get; }
        public OpenFn Open { get; }
        public FreeFn Free { get; }
        public CountFn ActiveCount { get; }
        public SnapshotFn Snapshot { get; }
        public SnapshotFn SnapshotAny { get; }
        public CopyFn CopyPayload { get; }
        public CopyFn CopyPfilHeader { get; }

        public Api(string path, IntPtr library)
        {
            Path = path;
            _library = library;
            T Bind<T>(string name) where T : Delegate
                => Marshal.GetDelegateForFunctionPointer<T>(NativeLibrary.GetExport(library, name));
            AbiVersion = Bind<U32Fn>("ar_light_abi_version")();
            StructSize = Bind<U32Fn>("ar_light_snapshot_struct_size")();
            _message = Bind<MessageFn>("ar_light_error_message");
            Open = Bind<OpenFn>("ar_light_pool_open");
            Free = Bind<FreeFn>("ar_light_pool_free");
            ActiveCount = Bind<CountFn>("ar_light_pool_active_count");
            Snapshot = Bind<SnapshotFn>("ar_light_pool_snapshot");
            SnapshotAny = Bind<SnapshotFn>("ar_light_pool_snapshot_any");
            CopyPayload = Bind<CopyFn>("ar_light_pool_copy_payload");
            CopyPfilHeader = Bind<CopyFn>("ar_light_pool_copy_pfil_header");
        }

        internal T BindExport<T>(string name) where T : Delegate
            => Marshal.GetDelegateForFunctionPointer<T>(NativeLibrary.GetExport(_library, name));

        /// <summary>Lazy projection binding; failures (missing export / size mismatch) are not cached.</summary>
        public ProjectionApi Projection
        {
            get { lock (Gate) return _projection ??= new ProjectionApi(this); }
        }

        public string Message(int code) => Marshal.PtrToStringAnsi(_message(code)) ?? "unknown";
    }
}

/// <summary>
/// Immutable bit-preserving copy of one 56-byte saved light slot read through the Rust core.
/// Typed values are views of <see cref="RawFields"/>, which remain the authoritative saved bytes.
/// </summary>
public sealed class RustLightSnapshot
{
    private readonly byte[] _raw;

    internal RustLightSnapshot(in ArLightSnapshotNative n)
    {
        if (n.RawFields is not { Length: 56 }) throw new InvalidDataException("原生快照缺少 56-byte raw_fields。");
        _raw = (byte[])n.RawFields.Clone();
        Slot = checked((int)n.Slot); ActiveRaw = n.ActiveRaw;
        Position = new(n.PositionX, n.PositionY, n.PositionZ);
        Color = new(n.ColorR, n.ColorG, n.ColorB);
        EffectiveRadius = n.EffectiveRadius; BaselineRadius = n.BaselineRadius; TypeParameter = n.TypeParameter;
        AgeRaw = n.AgeRaw; SpecialFxRaw = n.SpecialFxRaw; TypeRaw = n.TypeRaw;
        OwnerObject = n.Owner; ParentObject = n.Parent; Definition = n.Definition;
    }

    public int Slot { get; }
    /// <summary>Saved active word as stored (not normalized to 1).</summary>
    public uint ActiveRaw { get; }
    public bool IsActive => ActiveRaw != 0;
    public Vector3 Position { get; }
    public Vector3 Color { get; }
    public float EffectiveRadius { get; }
    public float BaselineRadius { get; }
    public float TypeParameter { get; }
    public uint AgeRaw { get; }
    public int AgeSigned => unchecked((int)AgeRaw);
    public uint SpecialFxRaw { get; }
    public ushort TypeRaw { get; }
    public short OwnerObject { get; }
    public short ParentObject { get; }
    public short Definition { get; }

    /// <summary>The saved 56 bytes (16 columns concatenated).</summary>
    public ReadOnlySpan<byte> RawFields => _raw;
    public byte[] ToRawBytes() => (byte[])_raw.Clone();

    // Raw IEEE bits, read from the authoritative bytes (offsets 4,8,12 / 16,20,24 / 28 / 32 / 38).
    public (uint X, uint Y, uint Z) PositionBits => (U32(4), U32(8), U32(12));
    public (uint R, uint G, uint B) ColorBits => (U32(16), U32(20), U32(24));
    public uint EffectiveRadiusBits => U32(28);
    public uint BaselineRadiusBits => U32(32);
    public uint TypeParameterBits => U32(38);

    private uint U32(int offset) => BinaryPrimitives.ReadUInt32LittleEndian(_raw.AsSpan(offset));
}

/// <summary>
/// Thin disposable bridge over the Rust <c>ArLightPool</c> handle. The native handle is freed exactly once
/// (SafeHandle); every call holds a reference so a concurrent <see cref="Dispose"/> cannot free it mid-call.
/// Native error codes become <see cref="InvalidDataException"/>; disposed use throws
/// <see cref="ObjectDisposedException"/>. Contains no parser or lighting logic.
/// </summary>
public sealed partial class RustLightPool : IDisposable
{
    public const int Capacity = 1024;
    public const int PayloadLength = 57352;
    public const int PfilHeaderLength = 64;

    private const int CodeBufferTooSmall = 2, CodeInvalidSlot = 4, CodeInactiveSlot = 5;
    private static long _opened, _freed;

    private readonly Handle _handle;
    private readonly RustNativeData.Api _api;

    /// <summary>Diagnostics: native handles opened / freed by this process (equal once all are disposed).</summary>
    public static long OpenedHandleCount => Interlocked.Read(ref _opened);
    public static long FreedHandleCount => Interlocked.Read(ref _freed);

    private RustLightPool(Handle handle, RustNativeData.Api api) { _handle = handle; _api = api; }

    public static RustLightPool Load(string path) => Open(File.ReadAllBytes(path));

    /// <summary>Opens a light.dat file image (PFIL-wrapped or raw 57352-byte payload); the input is copied.</summary>
    public static RustLightPool Open(ReadOnlySpan<byte> fileImage)
    {
        var api = RustNativeData.GetApi();
        int code = api.Open(fileImage.ToArray(), (nuint)fileImage.Length, out IntPtr raw);
        if (code != 0)
        {
            if (raw != IntPtr.Zero) api.Free(raw);
            throw Fail(api, "ar_light_pool_open", code);
        }
        if (raw == IntPtr.Zero) throw new InvalidDataException("ar_light_pool_open 回報成功但 handle 為 null。");
        Interlocked.Increment(ref _opened);
        return new RustLightPool(new Handle(raw, api), api);
    }

    public bool IsDisposed => _handle.IsClosed;

    public int ActiveCount => WithHandle(h =>
    {
        int code = _api.ActiveCount(h, out uint count);
        if (code != 0) throw Fail(_api, "ar_light_pool_active_count", code);
        return checked((int)count);
    });

    /// <summary>Active slot snapshot; null for an inactive slot. Out-of-range slots throw.</summary>
    public RustLightSnapshot? Capture(int slot)
    {
        ValidateSlot(slot);
        return WithHandle(h => ReadSnapshot(_api.Snapshot, "ar_light_pool_snapshot", h, slot, allowInactive: true));
    }

    /// <summary>Any in-range slot including inactive ones (stored bytes preserved).</summary>
    public RustLightSnapshot CaptureAny(int slot)
    {
        ValidateSlot(slot);
        return WithHandle(h => ReadSnapshot(_api.SnapshotAny, "ar_light_pool_snapshot_any", h, slot, allowInactive: false))!;
    }

    /// <summary>The complete decoded 57352-byte payload (fresh copy).</summary>
    public byte[] CopyPayload() => WithHandle(h => CopyBytes(_api.CopyPayload, "ar_light_pool_copy_payload", h));

    /// <summary>The original 64-byte PFIL header, or an empty array for raw-payload input.</summary>
    public byte[] CopyPfilHeader() => WithHandle(h => CopyBytes(_api.CopyPfilHeader, "ar_light_pool_copy_pfil_header", h));

    /// <summary>
    /// Atomic batch projection of saved lights through the Rust core (direct float projection; NOT particle-linked
    /// lights). Any failure throws <see cref="InvalidDataException"/> naming the failed request index; nothing partial
    /// is returned and the pool is unchanged. The numeric result is whatever Rust computed — only identity,
    /// ordering and raw-byte consistency are checked here.
    /// </summary>
    public RustLightProjectionBatch ProjectBatch(IEnumerable<RustLightProjectionRequest> requests)
    {
        ArgumentNullException.ThrowIfNull(requests);
        var list = requests.ToArray();
        if (list.Any(r => r is null)) throw new ArgumentException("請求不可為 null。", nameof(requests));
        return WithHandle(h =>
        {
            var api = _api.Projection;
            const int rs = RustNativeData.ProjectionApi.RequestSize, ps = RustNativeData.ProjectionApi.ProjectedSize;
            byte[]? input = null, output = null;
            if (list.Length > 0)
            {
                input = new byte[checked(list.Length * rs)];
                output = new byte[checked(list.Length * ps)];
                for (int i = 0; i < list.Length; i++)
                {
                    list[i].WriteTo(input.AsSpan(i * rs, rs));
                    BinaryPrimitives.WriteUInt32LittleEndian(output.AsSpan(i * ps, ps), ps);
                }
            }
            int code = api.ProjectBatch(h, input, (nuint)list.Length, output, (nuint)list.Length,
                out nuint produced, out uint failed);
            if (code != 0)
            {
                string where = failed == uint.MaxValue ? "非特定請求" : $"請求索引 {failed}";
                throw new InvalidDataException(
                    $"ar_light_pool_project_batch 失敗（{where}）：code {code}（{_api.Message(code)}）。");
            }
            if (produced != (nuint)list.Length)
                throw new InvalidDataException($"ar_light_pool_project_batch 回報 {produced} 筆，預期 {list.Length}。");
            var result = new RustProjectedLight[list.Length];
            for (int i = 0; i < list.Length; i++)
                result[i] = RustProjectedLight.Parse(output!.AsSpan(i * ps, ps), i, list[i]);
            return new RustLightProjectionBatch(result);
        });
    }

    public void Dispose() => _handle.Dispose();

    private static void ValidateSlot(int slot)
    {
        if (slot < 0) throw new ArgumentOutOfRangeException(nameof(slot), slot, "光源槽位不可為負。");
    }

    private RustLightSnapshot? ReadSnapshot(RustNativeData.SnapshotFn fn, string name, IntPtr handle, int slot, bool allowInactive)
    {
        var native = new ArLightSnapshotNative { StructSize = _api.StructSize, RawFields = new byte[56] };
        int code = fn(handle, (uint)slot, ref native);
        if (code == CodeInvalidSlot)
            throw new ArgumentOutOfRangeException(nameof(slot), slot, $"光源槽位超出 0..{Capacity - 1}。");
        if (code == CodeInactiveSlot && allowInactive) return null;
        if (code != 0) throw Fail(_api, name, code);
        if (native.Slot != (uint)slot)
            throw new InvalidDataException($"原生快照槽位 {native.Slot} 與請求 {slot} 不符（ABI 錯位？）。");
        return new RustLightSnapshot(native);
    }

    private static byte[] CopyBytes(RustNativeData.CopyFn fn, string name, IntPtr handle)
    {
        int code = fn(handle, null, 0, out nuint needed);
        if (code == 0 && needed == 0) return [];
        if (code != CodeBufferTooSmall)
            throw Fail(RustNativeData.GetApi(), name, code == 0 ? -1 : code);
        byte[] buffer = new byte[checked((int)needed)];
        code = fn(handle, buffer, (nuint)buffer.Length, out nuint written);
        if (code != 0) throw Fail(RustNativeData.GetApi(), name, code);
        if (written != (nuint)buffer.Length) throw new InvalidDataException($"{name} 回報長度 {written}，預期 {buffer.Length}。");
        return buffer;
    }

    private T WithHandle<T>(Func<IntPtr, T> action)
    {
        ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
        bool added = false;
        try
        {
            try { _handle.DangerousAddRef(ref added); }
            catch (ObjectDisposedException) { throw new ObjectDisposedException(nameof(RustLightPool)); }
            return action(_handle.DangerousGetHandle());
        }
        finally { if (added) _handle.DangerousRelease(); }
    }

    private static InvalidDataException Fail(RustNativeData.Api api, string function, int code)
        => new($"{function} 失敗：code {code}（{(code < 0 ? "unexpected success state" : api.Message(code))}）。");

    private sealed class Handle : SafeHandle
    {
        private readonly RustNativeData.Api _owner;
        public Handle(IntPtr raw, RustNativeData.Api api) : base(IntPtr.Zero, ownsHandle: true)
        {
            _owner = api;
            SetHandle(raw);
        }
        public override bool IsInvalid => handle == IntPtr.Zero;
        protected override bool ReleaseHandle()
        {
            _owner.Free(handle);
            Interlocked.Increment(ref _freed);
            return true;
        }
    }
}

/// <summary>
/// Immutable projection request: host target GUID (opaque, never deduplicated), source slot and signed 16.16
/// fixed-point XYZ delta. Layout written: size(4) slot(4) guid(16) delta(3 x int32) = 36 bytes.
/// </summary>
public sealed class RustLightProjectionRequest
{
    public RustLightProjectionRequest(Guid targetGuid, int sourceSlot, int deltaFixedX, int deltaFixedY, int deltaFixedZ)
    {
        if (sourceSlot < 0) throw new ArgumentOutOfRangeException(nameof(sourceSlot), sourceSlot, "光源槽位不可為負。");
        TargetGuid = targetGuid; SourceSlot = sourceSlot;
        DeltaFixedX = deltaFixedX; DeltaFixedY = deltaFixedY; DeltaFixedZ = deltaFixedZ;
    }

    public Guid TargetGuid { get; }
    public int SourceSlot { get; }
    public int DeltaFixedX { get; }
    public int DeltaFixedY { get; }
    public int DeltaFixedZ { get; }

    internal void WriteTo(Span<byte> d)
    {
        BinaryPrimitives.WriteUInt32LittleEndian(d, RustNativeData.ProjectionApi.RequestSize);
        BinaryPrimitives.WriteUInt32LittleEndian(d[4..], (uint)SourceSlot);
        if (!TargetGuid.TryWriteBytes(d.Slice(8, 16), bigEndian: false, out _)) throw new InvalidOperationException("GUID 寫入失敗。");
        BinaryPrimitives.WriteInt32LittleEndian(d[24..], DeltaFixedX);
        BinaryPrimitives.WriteInt32LittleEndian(d[28..], DeltaFixedY);
        BinaryPrimitives.WriteInt32LittleEndian(d[32..], DeltaFixedZ);
    }
}

/// <summary>One projected light: request identity plus the returned immutable snapshot (raw 56 bytes authoritative).</summary>
public sealed class RustProjectedLight
{
    private RustProjectedLight(int requestIndex, Guid targetGuid, int sourceSlot, RustLightSnapshot snapshot)
    { RequestIndex = requestIndex; TargetGuid = targetGuid; SourceSlot = sourceSlot; Snapshot = snapshot; }

    public int RequestIndex { get; }
    public Guid TargetGuid { get; }
    public int SourceSlot { get; }
    public RustLightSnapshot Snapshot { get; }

    /// <summary>ArProjectedLight header offsets (size0 index4 guid8 slot24 active28 pos32 color44 eff56 base60 param64 age68 fx72 type76 owner78 parent80 def82 raw84 = 140).</summary>
    public static IReadOnlyDictionary<string, int> ExpectedFieldOffsets { get; } = new Dictionary<string, int>
    {
        ["struct_size"] = 0, ["request_index"] = 4, ["target_guid"] = 8, ["slot"] = 24, ["active_raw"] = 28,
        ["position"] = 32, ["color"] = 44, ["effective_radius"] = 56, ["baseline_radius"] = 60, ["type_parameter"] = 64,
        ["age_raw"] = 68, ["special_fx_raw"] = 72, ["type_raw"] = 76, ["owner"] = 78, ["parent"] = 80,
        ["definition"] = 82, ["raw_fields"] = 84,
    };

    internal static RustProjectedLight Parse(ReadOnlySpan<byte> s, int index, RustLightProjectionRequest request)
    {
        string at = $"請求索引 {index}";
        if (BinaryPrimitives.ReadUInt32LittleEndian(s) != RustNativeData.ProjectionApi.ProjectedSize)
            throw new InvalidDataException($"{at}：struct_size 不符。");
        uint returnedIndex = BinaryPrimitives.ReadUInt32LittleEndian(s[4..]);
        if (returnedIndex != (uint)index) throw new InvalidDataException($"{at}：request_index 不符（回傳 {returnedIndex}）。");
        var guid = new Guid(s.Slice(8, 16), bigEndian: false);
        if (guid != request.TargetGuid) throw new InvalidDataException($"{at}：target_guid 不符。");
        uint slot = BinaryPrimitives.ReadUInt32LittleEndian(s[24..]);
        if (slot != (uint)request.SourceSlot) throw new InvalidDataException($"{at}：slot {slot} 與請求 {request.SourceSlot} 不符。");

        var snapshot = ParseSnapshot(s, slot, at);
        return new RustProjectedLight(index, guid, request.SourceSlot, snapshot);
    }

    /// <summary>Shared by the projected-light outputs (identical typed/raw layout up to offset 140): validates typed fields against raw_fields and builds the snapshot.</summary>
    internal static RustLightSnapshot ParseSnapshot(ReadOnlySpan<byte> s, uint slot, string at)
    {
        var raw = s.Slice(84, 56);
        // typed fields must be views of raw_fields (consistency only; no numeric recomputation)
        ReadOnlySpan<(int Typed, int Raw, int Len)> map =
        [
            (28, 0, 4), (32, 4, 12), (44, 16, 12), (56, 28, 4), (60, 32, 4), (64, 38, 4), (68, 42, 4), (72, 46, 4),
            (76, 36, 2), (78, 50, 2), (80, 52, 2), (82, 54, 2),
        ];
        foreach (var (typed, rawOffset, len) in map)
            if (!s.Slice(typed, len).SequenceEqual(raw.Slice(rawOffset, len)))
                throw new InvalidDataException($"{at}：typed 欄位 offset {typed} 與 raw_fields offset {rawOffset} 不一致。");

        var n = new ArLightSnapshotNative
        {
            StructSize = RustNativeData.ExpectedSnapshotStructSize, Slot = slot,
            ActiveRaw = BinaryPrimitives.ReadUInt32LittleEndian(s[28..]),
            PositionX = BinaryPrimitives.ReadSingleLittleEndian(s[32..]), PositionY = BinaryPrimitives.ReadSingleLittleEndian(s[36..]),
            PositionZ = BinaryPrimitives.ReadSingleLittleEndian(s[40..]),
            ColorR = BinaryPrimitives.ReadSingleLittleEndian(s[44..]), ColorG = BinaryPrimitives.ReadSingleLittleEndian(s[48..]),
            ColorB = BinaryPrimitives.ReadSingleLittleEndian(s[52..]),
            EffectiveRadius = BinaryPrimitives.ReadSingleLittleEndian(s[56..]), BaselineRadius = BinaryPrimitives.ReadSingleLittleEndian(s[60..]),
            TypeParameter = BinaryPrimitives.ReadSingleLittleEndian(s[64..]),
            AgeRaw = BinaryPrimitives.ReadUInt32LittleEndian(s[68..]), SpecialFxRaw = BinaryPrimitives.ReadUInt32LittleEndian(s[72..]),
            TypeRaw = BinaryPrimitives.ReadUInt16LittleEndian(s[76..]), Owner = BinaryPrimitives.ReadInt16LittleEndian(s[78..]),
            Parent = BinaryPrimitives.ReadInt16LittleEndian(s[80..]), Definition = BinaryPrimitives.ReadInt16LittleEndian(s[82..]),
            RawFields = raw.ToArray(),
        };
        return new RustLightSnapshot(n);
    }
}

/// <summary>Immutable, request-ordered result of an atomic projection batch.</summary>
public sealed class RustLightProjectionBatch
{
    private readonly RustProjectedLight[] _items;
    internal RustLightProjectionBatch(RustProjectedLight[] items) => _items = items;
    public int Count => _items.Length;
    public RustProjectedLight this[int index] => _items[index];
    public IReadOnlyList<RustProjectedLight> Items => Array.AsReadOnly(_items);
}
