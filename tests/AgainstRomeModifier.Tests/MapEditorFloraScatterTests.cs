using System.Drawing;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Flora_scatter_tool_plants_in_rectangle_with_single_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "FloraTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var names = new Dictionary<int, string> { [42] = "LanGerNad00_Tanne_gross", [43] = "LanGerLau00_Buche_gross", [44] = "LanGerBus00_Busch" };
            var templates = names.ToDictionary(pair => pair.Key, pair => (LevelObjectTemplate)Activator.CreateInstance(typeof(LevelObjectTemplate),
                BindingFlags.Instance | BindingFlags.NonPublic, null,
                new object[] { pair.Key, new byte[79], new uint[18], Array.Empty<byte[]>(), new byte[17], new byte[17] }, null)!);
            void Set(string field, object value) => typeof(MapEditorForm).GetField(field, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, value);
            Set("_objdefNames", names);
            Set("_natureTemplates", templates);
            Set("_natureStoreAvailable", true);

            GetField<System.Windows.Forms.NumericUpDown>(form, "_waterLevel").Value = 0; // fixture 高度低於預設水位，會被視為深水
            int planted = form.ApplyFloraScatter(new Rectangle(20, 20, 24, 24));
            var additions = GetField<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").Additions;
            Assert.True(planted > 0, "應散播至少一株植被：" + GetField<System.Windows.Forms.ToolStripStatusLabel>(form, "_status").Text);
            Assert.Equal(planted, additions.Count);
            Assert.All(additions, item => Assert.InRange(item.X / 256f, 20, 44));

            Invoke(form, "Undo");
            Assert.Empty(GetField<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").Additions);
            Invoke(form, "Redo");
            Assert.Equal(planted, GetField<AgainstRomeMapEditor.Modules.Nature.NatureEditSession>(form, "_natureSession").Additions.Count);
        });
    }
}
