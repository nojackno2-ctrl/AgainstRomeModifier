using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeParticlePoolTests
{
    private static byte[] Empty()
    {
        byte[] bytes = new byte[NativeParticlePool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, 1);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(4), 1024);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(8), 64);
        for (int slot = 0; slot < 1024; slot++)
        {
            I16(bytes, Record(slot) + 6, -1);
            for (int child = 0; child < 64; child++) I16(bytes, Record(slot) + 12 + child * 35 + 33, -1);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(Array(3, slot)), 30);
        }
        return bytes;
    }
    private static int Record(int slot) => 12 + slot * 2252;
    private static int Array(int column, int slot) => 12 + 1024 * 2252 + column * 4096 + slot * 4;
    private static void I16(byte[] bytes, int offset, short value) => BinaryPrimitives.WriteInt16LittleEndian(bytes.AsSpan(offset), value);

    [Fact]
    public void Copy_remaps_inactive_last_child_and_preserves_every_other_saved_byte()
    {
        byte[] source = Empty(); int offset = Record(1023);
        I16(source, offset, 1); I16(source, offset + 6, 42);
        source[offset + 12 + 63 * 35 + 19] = 0xB7;
        I16(source, offset + 12 + 63 * 35 + 33, 7);
        BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Array(2, 1023)), -12345);
        var pool = NativeParticlePool.ParsePayload(source);
        var snapshot = pool.CaptureSnapshot(1023)!;
        byte[] expected = source.ToArray();
        source.AsSpan(offset, 2252).CopyTo(expected.AsSpan(Record(1)));
        I16(expected, Record(1) + 6, 13999);
        I16(expected, Record(1) + 12 + 63 * 35 + 33, 1023);
        for (int column = 0; column < 4; column++)
            source.AsSpan(Array(column, 1023), 4).CopyTo(expected.AsSpan(Array(column, 1)));
        Assert.Equal(new[] { 1 }, pool.AddCopies([new(snapshot, 13999, new Dictionary<int, int> { [7] = 1023 })],
            new HashSet<int> { 0 }));
        Assert.Equal(expected, pool.ToPayload());
        Assert.Equal(new[] { 7 }, snapshot.LightSlots);
    }

    [Fact]
    public void Copy_batch_late_invalid_mapping_and_capacity_failure_leave_pool_unchanged()
    {
        byte[] source = Empty(); I16(source, Record(1023), 1);
        I16(source, Record(1023) + 12 + 63 * 35 + 33, 7);
        var pool = NativeParticlePool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(1023)!;
        var valid = new NativeParticleCopy(snapshot, 2, new Dictionary<int, int> { [7] = 9 });
        var invalid = new NativeParticleCopy(snapshot, 3, new Dictionary<int, int>());
        Assert.Throws<InvalidDataException>(() => pool.AddCopies([valid, invalid]));
        Assert.Equal(source, pool.ToPayload());
        var another = new NativeParticleCopy(snapshot, 3, new Dictionary<int, int> { [7] = 10 });
        Assert.Null(pool.AddCopies([valid, another], Enumerable.Range(0, 1023).ToHashSet()));
        Assert.Equal(source, pool.ToPayload());
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void Copy_batch_rejects_duplicate_owner_or_cross_emitter_light_before_writes(bool duplicateOwner)
    {
        byte[] source = Empty(); I16(source, Record(1023), 1);
        I16(source, Record(1023) + 12 + 63 * 35 + 33, 7);
        var pool = NativeParticlePool.ParsePayload(source); var snapshot = pool.CaptureSnapshot(1023)!;
        var first = new NativeParticleCopy(snapshot, 2, new Dictionary<int, int> { [7] = 9 });
        var second = new NativeParticleCopy(snapshot, duplicateOwner ? 2 : 3,
            new Dictionary<int, int> { [7] = duplicateOwner ? 10 : 9 });
        Assert.Throws<InvalidDataException>(() => pool.AddCopies([first, second]));
        Assert.Equal(source, pool.ToPayload());
    }

    [Fact]
    public void Snapshot_preserves_all_saved_children_and_parallel_state_after_source_release()
    {
        byte[] source = Empty(); int offset = Record(1023);
        I16(source, offset, 1); I16(source, offset + 6, 13999);
        // Current count stays zero; even inactive child 63 retains an allocated light.
        source[offset + 12] = 1;
        I16(source, offset + 12 + 63 * 35 + 33, 1023);
        source[offset + 12 + 63 * 35 + 19] = 0xA7;
        int[] expectedState = [int.MinValue, 123, -42, int.MaxValue];
        for (int column = 0; column < 4; column++)
            BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Array(column, 1023)), expectedState[column]);
        byte[] expectedRecord = source.AsSpan(offset, 2252).ToArray();
        var pool = NativeParticlePool.ParsePayload(source);
        var snapshot = pool.CaptureSnapshot(1023)!;
        pool.ReleaseBatch([1023]);
        Assert.Equal(expectedRecord, snapshot.RecordBytes());
        Assert.Equal(expectedState, snapshot.StateWords());
        Assert.Equal(13999, snapshot.OwnerObject);
        Assert.Equal(new[] { 1023 }, snapshot.LightSlots);
        snapshot.RecordBytes()[0] = 0;
        snapshot.StateWords()[0] = 0;
        Assert.Equal(expectedRecord, snapshot.RecordBytes());
        Assert.Equal(expectedState, snapshot.StateWords());
        Assert.Null(pool.CaptureSnapshot(1023));
        Assert.Null(pool.CaptureSnapshot(-1));
        Assert.Null(pool.CaptureSnapshot(1024));
    }

    [RequiresMarkerGameFact]
    public void Original_particle_light_translation_preserves_other_bytes_and_two_file_rollback()
    {
        string game = Environment.GetEnvironmentVariable("ARM_GAME_PATH")!;
        string? sourcePath = null; int selected = -1;
        foreach (string path in Directory.GetFiles(Path.Combine(game, "MAPS"), "particle.dat", SearchOption.AllDirectories).Order())
        {
            byte[] payload = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
            for (int slot = 0; slot < 1024 && selected < 0; slot++)
            {
                if (BinaryPrimitives.ReadUInt16LittleEndian(payload.AsSpan(Record(slot))) == 0) continue;
                for (int child = 0; child < 64; child++)
                {
                    int offset = Record(slot) + 12 + child * 35;
                    if (payload[offset] != 0 && BinaryPrimitives.ReadInt16LittleEndian(payload.AsSpan(offset + 33)) >= 0)
                    { selected = slot; sourcePath = path; break; }
                }
            }
            if (sourcePath is not null) break;
        }
        Assert.NotNull(sourcePath);
        string lightSource = Path.Combine(Path.GetDirectoryName(sourcePath)!, "light.dat");
        byte[] originalParticle = File.ReadAllBytes(sourcePath), originalLight = File.ReadAllBytes(lightSource);
        string root = Path.Combine(Path.GetTempPath(), "ArmOriginalParticleMove_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            string particlePath = Path.Combine(root, "particle.dat"), lightPath = Path.Combine(root, "light.dat");
            File.WriteAllBytes(particlePath, originalParticle); File.WriteAllBytes(lightPath, originalLight);
            var particles = NativeParticlePool.Load(particlePath); var lights = NativeLightPool.Load(lightPath);
            byte[] expectedParticle = particles.ToPayload(), expectedLight = lights.ToPayload();
            int affected = 0;
            for (int child = 0; child < 64; child++)
            {
                int offset = Record(selected) + 12 + child * 35;
                if (expectedParticle[offset] == 0) continue;
                int[] xyz = [checked(BinaryPrimitives.ReadInt32LittleEndian(expectedParticle.AsSpan(offset + 1)) + 65536),
                    checked(BinaryPrimitives.ReadInt32LittleEndian(expectedParticle.AsSpan(offset + 5)) - 32768),
                    checked(BinaryPrimitives.ReadInt32LittleEndian(expectedParticle.AsSpan(offset + 9)) + 1)];
                int light = BinaryPrimitives.ReadInt16LittleEndian(expectedParticle.AsSpan(offset + 33));
                for (int axis = 0; axis < 3; axis++)
                {
                    BinaryPrimitives.WriteInt32LittleEndian(expectedParticle.AsSpan(offset + 1 + axis * 4), xyz[axis]);
                    if (light >= 0) BinaryPrimitives.WriteSingleLittleEndian(expectedLight.AsSpan(8 + (axis + 1) * 4096 + light * 4), xyz[axis] >> 16);
                }
                if (light >= 0) affected++;
            }
            Assert.True(affected > 0);
            Assert.Equal(affected, particles.TranslateBatchWithLightsFixed([selected], 65536, -32768, 1, lights).Count);
            Assert.Equal(expectedParticle, particles.ToPayload()); Assert.Equal(expectedLight, lights.ToPayload());
            Assert.Throws<IOException>((Action)(() =>
            {
                using var rollback = new FileRollbackScope();
                particles.Save(particlePath, rollback); lights.Save(lightPath, rollback);
                throw new IOException("Injected later transaction failure");
            }));
            Assert.Equal(originalParticle, File.ReadAllBytes(particlePath)); Assert.Equal(originalLight, File.ReadAllBytes(lightPath));
            using (var rollback = new FileRollbackScope()) { particles.Save(particlePath, rollback); lights.Save(lightPath, rollback); rollback.Commit(); }
            Assert.Equal(expectedParticle, NativeParticlePool.Load(particlePath).ToPayload());
            Assert.Equal(expectedLight, NativeLightPool.Load(lightPath).ToPayload());
            foreach (var pair in new[] { (Original: originalParticle, Path: particlePath), (Original: originalLight, Path: lightPath) })
                if (pair.Original.AsSpan().StartsWith("PFIL"u8)) Assert.Equal(pair.Original.AsSpan(16, 48).ToArray(), File.ReadAllBytes(pair.Path).AsSpan(16, 48).ToArray());
        }
        finally { Directory.Delete(root, true); }
        Assert.Equal(originalParticle, File.ReadAllBytes(sourcePath)); Assert.Equal(originalLight, File.ReadAllBytes(lightSource));
    }

    [Theory]
    [InlineData("success")]
    [InlineData("missing-light")]
    [InlineData("shared-conflict")]
    [InlineData("particle-overflow")]
    public void Particle_and_light_translation_is_atomic_across_both_pools(string outcome)
    {
        byte[] source = Empty(); I16(source, Record(0), 1); I16(source, Record(1023), 1);
        int first = Record(0) + 12, last = Record(1023) + 12 + 63 * 35;
        source[first] = 1; source[last] = 1;
        BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(first + 1), -32768);
        BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(last + 1), outcome == "particle-overflow" ? int.MaxValue : 131072);
        I16(source, first + 33, 0); I16(source, last + 33, outcome == "shared-conflict" ? (short)0 : (short)1);
        var particles = NativeParticlePool.ParsePayload(source);
        var lights = NativeLightPool.ParsePayload(NativeLightPoolTests.EmptyPool());
        var definition = new NativeLightCreation(new(4, 5, 6), new(.1f, .2f, .3f), 7, 2, .4f, 5, 6);
        lights.AddBatch(outcome == "missing-light" ? [definition] : [definition, definition]);
        byte[] lightBefore = lights.ToPayload();
        if (outcome == "success")
        {
            var result = particles.TranslateBatchWithLightsFixed([0, 1023], 65536, -1, 32768, lights);
            Assert.Equal(2, result.Count);
            Assert.Equal(new System.Numerics.Vector3(0, -1, 0), lights.Capture(0)!.Position);
            Assert.Equal(new System.Numerics.Vector3(3, -1, 0), lights.Capture(1)!.Position);
            byte[] expected = source.ToArray();
            foreach (int offset in new[] { first, last })
            {
                BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 1), offset == first ? 32768 : 196608);
                BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 5), -1);
                BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 9), 32768);
            }
            Assert.Equal(expected, particles.ToPayload());
            byte[] expectedLight = lightBefore.ToArray();
            for (int slot = 0; slot < 2; slot++)
            {
                BinaryPrimitives.WriteSingleLittleEndian(expectedLight.AsSpan(8 + 4096 + slot * 4), slot == 0 ? 0 : 3);
                BinaryPrimitives.WriteSingleLittleEndian(expectedLight.AsSpan(8 + 8192 + slot * 4), -1);
                BinaryPrimitives.WriteSingleLittleEndian(expectedLight.AsSpan(8 + 12288 + slot * 4), 0);
            }
            Assert.Equal(expectedLight, lights.ToPayload());
        }
        else
        {
            Action translate = () => particles.TranslateBatchWithLightsFixed([0, 1023], 65536, -1, 32768, lights);
            if (outcome == "particle-overflow") Assert.Throws<OverflowException>(translate);
            else Assert.Throws<InvalidDataException>(translate);
            Assert.Equal(source, particles.ToPayload()); Assert.Equal(lightBefore, lights.ToPayload());
        }
    }

    [Fact]
    public void Translation_preserves_fractional_signed_coordinates_and_all_noncoordinate_bytes()
    {
        byte[] source = Empty(); I16(source, Record(0), 1); I16(source, Record(1023), 1);
        int first = Record(0) + 12, last = Record(1023) + 12 + 63 * 35;
        foreach (int offset in new[] { first, last })
        {
            source[offset] = 7;
            BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(offset + 1), -32768);
            BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(offset + 5), 131073);
            BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(offset + 9), -65537);
            source[offset + 13] = 0x55; // Velocity/state, not a coordinate.
        }
        I16(source, first + 33, 1023); I16(source, last + 33, 0);
        // Inactive children can retain stale data; translation must leave them byte-identical.
        BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(first + 35 + 1), int.MaxValue);
        var pool = NativeParticlePool.ParsePayload(source);
        var lights = pool.TranslateBatchFixed([0, 1023, 0], 65536, -65536, 32768);
        Assert.Equal(2, lights.Count);
        Assert.Equal(new NativeParticleLightPosition(0, 0, 1023, new(0, 1, -1)), lights[0]);
        Assert.Equal(new NativeParticleLightPosition(1023, 63, 0, new(0, 1, -1)), lights[1]);
        byte[] expected = source.ToArray();
        foreach (int offset in new[] { first, last })
        {
            BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 1), 32768);
            BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 5), 65537);
            BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(offset + 9), -32769);
        }
        Assert.Equal(expected, pool.ToPayload());
        pool.TranslateBatchFixed([0, 1023], -65536, 65536, -32768);
        Assert.Equal(source, pool.ToPayload());
    }

    [Theory]
    [InlineData("overflow")]
    [InlineData("overflow-y")]
    [InlineData("overflow-z")]
    [InlineData("underflow")]
    [InlineData("inactive")]
    [InlineData("slot")]
    [InlineData("negative-light")]
    [InlineData("large-light")]
    public void Translation_late_batch_failure_leaves_entire_payload_unchanged(string fault)
    {
        byte[] source = Empty(); I16(source, Record(0), 1); I16(source, Record(1023), 1);
        source[Record(0) + 12] = 1; source[Record(1023) + 12] = 1;
        if (fault == "overflow") BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Record(1023) + 13), int.MaxValue);
        if (fault == "overflow-y") BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Record(1023) + 17), int.MaxValue);
        if (fault == "overflow-z") BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Record(1023) + 21), int.MaxValue);
        if (fault == "underflow") BinaryPrimitives.WriteInt32LittleEndian(source.AsSpan(Record(1023) + 13), int.MinValue);
        if (fault == "inactive") I16(source, Record(1023), 0);
        if (fault == "negative-light") I16(source, Record(1023) + 45, -2);
        if (fault == "large-light") I16(source, Record(1023) + 45, 1024);
        var pool = NativeParticlePool.ParsePayload(source);
        Action translate = () => pool.TranslateBatchFixed([0, fault == "slot" ? 1024 : 1023], fault == "underflow" ? -1 : 1, 2, 3);
        if (fault.StartsWith("overflow", StringComparison.Ordinal) || fault == "underflow") Assert.Throws<OverflowException>(translate);
        else if (fault == "slot") Assert.Throws<ArgumentOutOfRangeException>(translate);
        else Assert.Throws<InvalidDataException>(translate);
        Assert.Equal(source, pool.ToPayload());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Translated_particles_save_reload_and_rollback_without_losing_wrapper(bool wrapped)
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmParticleMove_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            string path = Path.Combine(root, "particle.dat"); byte[] source = Empty();
            I16(source, Record(1023), 1); source[Record(1023) + 12] = 1;
            byte[] header = new byte[64]; "PFIL"u8.CopyTo(header); header[20] = 123;
            byte[] original = wrapped ? GameLZSS.CompressPfil(source, header) : source;
            File.WriteAllBytes(path, original);
            var pool = NativeParticlePool.Load(path); pool.TranslateBatchFixed([1023], -1, 65536, int.MinValue);
            using (var rollback = new FileRollbackScope()) pool.Save(path, rollback);
            Assert.Equal(original, File.ReadAllBytes(path));
            using (var rollback = new FileRollbackScope()) { pool.Save(path, rollback); rollback.Commit(); }
            Assert.Equal(pool.ToPayload(), NativeParticlePool.Load(path).ToPayload());
            if (wrapped) Assert.Equal(123, File.ReadAllBytes(path)[20]);
        }
        finally { Directory.Delete(root, true); }
    }

    [Fact]
    public void Release_resets_full_record_and_separate_arrays_without_touching_other_emitters()
    {
        byte[] source = Empty(); int record = Record(7);
        source.AsSpan(record, 2252).Fill(0x55);
        I16(source, record, 1); I16(source, record + 6, 42);
        for (int child = 0; child < 64; child++) I16(source, record + 12 + child * 35 + 33, -1);
        // Native reset visits lights even when the individual child is inactive.
        source[record + 12] = 0; I16(source, record + 12 + 33, 9);
        I16(source, record + 12 + 63 * 35 + 33, 10);
        for (int column = 0; column < 4; column++) source.AsSpan(Array(column, 7), 4).Fill(0x66);
        var pool = NativeParticlePool.ParsePayload(source);
        Assert.Equal(new[] { 9, 10 }, pool.Capture(7)!.LightSlots);
        byte[] before = pool.ToPayload();
        Assert.Throws<ArgumentOutOfRangeException>(() => pool.ReleaseBatch([7, 1024]));
        Assert.Equal(before, pool.ToPayload());
        var released = pool.ReleaseBatch([7, 7]);
        Assert.Equal(new[] { 42 }, released.OwnerObjects); Assert.Equal(new[] { 9, 10 }, released.LightSlots);
        Assert.Null(pool.Capture(7)); Assert.Equal(Empty(), pool.ToPayload());
        Assert.Equal(before, source); // Parse must detach its input.
    }

    [Theory]
    [InlineData(0, 2)]
    [InlineData(4, 1023)]
    [InlineData(8, 63)]
    public void Wrong_header_is_rejected(int offset, int value)
    {
        byte[] bytes = Empty(); BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
        Assert.Throws<InvalidDataException>(() => NativeParticlePool.ParsePayload(bytes));
    }

    [Fact]
    public void Truncated_payload_and_invalid_slots_are_rejected_and_export_is_detached()
    {
        byte[] bytes = Empty();
        Assert.Throws<InvalidDataException>(() => NativeParticlePool.ParsePayload(bytes.AsSpan(0, bytes.Length - 1)));
        var pool = NativeParticlePool.ParsePayload(bytes);
        Assert.Null(pool.Capture(-1)); Assert.Null(pool.Capture(1024));
        Assert.Throws<ArgumentOutOfRangeException>(() => pool.ReleaseBatch([-1]));
        pool.ToPayload()[0] = 0; Assert.Equal(bytes, pool.ToPayload());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Raw_and_pfil_save_share_rollback_and_preserve_emitter_payload(bool wrapped)
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmParticle_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            string path = Path.Combine(root, "particle.dat"); byte[] bytes = Empty();
            I16(bytes, Record(1023), 1); I16(bytes, Record(1023) + 6, 17);
            byte[] header = new byte[64]; "PFIL"u8.CopyTo(header); header[20] = 123;
            byte[] original = wrapped ? GameLZSS.CompressPfil(bytes, header) : bytes;
            File.WriteAllBytes(path, original);
            var pool = NativeParticlePool.Load(path); Assert.Equal(17, pool.Capture(1023)!.OwnerObject);
            pool.ReleaseBatch([1023]);
            using (var rollback = new FileRollbackScope()) pool.Save(path, rollback);
            Assert.Equal(original, File.ReadAllBytes(path));
            using (var rollback = new FileRollbackScope()) { pool.Save(path, rollback); rollback.Commit(); }
            Assert.Equal(Empty(), NativeParticlePool.Load(path).ToPayload());
            if (wrapped) Assert.Equal(123, File.ReadAllBytes(path)[20]);
        }
        finally { Directory.Delete(root, true); }
    }
}
