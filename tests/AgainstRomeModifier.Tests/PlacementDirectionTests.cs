using System.Buffers.Binary;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(0)]
    [InlineData(315)]
    [InlineData(359)]
    public void Placement_direction_keyboard_undo_redo_save_reload(int initialAngle)
    {
        string map = CreateFixture("ENDL_008");
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        RunInSta(() =>
        {
            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_008", map, true, "Direction", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
            var input = GetField<NumericUpDown>(form, "_placeAngle");
            input.Value = initialAngle;
            form.HandlePlacementRotationKey(new(Keys.R), false);
            int expected = (initialAngle + 45) % 360;
            Assert.Equal(expected, input.Value);
            form.PlacementSession.Add(new(type, 4000, 0, 5000, 0, (float)input.Value, 1));
            Guid id = form.PlacementSession[0].ScenarioId;
            Invoke(form, "RefreshPlacedList");
            _ = form.PlacedList.Handle;
            form.PlacedList.Items[0].Selected = true;
            form.HandlePlacementRotationKey(new(Keys.R | Keys.Shift), true);
            Assert.Equal(initialAngle, form.PlacementSession[0].Angle);
            Assert.True(form.PlacementSession.Undo());
            Assert.Equal(expected, form.PlacementSession[0].Angle);
            Assert.True(form.PlacementSession.Redo());
            Assert.Equal(initialAngle, form.PlacementSession[0].Angle);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Assert.Equal(initialAngle, Assert.Single(ScenarioDocument.Load(map).Spawns).Angle);
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(initialAngle, form.PlacementSession[0].Angle);
            Assert.Equal(id, form.PlacementSession[0].ScenarioId);
        });
    }

    [Fact]
    public void Real_map_position_direction_is_float_degrees_at_record_offset_13()
    {
        const string map = @"C:\Program Files (x86)\Against Rome\MAPS\ENDL_000";
        Assert.True(Directory.Exists(map), "Real map required for direction contract evidence.");
        byte[] payload = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map, "DATA", "position.dat")));
        int count = BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(4));
        Assert.Equal(8 + count * 17, payload.Length);
        var angles = Enumerable.Range(0, count).Where(i => payload[8 + i * 17] != 0)
            .Select(i => BinaryPrimitives.ReadSingleLittleEndian(payload.AsSpan(8 + i * 17 + 13))).ToArray();
        Assert.NotEmpty(angles);
        Console.WriteLine($"ENDL_000 active position directions: count={angles.Length}, min={angles.Min()}, max={angles.Max()}");
        Assert.All(angles, angle => Assert.True(float.IsFinite(angle)));
        // Degree-sized values distinguish this field from a radians encoding.
        Assert.Contains(angles, angle => angle > 2 * Math.PI && angle <= 360);
        Assert.Equal(17, LevelObjectStore.PositionSize);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(45)]
    [InlineData(359)]
    public void Placement_direction_DATA_save_reload_preserves_degree_field(int degrees)
    {
        string map = CreateFixture("ENDL_009");
        WriteEmptyWorldStore(map, 8);
        byte[] record = new byte[79];
        record[0] = 1;
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(75), 1);
        uint[] columns = new uint[18];
        foreach (int column in new[] { 2, 3, 4, 5, 8, 10, 11, 12, 13 }) columns[column] = 0xFFFF;
        var template = new LevelObjectTemplate(1, record, columns,
            LevelObjectStore.ObjDataWidths.Select(width => new byte[width]).ToArray(), new byte[17], new byte[17]);
        var store = LevelObjectStore.Load(map);
        int slot = store.Add(template, 4000, 0, 5000, (float)(degrees * Math.PI / 180d), 0);
        using (var rollback = new FileRollbackScope())
        {
            store.Save(map, rollback);
            rollback.Commit();
        }
        var reloaded = LevelObjectStore.Load(map);
        Assert.Equal(degrees, reloaded.Objects().Single(x => x.Slot == slot).Rotation * 180d / Math.PI, 3);
        var saved = reloaded.OwnedTemplate(slot, reloaded.Objects().Single(x => x.Slot == slot).Uid)!;
        Assert.Equal(degrees, BinaryPrimitives.ReadSingleLittleEndian(saved.Position0.AsSpan(13)), 3);
        Assert.Equal(degrees, BinaryPrimitives.ReadSingleLittleEndian(saved.Position1.AsSpan(13)), 3);
    }
}
