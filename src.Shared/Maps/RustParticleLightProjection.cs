using System.Buffers.Binary;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier.Maps;

/// <summary>
/// Thin C# bridge for the appended Rust particle-linked saved-light projection ABI
/// (<c>ar_light_pool_project_particle_batch</c>, request 44 bytes / output 160 bytes). Emitter record bytes are only
/// length-checked and copied: child numerics, fixed 16.16 sums and SAR16 are all computed by Rust.
/// </summary>
public static class RustParticleLightProjection
{
    public const int RequestSize = 44, ProjectedSize = 160, RecordSize = 2252, ChildCount = 64;

    /// <summary>Native error codes from the header (appended particle enum) that tests/callers may branch on.</summary>
    public const int CodeInvalidSlot = 4, CodeInactiveSlot = 5, CodeInvalidRecordIndex = 9, CodeInactiveEmitter = 10,
        CodeInvalidChild = 11, CodeLightLinkMismatch = 12, CodeFixedOverflow = 13, CodeBadRecordsLength = 14;

    /// <summary>ArParticleProjectedLight offsets beyond the shared light layout (raw_fields@84).</summary>
    public static IReadOnlyDictionary<string, int> ExpectedFieldOffsets { get; } = new Dictionary<string, int>
    {
        ["struct_size"] = 0, ["request_index"] = 4, ["target_guid"] = 8, ["slot"] = 24, ["active_raw"] = 28,
        ["position"] = 32, ["color"] = 44, ["effective_radius"] = 56, ["baseline_radius"] = 60, ["type_parameter"] = 64,
        ["age_raw"] = 68, ["special_fx_raw"] = 72, ["type_raw"] = 76, ["owner"] = 78, ["parent"] = 80,
        ["definition"] = 82, ["raw_fields"] = 84, ["child_ordinal"] = 140, ["child_active_raw"] = 144, ["fixed_position"] = 148,
    };

    /// <summary>ArParticleLightRequest offsets: size0 record_index4 child_ordinal8 light_slot12 guid16 delta32 = 44.</summary>
    public static IReadOnlyDictionary<string, int> ExpectedRequestOffsets { get; } = new Dictionary<string, int>
    {
        ["struct_size"] = 0, ["record_index"] = 4, ["child_ordinal"] = 8, ["light_slot"] = 12, ["target_guid"] = 16, ["delta_fixed"] = 32,
    };

    /// <summary>True when the particle exports bind and report 44/160; otherwise explains why. Failures are not cached.</summary>
    public static bool TryEnsureLoaded(out string? error)
    {
        try { _ = RustLightPool.GetParticleApi(); error = null; return true; }
        catch (Exception ex) when (ex is IOException or InvalidOperationException or DllNotFoundException
            or BadImageFormatException or EntryPointNotFoundException or ArgumentException)
        {
            error = ex.Message; return false;
        }
    }

    /// <summary>Native-reported <c>ar_particle_light_request_struct_size()</c> / <c>ar_particle_projected_light_struct_size()</c>.</summary>
    public static (uint Request, uint Projected) NativeStructSizes
    {
        get { var api = RustLightPool.GetParticleApi(); return (api.NativeRequestSize, api.NativeProjectedSize); }
    }
}

/// <summary>
/// Immutable copy of one saved emitter record (exactly 2252 bytes). Requests that share the same instance are packed
/// once per batch; distinct instances are always packed separately even if their bytes are equal.
/// </summary>
public sealed class RustParticleEmitterRecord
{
    private readonly byte[] _bytes;

    private RustParticleEmitterRecord(byte[] bytes) => _bytes = bytes;

    /// <summary>Copies <paramref name="record"/>; throws unless it is exactly 2252 bytes. No field is parsed.</summary>
    public static RustParticleEmitterRecord Copy(ReadOnlySpan<byte> record)
    {
        if (record.Length != RustParticleLightProjection.RecordSize)
            throw new ArgumentException($"發射器記錄必須為 {RustParticleLightProjection.RecordSize} bytes，實際 {record.Length}。", nameof(record));
        return new RustParticleEmitterRecord(record.ToArray());
    }

