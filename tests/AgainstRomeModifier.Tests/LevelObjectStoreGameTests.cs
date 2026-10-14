using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using System.Buffers.Binary;
using System.Security.Cryptography;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 以本機原版地圖（唯讀）驗證 <see cref="LevelObjectStore"/>；設定 ARM_GAME_PATH 才執行，只在記憶體中操作，不寫回遊戲目錄。
/// </summary>
public sealed class LevelObjectStoreGameTests
{
    private readonly Xunit.Abstractions.ITestOutputHelper _output;
    public LevelObjectStoreGameTests(Xunit.Abstractions.ITestOutputHelper output) => _output = output;
    private static string? GamePath => Environment.GetEnvironmentVariable("ARM_GAME_PATH");

    [Theory]
    [InlineData(318, 44048)]
    [InlineData(1970, 49270)]
    public void Original_family_copy_in_temp_preserves_outside_payloads_and_persists_uid_cursor(int rootSlot, uint rootUid)
        => VerifyOriginalFamilyCopy(rootSlot, rootUid, 0, 0, 0);

    [Theory]
    [InlineData(318, 44048, 81920, -32768, 131072)]
    [InlineData(1970, 49270, 81920, -32768, 131072)]
    public void Original_family_copy_in_temp_supports_nonzero_fixed_displacement_and_preserves_expected_payloads(
        int rootSlot, uint rootUid, int dx, int dy, int dz)
        => VerifyOriginalFamilyCopy(rootSlot, rootUid, dx, dy, dz);

