using System.Buffers.Binary;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FamilyCopyFailureTests
{
    [Theory]
    [InlineData(false, "object-capacity")]
    [InlineData(true, "object-capacity")]
    [InlineData(false, "position-capacity")]
    [InlineData(true, "position-capacity")]
    [InlineData(false, "objdata-capacity")]
    [InlineData(true, "objdata-capacity")]
    [InlineData(false, "light-capacity")]
    [InlineData(true, "light-capacity")]
    [InlineData(false, "particle-capacity")]
    [InlineData(true, "particle-capacity")]
    [InlineData(false, "camp-capacity")]
    [InlineData(true, "camp-capacity")]
    [InlineData(false, "bigcamp-capacity")]
    [InlineData(true, "bigcamp-capacity")]
    public void Allocation_capacity_exhaustion_returns_null_and_leaves_source_identical(bool pfil, string capacityKind)
    {
        using var fixture = CreateCapacityConstrainedFixture(pfil, capacityKind);
        var beforeFiles = fixture.Files();
        var store = LevelObjectStore.Load(fixture.Map);
        var beforeObjects = store.Objects();
        var beforeFamily = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));

        var plan = store.PlanFamilyCoreAllocation(0, 100);
        Assert.Null(plan);

        var candidate = store.CreateFamilyCopyCandidate(0, 100, 65536, 0, 0);
        Assert.Null(candidate);

        Assert.Equal(beforeObjects, store.Objects());
        var afterFamily = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Equal(beforeFamily.ObjectSlots, afterFamily.ObjectSlots);
        Assert.Equal(beforeFamily.LightSlots, afterFamily.LightSlots);
        Assert.Equal(beforeFamily.ParticleSlots, afterFamily.ParticleSlots);
        Assert.Equal(beforeFamily.LagerSlots, afterFamily.LagerSlots);
        Assert.Equal(beforeFamily.BigLagerSlots, afterFamily.BigLagerSlots);

        using (var rollback = new FileRollbackScope())
        {
            store.Save(fixture.Map, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in beforeFiles)
        {
            Assert.Equal(bytes, File.ReadAllBytes(path));
        }
    }

    [Theory]
    [InlineData(false, 0x00FFFFFFu)]
    [InlineData(true, 0x00FFFFFFu)]
    [InlineData(false, 0x01000000u)]
    [InlineData(true, 0x01000000u)]
    [InlineData(false, 0xFFFFFFFFu)]
    [InlineData(true, 0xFFFFFFFFu)]
    public void Uid_overflow_or_boundary_exhaustion_rejects_and_preserves_every_payload(bool pfil, uint initialNextUid)
    {
        using var fixture = new SyntheticMapFixture(pfil, count: 5);
        fixture.InjectFamilyTree();
        fixture.SetEngineNextUid(initialNextUid);

        var beforeFiles = fixture.Files();
        var store = LevelObjectStore.Load(fixture.Map);
        var beforeObjects = store.Objects();
        Assert.NotNull(store.FamilyAt(0, 100));

        var exPlan = Assert.Throws<InvalidDataException>(() => store.PlanFamilyCoreAllocation(0, 100));
        Assert.Contains("24-bit", exPlan.Message);

        var exCandidate = Assert.Throws<InvalidDataException>(() => store.CreateFamilyCopyCandidate(0, 100, 65536, 0, 0));
        Assert.Contains("24-bit", exCandidate.Message);

        Assert.Equal(beforeObjects, store.Objects());
        using (var rollback = new FileRollbackScope())
        {
            store.Save(fixture.Map, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in beforeFiles)
        {
            Assert.Equal(bytes, File.ReadAllBytes(path));
        }
    }

    [Theory]
    [InlineData(false, 999u, false)]
    [InlineData(true, 999u, false)]
    [InlineData(false, 0u, false)]
    [InlineData(true, 0u, false)]
    [InlineData(false, 100u, true)]
    [InlineData(true, 100u, true)]
    public void Invalid_root_uid_or_non_root_slot_rejects_without_mutating_source(bool pfil, uint rootUid, bool useChildSlot)
    {
        using var fixture = new SyntheticMapFixture(pfil, count: 5);
        fixture.InjectFamilyTree();

        var beforeFiles = fixture.Files();
        var store = LevelObjectStore.Load(fixture.Map);
        var beforeObjects = store.Objects();

        int testSlot = useChildSlot ? 1 : 0;
        uint testUid = useChildSlot ? store.UidAt(1)!.Value : rootUid;

        Assert.Null(store.FamilyAt(testSlot, testUid));
        Assert.Null(store.PlanFamilyCoreAllocation(testSlot, testUid));
        Assert.Null(store.CreateFamilyCopyCandidate(testSlot, testUid, 65536, 0, 0));

        Assert.Equal(beforeObjects, store.Objects());
        using (var rollback = new FileRollbackScope())
        {
            store.Save(fixture.Map, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in beforeFiles)
        {
            Assert.Equal(bytes, File.ReadAllBytes(path));
        }
    }

    [Theory]
    [InlineData(false, int.MaxValue, 0, 0)]
    [InlineData(true, int.MaxValue, 0, 0)]
    [InlineData(false, 0, int.MinValue, 0)]
    [InlineData(true, 0, int.MinValue, 0)]
    public void Particle_fixed_delta_overflow_late_failure_leaves_source_and_candidate_unapplied(bool pfil, int dx, int dy, int dz)
    {
        using var fixture = new SyntheticMapFixture(pfil, count: 8);
        fixture.InjectFamilyTree();
        fixture.InjectParticleEmitter(ownerSlot: 1, particleSlot: 7, lightSlot: 2);

        string particlePath = Path.Combine(fixture.Data, "particle.dat");
        byte[] particlePayload = fixture.ReadPayload("particle.dat");
        int childOffset = 12 + 7 * 2252 + 12;
        particlePayload[childOffset] = 1;
        if (dx != 0) BinaryPrimitives.WriteInt32LittleEndian(particlePayload.AsSpan(childOffset + 1), 0x7FFF0000);
        if (dy != 0) BinaryPrimitives.WriteInt32LittleEndian(particlePayload.AsSpan(childOffset + 5), unchecked((int)0x80010000));
        fixture.WritePayload("particle.dat", particlePayload);

        var beforeFiles = fixture.Files();
        var store = LevelObjectStore.Load(fixture.Map);
        var beforeObjects = store.Objects();
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Contains(7, family.ParticleSlots);

        Assert.Throws<OverflowException>(() => store.CreateFamilyCopyCandidate(0, 100, dx, dy, dz));

        Assert.Equal(beforeObjects, store.Objects());
        using (var rollback = new FileRollbackScope())
        {
            store.Save(fixture.Map, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in beforeFiles)
        {
            Assert.Equal(bytes, File.ReadAllBytes(path));
        }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Corrupted_source_direct_light_nan_coordinate_fails_late_translation_leaving_source_untouched(bool pfil)
    {
        using var fixture = new SyntheticMapFixture(pfil, count: 8);
        fixture.InjectFamilyTree();

        byte[] lightPayload = fixture.ReadPayload("light.dat");
        BinaryPrimitives.WriteSingleLittleEndian(lightPayload.AsSpan(8 + 1024 * 4 + 0 * 4), float.NaN);
        fixture.WritePayload("light.dat", lightPayload);

        var beforeFiles = fixture.Files();
        var store = LevelObjectStore.Load(fixture.Map);
        var beforeObjects = store.Objects();
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Contains(0, family.LightSlots);

        var ex = Assert.Throws<InvalidDataException>(() => store.CreateFamilyCopyCandidate(0, 100, 65536, 0, 0));
        Assert.True(ex.Message.Contains("float32") || ex.Message.Contains("座標無效"));

        Assert.Equal(beforeObjects, store.Objects());
        using (var rollback = new FileRollbackScope())
        {
            store.Save(fixture.Map, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in beforeFiles)
        {
            Assert.Equal(bytes, File.ReadAllBytes(path));
        }
    }

    private static SyntheticMapFixture CreateCapacityConstrainedFixture(bool pfil, string capacityKind)
    {
        const int count = 8;
        var fixture = new SyntheticMapFixture(pfil, count: count);
        fixture.InjectFamilyTree();

        switch (capacityKind)
        {
            case "object-capacity":
                var objLights = NativeLightPool.ParsePayload(fixture.ReadPayload("light.dat"));
                objLights.AddBatch([
                    new(new(10, 10, 10), new(1, 1, 1), 100, 0, 0, 0, 2, OwnerObject: 3, ParentObject: -1),
                    new(new(10, 10, 10), new(1, 1, 1), 100, 0, 0, 0, 2, OwnerObject: 4, ParentObject: -1),
                    new(new(10, 10, 10), new(1, 1, 1), 100, 0, 0, 0, 2, OwnerObject: 5, ParentObject: -1),
                ]);
                fixture.WritePayload("light.dat", objLights.ToPayload());
                break;

            case "position-capacity":
                byte[] pos = fixture.ReadPayload("position.dat");
                for (int slot = 6; slot <= 11; slot++)
                {
                    pos[8 + slot * 17] = 1;
                }
                fixture.WritePayload("position.dat", pos);
                break;

            case "objdata-capacity":
                byte[] objdata = fixture.ReadPayload("objdata.dat");
                for (int slot = 3; slot <= 5; slot++)
                {
                    objdata[8 + slot * LevelObjectStore.ObjDataWidths[0]] = 1;
                }
                fixture.WritePayload("objdata.dat", objdata);
                break;

            case "light-capacity":
                byte[] light = fixture.ReadPayload("light.dat");
                for (int slot = 0; slot < 1024; slot++)
                {
                    BinaryPrimitives.WriteUInt32LittleEndian(light.AsSpan(8 + slot * 4), 1);
                }
                fixture.WritePayload("light.dat", light);
                break;

            case "particle-capacity":
                fixture.InjectParticleEmitter(ownerSlot: 1, particleSlot: 7, lightSlot: 2);
                byte[] particles = fixture.ReadPayload("particle.dat");
                for (int slot = 0; slot < 1024; slot++)
                {
                    BinaryPrimitives.WriteUInt16LittleEndian(particles.AsSpan(12 + slot * 2252), 1);
                }
                fixture.WritePayload("particle.dat", particles);
                break;

            case "camp-capacity":
                fixture.InjectCamp(campSlot: 7, ownerSlot: 0, smallMember: 1);
                byte[] camps = fixture.ReadPayload("lager.dat");
                for (int slot = 0; slot < 3200; slot++)
                {
                    camps[16 + slot * 43] = 1;
                }
                fixture.WritePayload("lager.dat", camps);
                break;

            case "bigcamp-capacity":
                fixture.InjectBigCamp(campSlot: 7, bigLagerSlot: 2, ownerSlot: 0, bigMember: 1);
                byte[] bigs = fixture.ReadPayload("biglager.dat");
                for (int slot = 0; slot < 32; slot++)
                {
                    bigs[12 + slot * 1601] = 1;
                }
                fixture.WritePayload("biglager.dat", bigs);
                break;
        }

        return fixture;
    }

    private sealed class SyntheticMapFixture : IDisposable
    {
        private readonly bool _pfil;
        private readonly int _count;
        private readonly int _positionCount;

        public string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmFamilyCopyFail_" + Guid.NewGuid().ToString("N"));
        public string Map => Path.Combine(Root, "MAPS", "TEST");
        public string Data => Path.Combine(Map, "DATA");
        public string LightPath => Path.Combine(Data, "light.dat");

        public SyntheticMapFixture(bool pfil, int count = 5)
        {
            _pfil = pfil;
            _count = count;
            _positionCount = Math.Max(10, count * 2);
            Directory.CreateDirectory(Data);

            byte[] objects = new byte[16 + _count * (79 + LevelObjectStore.ColumnWidths.Sum())];
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), _count);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);
            for (int slot = 0; slot < _count; slot++)
            {
                for (int col = 0; col < LevelObjectStore.ColumnWidths.Length; col++)
                {
                    if (LevelObjectStore.ColumnWidths[col] == 2)
                        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(col, slot)), ushort.MaxValue);
                }
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 })
                    BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + field), ushort.MaxValue);
            }

            byte[] objdata = new byte[8 + _count * LevelObjectStore.ObjDataWidths.Sum()];
            BinaryPrimitives.WriteInt32LittleEndian(objdata.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(objdata.AsSpan(4), _count);

            byte[] positions = new byte[8 + _positionCount * 17];
            BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), _positionCount);

            byte[] anim = new byte[8 + _count * 25];
            BinaryPrimitives.WriteInt32LittleEndian(anim.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(anim.AsSpan(4), _count);

            byte[] gfxtype = new byte[8 + _count * 17];
            BinaryPrimitives.WriteInt32LittleEndian(gfxtype.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(gfxtype.AsSpan(4), _count);

            byte[] action = new byte[12 + _count * 25];
            BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(4), _count);
            BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(8), 6);

            byte[] engine = new byte[94];
            BinaryPrimitives.WriteInt32LittleEndian(engine.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(engine.AsSpan(34), 1000);

            byte[] light = new byte[NativeLightPool.PayloadLength];
            BinaryPrimitives.WriteInt32LittleEndian(light.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(light.AsSpan(4), 1024);
            light.AsSpan(8 + 1024 * 50).Fill(255);

            byte[] particle = new byte[NativeParticlePool.PayloadLength];
            BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(4), 1024);
            BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(8), 64);
            var particles = NativeParticlePool.ParsePayload(particle);
            particles.ReleaseBatch(Enumerable.Range(0, 1024).ToArray());
            particle = particles.ToPayload();

            byte[] lager = new byte[NativeLagerPool.PayloadLength];
            BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(4), 3200);
            BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(8), 6);
            BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(12), 10);
            var lagers = NativeLagerPool.ParsePayload(lager);
            lagers.ReleaseBatch(Enumerable.Range(0, 3200).ToArray());
            lager = lagers.ToPayload();

            byte[] biglager = new byte[NativeBigLagerPool.PayloadLength];
            BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(0), 1);
            BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(4), 32);
            BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(8), 800);
            var bigs = NativeBigLagerPool.ParsePayload(biglager);
            bigs.ReleaseBatch(Enumerable.Range(0, 32).ToArray());
            biglager = bigs.ToPayload();

            WriteFile("objects.dat", objects);
            WriteFile("objdata.dat", objdata);
            WriteFile("position.dat", positions);
            WriteFile("anim.dat", anim);
            WriteFile("gfxtype.dat", gfxtype);
            WriteFile("action.dat", action);
            WriteFile("engine.dat", engine);
            WriteFile("light.dat", light);
            WriteFile("particle.dat", particle);
            WriteFile("lager.dat", lager);
            WriteFile("biglager.dat", biglager);
        }

        public int ColumnOffset(int col, int slot)
            => 16 + _count * (79 + LevelObjectStore.ColumnWidths.Take(col).Sum()) + slot * LevelObjectStore.ColumnWidths[col];

        public void InjectFamilyTree()
        {
            byte[] obj = ReadPayload("objects.dat");
            byte[] data = ReadPayload("objdata.dat");
            byte[] pos = ReadPayload("position.dat");
            byte[] anim = ReadPayload("anim.dat");
            byte[] gfx = ReadPayload("gfxtype.dat");
            byte[] act = ReadPayload("action.dat");

            void SetupActiveSlot(int slot, uint uid, int p0, int p1, int typeId = 555)
            {
                int rec = 16 + slot * 79;
                obj[rec] = 1;
                BinaryPrimitives.WriteUInt32LittleEndian(obj.AsSpan(rec + 3), uid);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 67), (ushort)p0);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 69), (ushort)p1);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 71), (ushort)slot);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 73), (ushort)slot);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 75), (ushort)typeId);
                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(rec + 77), (ushort)slot);

                BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(1, slot)), (ushort)slot);

                data[8 + slot * LevelObjectStore.ObjDataWidths[0]] = 1;

                pos[8 + p0 * 17] = 1;
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p0 * 17 + 1), slot * 100f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p0 * 17 + 5), 20f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p0 * 17 + 9), 30f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p0 * 17 + 13), 0f);

                pos[8 + p1 * 17] = 1;
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p1 * 17 + 1), slot * 100f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p1 * 17 + 5), 20f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p1 * 17 + 9), 30f);
                BinaryPrimitives.WriteSingleLittleEndian(pos.AsSpan(8 + p1 * 17 + 13), 0f);

                anim[8 + slot * 21] = 1;
                gfx[8 + slot * 15] = 1;
                act[12 + slot * 25] = 1;
            }

            SetupActiveSlot(0, 100u, 0, 1);
            SetupActiveSlot(1, 101u, 2, 3);
            SetupActiveSlot(2, 102u, 4, 5);

            BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(13, 1)), 0);
            BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(13, 2)), 1);

            WritePayload("objects.dat", obj);
            WritePayload("objdata.dat", data);
            WritePayload("position.dat", pos);
            WritePayload("anim.dat", anim);
            WritePayload("gfxtype.dat", gfx);
            WritePayload("action.dat", act);

            var lights = NativeLightPool.ParsePayload(ReadPayload("light.dat"));
            lights.AddBatch([
                new(new(10, 40, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 0),
                new(new(200, 40, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 2)
            ]);
            WritePayload("light.dat", lights.ToPayload());
        }

        public void InjectParticleEmitter(int ownerSlot, int particleSlot, int lightSlot = 2)
        {
            byte[] obj = ReadPayload("objects.dat");
            BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(11, ownerSlot)), (ushort)particleSlot);
            WritePayload("objects.dat", obj);

            var lights = NativeLightPool.ParsePayload(ReadPayload("light.dat"));
            int[] added = lights.AddBatch([new(new(100, 20, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1, -1)])
                ?? throw new InvalidOperationException("無法為粒子發射器新增子光源。");
            int actualLightSlot = lightSlot >= 0 ? lightSlot : added[0];
            WritePayload("light.dat", lights.ToPayload());

            byte[] part = ReadPayload("particle.dat");
            int record = 12 + particleSlot * 2252;
            BinaryPrimitives.WriteUInt16LittleEndian(part.AsSpan(record), 1);
            BinaryPrimitives.WriteInt16LittleEndian(part.AsSpan(record + 6), (short)ownerSlot);
            part[record + 12] = 1;
            BinaryPrimitives.WriteInt16LittleEndian(part.AsSpan(record + 12 + 33), (short)actualLightSlot);
            WritePayload("particle.dat", part);
        }

        public void InjectCamp(int campSlot, int ownerSlot, int smallMember)
        {
            byte[] obj = ReadPayload("objects.dat");
            BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(5, ownerSlot)), (ushort)campSlot);
            WritePayload("objects.dat", obj);

            byte[] camp = ReadPayload("lager.dat");
            int rec = 16 + campSlot * 43;
            camp[rec] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(camp.AsSpan(rec + 9), 1);
            BinaryPrimitives.WriteUInt16LittleEndian(camp.AsSpan(rec + 23), (ushort)smallMember);
            WritePayload("lager.dat", camp);
        }

        public void InjectBigCamp(int campSlot, int bigLagerSlot, int ownerSlot, int bigMember)
        {
            byte[] obj = ReadPayload("objects.dat");
            BinaryPrimitives.WriteUInt16LittleEndian(obj.AsSpan(ColumnOffset(5, ownerSlot)), (ushort)campSlot);
            WritePayload("objects.dat", obj);

            byte[] camp = ReadPayload("lager.dat");
            int rec = 16 + campSlot * 43;
            camp[rec] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(camp.AsSpan(rec + 9), 1);
            BinaryPrimitives.WriteUInt16LittleEndian(camp.AsSpan(16 + 3200 * 43 + campSlot * 2), (ushort)bigLagerSlot);
            WritePayload("lager.dat", camp);

            byte[] big = ReadPayload("biglager.dat");
            int bigRec = 12 + bigLagerSlot * 1601;
            big[bigRec] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(big.AsSpan(bigRec + 1), (ushort)bigMember);
            WritePayload("biglager.dat", big);
        }

        public void SetEngineNextUid(uint nextUid)
        {
            byte[] engine = ReadPayload("engine.dat");
            BinaryPrimitives.WriteUInt32LittleEndian(engine.AsSpan(34), nextUid);
            WritePayload("engine.dat", engine);
        }

        public byte[] ReadPayload(string name)
        {
            byte[] raw = File.ReadAllBytes(Path.Combine(Data, name));
            return _pfil ? GameLZSS.DecompressPfil(raw) : raw;
        }

        public void WritePayload(string name, byte[] payload)
        {
            WriteFile(name, payload);
        }

        private void WriteFile(string name, byte[] payload)
        {
            byte[] header = new byte[64];
            "PFIL"u8.CopyTo(header);
            File.WriteAllBytes(Path.Combine(Data, name), _pfil ? GameLZSS.CompressPfil(payload, header) : payload);
        }

        public Dictionary<string, byte[]> Files()
            => Directory.GetFiles(Data).ToDictionary(p => p, File.ReadAllBytes);

        public void Dispose()
        {
            if (Directory.Exists(Root))
            {
                Directory.Delete(Root, true);
            }
        }
    }
}