    public ReadOnlySpan<byte> Bytes => _bytes;
    public byte[] ToBytes() => (byte[])_bytes.Clone();
}

/// <summary>
/// Immutable request for one particle-linked light instance: opaque host target GUID (never deduplicated), the full
/// source emitter record, child ordinal, the light slot the child must link to, and the signed 16.16 XYZ delta.
/// </summary>
public sealed class RustParticleLightRequest
{
    public RustParticleLightRequest(Guid targetGuid, RustParticleEmitterRecord emitterRecord, int childOrdinal, int lightSlot,
        int deltaFixedX, int deltaFixedY, int deltaFixedZ)
    {
        ArgumentNullException.ThrowIfNull(emitterRecord);
        if (childOrdinal < 0) throw new ArgumentOutOfRangeException(nameof(childOrdinal), childOrdinal, "子項序號不可為負。");
        if (lightSlot < 0) throw new ArgumentOutOfRangeException(nameof(lightSlot), lightSlot, "光源槽位不可為負。");
        TargetGuid = targetGuid; EmitterRecord = emitterRecord; ChildOrdinal = childOrdinal; LightSlot = lightSlot;
        DeltaFixedX = deltaFixedX; DeltaFixedY = deltaFixedY; DeltaFixedZ = deltaFixedZ;
    }

    /// <summary>Convenience: copies <paramref name="emitterRecord"/> into a new (not shared) record instance.</summary>
    public RustParticleLightRequest(Guid targetGuid, ReadOnlySpan<byte> emitterRecord, int childOrdinal, int lightSlot,
        int deltaFixedX, int deltaFixedY, int deltaFixedZ)
        : this(targetGuid, RustParticleEmitterRecord.Copy(emitterRecord), childOrdinal, lightSlot, deltaFixedX, deltaFixedY, deltaFixedZ) { }

    public Guid TargetGuid { get; }
    public RustParticleEmitterRecord EmitterRecord { get; }
    public int ChildOrdinal { get; }
    public int LightSlot { get; }
    public int DeltaFixedX { get; }
    public int DeltaFixedY { get; }
    public int DeltaFixedZ { get; }

    internal void WriteTo(Span<byte> d, uint packedRecordIndex)
    {
        BinaryPrimitives.WriteUInt32LittleEndian(d, RustParticleLightProjection.RequestSize);
        BinaryPrimitives.WriteUInt32LittleEndian(d[4..], packedRecordIndex);
        BinaryPrimitives.WriteUInt32LittleEndian(d[8..], (uint)ChildOrdinal);
        BinaryPrimitives.WriteUInt32LittleEndian(d[12..], (uint)LightSlot);
        if (!TargetGuid.TryWriteBytes(d.Slice(16, 16), bigEndian: false, out _)) throw new InvalidOperationException("GUID 寫入失敗。");
        BinaryPrimitives.WriteInt32LittleEndian(d[32..], DeltaFixedX);
        BinaryPrimitives.WriteInt32LittleEndian(d[36..], DeltaFixedY);
        BinaryPrimitives.WriteInt32LittleEndian(d[40..], DeltaFixedZ);
    }
}

/// <summary>Native failure of the particle projection batch: header error code plus the failing request index (null if unspecific).</summary>
public sealed class RustParticleProjectionException : IOException
{
    public RustParticleProjectionException(int code, int? failedRequestIndex, string message) : base(message)
    { Code = code; FailedRequestIndex = failedRequestIndex; }

    public int Code { get; }
    public int? FailedRequestIndex { get; }
}

/// <summary>One projected particle-linked light. <see cref="Snapshot"/> raw 56 bytes are authoritative; all numbers come from Rust.</summary>
public sealed class RustParticleProjectedLight
{
    private RustParticleProjectedLight(int requestIndex, Guid guid, int slot, int childOrdinal, uint childActiveRaw,
        (int X, int Y, int Z) fixedPosition, RustLightSnapshot snapshot)
    {
        RequestIndex = requestIndex; TargetGuid = guid; SourceSlot = slot; ChildOrdinal = childOrdinal;
        ChildActiveRaw = childActiveRaw; FixedPosition = fixedPosition; Snapshot = snapshot;
    }

