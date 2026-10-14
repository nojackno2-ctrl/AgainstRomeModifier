using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>Attribute: real FFI particle projection tests; never skipped when ARM_REQUIRE_NATIVE_DATA=1.</summary>
internal sealed class NativeParticleProjectionFactAttribute : FactAttribute
{
    public NativeParticleProjectionFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustNativeData.TryEnsureLoaded(out string? why)) { Skip = "against_rome_data 原生庫不可用：" + why; return; }
        if (!RustParticleLightProjection.TryEnsureLoaded(out string? pwhy)) Skip = "粒子投影 ABI 不可用：" + pwhy;
    }
}

/// <summary>Real FFI tests (no mocks) of the C# particle-linked light bridge. Numeric expectations are independent test oracles.</summary>
[Collection("Rust native interop")]
public sealed class RustParticleLightProjectionTests
{
    private static readonly int[] Widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
    private const int Capacity = 1024, PayloadLength = 57352, RecordSize = 2252;

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

    private static void SetLight(byte[] p, int slot, uint active, float x, float y, float z)
    {
        PutU32(p, 0, slot, active); PutF(p, 1, slot, x); PutF(p, 2, slot, y); PutF(p, 3, slot, z);
        PutF(p, 4, slot, 2.5f); PutF(p, 5, slot, 0.25f); PutF(p, 6, slot, 0.5f);
        PutF(p, 7, slot, 300f); PutF(p, 8, slot, 450f);
        PutS(p, 9, slot, 0xFFFE); PutF(p, 10, slot, 0.15f); PutU32(p, 11, slot, 0xFFFFFFFF); PutU32(p, 12, slot, 0xDEADBEEF);
        PutS(p, 13, slot, unchecked((ushort)-5)); PutS(p, 14, slot, 12); PutS(p, 15, slot, 39);
    }

    /// <summary>Lights: slot 3 active, slot 4 active, slot 9 inactive; saved XYZ deliberately unlike any fixed-derived value.</summary>
    private static byte[] Payload()
    {
        byte[] p = EmptyPayload();
        SetLight(p, 3, 7, 9.5f, -8.25f, 77.75f);
        SetLight(p, 4, 1, -0.0f, 123.5f, 4.5f);
        SetLight(p, 9, 0, 1f, 2f, 3f);
        return p;
    }

    private static int ChildBase(int ordinal) => 12 + 35 * ordinal;

    /// <summary>Emitter record: u16 active at 0; child c: active byte +0, fixed i32 x/y/z +1/+5/+9, light link i16 +33.</summary>
    private static byte[] Record(bool emitterActive = true)
    {
        byte[] r = new byte[RecordSize];
        BinaryPrimitives.WriteUInt16LittleEndian(r, (ushort)(emitterActive ? 1 : 0));
        for (int i = 0; i < r.Length; i++) if (i >= 2) r[i] = (byte)(i * 31 + 7); // non-zero noise, rewritten per child below
        for (int c = 0; c < 64; c++) { SetChild(r, c, active: 0, 0, 0, 0, link: -1); }
        return r;
    }

    private static void SetChild(byte[] r, int ordinal, byte active, int x, int y, int z, short link)
    {
        int b = ChildBase(ordinal);
        r[b] = active;
        BinaryPrimitives.WriteInt32LittleEndian(r.AsSpan(b + 1), x);
        BinaryPrimitives.WriteInt32LittleEndian(r.AsSpan(b + 5), y);
        BinaryPrimitives.WriteInt32LittleEndian(r.AsSpan(b + 9), z);
        BinaryPrimitives.WriteInt16LittleEndian(r.AsSpan(b + 33), link);
    }

    private static RustLightPool OpenPayload(byte[] p) => RustLightPool.Open(p);

    private static float ExpectedPos(int fixedValue, int delta) => (int)(((long)fixedValue + delta) >> 16);


    private static RustParticleProjectionException Rejects(RustLightPool pool, params RustParticleLightRequest[] requests)
        => Assert.Throws<RustParticleProjectionException>(() => pool.ProjectParticleBatch(requests));

