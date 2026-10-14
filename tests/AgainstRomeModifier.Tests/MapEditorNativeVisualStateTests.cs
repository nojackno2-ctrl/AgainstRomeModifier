using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

// Reuse the host's STA, map fixture, reflection and synthetic ALR helpers.
public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Native_visual_state_is_optional_without_changing_scene_positional_API()
    {
        var scene = new MapSceneObject("Legacy", 1, 2, 3, 4, "legacy.sdl", 5, 6, 7, 8, 9);
        var (name, x, y, z, team, source, index, localX, localY, localZ, angle) = scene;
        Assert.Equal(("Legacy", 1f, 2f, 3f, 4, "legacy.sdl", 5, 6f, 7f, 8f, (float?)9),
            (name, x, y, z, team, source, index, localX, localY, localZ, angle));
        Assert.Null(scene.NativeVisualState);
        var state = new NativeAnimationVisualState(-1, -2, 3, -4, 5);
        Assert.Same(state, (scene with { NativeVisualState = state }).NativeVisualState);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Native_visual_state_loads_child_own_animation_and_existing_nature_only(bool pfil)
    {
        string map = CreateFixture();
        var pools = new NativeVisualFixture(map, pfil);
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = NativeVisualForm(map);
            Invoke(form, "LoadLevelObjects", map);
            var scene = NativeVisualScene(form);
            var root = Assert.Single(scene, item => item.ObjectIndex == -100000);
            var child = Assert.Single(scene, item => item.ObjectIndex == -100002);
            var nature = Assert.Single(scene, item => item.ObjectIndex == -100004);
            Assert.Equal(NativeVisualFixture.RootState, root.NativeVisualState);
            Assert.Equal(NativeVisualFixture.ChildState, child.NativeVisualState);
            Assert.NotEqual(root.NativeVisualState, child.NativeVisualState);
            Assert.Equal(NativeVisualFixture.NatureState, nature.NativeVisualState);
            Assert.Equal(3, root.Team); Assert.Equal(4, child.Team); Assert.Equal(5, nature.Team);
            Assert.Equal(37.25f, root.Angle!.Value, 4);
            Assert.Equal(143.5f, child.Angle!.Value, 4);
            Assert.Equal(271.125f, nature.Angle!.Value, 4);

            // A bare slot set is not a UID-validated family binding; it cannot hide native data.
            GetField<HashSet<int>>(form, "_boundRootSlots").Add(0);
            Assert.Equal(NativeVisualFixture.RootState,
                Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100000).NativeVisualState);
            Assert.Equal(NativeVisualFixture.ChildState,
                Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100002).NativeVisualState);

            var template = Assert.IsType<LevelObjectTemplate>(LevelObjectStore.Load(map).OwnedTemplate(4, 300));
            var session = GetField<NatureEditSession>(form, "_natureSession");
            session.Plant(new NatureAddition(template, "LanGerNad18", 123, 0, 456, 1));
            var fresh = Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -200000);
            Assert.Null(fresh.NativeVisualState);
            Assert.Equal(8, fresh.Team); Assert.Null(fresh.Angle);
            Assert.All(SdlSceneCatalog.LoadDirectory(map), item => Assert.Null(item.NativeVisualState));

            // Nature mode sends these same snapshots via EffectiveSceneObjects, once each.
            Type editMode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            typeof(MapEditorForm).GetField("_editMode", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, Enum.Parse(editMode, "Nature"));
            var effective = (IReadOnlyList<MapSceneObject>)Invoke(form, "EffectiveSceneObjects")!;
            var natureMode = (IReadOnlyList<MapSceneObject>)Invoke(form, "SceneObjectsFor3D", (object)effective)!;
            Assert.Equal(NativeVisualFixture.NatureState,
                Assert.Single(natureMode, item => item.ObjectIndex == -100004).NativeVisualState);
            Assert.Null(Assert.Single(natureMode, item => item.ObjectIndex == -200000).NativeVisualState);
            typeof(MapEditorForm).GetField("_editMode", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, Enum.Parse(editMode, "Texture"));

            session.Remove([4], []);
            Assert.DoesNotContain(NativeVisualScene(form), item => item.ObjectIndex == -100004);
        });
        AssertNativeVisualSnapshot(map, before);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Native_visual_state_rejects_replaced_slot_UID_and_refreshes_on_map_reload(bool pfil)
    {
        string map = CreateFixture();
        var pools = new NativeVisualFixture(map, pfil);
        RunInSta(() =>
        {
            using var form = NativeVisualForm(map);
            Invoke(form, "LoadLevelObjects", map);
            var oldState = Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100000).NativeVisualState;
            var objects = GetField<IReadOnlyList<LevelWorldObject>>(form, "_levelObjects");
            typeof(MapEditorForm).GetField("_levelObjects", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                objects.Select(item => item.Slot == 0 ? item with { Uid = item.Uid ^ 0x01000000u } : item).ToArray());
            Assert.Null(Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100000).NativeVisualState);

            // A replacement in the SAME slot must not reuse the previous UID's snapshot.
            BinaryPrimitives.WriteUInt32LittleEndian(pools.Objects.AsSpan(16 + 3), 0xAC000064u);
            BinaryPrimitives.WriteUInt16LittleEndian(pools.Objects.AsSpan(16 + 71), ushort.MaxValue);
            pools.WriteObjects();
            var before = SnapshotDirectory(map);
            Invoke(form, "LoadLevelObjects", map);
            Assert.Null(Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100000).NativeVisualState);
            Assert.Equal(NativeVisualFixture.RootState, oldState);
            AssertNativeVisualSnapshot(map, before);

            BinaryPrimitives.WriteUInt16LittleEndian(pools.Objects.AsSpan(16 + 71), 3);
            BinaryPrimitives.WriteInt16LittleEndian(pools.Anim.AsSpan(8 + 3 * 21 + 5), 0);
            pools.WriteObjects(); pools.WriteAnim();
            before = SnapshotDirectory(map);
            Invoke(form, "LoadLevelObjects", map);
            Assert.Equal((short)0, Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100000).NativeVisualState!.AlrFrame);
            Assert.Equal(NativeVisualFixture.RootState, oldState);
            AssertNativeVisualSnapshot(map, before);

            string otherMap = CreateFixture("ENDL_006");
            Invoke(form, "LoadLevelObjects", otherMap); // No native objects pool in this map.
            Assert.Empty(NativeVisualScene(form));
            Assert.Empty(GetField<IReadOnlyList<LevelWorldObject>>(form, "_levelObjects"));
            // Even if an old object is accidentally retained elsewhere, no old cache remains.
            Assert.Null(Invoke(form, "LoadedVisualState", objects[0]));
        });
    }

    [Theory]
    [InlineData(false, "objects-missing")]
    [InlineData(true, "objects-missing")]
    [InlineData(false, "objects-corrupt")]
    [InlineData(true, "objects-corrupt")]
    [InlineData(false, "anim-missing")]
    [InlineData(true, "anim-missing")]
    [InlineData(false, "anim-corrupt")]
    [InlineData(true, "anim-corrupt")]
    public void Native_visual_state_reload_clears_missing_or_corrupt_saved_state(bool pfil, string fault)
    {
        string map = CreateFixture();
        var pools = new NativeVisualFixture(map, pfil);
        RunInSta(() =>
        {
            using var form = NativeVisualForm(map);
            Invoke(form, "LoadLevelObjects", map);
            var objects = GetField<IReadOnlyList<LevelWorldObject>>(form, "_levelObjects");
            Assert.NotNull(Invoke(form, "LoadedVisualState", objects[0]));
            string pool = fault.StartsWith("objects", StringComparison.Ordinal) ? "objects.dat" : "anim.dat";
            if (fault.EndsWith("missing", StringComparison.Ordinal)) File.Delete(Path.Combine(map, "DATA", pool));
            else pools.Write(pool, new byte[3]);
            var before = SnapshotDirectory(map);
            Invoke(form, "LoadLevelObjects", map);
            Assert.Null(Invoke(form, "LoadedVisualState", objects[0]));
            Assert.All(NativeVisualScene(form), item => Assert.Null(item.NativeVisualState));
            if (pool == "objects.dat") Assert.Empty(NativeVisualScene(form));
            else
            {
                // Legacy fallback is unchanged for Nature when no saved state is available.
                var nature = Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -100004);
                Assert.Equal(8, nature.Team); Assert.Null(nature.Angle);
            }
            AssertNativeVisualSnapshot(map, before);
        });
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Native_visual_state_transfers_through_host_3D_sprite_and_animation_paths_without_disk_reads(bool pfil)
    {
        string map = CreateFixture();
        var pools = new NativeVisualFixture(map, pfil);
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = NativeVisualForm(map);
            Invoke(form, "LoadLevelObjects", map);
            byte[] alr = SolidIdleAlr(8);
            string Definition(int id, string name) => string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
            {
                0 => id.ToString(), 2 => "1000", 5 => "0", 6 => "0", 8 => "-1", 10 => "-1", 14 => "-1",
                17 => "1", 45 => "00008008", 52 => name, _ => "0"
            }));
            using var catalog = NativeSpriteCatalog.FromText(string.Join("\n",
                Definition(555, "UnitRoot"), Definition(556, "UnitChild"), Definition(557, "LanGerNad18")),
                "0,idle.alr", "", _ => alr, _ => null);
            var view = GetField<Map3DViewControl>(form, "_view3d");
            view.AnimationTimeMs = 0;
            view.SpriteCatalog = catalog;
            Invoke(form, "PushSceneObjects", (object)Array.Empty<MapSceneObject>());
            var scene = (IReadOnlyList<MapSceneObject>)typeof(Map3DViewControl).GetField("_objects", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            Assert.Equal(3, scene.Count);
            Assert.Equal(3, view.AnimatedObjectCount);
            Assert.False(view.AnimationTimerRunning);
            NativeSprite?[] Sprites() => (NativeSprite?[])typeof(Map3DViewControl).GetField("_objectSprites", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            for (int i = 0; i < scene.Count; i++)
                Assert.Same(catalog.GetSprite(scene[i].Name, scene[i].Team, angleDegrees: scene[i].Angle,
                    visualState: scene[i].NativeVisualState), Sprites()[i]);
            Assert.False(Sprites()[0]!.ArgbPixels.SequenceEqual(Sprites()[1]!.ArgbPixels));

            // Corrupt the fixture AFTER loading: pushing/rendering the detached scene must not reload DATA.
            pools.Write("objects.dat", new byte[3]); pools.Write("anim.dat", new byte[3]);
            var before = SnapshotDirectory(map);
            Invoke(form, "PushSceneObjects", (object)Array.Empty<MapSceneObject>());
            view.AnimationTimeMs = 500;
            for (int i = 0; i < scene.Count; i++)
                Assert.Same(catalog.GetAnimation(scene[i].Name, scene[i].Team, scene[i].Angle,
                    visualState: scene[i].NativeVisualState)!.Frames[1], Sprites()[i]);
            view.AnimationsEnabled = false;
            for (int i = 0; i < scene.Count; i++)
                Assert.Same(catalog.GetSprite(scene[i].Name, scene[i].Team, angleDegrees: scene[i].Angle,
                    visualState: scene[i].NativeVisualState), Sprites()[i]);
            AssertNativeVisualSnapshot(map, before);
            view.SpriteCatalog = null;
        });
    }

    private MapEditorForm NativeVisualForm(string map)
    {
        var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Native visual state", "Test"));
        typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
            new Dictionary<int, string> { [555] = "UnitRoot", [556] = "UnitChild", [557] = "LanGerNad18" });
        return form;
    }

    private static IReadOnlyList<MapSceneObject> NativeVisualScene(MapEditorForm form)
        => (IReadOnlyList<MapSceneObject>)Invoke(form, "SceneObjectsFor3D", (object)Array.Empty<MapSceneObject>())!;

    private static void AssertNativeVisualSnapshot(string map, Dictionary<string, byte[]> expected)
    {
        var actual = SnapshotDirectory(map);
        Assert.Equal(expected.Keys.Order(), actual.Keys.Order());
        foreach (var (name, bytes) in expected) Assert.Equal(bytes, actual[name]);
    }

    private sealed class NativeVisualFixture
    {
        private const ushort Count = 6;
        public static readonly NativeAnimationVisualState RootState = new(1, -234, 345, short.MinValue, short.MaxValue);
        public static readonly NativeAnimationVisualState ChildState = new(0, 11, -22, 33, -44);
        public static readonly NativeAnimationVisualState NatureState = new(1, -7, 8, -9, 10);
        private readonly string _map;
        private readonly bool _pfil;
        public byte[] Objects { get; } = new byte[16 + Count * (79 + LevelObjectStore.ColumnWidths.Sum())];
        public byte[] Anim { get; } = new byte[8 + Count * 25];

        public NativeVisualFixture(string map, bool pfil)
        {
            _map = map; _pfil = pfil;
            U32(Objects, 0, 1); U32(Objects, 4, Count); U32(Objects, 8, 30); U32(Objects, 12, 30);
            for (int slot = 0; slot < Count; slot++)
            {
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 }) U16(Objects, 16 + slot * 79 + field, ushort.MaxValue);
                for (int col = 0; col < LevelObjectStore.ColumnWidths.Length; col++)
                    if (LevelObjectStore.ColumnWidths[col] == 2) U16(Objects, Column(col, slot), ushort.MaxValue);
            }
            byte[] objdata = new byte[8 + Count * LevelObjectStore.ObjDataWidths.Sum()];
            byte[] positions = new byte[8 + Count * 17];
            U32(objdata, 0, 1); U32(objdata, 4, Count);
            U32(positions, 0, 1); U32(positions, 4, Count);
            U32(Anim, 0, 1); U32(Anim, 4, Count);
            foreach (var (slot, type, team, uid, anim, angle, state) in new[]
            {
                (0, 555, 3, 0xAB000064u, 3, 37.25f, RootState),
                (2, 556, 4, 200u, 1, 143.5f, ChildState),
                (4, 557, 5, 300u, 5, 271.125f, NatureState)
            })
            {
                int core = 16 + slot * 79;
                Objects[core] = 1; U16(Objects, core + 1, (ushort)team); U32(Objects, core + 3, uid);
                U16(Objects, core + 67, (ushort)slot); U16(Objects, core + 69, (ushort)slot);
                U16(Objects, core + 71, (ushort)anim); U16(Objects, core + 75, (ushort)type);
                U16(Objects, Column(1, slot), (ushort)slot);
                objdata[8 + slot * 9] = 1;
                int pos = 8 + slot * 17; positions[pos] = 1;
                BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(pos + 1), 100 + slot * 256);
                BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(pos + 9), 200 + slot * 256);
                BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(pos + 13), angle);
                int record = 8 + anim * 21; Anim[record] = 1;
                I16(Anim, record + 5, state.AlrFrame); I16(Anim, record + 11, 777);
                I16(Anim, record + 13, state.AptAxis0); I16(Anim, record + 17, state.AptAxis1);
                I16(Anim, record + 15, state.AptAxis2); I16(Anim, record + 19, state.AptAxis3);
            }
            U16(Objects, Column(13, 2), 0); // Child uses anim slot 1, never the root's slot 3.
            WriteObjects(); WriteAnim(); Write("objdata.dat", objdata); Write("position.dat", positions);
        }

        private static int Column(int col, int slot) => 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(col).Sum()) + slot * LevelObjectStore.ColumnWidths[col];
        private static void U16(byte[] bytes, int offset, ushort value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), value);
        private static void I16(byte[] bytes, int offset, short value) => BinaryPrimitives.WriteInt16LittleEndian(bytes.AsSpan(offset), value);
        private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
        public void WriteObjects() => Write("objects.dat", Objects);
        public void WriteAnim() => Write("anim.dat", Anim);
        public void Write(string name, byte[] payload)
        {
            byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
            File.WriteAllBytes(Path.Combine(_map, "DATA", name), _pfil ? GameLZSS.CompressPfil(payload, header) : payload);
        }
    }
}