    private void VerifyOriginalFamilyCopy(int rootSlot, uint rootUid, int dx, int dy, int dz)
    {
        if (GamePath is not { } game) return;
        string sourceMap = Path.Combine(game, "MAPS", "HIST_004");
        string[] names = ["objects.dat", "objdata.dat", "position.dat", "anim.dat", "gfxtype.dat", "action.dat",
            "engine.dat", "light.dat", "particle.dat", "lager.dat", "biglager.dat"];
        var originals = names.ToDictionary(name => name, name => File.ReadAllBytes(Path.Combine(sourceMap, "DATA", name)));
        var payloads = originals.ToDictionary(pair => pair.Key, pair => GameLZSS.DecompressPfil(pair.Value));
        string temp = Path.Combine(Path.GetTempPath(), "ArmOriginalFamilyCopy_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(temp, "DATA"));
        try
        {
            foreach (var pair in originals) File.WriteAllBytes(Path.Combine(temp, "DATA", pair.Key), pair.Value);
            var source = LevelObjectStore.Load(temp); var before = source.Objects();
            var family = source.FamilyAt(rootSlot, rootUid)!;
            var copy = Assert.IsType<LevelObjectFamilyCopyCandidate>(source.CreateFamilyCopyCandidate(rootSlot, rootUid, dx, dy, dz));
            // 獨立建立全部 11 個 payload 的預期 byte 內容（以原始 decompressed raw payload 為基礎，結合 allocation/remap 規則），不呼叫 production copy/capture/save 輔助
            var expectedPayloads = payloads.ToDictionary(pair => pair.Key, pair => pair.Value.ToArray());
            int count = BinaryPrimitives.ReadInt32LittleEndian(payloads["objects.dat"].AsSpan(4));
            double[] delta = [dx / 65536d, dy / 65536d, dz / 65536d];

            int Remap(int srcSlot, IReadOnlyDictionary<int, int> map)
                => srcSlot == 0xFFFF || srcSlot == -1 ? srcSlot : map.TryGetValue(srcSlot, out int target) ? target
                    : throw new InvalidDataException("缺少預期映射槽位");

            // 1. position.dat: 8 + slot * 17
            foreach (var pair in copy.Allocation.Positions)
            {
                payloads["position.dat"].AsSpan(8 + pair.Key * 17, 17)
                    .CopyTo(expectedPayloads["position.dat"].AsSpan(8 + pair.Value * 17));
                if (dx != 0 || dy != 0 || dz != 0)
                {
                    for (int axis = 0; axis < 3; axis++)
                    {
                        int offset = 8 + pair.Value * 17 + 1 + axis * 4;
                        float value = BinaryPrimitives.ReadSingleLittleEndian(expectedPayloads["position.dat"].AsSpan(offset));
                        BinaryPrimitives.WriteSingleLittleEndian(expectedPayloads["position.dat"].AsSpan(offset), (float)(value + delta[axis]));
                    }
                }
            }

            // 2. objdata.dat: 36 segments
            foreach (var pair in copy.Allocation.ObjData)
            {
                int segOffset = 8;
                for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
                {
                    int width = LevelObjectStore.ObjDataWidths[segment];
                    payloads["objdata.dat"].AsSpan(segOffset + pair.Key * width, width)
                        .CopyTo(expectedPayloads["objdata.dat"].AsSpan(segOffset + pair.Value * width));
                    segOffset += count * width;
                }
            }

            // 3. anim.dat: 8 + slot * 21 (record), tails at 8 + count*21 + slot*2 and 8 + count*23 + slot*2
            foreach (var pair in copy.Allocation.Anim)
            {
                payloads["anim.dat"].AsSpan(8 + pair.Key * 21, 21)
                    .CopyTo(expectedPayloads["anim.dat"].AsSpan(8 + pair.Value * 21));
                payloads["anim.dat"].AsSpan(8 + count * 21 + pair.Key * 2, 2)
                    .CopyTo(expectedPayloads["anim.dat"].AsSpan(8 + count * 21 + pair.Value * 2));
                payloads["anim.dat"].AsSpan(8 + count * 23 + pair.Key * 2, 2)
                    .CopyTo(expectedPayloads["anim.dat"].AsSpan(8 + count * 23 + pair.Value * 2));
            }

            // 4. gfxtype.dat: 8 + slot * 15 (record), tail at 8 + count*15 + slot*2
            foreach (var pair in copy.Allocation.GfxType)
            {
                payloads["gfxtype.dat"].AsSpan(8 + pair.Key * 15, 15)
                    .CopyTo(expectedPayloads["gfxtype.dat"].AsSpan(8 + pair.Value * 15));
                payloads["gfxtype.dat"].AsSpan(8 + count * 15 + pair.Key * 2, 2)
                    .CopyTo(expectedPayloads["gfxtype.dat"].AsSpan(8 + count * 15 + pair.Value * 2));
            }

            // 5. action.dat: 12 + slot * 25 (record)
            foreach (var pair in copy.Allocation.Action)
            {
                payloads["action.dat"].AsSpan(12 + pair.Key * 25, 25)
                    .CopyTo(expectedPayloads["action.dat"].AsSpan(12 + pair.Value * 25));
            }

            // 6. objects.dat: 79-byte record, then 18 column arrays
            int colBase = 16 + count * 79;
            int[] colOffsets = new int[LevelObjectStore.ColumnWidths.Length];
            int currentOffset = colBase;
            for (int c = 0; c < LevelObjectStore.ColumnWidths.Length; c++)
            {
                colOffsets[c] = currentOffset;
                currentOffset += count * LevelObjectStore.ColumnWidths[c];
            }

            foreach (var pair in copy.Allocation.Objects)
            {
                int srcRec = 16 + pair.Key * 79;
                int dstRec = 16 + pair.Value * 79;
                payloads["objects.dat"].AsSpan(srcRec, 79).CopyTo(expectedPayloads["objects.dat"].AsSpan(dstRec));
                // Update new UID (offset +3, 4 bytes)
                BinaryPrimitives.WriteUInt32LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 3), copy.Allocation.Uids[pair.Key]);
                // Remap record links: 67 (pos0), 69 (pos1), 71 (anim), 73 (gfxtype), 77 (action)
                BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 67),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcRec + 67)), copy.Allocation.Positions));
                BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 69),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcRec + 69)), copy.Allocation.Positions));
                BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 71),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcRec + 71)), copy.Allocation.Anim));
                BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 73),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcRec + 73)), copy.Allocation.GfxType));
                BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstRec + 77),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcRec + 77)), copy.Allocation.Action));

                // 18 parallel column arrays
                for (int col = 0; col < LevelObjectStore.ColumnWidths.Length; col++)
                {
                    int width = LevelObjectStore.ColumnWidths[col];
                    int srcColOffset = colOffsets[col] + pair.Key * width;
                    int dstColOffset = colOffsets[col] + pair.Value * width;

                    IReadOnlyDictionary<int, int>? remapDict = col switch
                    {
                        1 => copy.Allocation.ObjData,
                        5 => copy.Allocation.Lagers,
                        10 => copy.Allocation.Positions,
                        11 => copy.Allocation.Particles,
                        12 => copy.Allocation.Lights,
                        13 => copy.Allocation.Objects,
                        _ => null
                    };

                    if (remapDict is null)
                    {
                        payloads["objects.dat"].AsSpan(srcColOffset, width).CopyTo(expectedPayloads["objects.dat"].AsSpan(dstColOffset));
                    }
                    else
                    {
                        int srcVal = width == 2
                            ? BinaryPrimitives.ReadUInt16LittleEndian(payloads["objects.dat"].AsSpan(srcColOffset))
                            : (int)BinaryPrimitives.ReadUInt32LittleEndian(payloads["objects.dat"].AsSpan(srcColOffset));
                        int dstVal = Remap(srcVal, remapDict);
                        if (width == 2)
                            BinaryPrimitives.WriteUInt16LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstColOffset), checked((ushort)dstVal));
                        else
                            BinaryPrimitives.WriteUInt32LittleEndian(expectedPayloads["objects.dat"].AsSpan(dstColOffset), (uint)dstVal);
                    }
                }
            }

            // 7. light.dat: 16 columns of widths [4,4,4,4,4,4,4,4,4,2,4,4,4,2,2,2]
            int[] lightWidths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
            int[] lightOffsets = new int[lightWidths.Length];
            int lightOffsetAcc = 8;
            for (int i = 0; i < lightWidths.Length; i++)
            {
                lightOffsets[i] = lightOffsetAcc;
                lightOffsetAcc += 1024 * lightWidths[i];
            }

            foreach (var pair in copy.Allocation.Lights)
            {
                for (int col = 0; col < lightWidths.Length; col++)
                {
                    int width = lightWidths[col];
                    int srcOff = lightOffsets[col] + pair.Key * width;
                    int dstOff = lightOffsets[col] + pair.Value * width;

                    if (col == 13) // ownerObject (short)
                    {
                        short srcOwner = BinaryPrimitives.ReadInt16LittleEndian(payloads["light.dat"].AsSpan(srcOff));
                        short dstOwner = (short)Remap(srcOwner, copy.Allocation.Objects);
                        BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["light.dat"].AsSpan(dstOff), dstOwner);
                    }
                    else if (col == 14) // parentObject (short)
                    {
                        short srcParent = BinaryPrimitives.ReadInt16LittleEndian(payloads["light.dat"].AsSpan(srcOff));
                        short dstParent = (short)Remap(srcParent, copy.Allocation.Objects);
                        BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["light.dat"].AsSpan(dstOff), dstParent);
                    }
                    else if (col is >= 1 and <= 3 && (dx != 0 || dy != 0 || dz != 0))
                    {
                        float srcVal = BinaryPrimitives.ReadSingleLittleEndian(payloads["light.dat"].AsSpan(srcOff));
                        BinaryPrimitives.WriteSingleLittleEndian(expectedPayloads["light.dat"].AsSpan(dstOff), (float)(srcVal + delta[col - 1]));
                    }
                    else
                    {
                        payloads["light.dat"].AsSpan(srcOff, width).CopyTo(expectedPayloads["light.dat"].AsSpan(dstOff));
                    }
                }
            }

            // 8. particle.dat: 12 + slot * 2252; owner at +6; 64 child lights at +12 + child*35 + 33; state words at 12 + 1024*2252 + col*4096 + slot*4
            foreach (var pair in copy.Allocation.Particles)
            {
                int srcRec = 12 + pair.Key * 2252;
                int dstRec = 12 + pair.Value * 2252;
                payloads["particle.dat"].AsSpan(srcRec, 2252).CopyTo(expectedPayloads["particle.dat"].AsSpan(dstRec));
                // Remap ownerObject at +6 (short)
                short srcOwner = BinaryPrimitives.ReadInt16LittleEndian(payloads["particle.dat"].AsSpan(srcRec + 6));
                BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["particle.dat"].AsSpan(dstRec + 6), (short)Remap(srcOwner, copy.Allocation.Objects));
                // Remap 64 child lights at +12 + child*35 + 33 (short) and translate active children
                for (int child = 0; child < 64; child++)
                {
                    int childBase = dstRec + 12 + child * 35;
                    int lightOff = childBase + 33;
                    short srcLight = BinaryPrimitives.ReadInt16LittleEndian(expectedPayloads["particle.dat"].AsSpan(lightOff));
                    short dstLight = -1;
                    if (srcLight != -1)
                    {
                        dstLight = (short)Remap(srcLight, copy.Allocation.Lights);
                        BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["particle.dat"].AsSpan(lightOff), dstLight);
                    }
                    if ((dx != 0 || dy != 0 || dz != 0) && expectedPayloads["particle.dat"][childBase] != 0)
                    {
                        int x = checked(BinaryPrimitives.ReadInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 1)) + dx);
                        int y = checked(BinaryPrimitives.ReadInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 5)) + dy);
                        int z = checked(BinaryPrimitives.ReadInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 9)) + dz);
                        BinaryPrimitives.WriteInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 1), x);
                        BinaryPrimitives.WriteInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 5), y);
                        BinaryPrimitives.WriteInt32LittleEndian(expectedPayloads["particle.dat"].AsSpan(childBase + 9), z);
                        if (dstLight != -1)
                        {
                            BinaryPrimitives.WriteSingleLittleEndian(expectedPayloads["light.dat"].AsSpan(lightOffsets[1] + dstLight * 4), x >> 16);
                            BinaryPrimitives.WriteSingleLittleEndian(expectedPayloads["light.dat"].AsSpan(lightOffsets[2] + dstLight * 4), y >> 16);
                            BinaryPrimitives.WriteSingleLittleEndian(expectedPayloads["light.dat"].AsSpan(lightOffsets[3] + dstLight * 4), z >> 16);
                        }
                    }
                }
                // 4 parallel state words
                for (int col = 0; col < 4; col++)
                {
                    int srcStateOff = 12 + 1024 * 2252 + col * 4096 + pair.Key * 4;
                    int dstStateOff = 12 + 1024 * 2252 + col * 4096 + pair.Value * 4;
                    payloads["particle.dat"].AsSpan(srcStateOff, 4).CopyTo(expectedPayloads["particle.dat"].AsSpan(dstStateOff));
                }
            }

            // 9. lager.dat: 16 + slot * 43; 10 small member objects at +23 + i*2 (short); separate big index at 16 + 3200*43 + slot*2 (short)
            foreach (var pair in copy.Allocation.Lagers)
            {
                int srcRec = 16 + pair.Key * 43;
                int dstRec = 16 + pair.Value * 43;
                payloads["lager.dat"].AsSpan(srcRec, 43).CopyTo(expectedPayloads["lager.dat"].AsSpan(dstRec));
                for (int m = 0; m < 10; m++)
                {
                    int mOff = dstRec + 23 + m * 2;
                    short srcMem = BinaryPrimitives.ReadInt16LittleEndian(expectedPayloads["lager.dat"].AsSpan(mOff));
                    if (srcMem != -1)
                    {
                        BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["lager.dat"].AsSpan(mOff), (short)Remap(srcMem, copy.Allocation.Objects));
                    }
                }
                int srcBigOff = 16 + 3200 * 43 + pair.Key * 2;
                int dstBigOff = 16 + 3200 * 43 + pair.Value * 2;
                short srcBig = BinaryPrimitives.ReadInt16LittleEndian(payloads["lager.dat"].AsSpan(srcBigOff));
                short dstBig = srcBig == -1 ? (short)-1 : (short)Remap(srcBig, copy.Allocation.BigLagers);
                BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["lager.dat"].AsSpan(dstBigOff), dstBig);
            }

            // 10. biglager.dat: 12 + slot * 1601; active byte at +0; 800 member objects at +1 + i*2 (short)
            foreach (var pair in copy.Allocation.BigLagers)
            {
                int srcRec = 12 + pair.Key * 1601;
                int dstRec = 12 + pair.Value * 1601;
                payloads["biglager.dat"].AsSpan(srcRec, 1601).CopyTo(expectedPayloads["biglager.dat"].AsSpan(dstRec));
                for (int m = 0; m < 800; m++)
                {
                    int mOff = dstRec + 1 + m * 2;
                    short srcMem = BinaryPrimitives.ReadInt16LittleEndian(expectedPayloads["biglager.dat"].AsSpan(mOff));
                    if (srcMem != -1)
                    {
                        BinaryPrimitives.WriteInt16LittleEndian(expectedPayloads["biglager.dat"].AsSpan(mOff), (short)Remap(srcMem, copy.Allocation.Objects));
                    }
                }
            }

            // 11. engine.dat: 94 bytes; cursor at 34 (4 bytes) = max(allocated UIDs) + 1
            uint expectedCursor = copy.Allocation.Uids.Values.Max() + 1;
            BinaryPrimitives.WriteUInt32LittleEndian(expectedPayloads["engine.dat"].AsSpan(34), expectedCursor);

            using (var rollback = new FileRollbackScope()) copy.Store.Save(temp, rollback);
            foreach (var pair in originals) Assert.Equal(pair.Value, File.ReadAllBytes(Path.Combine(temp, "DATA", pair.Key)));
            using (var rollback = new FileRollbackScope()) { copy.Store.Save(temp, rollback); rollback.Commit(); }
            foreach (var pair in payloads)
            {
                byte[] actual = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(temp, "DATA", pair.Key)));
                Assert.Equal(expectedPayloads[pair.Key].Length, actual.Length);
                Assert.Equal(expectedPayloads[pair.Key], actual);
            }
            byte[] engine = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(temp, "DATA", "engine.dat")));
            Assert.Equal(expectedCursor, BinaryPrimitives.ReadUInt32LittleEndian(engine.AsSpan(34)));
            var reloaded = LevelObjectStore.Load(temp);
            Assert.NotNull(reloaded.FamilyAt(rootSlot, rootUid));
            var cloned = Assert.IsType<LevelObjectFamily>(reloaded.FamilyAt(copy.RootSlot, copy.RootUid));
            Assert.Equal(family.ObjectSlots.Count, cloned.ObjectSlots.Count);
            Assert.Equal(family.LightSlots.Count, cloned.LightSlots.Count);
            Assert.Equal(family.ParticleSlots.Count, cloned.ParticleSlots.Count);
            Assert.Equal(family.LagerSlots.Count, cloned.LagerSlots.Count);
            Assert.Equal(family.BigLagerSlots.Count, cloned.BigLagerSlots.Count);
            foreach (var obj in before) Assert.Equal(obj, reloaded.Objects().Single(item => item.Slot == obj.Slot));
            Assert.Equal(before, source.Objects());
            foreach (int srcSlot in family.ObjectSlots)
            {
                var srcObj = before.Single(item => item.Slot == srcSlot);
                var dstObj = reloaded.Objects().Single(item => item.Slot == copy.Allocation.Objects[srcSlot]);
                Assert.Equal(srcObj.TypeId, dstObj.TypeId);
                Assert.Equal(srcObj.Rotation, dstObj.Rotation);
                Assert.Equal((float)(srcObj.X + delta[0]), dstObj.X);
                Assert.Equal((float)(srcObj.Y + delta[1]), dstObj.Y);
                Assert.Equal((float)(srcObj.Z + delta[2]), dstObj.Z);
            }
            _output.WriteLine($"Copy HIST_004 root={rootSlot}/UID={rootUid} -> {copy.RootSlot}/{copy.RootUid}, objects={cloned.ObjectSlots.Count}, delta=({dx},{dy},{dz})");
        }
        finally
        {
            Directory.Delete(temp, true);
            foreach (var pair in originals) Assert.Equal(SHA256.HashData(pair.Value),
                SHA256.HashData(File.ReadAllBytes(Path.Combine(sourceMap, "DATA", pair.Key))));
        }
    }

    [Theory]
    [InlineData(318, 44048)]
    [InlineData(1970, 49270)]
    public void Original_camp_family_translation_preserves_all_other_payload_bytes_and_source_hashes(int rootSlot, int rootUid)
    {
        if (GamePath is not { } game || !Directory.Exists(Path.Combine(game, "MAPS"))) return;
        string sourceMap = Path.Combine(game, "MAPS", "HIST_004");
        string[] names = ["objects.dat", "objdata.dat", "position.dat", "anim.dat", "gfxtype.dat", "action.dat", "engine.dat", "light.dat", "particle.dat", "lager.dat", "biglager.dat"];
        var source = names.ToDictionary(name => name, name => File.ReadAllBytes(Path.Combine(sourceMap, "DATA", name)));
        var expected = source.ToDictionary(pair => pair.Key, pair => GameLZSS.DecompressPfil(pair.Value).ToArray());
        string temp = Path.Combine(Path.GetTempPath(), "ArmOriginalFamilyMove_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(temp, "DATA"));
        try
        {
            foreach (var pair in source) File.WriteAllBytes(Path.Combine(temp, "DATA", pair.Key), pair.Value);
            var store = LevelObjectStore.Load(temp); var before = store.Objects();
            var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(rootSlot, (uint)rootUid));
            Assert.NotEmpty(family.LagerSlots);
            if (rootSlot == 318) Assert.NotEmpty(family.BigLagerSlots);
            else Assert.NotEmpty(family.ParticleSlots);
            _output.WriteLine($"Move Map=HIST_004 root={rootSlot} UID={rootUid} objects={family.ObjectSlots.Count} camps={family.LagerSlots.Count} big={family.BigLagerSlots.Count} particles={family.ParticleSlots.Count} lights={family.LightSlots.Count}");
            var members = family.ObjectSlots.ToHashSet(); int count = BinaryPrimitives.ReadInt32LittleEndian(expected["objects.dat"].AsSpan(4));
            int thirdColumn = 16 + count * (79 + LevelObjectStore.ColumnWidths.Take(10).Sum());
            var positions = family.ObjectSlots.SelectMany(slot => new[] {
                (int)BinaryPrimitives.ReadUInt16LittleEndian(expected["objects.dat"].AsSpan(16 + slot * 79 + 67)),
                (int)BinaryPrimitives.ReadUInt16LittleEndian(expected["objects.dat"].AsSpan(16 + slot * 79 + 69)),
                (int)BinaryPrimitives.ReadUInt16LittleEndian(expected["objects.dat"].AsSpan(thirdColumn + slot * 2)) })
                .Where(slot => slot != ushort.MaxValue).Distinct();
            double[] delta = [1.25d, -.5d, 2d]; int[] fixedDelta = [81920, -32768, 131072];
            foreach (int position in positions)
                for (int axis = 0; axis < 3; axis++)
                {
                    int offset = 8 + position * 17 + 1 + axis * 4;
                    float value = BinaryPrimitives.ReadSingleLittleEndian(expected["position.dat"].AsSpan(offset));
                    BinaryPrimitives.WriteSingleLittleEndian(expected["position.dat"].AsSpan(offset), (float)(value + delta[axis]));
                }
            foreach (int light in family.LightSlots)
                for (int axis = 0; axis < 3; axis++)
                {
                    int offset = 8 + (axis + 1) * 4096 + light * 4;
                    float value = BinaryPrimitives.ReadSingleLittleEndian(expected["light.dat"].AsSpan(offset));
                    BinaryPrimitives.WriteSingleLittleEndian(expected["light.dat"].AsSpan(offset), (float)(value + delta[axis]));
                }
            foreach (int emitter in family.ParticleSlots)
                for (int child = 0; child < 64; child++)
                {
                    int offset = 12 + emitter * 2252 + 12 + child * 35;
                    if (expected["particle.dat"][offset] == 0) continue;
                    int light = BinaryPrimitives.ReadInt16LittleEndian(expected["particle.dat"].AsSpan(offset + 33));
                    for (int axis = 0; axis < 3; axis++)
                    {
                        int value = checked(BinaryPrimitives.ReadInt32LittleEndian(expected["particle.dat"].AsSpan(offset + 1 + axis * 4)) + fixedDelta[axis]);
                        BinaryPrimitives.WriteInt32LittleEndian(expected["particle.dat"].AsSpan(offset + 1 + axis * 4), value);
                        if (light >= 0) BinaryPrimitives.WriteSingleLittleEndian(expected["light.dat"].AsSpan(8 + (axis + 1) * 4096 + light * 4), value >> 16);
                    }
                }
            var root = before.Single(obj => obj.Slot == rootSlot); Guid spawnId = Guid.NewGuid();
            var spawn = new ScenarioSpawn("BoundCamp", root.X, root.Z, root.Team,
                Angle: (int)Math.Round(root.Rotation * (180d / Math.PI)), Y: root.Y, Prebuilt: true) { Id = spawnId };
            var binding = new ScenarioDataSlot(rootSlot, (uint)rootUid) { SpawnId = spawnId };
            var previous = new ScenarioDocument { Spawns = [spawn], DataSlots = [binding] };
            var next = new ScenarioDocument { Spawns = [spawn with { X = spawn.X + 1.25f, Y = spawn.Y - .5f, Z = spawn.Z + 2f }], DataSlots = [binding] };
            Assert.Empty(ScenarioLevelObjects.Apply(store, previous, next, _ => throw new InvalidOperationException("Retained camp must not need catalogue")));
            Assert.Equal(binding, Assert.Single(next.DataSlots));
            foreach (var old in before)
                Assert.Equal(members.Contains(old.Slot) ? old with { X = (float)(old.X + 1.25d), Y = (float)(old.Y - .5d), Z = (float)(old.Z + 2d) } : old,
                    store.Objects().Single(obj => obj.Slot == old.Slot));
            Assert.Throws<IOException>((Action)(() =>
            {
                using var rollback = new FileRollbackScope(); store.Save(temp, rollback);
                throw new IOException("Injected later family move save failure");
            }));
            foreach (var pair in source) Assert.Equal(pair.Value, File.ReadAllBytes(Path.Combine(temp, "DATA", pair.Key)));
            using (var rollback = new FileRollbackScope()) { store.Save(temp, rollback); rollback.Commit(); }
            foreach (var pair in expected) Assert.Equal(pair.Value, GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(temp, "DATA", pair.Key))));
            var reloaded = LevelObjectStore.Load(temp); Assert.Equal(store.Objects(), reloaded.Objects());
            var again = Assert.IsType<LevelObjectFamily>(reloaded.FamilyAt(rootSlot, (uint)rootUid));
            Assert.Equal(family.ObjectSlots, again.ObjectSlots); Assert.Equal(family.LightSlots, again.LightSlots);
            Assert.Equal(family.ParticleSlots, again.ParticleSlots); Assert.Equal(family.LagerSlots, again.LagerSlots); Assert.Equal(family.BigLagerSlots, again.BigLagerSlots);
        }
        finally
        {
            Directory.Delete(temp, true);
            foreach (var pair in source) Assert.Equal(SHA256.HashData(pair.Value), SHA256.HashData(File.ReadAllBytes(Path.Combine(sourceMap, "DATA", pair.Key))));
        }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Original_family_deletes_in_temp_with_rollback_and_preserves_outside_objects_and_source_hashes(bool requireLargeCamp)
    {
        if (GamePath is not { } game || !Directory.Exists(Path.Combine(game, "MAPS"))) return;
        string? selected = null; LevelObjectFamily? family = null;
        foreach (string map in Directory.GetDirectories(Path.Combine(game, "MAPS")).OrderBy(path => Path.GetFileName(path) == "HIST_004" ? 0 : 1))
        {
            string path = Path.Combine(map, "DATA", "objects.dat"); if (!File.Exists(path)) continue;
            byte[] objects = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
            int count = BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4));
            int parents = 16 + count * (79 + LevelObjectStore.ColumnWidths.Take(13).Sum());
            var roots = Enumerable.Range(0, count).Where(slot => objects[16 + slot * 79] != 0)
                .Select(slot => (int)BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(parents + slot * 2)))
                .Where(slot => slot != ushort.MaxValue).Distinct().ToArray();
            if (roots.Length == 0 || !File.Exists(Path.Combine(map, "DATA", "light.dat"))) continue;
            var store = LevelObjectStore.Load(map);
            foreach (int root in roots)
                if (store.UidAt(root) is { } uid && store.FamilyAt(root, uid) is { ObjectSlots.Count: > 1 } found
                    && (!requireLargeCamp || found.LagerSlots.Count > 0 && found.BigLagerSlots.Count > 0))
                { selected = map; family = found; break; }
            if (selected is not null) break;
        }
        Assert.NotNull(selected); Assert.NotNull(family);
        _output.WriteLine($"Map={Path.GetFileName(selected)} root={family!.RootSlot} UID={family.RootUid} objects={family.ObjectSlots.Count} lagers={family.LagerSlots.Count} biglagers={family.BigLagerSlots.Count} particles={family.ParticleSlots.Count} lights={family.LightSlots.Count}");
        string temp = Path.Combine(Path.GetTempPath(), "ArmOriginalFamily_" + Guid.NewGuid().ToString("N"));
        string[] names = ["objects.dat", "objdata.dat", "position.dat", "anim.dat", "gfxtype.dat", "action.dat", "engine.dat", "light.dat", "particle.dat", "lager.dat", "biglager.dat"];
        var sourceHashes = names.Where(name => File.Exists(Path.Combine(selected!, "DATA", name)))
            .ToDictionary(name => name, name => SHA256.HashData(File.ReadAllBytes(Path.Combine(selected!, "DATA", name))));
        Directory.CreateDirectory(Path.Combine(temp, "DATA"));
        try
        {
            foreach (string name in sourceHashes.Keys) File.Copy(Path.Combine(selected!, "DATA", name), Path.Combine(temp, "DATA", name));
            var store = LevelObjectStore.Load(temp); var before = store.Objects();
            byte[] originalLager = NativeLagerPool.Load(Path.Combine(temp, "DATA", "lager.dat")).ToPayload();
            byte[] originalBig = NativeBigLagerPool.Load(Path.Combine(temp, "DATA", "biglager.dat")).ToPayload();
            var members = family!.ObjectSlots.ToHashSet();
            Assert.True(store.RemoveFamilyIfUid(family.RootSlot, family.RootUid));
            Assert.Equal(before.Where(obj => !members.Contains(obj.Slot)), store.Objects());
            Assert.Throws<IOException>((Action)(() =>
            {
                using var rollback = new FileRollbackScope(); store.Save(temp, rollback);
                throw new IOException("simulate later file failure");
            }));
            foreach (var (name, hash) in sourceHashes) Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(Path.Combine(temp, "DATA", name))));
            using (var rollback = new FileRollbackScope()) { store.Save(temp, rollback); rollback.Commit(); }
            Assert.Equal(before.Count - members.Count, LevelObjectStore.Load(temp).Objects().Count);
            Assert.DoesNotContain(NativeLightPool.Load(Path.Combine(temp, "DATA", "light.dat")).ActiveLights(),
                light => members.Contains(light.OwnerObject) || members.Contains(light.ParentObject));
            var camps = NativeLagerPool.Load(Path.Combine(temp, "DATA", "lager.dat"));
            var bigs = NativeBigLagerPool.Load(Path.Combine(temp, "DATA", "biglager.dat"));
            Assert.All(family.LagerSlots, slot => Assert.Null(camps.Capture(slot)));
            Assert.All(family.BigLagerSlots, slot => Assert.Null(bigs.Capture(slot)));
            byte[] savedLager = camps.ToPayload(), savedBig = bigs.ToPayload();
            foreach (int slot in Enumerable.Range(0, 3200).Except(family.LagerSlots))
            {
                Assert.Equal(originalLager.AsSpan(16 + slot * 43, 43).ToArray(), savedLager.AsSpan(16 + slot * 43, 43).ToArray());
                Assert.Equal(originalLager.AsSpan(16 + 3200 * 43 + slot * 2, 2).ToArray(), savedLager.AsSpan(16 + 3200 * 43 + slot * 2, 2).ToArray());
            }
            foreach (int slot in Enumerable.Range(0, 32).Except(family.BigLagerSlots))
                Assert.Equal(originalBig.AsSpan(12 + slot * 1601, 1601).ToArray(), savedBig.AsSpan(12 + slot * 1601, 1601).ToArray());
        }
        finally
        {
            Directory.Delete(temp, true);
            foreach (var (name, hash) in sourceHashes) Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(Path.Combine(selected!, "DATA", name))));
        }
    }

    [Fact]
    public void Original_endless_map_parses_and_add_remove_round_trips_in_memory()
    {
        if (GamePath is not { } game || !Directory.Exists(Path.Combine(game, "MAPS", "ENDL_000"))) return;
        LevelObjectStore store = LevelObjectStore.Load(Path.Combine(game, "MAPS", "ENDL_000"));
        IReadOnlyDictionary<int, string> names = ObjDefNames.Load(game);
        IReadOnlyList<LevelWorldObject> before = store.Objects();
        Assert.Equal(6618, before.Count);
        Assert.All(before, item => Assert.InRange(item.X, 0, 16384));
        Assert.Equal("LanGerGra00_Gras_M", names[146]);

        LevelObjectTemplate tree = store.Templates().First(template => names.TryGetValue(template.TypeId, out string? name) && name.Contains("Tanne", StringComparison.OrdinalIgnoreCase));
        int slot = store.Add(tree, 8000, 200, 8100, 1.5f);
        Assert.True(slot >= 0);
        LevelWorldObject added = Assert.Single(store.Objects(), item => item.Slot == slot);
        Assert.Equal(tree.TypeId, added.TypeId);
        Assert.Equal((8000f, 200f, 8100f, 1.5f), (added.X, added.Y, added.Z, added.Rotation));
        Assert.True(added.Uid > before.Max(item => item.Uid));
        Assert.Equal(before.Count + 1, store.Objects().Count);

        Assert.True(store.Remove(slot));
        Assert.Equal(before.Count, store.Objects().Count);
        // ENDL_000 has third positions and nonzero float scalars, but no actual
        // references in columns 2/3/4/5/8/11. Those values must not imply Linked.
        Assert.DoesNotContain(before, item => item.Linked);
    }
}