    // ---- ABI ----

    [NativeParticleProjectionFact]
    public void AbiSizesMatchHeader()
    {
        Assert.True(RustParticleLightProjection.TryEnsureLoaded(out string? error), error);
        Assert.Equal((44u, 160u), RustParticleLightProjection.NativeStructSizes);
        Assert.Equal(1u, RustNativeData.NativeAbiVersion);
        Assert.Equal(140, RustParticleLightProjection.ExpectedFieldOffsets["raw_fields"] + 56);
        Assert.Equal(160, RustParticleLightProjection.ExpectedFieldOffsets["fixed_position"] + 12);
        Assert.Equal(44, RustParticleLightProjection.ExpectedRequestOffsets["delta_fixed"] + 12);
        Assert.Equal(12 + 64 * 35, RecordSize);
    }

    // ---- active child ----

    [NativeParticleProjectionFact]
    public void ZeroDeltaActiveChildDerivesFromFixedAndNegativeFractionIsArithmeticShift()
    {
        using var pool = OpenPayload(Payload());
        var source = pool.CaptureAny(3);
        byte[] record = Record();
        SetChild(record, 5, 1, -65537, 3 * 65536 + 5, -1, 3);
        var guid = Guid.NewGuid();
        var batch = pool.ProjectParticleBatch([new RustParticleLightRequest(guid, record, 5, 3, 0, 0, 0)]);

        Assert.Equal(1, batch.Count);
        var item = batch[0];
        Assert.Equal(0, item.RequestIndex);
        Assert.Equal(guid, item.TargetGuid);
        Assert.Equal(3, item.SourceSlot);
        Assert.Equal(5, item.ChildOrdinal);
        Assert.True(item.ChildActive);
        Assert.Equal((-65537, 3 * 65536 + 5, -1), item.FixedPosition);
        Assert.Equal(Bits(-2f), item.Snapshot.PositionBits.X);   // -65537 SAR 16 = -2 (not -1)
        Assert.Equal(Bits(3f), item.Snapshot.PositionBits.Y);
        Assert.Equal(Bits(-1f), item.Snapshot.PositionBits.Z);
        Assert.NotEqual(source.PositionBits, item.Snapshot.PositionBits); // not the saved light bits
        Assert.True(source.RawFields[..4].SequenceEqual(item.Snapshot.RawFields[..4]));
        Assert.True(source.RawFields[16..].SequenceEqual(item.Snapshot.RawFields[16..]));
    }

    [NativeParticleProjectionFact]
    public void DeltaIsAddedInFixedPointBeforeShift()
    {
        using var pool = OpenPayload(Payload());
        byte[] record = Record();
        SetChild(record, 0, 1, 10 * 65536, -5, 65535, 4);
        int dx = -3 * 65536 - 1, dy = 65535 + 2, dz = 1;
        var item = pool.ProjectParticleBatch([new RustParticleLightRequest(Guid.Empty, record, 0, 4, dx, dy, dz)])[0];
        Assert.Equal(Bits(ExpectedPos(10 * 65536, dx)), item.Snapshot.PositionBits.X);
        Assert.Equal(Bits(ExpectedPos(-5, dy)), item.Snapshot.PositionBits.Y);
        Assert.Equal(Bits(ExpectedPos(65535, dz)), item.Snapshot.PositionBits.Z);
        Assert.Equal((10 * 65536 + dx, -5 + dy, 65535 + dz), item.FixedPosition);
    }

    [NativeParticleProjectionFact]
    public void InactiveChildKeepsSavedLightBytesUntouched()
    {
        using var pool = OpenPayload(Payload());
        var source = pool.CaptureAny(3);
        byte[] record = Record();
        SetChild(record, 2, 0, 5 * 65536, 6 * 65536, 7 * 65536, 3);
        var item = pool.ProjectParticleBatch([new RustParticleLightRequest(Guid.NewGuid(), record, 2, 3, 65536, 65536, 65536)])[0];
        Assert.False(item.ChildActive);
        Assert.Equal(0u, item.ChildActiveRaw);
        Assert.True(source.RawFields.SequenceEqual(item.Snapshot.RawFields));
    }