    public int RequestIndex { get; }
    public Guid TargetGuid { get; }
    public int SourceSlot { get; }
    public int ChildOrdinal { get; }
    /// <summary>Child active byte as reported by Rust (0 = inactive child, light left untouched).</summary>
    public uint ChildActiveRaw { get; }
    public bool ChildActive => ChildActiveRaw != 0;
    /// <summary>Rust-reported fixed XYZ (active: fixed + delta; inactive: saved, unchanged).</summary>
    public (int X, int Y, int Z) FixedPosition { get; }
    public RustLightSnapshot Snapshot { get; }

    internal static RustParticleProjectedLight Parse(ReadOnlySpan<byte> s, int index, RustParticleLightRequest request)
    {
        string at = $"請求索引 {index}";
        if (BinaryPrimitives.ReadUInt32LittleEndian(s) != RustParticleLightProjection.ProjectedSize)
            throw new InvalidDataException($"{at}：struct_size 不符。");
        uint returnedIndex = BinaryPrimitives.ReadUInt32LittleEndian(s[4..]);
        if (returnedIndex != (uint)index) throw new InvalidDataException($"{at}：request_index 不符（回傳 {returnedIndex}）。");
        var guid = new Guid(s.Slice(8, 16), bigEndian: false);
        if (guid != request.TargetGuid) throw new InvalidDataException($"{at}：target_guid 不符。");
        uint slot = BinaryPrimitives.ReadUInt32LittleEndian(s[24..]);
        if (slot != (uint)request.LightSlot) throw new InvalidDataException($"{at}：slot {slot} 與請求 {request.LightSlot} 不符。");
        uint ordinal = BinaryPrimitives.ReadUInt32LittleEndian(s[140..]);
        if (ordinal != (uint)request.ChildOrdinal) throw new InvalidDataException($"{at}：child_ordinal {ordinal} 與請求 {request.ChildOrdinal} 不符。");
        uint childActive = BinaryPrimitives.ReadUInt32LittleEndian(s[144..]);
        if (childActive > 0xFF) throw new InvalidDataException($"{at}：child_active_raw {childActive} 不是 byte 值（ABI 錯位？）。");

        var snapshot = RustProjectedLight.ParseSnapshot(s, slot, at);
        return new RustParticleProjectedLight(index, guid, request.LightSlot, request.ChildOrdinal, childActive,
            (BinaryPrimitives.ReadInt32LittleEndian(s[148..]), BinaryPrimitives.ReadInt32LittleEndian(s[152..]),
                BinaryPrimitives.ReadInt32LittleEndian(s[156..])), snapshot);
    }
}

/// <summary>Immutable, request-ordered result of an atomic particle projection batch.</summary>
public sealed class RustParticleLightProjectionBatch
{
    private readonly RustParticleProjectedLight[] _items;
    internal RustParticleLightProjectionBatch(RustParticleProjectedLight[] items) => _items = items;
    public int Count => _items.Length;
    public RustParticleProjectedLight this[int index] => _items[index];
    public IReadOnlyList<RustParticleProjectedLight> Items => Array.AsReadOnly(_items);
}

