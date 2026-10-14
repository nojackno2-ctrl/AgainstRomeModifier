using System.Buffers.Binary;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeLightPoolTests
{
    internal static byte[] EmptyPool()
    {
        byte[] bytes = new byte[NativeLightPool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, 1);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(4), 1024);
        for (int column = 13; column <= 15; column++)
            bytes.AsSpan(8 + 1024 * (50 + (column - 13) * 2), 2048).Fill(255);
        return bytes;
    }

    private static NativeLightCreation Light(int owner = -1, int parent = -1)
        => new(new(12, 34, 56), new(.8f, .4f, .1f), 300, 2, .15f, 7, 39, owner, parent);

    [Fact]
    public void Position_batch_changes_only_xyz_preserving_ownership_effects_and_unselected_slots()
    {
        var pool = NativeLightPool.ParsePayload(EmptyPool());
        pool.AddBatch([Light(7), Light(parent: 8), Light()]);
        byte[] expected = pool.ToPayload();
        pool.SetPositionsBatch([new(0, new(-1.5f, 2.25f, 3)), new(1, new(9, 8, 7)), new(0, new(-1.5f, 2.25f, 3))]);
        foreach (var item in new[] { new NativeLightPosition(0, new(-1.5f, 2.25f, 3)), new NativeLightPosition(1, new(9, 8, 7)) })
        {
            BinaryPrimitives.WriteSingleLittleEndian(expected.AsSpan(8 + 4096 + item.Slot * 4), item.Position.X);
            BinaryPrimitives.WriteSingleLittleEndian(expected.AsSpan(8 + 8192 + item.Slot * 4), item.Position.Y);
            BinaryPrimitives.WriteSingleLittleEndian(expected.AsSpan(8 + 12288 + item.Slot * 4), item.Position.Z);
        }
        Assert.Equal(expected, pool.ToPayload());
    }

    [Theory]
    [InlineData("inactive")]
    [InlineData("index")]
    [InlineData("nan")]
    [InlineData("infinity")]
    [InlineData("conflict")]
    public void Position_batch_late_failure_never_changes_first_light(string fault)
    {
        var pool = NativeLightPool.ParsePayload(EmptyPool()); pool.AddBatch([Light(), Light()]);
        byte[] before = pool.ToPayload();
        var invalid = fault switch
        {
            "inactive" => new NativeLightPosition(2, new(1, 2, 3)),
            "index" => new NativeLightPosition(1024, new(1, 2, 3)),
            "nan" => new NativeLightPosition(1, new(1, float.NaN, 3)),
            "infinity" => new NativeLightPosition(1, new(1, 2, float.PositiveInfinity)),
            _ => new NativeLightPosition(0, new(1, 2, 4))
        };
        Action update = () => pool.SetPositionsBatch([new(0, new(1, 2, 3)), invalid]);
        if (fault == "index") Assert.Throws<ArgumentOutOfRangeException>(update);
        else Assert.Throws<InvalidDataException>(update);
        Assert.Equal(before, pool.ToPayload());
    }

    [Fact]
    public void Batch_allocation_resets_reused_slot_initializes_native_fields_and_keeps_other_slots()
    {
        byte[] source = EmptyPool();
        source.AsSpan(8 + 1024 * 42, 4).Fill(0xCC); // old serialized counter in inactive slot0
        var pool = NativeLightPool.ParsePayload(source);
        var before = source.ToArray();
        Assert.Equal(new[] { 0, 1 }, pool.AddBatch(new[] { Light(4), Light(parent: 8) }));
        Assert.Equal(before, source);
        byte[] bytes = pool.ToPayload();
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(8)));
        Assert.Equal(300f, BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(8 + 1024 * 28)));
        Assert.Equal(300f, BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(8 + 1024 * 32)));
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(8 + 1024 * 42)));
        Assert.Equal(7u, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(8 + 1024 * 46)));
        Assert.Equal(new NativeSavedLight(0, 1, new(12, 34, 56), 4, -1, 39), pool.ActiveLights()[0]);
        Assert.Equal(new NativeSavedLight(1, 1, new(12, 34, 56), -1, 8, 39), pool.ActiveLights()[1]);
        foreach (int slot in Enumerable.Range(2, 1022))
            Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(8 + slot * 4)));
    }

    [Fact]
    public void Capacity_failure_is_atomic_and_first_released_slot_is_reused()
    {
        var pool = NativeLightPool.ParsePayload(EmptyPool());
        Assert.NotNull(pool.AddBatch(Enumerable.Repeat(Light(), 1023).ToArray()));
        byte[] before = pool.ToPayload();
        Assert.Null(pool.AddBatch(new[] { Light(), Light() }));
        Assert.Equal(before, pool.ToPayload());
        pool.ReleaseBatch(new[] { 3 });
        Assert.Equal(new[] { 3, 1023 }, pool.AddBatch(new[] { Light(), Light() }));
        Assert.Null(pool.AddBatch(new[] { Light() }));
        Assert.Empty(pool.ReleaseBatch(new[] { 3, 3 }));
    }

    [Fact]
    public void Invalid_later_item_never_partially_allocates_and_invalid_release_never_partially_resets()
    {
        var pool = NativeLightPool.ParsePayload(EmptyPool());
        byte[] before = pool.ToPayload();
        Assert.Throws<InvalidDataException>(() => pool.AddBatch(new[] { Light(), Light() with { Position = new(float.NaN, 0, 0) } }));
        Assert.Equal(before, pool.ToPayload());
        pool.AddBatch(new[] { Light(7), Light(parent: 7) });
        before = pool.ToPayload();
        Assert.Throws<ArgumentOutOfRangeException>(() => pool.ReleaseBatch(new[] { 0, 1024 }));
        Assert.Equal(before, pool.ToPayload());
        Assert.Equal(new[] { 7 }, pool.ReleaseBatch(new[] { 0, 1 }));
        Assert.Equal(EmptyPool(), pool.ToPayload());
    }

    [Theory]
    [InlineData(-2, -1, 0)]
    [InlineData(14000, -1, 0)]
    [InlineData(-1, 14000, 0)]
    [InlineData(-1, -1, 256)]
    public void Invalid_definition_or_relation_is_rejected(int owner, int parent, short definition)
    {
        var pool = NativeLightPool.ParsePayload(EmptyPool());
        Assert.Throws<InvalidDataException>(() => pool.AddBatch(new[] { Light(owner, parent) with { Definition = definition } }));
        Assert.Equal(EmptyPool(), pool.ToPayload());
    }

    [Fact]
    public void Parser_rejects_truncated_wrong_count_wrong_version_and_output_is_detached()
    {
        byte[] bytes = EmptyPool();
        Assert.Throws<InvalidDataException>(() => NativeLightPool.ParsePayload(bytes.AsSpan(0, bytes.Length - 1)));
        bytes[0] = 2; Assert.Throws<InvalidDataException>(() => NativeLightPool.ParsePayload(bytes));
        bytes[0] = 1; bytes[4] = 1; Assert.Throws<InvalidDataException>(() => NativeLightPool.ParsePayload(bytes));
        var pool = NativeLightPool.ParsePayload(EmptyPool());
        pool.ToPayload()[0] = 0;
        Assert.Equal(1, pool.ToPayload()[0]);
    }

    [RequiresMarkerGameFact]
    public void Original_light_pools_parse_and_temp_save_preserves_payload_and_original_hashes()
    {
        string game = Environment.GetEnvironmentVariable("ARM_GAME_PATH")!;
        int maps = 0, active = 0, owners = 0, parents = 0;
        foreach (string path in Directory.GetFiles(Path.Combine(game, "MAPS"), "light.dat", SearchOption.AllDirectories))
        {
            byte[] original = File.ReadAllBytes(path); byte[] hash = SHA256.HashData(original);
            var pool = NativeLightPool.Load(path);
            var lights = pool.ActiveLights(); maps++; active += lights.Count;
            owners += lights.Count(light => light.OwnerObject >= 0); parents += lights.Count(light => light.ParentObject >= 0);
            Assert.Equal(GameLZSS.DecompressPfil(original), pool.ToPayload());
            if (maps == 1)
            {
                string directory = Path.Combine(Path.GetTempPath(), "ArmLightPool_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(directory);
                try
                {
                    string target = Path.Combine(directory, "light.dat"); File.WriteAllBytes(target, original);
                    using (var rollback = new FileRollbackScope()) { pool.Save(target, rollback); rollback.Commit(); }
                    Assert.Equal(pool.ToPayload(), NativeLightPool.Load(target).ToPayload());
                    pool.AddBatch(new[] { Light(parent: 17) });
                    using (var rollback = new FileRollbackScope()) pool.Save(target, rollback);
                    Assert.Equal(GameLZSS.DecompressPfil(original), NativeLightPool.Load(target).ToPayload());
                }
                finally { Directory.Delete(directory, true); }
            }
            Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(path)));
        }
        Assert.Equal(74, maps); Assert.Equal(1415, active); Assert.Equal(447, owners); Assert.Equal(553, parents);
    }
}