    // ---- identity ----

    [NativeParticleProjectionFact]
    public void DuplicateInstancesAreIndependentOrderedAndNotMerged()
    {
        using var pool = OpenPayload(Payload());
        byte[] bytes = Record();
        SetChild(bytes, 1, 1, 65536 * 2, 65536 * 3, 65536 * 4, 3);
        var shared = RustParticleEmitterRecord.Copy(bytes);
        var other = RustParticleEmitterRecord.Copy(bytes); // equal bytes, distinct instance
        var g1 = Guid.NewGuid();
        var g2 = Guid.NewGuid();
        var batch = pool.ProjectParticleBatch(
        [
            new RustParticleLightRequest(g1, shared, 1, 3, 0, 0, 0),
            new RustParticleLightRequest(g2, shared, 1, 3, 0, 0, 0),
            new RustParticleLightRequest(g1, other, 1, 3, 0, 0, 0),
            new RustParticleLightRequest(g1, shared, 1, 3, 65536, 0, 0),
        ]);
        Assert.Equal(4, batch.Count);
        Assert.Equal(new[] { g1, g2, g1, g1 }, batch.Items.Select(i => i.TargetGuid));
        Assert.Equal(new[] { 0, 1, 2, 3 }, batch.Items.Select(i => i.RequestIndex));
        Assert.Equal(Bits(2f), batch[0].Snapshot.PositionBits.X);
        Assert.Equal(batch[0].Snapshot.RawFields.ToArray(), batch[1].Snapshot.RawFields.ToArray());
        Assert.Equal(batch[0].Snapshot.RawFields.ToArray(), batch[2].Snapshot.RawFields.ToArray());
        Assert.Equal(Bits(3f), batch[3].Snapshot.PositionBits.X);
    }

    [NativeParticleProjectionFact]
    public void MixedSlotsAndRecordsKeepRequestOrder()
    {
        using var pool = OpenPayload(Payload());
        byte[] a = Record(), b = Record();
        SetChild(a, 0, 1, 65536, 65536, 65536, 4);
        SetChild(b, 63, 1, 2 * 65536, 2 * 65536, 2 * 65536, 3);
        var batch = pool.ProjectParticleBatch(
        [
            new RustParticleLightRequest(Guid.NewGuid(), b, 63, 3, 0, 0, 0),
            new RustParticleLightRequest(Guid.NewGuid(), a, 0, 4, 0, 0, 0),
        ]);
        Assert.Equal(3, batch[0].SourceSlot);
        Assert.Equal(63, batch[0].ChildOrdinal);
        Assert.Equal(Bits(2f), batch[0].Snapshot.PositionBits.X);
        Assert.Equal(4, batch[1].SourceSlot);
        Assert.Equal(Bits(1f), batch[1].Snapshot.PositionBits.X);
    }

    [NativeParticleProjectionFact]
    public void EmptyBatchReturnsEmptyResult()
    {
        using var pool = OpenPayload(Payload());
        Assert.Equal(0, pool.ProjectParticleBatch([]).Count);
    }

    // ---- native failures (atomic, nothing partial, sources unchanged) ----