public sealed partial class RustLightPool
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate uint U32Fn();

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate int ProjectParticleBatchFn(IntPtr pool, byte[]? records, nuint recordCount, byte[]? requests, nuint count,
        [In, Out] byte[]? output, nuint outCapacity, out nuint outCount, out uint failedIndex);

    internal sealed class ParticleApi
    {
        internal ProjectParticleBatchFn ProjectBatch { get; }
        public uint NativeRequestSize { get; }
        public uint NativeProjectedSize { get; }

        internal ParticleApi(RustNativeData.Api api)
        {
            uint request = api.BindExport<U32Fn>("ar_particle_light_request_struct_size")();
            uint projected = api.BindExport<U32Fn>("ar_particle_projected_light_struct_size")();
            if (request != RustParticleLightProjection.RequestSize || projected != RustParticleLightProjection.ProjectedSize)
                throw new InvalidOperationException(
                    $"粒子投影 ABI 大小不符：request native={request}/預期={RustParticleLightProjection.RequestSize}，" +
                    $"projected native={projected}/預期={RustParticleLightProjection.ProjectedSize}。");
            NativeRequestSize = request; NativeProjectedSize = projected;
            ProjectBatch = api.BindExport<ProjectParticleBatchFn>("ar_light_pool_project_particle_batch");
        }
    }

    private static readonly object ParticleGate = new();
    private static ParticleApi? _particleApi;

    /// <summary>Lazy bind (base ABI untouched if the exports are missing); failures are not cached.</summary>
    internal static ParticleApi GetParticleApi()
    {
        var api = RustNativeData.GetApi();
        lock (ParticleGate) return _particleApi ??= new ParticleApi(api);
    }

    /// <summary>
    /// Atomic particle-linked light projection. Records are packed once per distinct <see cref="RustParticleEmitterRecord"/>
    /// instance (reference identity); every request still yields its own output so identical GUID/slot instances are never merged.
    /// Rust validates link/active/child/overflow for the whole batch before writing; any failure throws
    /// <see cref="RustParticleProjectionException"/> with nothing partial returned, and neither this pool nor the supplied
    /// records change. Only identity, ordering and typed-vs-raw consistency are checked here — no numeric recomputation.
    /// </summary>
    public RustParticleLightProjectionBatch ProjectParticleBatch(IEnumerable<RustParticleLightRequest> requests)
    {
        ArgumentNullException.ThrowIfNull(requests);
        var list = requests.ToArray();
        if (list.Any(r => r is null)) throw new ArgumentException("請求不可為 null。", nameof(requests));
        return WithHandle(h =>
        {
            if (list.Length == 0) return new RustParticleLightProjectionBatch([]);
            var api = GetParticleApi();
            const int rs = RustParticleLightProjection.RequestSize, ps = RustParticleLightProjection.ProjectedSize,
                recordSize = RustParticleLightProjection.RecordSize;

            var index = new Dictionary<RustParticleEmitterRecord, int>(ReferenceEqualityComparer.Instance);
            var unique = new List<RustParticleEmitterRecord>();
            var packedIndex = new int[list.Length];
            for (int i = 0; i < list.Length; i++)
            {
                if (!index.TryGetValue(list[i].EmitterRecord, out int at)) { at = unique.Count; index[list[i].EmitterRecord] = at; unique.Add(list[i].EmitterRecord); }
                packedIndex[i] = at;
            }

            byte[] records = new byte[checked(unique.Count * recordSize)];
            for (int i = 0; i < unique.Count; i++) unique[i].Bytes.CopyTo(records.AsSpan(i * recordSize, recordSize));
            byte[] input = new byte[checked(list.Length * rs)];
            byte[] output = new byte[checked(list.Length * ps)];
            for (int i = 0; i < list.Length; i++)
            {
                list[i].WriteTo(input.AsSpan(i * rs, rs), (uint)packedIndex[i]);
                BinaryPrimitives.WriteUInt32LittleEndian(output.AsSpan(i * ps, ps), ps);
            }

            int code = api.ProjectBatch(h, records, (nuint)unique.Count, input, (nuint)list.Length, output, (nuint)list.Length,
                out nuint produced, out uint failed);
            if (code != 0)
            {
                int? failedIndex = failed == uint.MaxValue ? null : checked((int)failed);
                string where = failedIndex is null ? "非特定請求" : $"請求索引 {failedIndex}";
                throw new RustParticleProjectionException(code, failedIndex,
                    $"ar_light_pool_project_particle_batch 失敗（{where}）：code {code}（{_api.Message(code)}）。");
            }
            if (produced != (nuint)list.Length)
                throw new InvalidDataException($"ar_light_pool_project_particle_batch 回報 {produced} 筆，預期 {list.Length}。");
            var result = new RustParticleProjectedLight[list.Length];
            for (int i = 0; i < list.Length; i++)
                result[i] = RustParticleProjectedLight.Parse(output.AsSpan(i * ps, ps), i, list[i]);
            return new RustParticleLightProjectionBatch(result);
        });
    }
}
