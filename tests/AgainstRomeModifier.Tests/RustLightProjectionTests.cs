using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>Attribute: real FFI projection tests; never skipped when ARM_REQUIRE_NATIVE_DATA=1.</summary>
internal sealed class NativeProjectionFactAttribute : FactAttribute
{
    public NativeProjectionFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustNativeData.TryEnsureLoaded(out string? why)) { Skip = "against_rome_data 原生庫不可用：" + why; return; }
        if (!RustNativeData.TryEnsureProjectionLoaded(out string? pwhy)) Skip = "投影 ABI 不可用：" + pwhy;
    }
}

/// <summary>Real FFI tests (no mocks) of the C# projection bridge. Numeric expectations are independent test oracles.</summary>
[Collection("Rust native interop")]
public sealed class RustLightProjectionTests
{
    private static readonly int[] Widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
    private const int Capacity = 1024, PayloadLength = 57352;

    private static int ColumnOffset(int column)
    {
        int offset = 8;
        for (int i = 0; i < column; i++) offset += Capacity * Widths[i];
        return offset;
    }

    private static byte[] EmptyPayload()
    {
        byte[] p = new byte[PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(p, 1);
        BinaryPrimitives.WriteInt32LittleEndian(p.AsSpan(4), Capacity);
        for (int c = 13; c < 16; c++) p.AsSpan(ColumnOffset(c), Capacity * 2).Fill(0xFF);
        return p;
    }

    private static void PutU32(byte[] p, int column, int slot, uint v)
        => BinaryPrimitives.WriteUInt32LittleEndian(p.AsSpan(ColumnOffset(column) + slot * Widths[column]), v);

    private static void PutF(byte[] p, int column, int slot, float v) => PutU32(p, column, slot, BitConverter.SingleToUInt32Bits(v));

    private static void PutS(byte[] p, int column, int slot, ushort v)
        => BinaryPrimitives.WriteUInt16LittleEndian(p.AsSpan(ColumnOffset(column) + slot * Widths[column]), v);

    private static uint Bits(float f) => BitConverter.SingleToUInt32Bits(f);

    private static void SetLight(byte[] p, int slot, uint active, uint x, uint y, uint z)
    {
        PutU32(p, 0, slot, active); PutU32(p, 1, slot, x); PutU32(p, 2, slot, y); PutU32(p, 3, slot, z);
        PutF(p, 4, slot, 2.5f); PutF(p, 5, slot, 0.25f); PutF(p, 6, slot, 0.5f);
        PutF(p, 7, slot, 300f); PutF(p, 8, slot, 450f);
        PutS(p, 9, slot, 0xFFFE); PutF(p, 10, slot, 0.15f); PutU32(p, 11, slot, 0xFFFFFFFF); PutU32(p, 12, slot, 0xDEADBEEF);
        PutS(p, 13, slot, unchecked((ushort)-5)); PutS(p, 14, slot, 12); PutS(p, 15, slot, 39);
    }

    private static byte[] Payload()
    {
        byte[] p = EmptyPayload();
        SetLight(p, 3, 7, Bits(1.5f), Bits(-2.0f), Bits(0.0f));       // active=7, RGB>1, eff != baseline
        SetLight(p, 4, 1, Bits(-0.0f), 0x7FA00001, 0x7F800000);       // -0, signaling NaN, +inf
        SetLight(p, 9, 0, Bits(1f), Bits(2f), Bits(3f));              // inactive
        return p;
    }

    private static RustLightPool OpenPayload(byte[] p) => RustLightPool.Open(p);

    private static void AssertRawExceptPosition(ReadOnlySpan<byte> expected, ReadOnlySpan<byte> actual, string label)
    {
        Assert.True(expected[..4].SequenceEqual(actual[..4]), $"{label}: active bytes");
        Assert.True(expected[16..].SequenceEqual(actual[16..]), $"{label}: bytes 16..56 preserved");
    }

    private static uint RawU32(RustLightSnapshot s, int offset) => BinaryPrimitives.ReadUInt32LittleEndian(s.RawFields[offset..]);

    // ---- ABI ----

    [NativeProjectionFact]
    public void AbiSizesAndOffsetsMatchHeader()
    {
        Assert.True(RustNativeData.TryEnsureProjectionLoaded(out string? error), error);
        Assert.Equal((36u, 140u), RustNativeData.NativeProjectionStructSizes);
        // Base ABI untouched by lazy projection binding.
        Assert.Equal(1u, RustNativeData.NativeAbiVersion);
        Assert.Equal(120u, RustNativeData.NativeSnapshotStructSize);

        // Header ArProjectedLight natural layout.
        var expected = new Dictionary<string, int>
        {
            ["struct_size"] = 0, ["request_index"] = 4, ["target_guid"] = 8, ["slot"] = 24, ["active_raw"] = 28,
            ["position"] = 32, ["color"] = 44, ["effective_radius"] = 56, ["baseline_radius"] = 60, ["type_parameter"] = 64,
            ["age_raw"] = 68, ["special_fx_raw"] = 72, ["type_raw"] = 76, ["owner"] = 78, ["parent"] = 80,
            ["definition"] = 82, ["raw_fields"] = 84,
        };
        Assert.Equal(expected.OrderBy(k => k.Key), RustProjectedLight.ExpectedFieldOffsets.OrderBy(k => k.Key));
        Assert.Equal(140, RustProjectedLight.ExpectedFieldOffsets["raw_fields"] + 56);
        // request: struct_size 0, slot 4, guid 8, delta 24..36.
        Assert.Equal(36, 24 + 3 * sizeof(int));
    }

    // ---- delta 0 / bit preservation ----

    [NativeProjectionFact]
    public void ZeroDeltaKeepsExactBitsIncludingNegativeZeroAndNaN()
    {
        using var pool = OpenPayload(Payload());
        var source = pool.CaptureAny(4);
        var guid = Guid.NewGuid();
        var batch = pool.ProjectBatch([new RustLightProjectionRequest(guid, 4, 0, 0, 0)]);
        Assert.Equal(1, batch.Count);
        var item = batch[0];
        Assert.Equal(0, item.RequestIndex);
        Assert.Equal(guid, item.TargetGuid);
        Assert.Equal(4, item.SourceSlot);
        Assert.Equal(4, item.Snapshot.Slot);
        Assert.True(source.RawFields.SequenceEqual(item.Snapshot.RawFields));
        Assert.Equal((Bits(-0.0f), 0x7FA00001u, 0x7F800000u), item.Snapshot.PositionBits);
    }

    [NativeProjectionFact]
    public void PartialZeroDeltaLeavesNonFiniteAxesIntactAndChangesOnlyNonZeroAxis()
    {
        using var pool = OpenPayload(Payload());
        var source = pool.CaptureAny(4);
        // Only X (finite -0.0) has a delta; NaN Y and inf Z are untouched because their delta is 0.
        var r = pool.ProjectBatch([new RustLightProjectionRequest(Guid.Empty, 4, 65536, 0, 0)])[0].Snapshot;
        Assert.Equal(Bits(1.0f), RawU32(r, 4));
        Assert.Equal(0x7FA00001u, RawU32(r, 8));
        Assert.Equal(0x7F800000u, RawU32(r, 12));
        AssertRawExceptPosition(source.RawFields, r.RawFields, "slot 4");
    }

    // ---- identity ----

    [NativeProjectionFact]
    public void DuplicateGuidAndSlotAreIndependentAndOrdered()
    {
        using var pool = OpenPayload(Payload());
        var g = Guid.NewGuid();
        var batch = pool.ProjectBatch(
        [
            new RustLightProjectionRequest(g, 3, 65536, 0, 0),
            new RustLightProjectionRequest(g, 3, 131072, 0, 0),   // same (guid, slot)
            new RustLightProjectionRequest(g, 4, 0, 0, 0),         // same guid, other slot
            new RustLightProjectionRequest(Guid.NewGuid(), 3, 0, 0, 0),
        ]);
        Assert.Equal(4, batch.Count);
        for (int i = 0; i < 4; i++) Assert.Equal(i, batch[i].RequestIndex);
        Assert.Equal(Bits(2.5f), RawU32(batch[0].Snapshot, 4));
        Assert.Equal(Bits(3.5f), RawU32(batch[1].Snapshot, 4));
        Assert.Equal(Bits(-0.0f), RawU32(batch[2].Snapshot, 4));
        Assert.Equal(Bits(1.5f), RawU32(batch[3].Snapshot, 4));
        Assert.Equal(g, batch[2].TargetGuid);
        Assert.Equal(4, batch[2].SourceSlot);
        Assert.Equal(4, batch.Items.Count);
    }

    // ---- preservation of non-position fields ----

    [NativeProjectionFact]
    public void NonOneActiveRgbAndUnequalRadiiArePreserved()
    {
        using var pool = OpenPayload(Payload());
        var source = pool.CaptureAny(3);
        var p = pool.ProjectBatch([new RustLightProjectionRequest(Guid.NewGuid(), 3, 65536, -65536, 32768)])[0].Snapshot;
        Assert.Equal(7u, p.ActiveRaw);
        Assert.Equal(source.ColorBits, p.ColorBits);
        Assert.True(p.Color.X > 1f);
        Assert.NotEqual(p.EffectiveRadiusBits, p.BaselineRadiusBits);
        Assert.Equal(source.EffectiveRadiusBits, p.EffectiveRadiusBits);
        Assert.Equal(source.BaselineRadiusBits, p.BaselineRadiusBits);
        Assert.Equal(source.TypeParameterBits, p.TypeParameterBits);
        Assert.Equal(source.AgeRaw, p.AgeRaw); Assert.Equal(-1, p.AgeSigned);
        Assert.Equal(source.SpecialFxRaw, p.SpecialFxRaw); Assert.Equal(source.TypeRaw, p.TypeRaw);
        Assert.Equal((short)-5, p.OwnerObject); Assert.Equal((short)12, p.ParentObject); Assert.Equal((short)39, p.Definition);
        AssertRawExceptPosition(source.RawFields, p.RawFields, "slot 3");
        Assert.Equal((Bits(2.5f), Bits(-3.0f), Bits(0.5f)), p.PositionBits);
    }

    [NativeProjectionFact]
    public void NegativeFractionLandsAtExactRawOffsets()
    {
        using var pool = OpenPayload(Payload());
        int[] deltas = [-1, -32768, 1, int.MinValue, int.MaxValue];
        foreach (int d in deltas)
        {
            var r = pool.ProjectBatch([new RustLightProjectionRequest(Guid.Empty, 3, d, d, d)])[0].Snapshot;
            float Expect(float saved) => (float)((double)saved + (double)d / 65536.0);
            Assert.Equal(Bits(Expect(1.5f)), RawU32(r, 4));
            Assert.Equal(Bits(Expect(-2.0f)), RawU32(r, 8));
            Assert.Equal(Bits(Expect(0.0f)), RawU32(r, 12));
            Assert.Equal((RawU32(r, 4), RawU32(r, 8), RawU32(r, 12)), r.PositionBits);
        }
        var tiny = pool.ProjectBatch([new RustLightProjectionRequest(Guid.Empty, 3, 0, 0, -1)])[0].Snapshot;
        Assert.Equal(Bits(-1.52587890625e-5f), RawU32(tiny, 12));
    }

    // ---- batch atomicity / errors / lifecycle ----

    [NativeProjectionFact]
    public void MixedBadBatchFailsWholeWithIndexAndLeavesSourceUnchanged()
    {
        using var pool = OpenPayload(Payload());
        byte[] before = pool.CopyPayload();
        var good = new RustLightProjectionRequest(Guid.NewGuid(), 3, 65536, 0, 0);

        (RustLightProjectionRequest Bad, string Label)[] bads =
        [
            (new(Guid.NewGuid(), 9, 0, 0, 0), "inactive"),
            (new(Guid.NewGuid(), 5000, 0, 0, 0), "out of range"),
            (new(Guid.NewGuid(), 4, 0, 1, 0), "non-finite saved coordinate with non-zero delta"),
        ];
        foreach (var (bad, label) in bads)
        {
            var ex = Assert.Throws<InvalidDataException>(() => pool.ProjectBatch([good, good, bad, good]));
            Assert.True(ex.Message.Contains("請求索引 2"), $"{label}: {ex.Message}");
            Assert.True(before.AsSpan().SequenceEqual(pool.CopyPayload()), $"{label}: pool unchanged");
        }
        // Pool still fully usable after failures.
        Assert.Equal(Bits(2.5f), RawU32(pool.ProjectBatch([good])[0].Snapshot, 4));
        Assert.Equal(2, pool.ActiveCount); // Payload activates only slots 3 and 4.
    }

    [NativeProjectionFact]
    public void EmptyBatchSucceedsAndArgumentsAreChecked()
    {
        using var pool = OpenPayload(Payload());
        Assert.Equal(0, pool.ProjectBatch([]).Count);
        Assert.Throws<ArgumentNullException>(() => pool.ProjectBatch(null!));
        Assert.Throws<ArgumentException>(() => pool.ProjectBatch([null!]));
        Assert.Throws<ArgumentOutOfRangeException>(() => new RustLightProjectionRequest(Guid.Empty, -1, 0, 0, 0));
    }

    [NativeProjectionFact]
    public void DisposedPoolThrowsAndHandlesAreFreedOnce()
    {
        long opened = RustLightPool.OpenedHandleCount, freed = RustLightPool.FreedHandleCount;
        var pool = OpenPayload(Payload());
        var req = new RustLightProjectionRequest(Guid.NewGuid(), 3, 0, 0, 0);
        var batch = pool.ProjectBatch([req]);
        pool.Dispose();
        pool.Dispose();
        Assert.True(pool.IsDisposed);
        Assert.Throws<ObjectDisposedException>(() => pool.ProjectBatch([req]));
        Assert.Equal(opened + 1, RustLightPool.OpenedHandleCount);
        Assert.Equal(freed + 1, RustLightPool.FreedHandleCount);
        // Results are detached copies; they stay valid after dispose and mutation of a clone does not leak back.
        byte[] copy = batch[0].Snapshot.ToRawBytes();
        copy[4] ^= 0xFF;
        Assert.NotEqual(copy[4], batch[0].Snapshot.RawFields[4]);
        Assert.Equal(Bits(1.5f), RawU32(batch[0].Snapshot, 4));
    }

    [NativeProjectionFact]
    public void ProjectionDoesNotMutateSourceSlotSnapshots()
    {
        using var pool = OpenPayload(Payload());
        var s3 = pool.CaptureAny(3).ToRawBytes();
        var s4 = pool.CaptureAny(4).ToRawBytes();
        pool.ProjectBatch([new RustLightProjectionRequest(Guid.NewGuid(), 3, 12345, -777, 99), new RustLightProjectionRequest(Guid.NewGuid(), 4, 0, 0, 0)]);
        Assert.True(s3.AsSpan().SequenceEqual(pool.CaptureAny(3).RawFields));
        Assert.True(s4.AsSpan().SequenceEqual(pool.CaptureAny(4).RawFields));
    }
}
