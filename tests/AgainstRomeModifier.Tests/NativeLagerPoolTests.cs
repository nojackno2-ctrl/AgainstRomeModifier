using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeLagerPoolTests
{
    [Fact]
    public void Small_copy_preserves_unknown_bytes_and_remaps_sparse_members_and_separate_big_link()
    {
        byte[] source = Empty(false); int record = 16 + 3199 * 43;
        source[record] = 1; source[record + 15] = 0xA5;
        I16(source, record + 9, 800); I16(source, record + 41, 55);
        I16(source, 16 + 3200 * 43 + 3199 * 2, 31);
        var pool = NativeLagerPool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(3199)!;
        byte[] expected = source.ToArray(); source.AsSpan(record, 43).CopyTo(expected.AsSpan(16 + 43));
        I16(expected, 16 + 43 + 41, 13999); I16(expected, 16 + 3200 * 43 + 2, 0);
        Assert.Equal(new[] { 1 }, pool.AddCopies([new(snapshot, new Dictionary<int, int> { [55] = 13999 },
            new Dictionary<int, int> { [31] = 0 })], new HashSet<int> { 0 }));
        Assert.Equal(expected, pool.ToPayload());
    }

    [Fact]
    public void Small_copy_missing_big_mapping_leaves_complete_batch_unchanged()
    {
        byte[] source = Empty(false); int record = 16 + 3199 * 43;
        source[record] = 1; I16(source, 16 + 3200 * 43 + 3199 * 2, 31);
        var pool = NativeLagerPool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(3199)!;
        var valid = new NativeLagerCopy(snapshot, new Dictionary<int, int>(), new Dictionary<int, int> { [31] = 0 });
        var invalid = new NativeLagerCopy(snapshot, new Dictionary<int, int>(), new Dictionary<int, int>());
        Assert.Throws<InvalidDataException>(() => pool.AddCopies([valid, invalid]));
        Assert.Equal(source, pool.ToPayload());
    }

    [Fact]
    public void Big_copy_remaps_first_and_last_members_preserving_sparse_layout_and_source()
    {
        byte[] source = Empty(true); int record = 12 + 31 * 1601;
        source[record] = 1; I16(source, record + 1, 44); I16(source, record + 1599, 55);
        var pool = NativeBigLagerPool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(31)!;
        byte[] expected = source.ToArray(); source.AsSpan(record, 1601).CopyTo(expected.AsSpan(12 + 1601));
        I16(expected, 12 + 1601 + 1, 13999); I16(expected, 12 + 1601 + 1599, 0);
        Assert.Equal(new[] { 1 }, pool.AddCopies([new(snapshot, new Dictionary<int, int> { [44] = 13999, [55] = 0 })],
            new HashSet<int> { 0 }));
        Assert.Equal(expected, pool.ToPayload());
        Assert.Equal(source.AsSpan(record, 1601).ToArray(), snapshot.RecordBytes());
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public void Big_copy_late_invalid_shared_member_or_capacity_failure_preserves_pool(int failure)
    {
        byte[] source = Empty(true); int record = 12 + 31 * 1601;
        source[record] = 1; I16(source, record + 1599, 55);
        var pool = NativeBigLagerPool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(31)!;
        var first = new NativeBigLagerCopy(snapshot, new Dictionary<int, int> { [55] = 2 });
        var second = new NativeBigLagerCopy(snapshot, failure == 0 ? new Dictionary<int, int>()
            : new Dictionary<int, int> { [55] = failure == 1 ? 2 : 3 });
        if (failure == 2) Assert.Null(pool.AddCopies([first, second], Enumerable.Range(0, 31).ToHashSet()));
        else Assert.Throws<InvalidDataException>(() => pool.AddCopies([first, second]));
        Assert.Equal(source, pool.ToPayload());
    }

    private static byte[] Empty(bool big)
    {
        byte[] bytes = new byte[big ? 51244 : 144016];
        I32(bytes, 0, 1); I32(bytes, 4, big ? 32 : 3200); I32(bytes, 8, big ? 800 : 6);
        if (!big) I32(bytes, 12, 10);
        for (int slot = 0; slot < (big ? 32 : 3200); slot++)
        {
            if (big) bytes.AsSpan(12 + slot * 1601 + 1, 1600).Fill(255);
            else
            {
                bytes.AsSpan(16 + slot * 43 + 23, 20).Fill(255);
                I16(bytes, 16 + 3200 * 43 + slot * 2, -1);
            }
        }
        return bytes;
    }
    private static void I16(byte[] bytes, int offset, short value) => BinaryPrimitives.WriteInt16LittleEndian(bytes.AsSpan(offset), value);
    private static void I32(byte[] bytes, int offset, int value) => BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);

    [Theory]
    [InlineData(false, false)]
    [InlineData(false, true)]
    [InlineData(true, false)]
    [InlineData(true, true)]
    public void Last_slot_snapshot_release_and_rollback_preserve_other_slots_and_native_reset(bool big, bool wrapped)
    {
        string directory = Path.Combine(Path.GetTempPath(), "ArmLager_" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(directory);
        try
        {
            int slot = big ? 31 : 3199, record = big ? 12 + slot * 1601 : 16 + slot * 43;
            byte[] bytes = Empty(big); bytes[record] = 1;
            I16(bytes, record + (big ? 1 : 23), 44);
            I16(bytes, record + (big ? 1599 : 41), 55);
            if (!big) { I16(bytes, record + 9, 2); bytes[record + 15] = 0xA5;
                I16(bytes, 16 + 3200 * 43 + slot * 2, 31); }
            byte[] header = new byte[64]; "PFIL"u8.CopyTo(header); header[20] = 123;
            byte[] original = wrapped ? GameLZSS.CompressPfil(bytes, header) : bytes;
            string path = Path.Combine(directory, big ? "biglager.dat" : "lager.dat"); File.WriteAllBytes(path, original);
            if (big)
            {
                var pool = NativeBigLagerPool.Load(path); Assert.Equal(new[] { 44, 55 }, pool.Capture(slot)!.Members);
                var complete = pool.CaptureSnapshot(slot)!;
                byte[] expectedRecord = bytes.AsSpan(record, 1601).ToArray();
                Assert.Throws<ArgumentOutOfRangeException>(() => pool.ReleaseBatch([slot, 32])); Assert.Equal(bytes, pool.ToPayload());
                Assert.Single(pool.ReleaseBatch([slot, slot])); Assert.Equal(Empty(true), pool.ToPayload());
                Assert.Equal(expectedRecord, complete.RecordBytes());
                complete.RecordBytes()[0] = 0;
                Assert.Equal(expectedRecord, complete.RecordBytes());
                Assert.Null(pool.CaptureSnapshot(slot));
                Assert.Null(pool.CaptureSnapshot(32));
                using (var rollback = new FileRollbackScope()) pool.Save(path, rollback);
                Assert.Equal(original, File.ReadAllBytes(path));
                using (var rollback = new FileRollbackScope()) { pool.Save(path, rollback); rollback.Commit(); }
                Assert.Equal(Empty(true), NativeBigLagerPool.Load(path).ToPayload());
            }
            else
            {
                var pool = NativeLagerPool.Load(path); var snapshot = pool.Capture(slot)!;
                var complete = pool.CaptureSnapshot(slot)!;
                byte[] expectedRecord = bytes.AsSpan(record, 43).ToArray();
                Assert.Equal(2, snapshot.Occupancy); Assert.Equal(31, snapshot.BigLager); Assert.Equal(new[] { 44, 55 }, snapshot.SmallMembers);
                Assert.Throws<ArgumentOutOfRangeException>(() => pool.ReleaseBatch([slot, 3200])); Assert.Equal(bytes, pool.ToPayload());
                Assert.Single(pool.ReleaseBatch([slot, slot])); Assert.Equal(Empty(false), pool.ToPayload());
                Assert.Equal(expectedRecord, complete.RecordBytes());
                Assert.Equal(31, complete.BigLager);
                complete.RecordBytes()[0] = 0;
                Assert.Equal(expectedRecord, complete.RecordBytes());
                Assert.Null(pool.CaptureSnapshot(slot));
                Assert.Null(pool.CaptureSnapshot(-1));
                using (var rollback = new FileRollbackScope()) pool.Save(path, rollback);
                Assert.Equal(original, File.ReadAllBytes(path));
                using (var rollback = new FileRollbackScope()) { pool.Save(path, rollback); rollback.Commit(); }
                Assert.Equal(Empty(false), NativeLagerPool.Load(path).ToPayload());
            }
            if (wrapped) Assert.Equal(123, File.ReadAllBytes(path)[20]);
        }
        finally { Directory.Delete(directory, true); }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Malformed_layout_is_rejected_and_payload_copies_are_detached(bool big)
    {
        byte[] bytes = Empty(big);
        if (big)
        {
            Assert.Throws<InvalidDataException>(() => NativeBigLagerPool.ParsePayload(bytes.AsSpan(0, bytes.Length - 1)));
            I32(bytes, 8, 799); Assert.Throws<InvalidDataException>(() => NativeBigLagerPool.ParsePayload(bytes));
            bytes = Empty(true); var pool = NativeBigLagerPool.ParsePayload(bytes); bytes[12] = 1; pool.ToPayload()[12] = 1;
            Assert.Empty(pool.ActiveLagers()); Assert.Null(pool.Capture(-1)); Assert.Null(pool.Capture(32));
        }
        else
        {
            Assert.Throws<InvalidDataException>(() => NativeLagerPool.ParsePayload(bytes.AsSpan(0, bytes.Length - 1)));
            I32(bytes, 8, 7); Assert.Throws<InvalidDataException>(() => NativeLagerPool.ParsePayload(bytes));
            bytes = Empty(false); var pool = NativeLagerPool.ParsePayload(bytes); bytes[16] = 1; pool.ToPayload()[16] = 1;
            Assert.Empty(pool.ActiveLagers()); Assert.Null(pool.Capture(-1)); Assert.Null(pool.Capture(3200));
        }
    }
}
