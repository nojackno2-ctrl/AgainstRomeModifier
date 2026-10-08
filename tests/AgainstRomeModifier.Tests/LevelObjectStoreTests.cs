using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class LevelObjectStoreTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Load_returns_only_active_objects_with_fields_and_both_link_flags(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        Assert.Equal(8, store.Capacity);
        Assert.Equal(new[]
        {
            new LevelWorldObject(0, 100, 2, 10, 1, 2, 3, .25f, false),
            new LevelWorldObject(2, 100, 4, 30, 21, 22, 23, 2.25f, false),
            new LevelWorldObject(3, 200, 5, 40, 31, 32, 33, 3.25f, true),
            new LevelWorldObject(4, 300, 6, 50, 41, 42, 43, 4.25f, true),
            new LevelWorldObject(5, 400, 7, 60, 0, 0, 0, 0, false),
            new LevelWorldObject(6, 500, 8, 70, 61, 62, 63, 6.25f, false),
        }, store.Objects());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Templates_deduplicate_types_and_skip_links_and_either_invalid_position(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        Assert.Equal(100, Assert.Single(level.Load().Templates()).TypeId);
        // An invalid earlier instance must not suppress a later valid instance of its type.
        U16(level.Objects, Record(0) + 69, LevelFixture.PositionCount);
        level.Write();
        Assert.Equal(100, Assert.Single(level.Load().Templates()).TypeId);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_uses_first_free_slot_and_even_pair_and_copies_template_bytes(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 101.5f, -2, 303, .75f));
        Assert.Equal(new LevelWorldObject(1, 100, 2, 71, 101.5f, -2, 303, .75f, false),
            Assert.Single(store.Objects(), item => item.Slot == 1));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        byte[] objects = level.ReadPayload("objects.dat");
        byte[] data = level.ReadPayload("objdata.dat");
        byte[] positions = level.ReadPayload("position.dat");
        byte[] expectedRecord = Slice(level.Objects, Record(0), LevelObjectStore.RecordSize);
        U32(expectedRecord, 3, 71);
        U16(expectedRecord, 67, 14); U16(expectedRecord, 69, 15);
        foreach (int offset in new[] { 71, 73, 77 }) U16(expectedRecord, offset, 1);
        Assert.Equal(expectedRecord, Slice(objects, Record(1), LevelObjectStore.RecordSize));
        for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
        {
            int width = LevelObjectStore.ColumnWidths[column];
            byte[] expected = Slice(level.Objects, Column(column, 0), width);
            if (column == 1) U16(expected, 0, 1);
            Assert.Equal(expected, Slice(objects, Column(column, 1), width));
        }
        for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
            Assert.Equal(Slice(level.ObjData, Segment(segment, 0), LevelObjectStore.ObjDataWidths[segment]),
                Slice(data, Segment(segment, 1), LevelObjectStore.ObjDataWidths[segment]));
        foreach (int index in new[] { 14, 15 })
        {
            int offset = Position(index);
            Assert.Equal(1, positions[offset]);
            Assert.Equal(101.5f, F32(positions, offset + 1));
            Assert.Equal(-2f, F32(positions, offset + 5));
            Assert.Equal(303f, F32(positions, offset + 9));
            Assert.Equal(.75f, F32(positions, offset + 13));
        }
        Assert.Equal(Slice(level.Positions, Position(12), 2 * LevelObjectStore.PositionSize),
            Slice(positions, Position(12), 2 * LevelObjectStore.PositionSize));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_returns_minus_one_when_slots_or_even_position_pairs_are_exhausted(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 1, 2, 3, 4));
        Assert.Equal(7, store.Add(template, 1, 2, 3, 4));
        var before = store.Objects().ToArray();
        Assert.Equal(-1, store.Add(template, 1, 2, 3, 4));
        Assert.Equal(before, store.Objects());
        // There are free individual positions, but no completely free even pair.
        for (int index = 0; index < LevelFixture.PositionCount; index++)
            level.Positions[Position(index)] = (byte)(index % 2 == 0 ? 0 : 1);
        level.Write();
        store = level.Load();
        before = store.Objects().ToArray();
        Assert.Equal(-1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 4));
        Assert.Equal(before, store.Objects());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_rejects_non_finite_coordinates_and_rotation_without_mutation(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        foreach (float value in new[] { float.NaN, float.PositiveInfinity, float.NegativeInfinity })
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, value, 2, 3, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, value, 3, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, 2, value, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, 2, 3, value));
        }
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        Assert.Equal(level.Objects, level.ReadPayload("objects.dat"));
        Assert.Equal(level.ObjData, level.ReadPayload("objdata.dat"));
        Assert.Equal(level.Positions, level.ReadPayload("position.dat"));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Remove_restores_captured_empty_bytes_and_frees_both_positions(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        Assert.True(store.Remove(0));
        Assert.DoesNotContain(store.Objects(), item => item.Slot == 0);
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        byte[] expectedObjects = (byte[])level.Objects.Clone();
        byte[] expectedData = (byte[])level.ObjData.Clone();
        byte[] expectedPositions = (byte[])level.Positions.Clone();
        Array.Copy(level.Objects, Record(1), expectedObjects, Record(0), LevelObjectStore.RecordSize);
        for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
            Array.Copy(level.Objects, Column(column, 1), expectedObjects, Column(column, 0), LevelObjectStore.ColumnWidths[column]);
        for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
            Array.Copy(level.ObjData, Segment(segment, 1), expectedData, Segment(segment, 0), LevelObjectStore.ObjDataWidths[segment]);
        foreach (int index in new[] { 0, 1 })
            Array.Copy(level.Positions, Position(12), expectedPositions, Position(index), LevelObjectStore.PositionSize);
        Assert.Equal(expectedObjects, level.ReadPayload("objects.dat"));
        Assert.Equal(expectedData, level.ReadPayload("objdata.dat"));
        Assert.Equal(expectedPositions, level.ReadPayload("position.dat"));
        Assert.Equal(0, store.Add(Assert.Single(store.Templates()), 9, 8, 7, 6));
        store.Save(level.Map, rollback);
        Assert.Equal(0, BinaryPrimitives.ReadUInt16LittleEndian(level.ReadPayload("objects.dat").AsSpan(Record(0) + 67)));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Remove_refuses_linked_inactive_and_out_of_range_slots_without_mutation(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        foreach (int slot in new[] { -1, 8, int.MaxValue, 1, 7, 3, 4 }) Assert.False(store.Remove(slot));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        Assert.Equal(level.Objects, level.ReadPayload("objects.dat"));
        Assert.Equal(level.ObjData, level.ReadPayload("objdata.dat"));
        Assert.Equal(level.Positions, level.ReadPayload("position.dat"));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Save_reloads_objects_preserves_encoding_and_headers_and_rolls_back(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        byte[][] originals = LevelFixture.Files.Select(level.ReadRaw).ToArray();
        LevelObjectStore store = level.Load();
        Assert.Equal(1, store.Add(Assert.Single(store.Templates()), 123, -45, 678, 1.25f));
        Assert.True(store.Remove(2));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        {
            store.Save(level.Map, rollback);
            Assert.Equal(store.Objects(), level.Load().Objects());
            for (int file = 0; file < LevelFixture.Files.Length; file++)
            {
                byte[] saved = level.ReadRaw(LevelFixture.Files[file]);
                Assert.False(originals[file].SequenceEqual(saved));
                if (pfil)
                {
                    Assert.Equal("PFIL", System.Text.Encoding.ASCII.GetString(saved, 0, 4));
                    for (int i = 0; i < 64; i++)
                        if (i < 16 || i >= 20) Assert.Equal(originals[file][i], saved[i]);
                    Assert.Equal(level.ReadPayload(LevelFixture.Files[file]).Length,
                        BinaryPrimitives.ReadInt32LittleEndian(saved.AsSpan(16)));
                }
                else Assert.Equal(level.ReadPayload(LevelFixture.Files[file]), saved);
            }
        }
        for (int file = 0; file < LevelFixture.Files.Length; file++)
            Assert.Equal(originals[file], level.ReadRaw(LevelFixture.Files[file]));
    }

    [Theory]
    [InlineData("objects-version")]
    [InlineData("name-width")]
    [InlineData("idname-width")]
    [InlineData("objects-length")]
    [InlineData("objdata-length")]
    [InlineData("position-length")]
    [InlineData("objdata-count")]
    [InlineData("no-empty-slot")]
    public void Constructor_rejects_invalid_layout(string corruption)
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            switch (corruption)
            {
                case "objects-version": U32(level.Objects, 0, 2); break;
                case "name-width": U32(level.Objects, 8, 29); break;
                case "idname-width": U32(level.Objects, 12, 31); break;
                case "objects-length": level.Objects = level.Objects[..^1]; break;
                case "objdata-length": level.ObjData = level.ObjData[..^1]; break;
                case "position-length": level.Positions = level.Positions[..^1]; break;
                case "objdata-count": U32(level.ObjData, 4, 7); break;
                case "no-empty-slot":
                    for (int slot = 0; slot < LevelFixture.Count; slot++) level.Objects[Record(slot)] = 1;
                    break;
            }
            level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Fact]
    public void Constructor_rejects_wrong_objdata_version()
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            U32(level.ObjData, 0, 2); level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Fact]
    public void Constructor_rejects_wrong_position_version()
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            U32(level.Positions, 0, 2); level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Theory]
    [InlineData(null, false)]
    [InlineData("", false)]
    [InlineData("Lan", true)]
    [InlineData("lanTree", true)]
    [InlineData("LAN_Rock", true)]
    [InlineData("La", false)]
    [InlineData("GerLan", false)]
    [InlineData(" LanTree", false)]
    [InlineData("Ger_House", false)]
    public void IsLandscape_matches_only_case_insensitive_Lan_prefix(string? name, bool expected)
        => Assert.Equal(expected, ObjDefNames.IsLandscape(name));

    [Fact]
    public void Add_with_team_rewrites_only_the_team_field_and_remove_if_uid_checks_identity()
    {
        using var level = new LevelFixture(pfil: false);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        int slot = store.Add(template, 1, 2, 3, 0, team: 0);
        Assert.Equal(0, Assert.Single(store.Objects(), item => item.Slot == slot).Team);
        Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, 2, 3, 0, team: 9));
        uint uid = store.UidAt(slot)!.Value;
        Assert.False(store.RemoveIfUid(slot, uid + 1));
        Assert.True(store.RemoveIfUid(slot, uid));
        Assert.Null(store.UidAt(slot));
    }

    [Fact]
    public void Scenario_identity_survives_reorder_while_runtime_slot_and_uid_change()
    {
        using var level = new LevelFixture(pfil: false);
        LevelObjectStore store = level.Load(); LevelObjectTemplate template = Assert.Single(store.Templates());
        var first = new AgainstRomeModifier.Scripting.ScenarioDocument
        { Spawns = [new("A", 10, 20, 0, Prebuilt: true), new("B", 30, 40, 0, Prebuilt: true)] };
        AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(store, new(), first, _ => template);
        Guid id = first.Spawns[0].Id;
        var before = AgainstRomeModifier.Scripting.ScenarioObjectIdentity.DataBinding(first, id)!;
        var next = new AgainstRomeModifier.Scripting.ScenarioDocument
        { Spawns = first.Spawns.AsEnumerable().Reverse().Select(spawn => spawn with { X = spawn.X + 100 }).ToList(), DataSlots = first.DataSlots.ToList() };
        AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(store, first, next, _ => template);
        var after = AgainstRomeModifier.Scripting.ScenarioObjectIdentity.DataBinding(next, id)!;
        Assert.NotEqual(before.Slot, after.Slot); Assert.NotEqual(before.Uid, after.Uid);
        Assert.Equal(110, store.Objects().Single(item => item.Slot == after.Slot).X);
        Assert.Equal(first.Spawns[0].Id, next.Spawns[1].Id);
    }

    [Fact]
    public void Scenario_level_objects_replace_previous_slots_and_report_missing_templates()
    {
        using var level = new LevelFixture(pfil: false);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        var first = new AgainstRomeModifier.Scripting.ScenarioDocument
        {
            Spawns = [new("HOUSE", 10, 20, 0, Y: 5, Prebuilt: true), new("UNIT", 0, 0, 0, Count: 5), new("UNKNOWN", 1, 1, 3, Prebuilt: true)],
        };
        IReadOnlyList<AgainstRomeModifier.Scripting.ScenarioSpawn> skipped = AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(
            store, new AgainstRomeModifier.Scripting.ScenarioDocument(), first, spawn => spawn.Alias == "HOUSE" ? template : null);
        Assert.Equal("UNKNOWN", Assert.Single(skipped).Alias);
        AgainstRomeModifier.Scripting.ScenarioDataSlot owned = Assert.Single(first.DataSlots);
        LevelWorldObject house = Assert.Single(store.Objects(), item => item.Slot == owned.Slot);
        Assert.Equal((0, 10f, 5f, 20f), (house.Team, house.X, house.Y, house.Z));
        Assert.Single(first.ScriptSpawns, spawn => spawn.Alias == "UNIT");

        var second = new AgainstRomeModifier.Scripting.ScenarioDocument();
        AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(store, first, second, _ => template);
        Assert.Null(store.UidAt(owned.Slot));
        Assert.Empty(second.DataSlots);
    }

    [Theory]
    [InlineData(false, false)]
    [InlineData(true, false)]
    [InlineData(false, true)]
    public void Existing_completed_building_uses_owned_template_only_for_matching_uid_and_alias(bool staleUid, bool changedAlias)
    {
        using var level = new LevelFixture(pfil: false);
        var store = level.Load();
        var template = Assert.Single(store.Templates());
        var first = new AgainstRomeModifier.Scripting.ScenarioDocument
        { Spawns = [new("HOUSE", 10, 20, 0, Prebuilt: true)] };
        AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(store, new(), first, _ => template);
        var binding = Assert.Single(first.DataSlots);
        if (staleUid) first.DataSlots[0] = binding with { Uid = binding.Uid + 1 };
        var next = new AgainstRomeModifier.Scripting.ScenarioDocument
        { Spawns = [first.Spawns[0] with { Alias = changedAlias ? "OTHER" : "HOUSE", X = 600, Team = 3, Angle = 90 }] };
        var skipped = AgainstRomeModifier.Scripting.ScenarioLevelObjects.Apply(store, first, next, _ => null);
        if (staleUid || changedAlias)
        {
            Assert.Single(skipped); Assert.Empty(next.DataSlots);
            if (staleUid) Assert.Equal(binding.Uid, store.UidAt(binding.Slot));
        }
        else
        {
            Assert.Empty(skipped);
            var saved = Assert.Single(next.DataSlots);
            var building = Assert.Single(store.Objects(), item => item.Slot == saved.Slot);
            Assert.Equal((600f, 3), (building.X, building.Team));
            Assert.Equal(MathF.PI / 2, building.Rotation, 5);
            Assert.Equal(first.Spawns[0].Id, saved.SpawnId);
        }
    }

    [Fact]
    public void Owned_template_rejects_linked_invalid_position_and_unowned_slots()
    {
        using var level = new LevelFixture(pfil: false);
        var store = level.Load();
        Assert.Null(store.OwnedTemplate(-1, 1));
        Assert.Null(store.OwnedTemplate(0, 0));
        foreach (int slot in new[] { 3, 4, 5, 6 })
            Assert.Null(store.OwnedTemplate(slot, store.UidAt(slot)!.Value));
    }

    private static int Record(int slot) => 16 + slot * LevelObjectStore.RecordSize;

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Blank_initialization_clears_linked_objects_orphan_positions_and_runtime_then_is_idempotent(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        string stage = BlankStage(level, pfil);
        Assert.Contains(LevelObjectStore.Load(stage).Objects(), obj => obj.Linked);
        // 聚落 SDL 範本必須原樣保留：清成沒有 object 區段會讓遊戲載入時卡死（2026-10-08 遊戲內實測）。
        string sdlPath = Path.Combine(stage, "Endlos_Ger_Siedlung1.sdl");
        File.WriteAllText(sdlPath, "[settlement]\r\nname=Endlos_Ger_Siedlung1\r\nrefpos 1,2,3\r\n\r\n[object0]\r\nobjdef = BauGerHau00_Haupthaus\r\n");
        byte[] sdlBefore = File.ReadAllBytes(sdlPath);
        BlankMapContent.ResetStagingDirectory(stage);
        Assert.Equal(sdlBefore, File.ReadAllBytes(sdlPath));
        Assert.Empty(LevelObjectStore.Load(stage).Objects());
        byte[] positions = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(stage, "DATA", "position.dat")));
        Assert.True(positions.AsSpan(8).IndexOfAnyExcept((byte)0) < 0); // also clears the invalid entry containing -999 and orphan 13
        var before = StageFiles(stage); BlankMapContent.ResetStagingDirectory(stage); Assert.Equal(before, StageFiles(stage));
        Assert.Contains(level.Load().Objects(), obj => obj.Linked);
        Assert.Throws<InvalidOperationException>(() => BlankMapContent.ResetStagingDirectory(level.Map));
    }

    [Theory]
    [InlineData(false, "version")]
    [InlineData(true, "version")]
    [InlineData(false, "full")]
    [InlineData(true, "full")]
    [InlineData(false, "missing-script")]
    [InlineData(true, "missing-script")]
    [InlineData(false, "full-light")]
    [InlineData(true, "full-light")]
    public void Blank_rejects_unknown_or_full_runtime_pools_and_rolls_back_failure_after_writes(bool pfil, string fault)
    {
        using var level = new LevelFixture(pfil);
        string stage = BlankStage(level, pfil), animation = Path.Combine(stage, "DATA", "anim.dat");
        byte[] raw = File.ReadAllBytes(animation), data = GameLZSS.DecompressPfil(raw);
        if (fault == "version") { U32(data, 0, 2); File.WriteAllBytes(animation, pfil ? GameLZSS.CompressPfil(data, raw[..64]) : data); }
        else if (fault == "full")
        {
            for (int slot = 0; slot < 8; slot++) data[8 + slot * 21] = 1;
            File.WriteAllBytes(animation, pfil ? GameLZSS.CompressPfil(data, raw[..64]) : data);
        }
        else if (fault == "full-light")
        {
            string light = Path.Combine(stage, "DATA", "light.dat"); byte[] lighting = File.ReadAllBytes(light), values = GameLZSS.DecompressPfil(lighting);
            // A nonzero high byte is active too; checking only the low byte would pick an occupied template.
            for (int slot = 0; slot < 8; slot++) U32(values, 8 + slot * 4, 256);
            File.WriteAllBytes(light, pfil ? GameLZSS.CompressPfil(values, lighting[..64]) : values);
        }
        else File.Delete(Path.Combine(stage, "SCRIPT", "ak_level.bci"));
        var before = StageFiles(stage);
        if (fault == "missing-script") Assert.Throws<FileNotFoundException>(() => BlankMapContent.ResetStagingDirectory(stage));
        else Assert.Throws<InvalidDataException>(() => BlankMapContent.ResetStagingDirectory(stage));
        Assert.Equal(before, StageFiles(stage)); Assert.NotEmpty(LevelObjectStore.Load(stage).Objects());
    }

    private static SortedDictionary<string, byte[]> StageFiles(string stage) => new(Directory.GetFiles(stage, "*", SearchOption.AllDirectories)
        .ToDictionary(path => Path.GetRelativePath(stage, path), File.ReadAllBytes), StringComparer.Ordinal);

    private static string BlankStage(LevelFixture level, bool pfil)
    {
        string stage = Path.Combine(level.Map, "MAPS", "ENDL_005.tmp_arm");
        Directory.CreateDirectory(Path.Combine(stage, "DATA")); Directory.CreateDirectory(Path.Combine(stage, "SCRIPT"));
        foreach (string file in LevelFixture.Files) File.Copy(Path.Combine(level.Map, "DATA", file), Path.Combine(stage, "DATA", file));
        foreach (var (name, header, record, columns, extra, stateWidth) in new[] {
            ("anim.dat", 8, 21, new[] { 2, 2 }, Array.Empty<int>(), 1), ("gfxtype.dat", 8, 15, new[] { 2 }, Array.Empty<int>(), 1),
            ("action.dat", 12, 25, Array.Empty<int>(), new[] { 6 }, 1), ("hirarchy.dat", 12, 103, new[] { 2 }, new[] { 50 }, 1),
            ("formatio.dat", 8, 15, new[] { 4, 2, 4 }, Array.Empty<int>(), 1), ("lager.dat", 16, 43, new[] { 2 }, new[] { 6, 10 }, 1),
            ("biglager.dat", 12, 1601, Array.Empty<int>(), new[] { 800 }, 1), ("light.dat", 8, 4, Enumerable.Repeat(4, 13).ToArray(), Array.Empty<int>(), 4),
            ("particle.dat", 12, 2252, new[] { 4, 4, 4, 4 }, new[] { 64 }, 2), ("explos.dat", 8, 46, new[] { 4, 4 }, Array.Empty<int>(), 2),
            ("hitex.dat", 8, 28, new[] { 4, 4, 4 }, Array.Empty<int>(), 2), ("flash.dat", 12, 201, new[] { 32 }, new[] { 16 }, 1) })
        {
            var data = Enumerable.Repeat((byte)0xFF, header + 8 * (record + columns.Sum())).ToArray();
            U32(data, 0, 1); U32(data, 4, 8);
            for (int index = 0; index < extra.Length; index++) U32(data, 8 + index * 4, (uint)extra[index]);
            for (int slot = 0; slot < 8; slot++) data[header + slot * record] = slot == 7 ? (byte)0 : (byte)1;
            data.AsSpan(header + 7 * record, stateWidth).Clear();
            byte[] h = new byte[64]; "PFIL"u8.CopyTo(h);
            File.WriteAllBytes(Path.Combine(stage, "DATA", name), pfil ? GameLZSS.CompressPfil(data, h) : data);
        }
        var ways = new byte[12 + 2 * 1030]; U32(ways, 0, 1); U32(ways, 4, 2); U32(ways, 8, 256); ways[12] = 3;
        File.WriteAllBytes(Path.Combine(stage, "DATA", "way.dat"), ways);
        File.WriteAllBytes(Path.Combine(stage, "SCRIPT", "ak_level.bci"), AgainstRomeModifier.Scripting.BciImage.CreateIdleLevel().Serialize());
        return stage;
    }
    private static int Position(int index) => 8 + index * LevelObjectStore.PositionSize;
    private static int Column(int column, int slot) => 16 + LevelFixture.Count * LevelObjectStore.RecordSize
        + LevelFixture.Count * LevelObjectStore.ColumnWidths.Take(column).Sum() + slot * LevelObjectStore.ColumnWidths[column];
    private static int Segment(int segment, int slot) => 8
        + LevelFixture.Count * LevelObjectStore.ObjDataWidths.Take(segment).Sum() + slot * LevelObjectStore.ObjDataWidths[segment];
    private static byte[] Slice(byte[] bytes, int offset, int length) => bytes.AsSpan(offset, length).ToArray();
    private static void U16(byte[] bytes, int offset, int value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), checked((ushort)value));
    private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
    private static float F32(byte[] bytes, int offset) => BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(offset));

    private sealed class LevelFixture : IDisposable
    {
        internal const int Count = 8, PositionCount = 20;
        internal static readonly string[] Files = ["objects.dat", "objdata.dat", "position.dat"];
        private readonly string _root = Path.Combine(Path.GetTempPath(), "LevelObjectStoreTests_" + Guid.NewGuid().ToString("N"));
        private readonly bool _pfil;
        internal string Map => Path.Combine(_root, "MAP");
        internal byte[] Objects = new byte[16 + Count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        internal byte[] ObjData = new byte[8 + Count * LevelObjectStore.ObjDataWidths.Sum()];
        internal byte[] Positions = new byte[8 + PositionCount * LevelObjectStore.PositionSize];

        internal LevelFixture(bool pfil)
        {
            _pfil = pfil;
            Directory.CreateDirectory(Path.Combine(Map, "DATA"));
            U32(Objects, 0, 1); U32(Objects, 4, Count); U32(Objects, 8, 30); U32(Objects, 12, 30);
            U32(ObjData, 0, 1); U32(ObjData, 4, Count);
            U32(Positions, 0, 1); U32(Positions, 4, PositionCount);
            int[] types = [100, 0, 100, 200, 300, 400, 500, 0];
            for (int slot = 0; slot < Count; slot++)
            {
                bool active = slot != 1 && slot != 7;
                int record = Record(slot);
                Objects[record] = active ? (byte)1 : (byte)0;
                U16(Objects, record + 1, active ? slot + 2 : 0x7FFF);
                U32(Objects, record + 3, active ? (uint)(slot + 1) * 10 : 0);
                if (active)
                {
                    SyntheticFixture.GameEncoding.GetBytes($"Name{slot}").CopyTo(Objects, record + 7);
                    SyntheticFixture.GameEncoding.GetBytes($"Id{slot}").CopyTo(Objects, record + 37);
                }
                foreach (int offset in new[] { 67, 69, 71, 73, 75, 77 }) U16(Objects, record + offset, 0xFFFF);
                if (active)
                {
                    int pair = slot == 0 ? 0 : (slot - 1) * 2;
                    U16(Objects, record + 67, slot == 5 ? PositionCount : pair);
                    U16(Objects, record + 69, slot == 6 ? PositionCount : pair + 1);
                    foreach (int offset in new[] { 71, 73, 77 }) U16(Objects, record + offset, slot);
                    U16(Objects, record + 75, types[slot]);
                    foreach (int index in new[] { pair, pair + 1 })
                    {
                        Positions[Position(index)] = 1;
                        float[] values = [slot * 10 + 1, slot * 10 + 2, slot * 10 + 3, slot + .25f];
                        for (int i = 0; i < values.Length; i++)
                            BinaryPrimitives.WriteSingleLittleEndian(Positions.AsSpan(Position(index) + 1 + i * 4), values[i]);
                    }
                }
                for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
                {
                    uint value = column == 1 ? (uint)(active ? slot : 0xFFFF)
                        : column == 10 ? (uint)(slot == 3 ? 4 : 0xFFFF) : (uint)(100 + column * 11 + slot);
                    if (LevelObjectStore.ColumnWidths[column] == 2) U16(Objects, Column(column, slot), (int)value);
                    else U32(Objects, Column(column, slot), 0x12340000 + value);
                }
                for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
                    for (int i = 0; i < LevelObjectStore.ObjDataWidths[segment]; i++)
                        ObjData[Segment(segment, slot) + i] = segment == 2
                            ? (byte)(slot == 4 && i == 3 ? 1 : 0) : (byte)(20 + segment + slot + i);
            }
            // Position 12 is free but 13 is occupied; 14,15 is the first free even pair.
            Positions[Position(13)] = 1;
            BinaryPrimitives.WriteSingleLittleEndian(Positions.AsSpan(Position(12) + 1), -999);
            Write();
        }

        internal LevelObjectStore Load() => LevelObjectStore.Load(Map);
        internal byte[] ReadRaw(string file) => File.ReadAllBytes(Path.Combine(Map, "DATA", file));
        internal byte[] ReadPayload(string file) => GameLZSS.DecompressPfil(ReadRaw(file));
        internal void Write()
        {
            byte[][] payloads = [Objects, ObjData, Positions];
            for (int file = 0; file < Files.Length; file++)
            {
                byte[] header = Enumerable.Range(0, 64).Select(i => (byte)(i + 30 + file)).ToArray();
                "PFIL"u8.CopyTo(header);
                File.WriteAllBytes(Path.Combine(Map, "DATA", Files[file]),
                    _pfil ? GameLZSS.CompressPfil(payloads[file], header) : payloads[file]);
            }
        }

        public void Dispose() => Directory.Delete(_root, recursive: true);
    }
}
