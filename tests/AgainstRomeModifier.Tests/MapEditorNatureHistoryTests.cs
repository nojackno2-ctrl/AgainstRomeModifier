using System.Buffers.Binary;
using System.Collections;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorNatureHistoryTests
{
    [Fact]
    public void Drag_is_one_undo_and_redo_restores_exact_positions_and_rotations()
    {
        Run(form =>
        {
            Paint(form, 10, 10); Paint(form, 14, 10); Invoke(form, "CommitStroke");
            object[] planted = Additions(form).Cast<object>().ToArray();
            Assert.Equal(5, planted.Length);
            Assert.True(Button(form, "_undoButton").Enabled);
            Invoke(form, "Undo");
            Assert.Empty(Additions(form));
            Assert.False(Button(form, "_undoButton").Enabled);
            Assert.True(Button(form, "_redoButton").Enabled);
            Invoke(form, "Redo");
            Assert.Equal(planted, Additions(form).Cast<object>());
            Assert.False(Button(form, "_redoButton").Enabled);
        });
    }

    [Fact]
    public void Removal_undo_redo_and_new_stroke_do_not_replay_texture_history()
    {
        Run(form =>
        {
            Paint(form, 10, 10); Invoke(form, "CommitStroke");
            object planted = Assert.Single(Additions(form).Cast<object>());
            Get<ComboBox>(form, "_natureOperation").SelectedIndex = 1;
            Paint(form, 10, 10); Invoke(form, "CommitStroke");
            Assert.Empty(Additions(form));
            Invoke(form, "Undo");
            Assert.Same(planted, Assert.Single(Additions(form).Cast<object>()));
            Invoke(form, "Redo");
            Assert.Empty(Additions(form));
            Invoke(form, "Undo");
            Get<ComboBox>(form, "_natureOperation").SelectedIndex = 0;
            Paint(form, 20, 20); Invoke(form, "CommitStroke");
            Assert.False(Button(form, "_redoButton").Enabled);
            Invoke(form, "Redo");
            Assert.Equal(2, Additions(form).Count);
        });
    }

    [Fact]
    public void Removal_of_existing_objects_is_reversible_and_linked_objects_are_preserved()
    {
        Run(form =>
        {
            const float position = 10.5f * SdlSceneCatalog.WorldUnitsPerMapPixel * 4;
            Set(form, "_levelObjects", new LevelWorldObject[]
            {
                new(0, 1, 8, 1, position, 0, position, 0, false),
                new(1, 1, 8, 2, position, 0, position, 0, true),
            });
            Get<ComboBox>(form, "_natureOperation").SelectedIndex = 1;
            Paint(form, 10, 10); Invoke(form, "CommitStroke");
            Assert.Equal(new[] { 0 }, Get<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").RemovedSlots);
            Invoke(form, "Undo");
            Assert.Empty(Get<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").RemovedSlots);
            Invoke(form, "Redo");
            Assert.Equal(new[] { 0 }, Get<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").RemovedSlots);
        });
    }

    [Fact]
    public void Picked_nature_objects_are_deleted_with_undo_in_move_mode_and_fixed_objects_are_kept()
    {
        Run(form =>
        {
            const float position = 10.5f * SdlSceneCatalog.WorldUnitsPerMapPixel * 4;
            Set(form, "_levelObjects", new LevelWorldObject[]
            {
                new(0, 1, 8, 1, position, 0, position, 0, false),
                new(1, 1, 8, 2, position, 0, position, 0, true), // linked: must stay
            });
            Paint(form, 20, 20); Invoke(form, "CommitStroke"); // one pending planted addition
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "SceneMove"));
            var session = Get<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession");
            MapSceneObject Data(int index) => new("LanGerLau00", position, 0, position, 8, "DATA/objects.dat", index);

            Invoke(form, "SelectPickedSceneObject", Data(-100001)); // linked slot 1
            Assert.False((bool)Invoke(form, "DeletePickedNature")!);
            Invoke(form, "SelectPickedSceneObject", Data(-100000)); // removable slot 0
            Assert.True((bool)Invoke(form, "DeletePickedNature")!);
            Assert.Equal(new[] { 0 }, session.RemovedSlots);
            Assert.False((bool)Invoke(form, "DeletePickedNature")!); // the pick is consumed
            Assert.True(Button(form, "_undoButton").Enabled);
            Invoke(form, "Undo");
            Assert.Empty(session.RemovedSlots);
            Invoke(form, "Redo");
            Assert.Equal(new[] { 0 }, session.RemovedSlots);

            Invoke(form, "SelectPickedSceneObject", Data(-200000)); // the pending planted addition
            Assert.True((bool)Invoke(form, "DeletePickedNature")!);
            Assert.Empty(Additions(form));
            Invoke(form, "Undo");
            Assert.Single(Additions(form).Cast<object>());
        });
    }

    [Fact]
    public void Empty_valid_world_store_keeps_nature_tool_enabled_and_clears_old_history()
    {
        Run(form =>
        {
            Paint(form, 10, 10); Invoke(form, "Undo");
            Assert.True(Button(form, "_redoButton").Enabled);
            string map = Get<GameMapInfo>(form, "_selected").DirectoryPath;
            Directory.CreateDirectory(Path.Combine(map, "DATA"));
            const int count = 8;
            byte[] objects = new byte[16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
            BinaryPrimitives.WriteInt32LittleEndian(objects, 1);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), count);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);
            Write("objects.dat", objects);
            byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
            BinaryPrimitives.WriteInt32LittleEndian(data, 1);
            BinaryPrimitives.WriteInt32LittleEndian(data.AsSpan(4), count);
            Write("objdata.dat", data);
            byte[] positions = new byte[8 + 16 * LevelObjectStore.PositionSize];
            BinaryPrimitives.WriteInt32LittleEndian(positions, 1);
            BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), 16);
            Write("position.dat", positions);
            Invoke(form, "LoadLevelObjects", map); Invoke(form, "UpdateEditorState");
            Assert.True(Button(form, "_natureTool").Enabled);
            Assert.False(Button(form, "_redoButton").Enabled);
            Paint(form, 12, 12); Invoke(form, "CommitStroke");
            Assert.Single(Additions(form).Cast<object>());
            void Write(string name, byte[] bytes) => File.WriteAllBytes(Path.Combine(map, "DATA", name), bytes);
        });
    }

    private static void Run(Action<MapEditorForm> action)
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmNatureHistory_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try
            {
                string path = Path.Combine(root, "boden.txt");
                File.WriteAllText(path, "[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", Enumerable.Repeat("4BB___51", 4096)));
                using var form = new MapEditorForm(root, new GameMapInfo("ENDL_005", root, true, "Nature", "Test"));
                Set(form, "_texturesDocument", BodenTexturesDocument.Load(path));
                Set(form, "_objdefNames", new Dictionary<int, string> { [1] = "LanGerLau00" });
                Set(form, "_natureCatalogTask", Task.FromResult<IReadOnlyDictionary<int, LevelObjectTemplate>>(new Dictionary<int, LevelObjectTemplate>()));
                var template = (LevelObjectTemplate)Activator.CreateInstance(typeof(LevelObjectTemplate), BindingFlags.Instance | BindingFlags.NonPublic, null,
                    new object[] { 1, new byte[79], new uint[18], Array.Empty<byte[]>(), new byte[17], new byte[17] }, null)!;
                Type itemType = typeof(MapEditorForm).GetNestedType("NatureTypeItem", BindingFlags.NonPublic)!;
                Get<ListBox>(form, "_natureTypes").Items.Add(Activator.CreateInstance(itemType, template, "LanGerLau00")!);
                Get<ListBox>(form, "_natureTypes").SelectedIndex = 0;
                Get<MapCanvasControl>(form, "_canvas").BrushSize = 1;
                Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                Invoke(form, "SetEditMode", Enum.Parse(mode, "Nature"));
                action(form);
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(30)), "Nature history test timed out.");
        try { Assert.Null(failure); }
        finally { Directory.Delete(root, true); }
    }

    [Fact]
    public void Large_brush_scatters_mixed_species_with_spacing_as_one_undo_step()
    {
        Run(form =>
        {
            var types = Get<ListBox>(form, "_natureTypes");
            object first = types.Items[0];
            var template = first.GetType().GetProperty("Template")!.GetValue(first)!;
            types.Items.Add(Activator.CreateInstance(first.GetType(), template, "LanGerBir00")!);
            types.Items.Add(Activator.CreateInstance(first.GetType(), template, "LanItaZyp06")!); // 不同地區，混合時不得選入
            Get<MapCanvasControl>(form, "_canvas").BrushSize = 9;
            Get<ComboBox>(form, "_natureDensity").SelectedIndex = 2;
            Get<CheckBox>(form, "_natureMix").Checked = true;
            Paint(form, 20, 20); Invoke(form, "CommitStroke");
            var planted = Additions(form).Cast<AgainstRomeMapEditor.Modules.Nature.NatureAddition>().ToArray();
            Assert.True(planted.Length > 10, $"9×9 茂密只種了 {planted.Length} 株");
            Assert.Equal(["LanGerBir00", "LanGerLau00"], planted.Select(item => item.Name).Distinct().Order());
            float tile = AgainstRomeModifier.Maps.SdlSceneCatalog.WorldUnitsPerMapPixel * 4f;
            for (int i = 0; i < planted.Length; i++)
                for (int j = i + 1; j < planted.Length; j++)
                    Assert.True(MathF.Sqrt(MathF.Pow(planted[i].X - planted[j].X, 2) + MathF.Pow(planted[i].Z - planted[j].Z, 2)) >= .45f * tile - .01f);
            Invoke(form, "Undo");
            Assert.Empty(Additions(form));
        });
    }

    private static IList Additions(MapEditorForm form) => (IList)Get<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").Additions;
    private static ToolStripButton Button(MapEditorForm form, string name) => Get<ToolStripButton>(form, name);
    private static void Paint(MapEditorForm form, int x, int y) => Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, y, "", ""));
    private static T Get<T>(object target, string name) => (T)target.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(target)!;
    private static void Set(object target, string name, object value) => target.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(target, value);
    private static object? Invoke(object target, string name, params object[] args) => target.GetType().GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(target, args);
}