    [NativeParticleProjectionFact]
    public void CheckedFixedOverflowFailsWithCodeAndIndexAndNothingPartial()
    {
        byte[] payload = Payload();
        using var pool = OpenPayload(payload);
        byte[] good = Record(), bad = Record();
        SetChild(good, 0, 1, 65536, 65536, 65536, 3);
        SetChild(bad, 0, 1, int.MaxValue, 0, 0, 3);
        byte[] goodBefore = (byte[])good.Clone(), badBefore = (byte[])bad.Clone();
        byte[] poolBefore = pool.CopyPayload();

        var ex = Rejects(pool,
            new RustParticleLightRequest(Guid.NewGuid(), good, 0, 3, 0, 0, 0),
            new RustParticleLightRequest(Guid.NewGuid(), bad, 0, 3, 1, 0, 0));
        Assert.Equal(RustParticleLightProjection.CodeFixedOverflow, ex.Code);
        Assert.Equal(1, ex.FailedRequestIndex);
        AssertUnchanged(pool, payload, poolBefore);
        Assert.Equal(goodBefore, good);
        Assert.Equal(badBefore, bad);
        // negative overflow too
        SetChild(bad, 0, 1, int.MinValue, 0, 0, 3);
        Assert.Equal(RustParticleLightProjection.CodeFixedOverflow,
            Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), bad, 0, 3, -1, 0, 0)).Code);
        // pool still serves a good batch afterwards
        Assert.Equal(Bits(1f), pool.ProjectParticleBatch([new RustParticleLightRequest(Guid.NewGuid(), good, 0, 3, 0, 0, 0)])[0].Snapshot.PositionBits.X);
    }

    [NativeParticleProjectionFact]
    public void LinkMismatchIsRejected()
    {
        using var pool = OpenPayload(Payload());
        byte[] record = Record();
        SetChild(record, 0, 1, 0, 0, 0, 4); // links slot 4, request says 3
        var ex = Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), record, 0, 3, 0, 0, 0));
        Assert.Equal(RustParticleLightProjection.CodeLightLinkMismatch, ex.Code);
        Assert.Equal(0, ex.FailedRequestIndex);
    }

    [NativeParticleProjectionFact]
    public void InactiveEmitterIsRejected()
    {
        using var pool = OpenPayload(Payload());
        byte[] record = Record(emitterActive: false);
        SetChild(record, 0, 1, 0, 0, 0, 3);
        var ex = Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), record, 0, 3, 0, 0, 0));
        Assert.Equal(RustParticleLightProjection.CodeInactiveEmitter, ex.Code);
        Assert.Equal(0, ex.FailedRequestIndex);
    }

    [NativeParticleProjectionFact]
    public void BadChildOrdinalIsRejected()
    {
        using var pool = OpenPayload(Payload());
        var ex = Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), Record(), 64, 3, 0, 0, 0));
        Assert.Equal(RustParticleLightProjection.CodeInvalidChild, ex.Code);
    }

    [NativeParticleProjectionFact]
    public void InactiveLightSlotIsRejected()
    {
        using var pool = OpenPayload(Payload());
        byte[] record = Record();
        SetChild(record, 0, 1, 0, 0, 0, 9); // slot 9 is inactive in the pool
        var ex = Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), record, 0, 9, 0, 0, 0));
        Assert.Equal(RustParticleLightProjection.CodeInactiveSlot, ex.Code);
    }

    [NativeParticleProjectionFact]
    public void OutOfRangeLightSlotIsRejected()
    {
        using var pool = OpenPayload(Payload());
        byte[] record = Record();
        SetChild(record, 0, 1, 0, 0, 0, 1024);
        var ex = Rejects(pool, new RustParticleLightRequest(Guid.NewGuid(), record, 0, 1024, 0, 0, 0));
        Assert.Contains(ex.Code, new[] { RustParticleLightProjection.CodeInvalidSlot, RustParticleLightProjection.CodeLightLinkMismatch });
    }

    [NativeParticleProjectionFact]
    public void BadRequestAfterGoodOnesLeavesPoolAndRecordsUnchanged()
    {
        byte[] payload = Payload();
        using var pool = OpenPayload(payload);
        byte[] poolBefore = pool.CopyPayload();
        byte[] good = Record();
        SetChild(good, 7, 1, 65536, 65536, 65536, 3);
        byte[] before = (byte[])good.Clone();
        Rejects(pool,
            new RustParticleLightRequest(Guid.NewGuid(), good, 7, 3, 65536, 0, 0),
            new RustParticleLightRequest(Guid.NewGuid(), good, 7, 3, 0, 0, 0),
            new RustParticleLightRequest(Guid.NewGuid(), good, 8, 3, 0, 0, 0)); // child 8 links -1
        AssertUnchanged(pool, payload, poolBefore);
        Assert.Equal(before, good);
        Assert.Equal(2, pool.ActiveCount); // slots 3 and 4 only
    }

    private static void AssertUnchanged(RustLightPool pool, byte[] payload, byte[] poolBefore)
    {
        Assert.Equal(poolBefore, pool.CopyPayload());
        Assert.Equal(payload, pool.CopyPayload());
        Assert.Equal(Bits(9.5f), pool.CaptureAny(3).PositionBits.X);
    }

    // ---- request/record construction (no native) ----

    [Fact]
    public void RecordAndRequestValidationNeedNoNative()
    {
        Assert.Throws<ArgumentException>(() => RustParticleEmitterRecord.Copy(new byte[RecordSize - 1]));
        Assert.Throws<ArgumentException>(() => RustParticleEmitterRecord.Copy(new byte[RecordSize + 1]));
        var rec = RustParticleEmitterRecord.Copy(new byte[RecordSize]);
        Assert.Throws<ArgumentNullException>(() => new RustParticleLightRequest(Guid.Empty, (RustParticleEmitterRecord)null!, 0, 0, 0, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => new RustParticleLightRequest(Guid.Empty, rec, -1, 0, 0, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => new RustParticleLightRequest(Guid.Empty, rec, 0, -1, 0, 0, 0));
    }

    [Fact]
    public void RecordIsDefensivelyCopied()
    {
        byte[] bytes = new byte[RecordSize];
        bytes[100] = 5;
        var rec = RustParticleEmitterRecord.Copy(bytes);
        bytes[100] = 9;
        Assert.Equal(5, rec.Bytes[100]);
        var copy = rec.ToBytes();
        copy[100] = 1;
        Assert.Equal(5, rec.Bytes[100]);
    }

    [Fact]
    public void RequestWritesTheHeader44ByteLayout()
    {
        var guid = Guid.NewGuid();
        var request = new RustParticleLightRequest(guid, new byte[RecordSize], 7, 513, -1, int.MinValue, int.MaxValue);
        byte[] d = new byte[44];
        WriteTo(request, d, 3);
        Assert.Equal(44u, BinaryPrimitives.ReadUInt32LittleEndian(d));
        Assert.Equal(3u, BinaryPrimitives.ReadUInt32LittleEndian(d.AsSpan(4)));
        Assert.Equal(7u, BinaryPrimitives.ReadUInt32LittleEndian(d.AsSpan(8)));
        Assert.Equal(513u, BinaryPrimitives.ReadUInt32LittleEndian(d.AsSpan(12)));
        Assert.Equal(guid, new Guid(d.AsSpan(16, 16), bigEndian: false));
        Assert.Equal(-1, BinaryPrimitives.ReadInt32LittleEndian(d.AsSpan(32)));
        Assert.Equal(int.MinValue, BinaryPrimitives.ReadInt32LittleEndian(d.AsSpan(36)));
        Assert.Equal(int.MaxValue, BinaryPrimitives.ReadInt32LittleEndian(d.AsSpan(40)));
    }

    // The test assembly has no InternalsVisibleTo; reach the two internal members by reflection.
    private static RustParticleProjectedLight Parse(byte[] s, int index, RustParticleLightRequest request)
    {
        return RustParticleProjectedLight.Parse(s, index, request);
    }

    private static void WriteTo(RustParticleLightRequest request, byte[] d, uint packedIndex)
        => request.WriteTo(d, packedIndex);

    // ---- output validation (hand-built 160-byte elements; no native) ----

    private static byte[] FakeOutput(Guid guid, int index, int slot, int ordinal)
    {
        byte[] s = new byte[160];
        BinaryPrimitives.WriteUInt32LittleEndian(s, 160);
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(4), (uint)index);
        guid.TryWriteBytes(s.AsSpan(8, 16), bigEndian: false, out _);
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(24), (uint)slot);
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(28), 1);
        BinaryPrimitives.WriteSingleLittleEndian(s.AsSpan(32), 1.5f);
        // raw_fields@84 mirror: active(0) x(4) ... only the typed views we set must agree
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(84), 1);
        BinaryPrimitives.WriteSingleLittleEndian(s.AsSpan(88), 1.5f);
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(140), (uint)ordinal);
        BinaryPrimitives.WriteUInt32LittleEndian(s.AsSpan(144), 1);
        return s;
    }

    [Fact]
    public void ParseAcceptsConsistentAndRejectsInconsistentOutput()
    {
        var guid = Guid.NewGuid();
        var request = new RustParticleLightRequest(guid, new byte[RecordSize], 2, 6, 0, 0, 0);
        var ok = Parse(FakeOutput(guid, 4, 6, 2), 4, request);
        Assert.Equal((4, 6, 2), (ok.RequestIndex, ok.SourceSlot, ok.ChildOrdinal));
        Assert.True(ok.ChildActive);
        Assert.Equal(1.5f, ok.Snapshot.Position.X);

        Assert.Throws<InvalidDataException>(() => Parse(FakeOutput(guid, 5, 6, 2), 4, request));       // request_index
        Assert.Throws<InvalidDataException>(() => Parse(FakeOutput(Guid.NewGuid(), 4, 6, 2), 4, request)); // guid
        Assert.Throws<InvalidDataException>(() => Parse(FakeOutput(guid, 4, 7, 2), 4, request));       // slot
        Assert.Throws<InvalidDataException>(() => Parse(FakeOutput(guid, 4, 6, 3), 4, request));       // child ordinal
        byte[] typed = FakeOutput(guid, 4, 6, 2);
        BinaryPrimitives.WriteSingleLittleEndian(typed.AsSpan(32), 2.5f);                                                          // typed != raw
        Assert.Throws<InvalidDataException>(() => Parse(typed, 4, request));
        byte[] size = FakeOutput(guid, 4, 6, 2);
        BinaryPrimitives.WriteUInt32LittleEndian(size, 140);
        Assert.Throws<InvalidDataException>(() => Parse(size, 4, request));
        byte[] childRaw = FakeOutput(guid, 4, 6, 2);
        BinaryPrimitives.WriteUInt32LittleEndian(childRaw.AsSpan(144), 256);
        Assert.Throws<InvalidDataException>(() => Parse(childRaw, 4, request));
    }

    // ---- lifetime ----

    [NativeParticleProjectionFact]
    public void DoubleDisposeIsSafeAndUseAfterDisposeThrows()
    {
        long opened = RustLightPool.OpenedHandleCount;
        var pool = OpenPayload(Payload());
        byte[] record = Record();
        SetChild(record, 0, 1, 0, 0, 0, 3);
        var request = new RustParticleLightRequest(Guid.NewGuid(), record, 0, 3, 0, 0, 0);
        Assert.Equal(1, pool.ProjectParticleBatch([request]).Count);
        pool.Dispose();
        pool.Dispose();
        Assert.True(pool.IsDisposed);
        Assert.Throws<ObjectDisposedException>(() => pool.ProjectParticleBatch([request]));
        Assert.Throws<ObjectDisposedException>(() => pool.ProjectParticleBatch([]));
        Assert.True(RustLightPool.OpenedHandleCount > opened);
    }

    [Fact]
    public void NullArgumentsAreRejected()
    {
        // No native needed: argument validation precedes the handle use only for null; a real pool is required otherwise.
        if (!RustNativeData.TryEnsureLoaded(out _)) return;
        using var pool = OpenPayload(EmptyPayload());
        Assert.Throws<ArgumentNullException>(() => pool.ProjectParticleBatch(null!));
        Assert.Throws<ArgumentException>(() => pool.ProjectParticleBatch(new RustParticleLightRequest[] { null! }));
    }
}
