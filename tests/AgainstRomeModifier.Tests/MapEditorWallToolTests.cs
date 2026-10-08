using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Wall_tool_places_available_pieces_with_single_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "WallTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            // 真實 cl_scint.ini 命名；沒有塔與端蓋，工具應退回直牆與轉角。
            var catalog = new[]
            {
                ("GER_PAL00", "BauGerPal00_Palisade"), ("GER_PAL02", "BauGerPal02_Palisadenecke"), ("GER_TOR00", "BauGerTor00_Palisadentor_auf"),
            }.Select(item => new SdlObjectType(item.Item2, -1, SdlObjectCategory.Building, "Ger", 0, new Dictionary<string, string> { ["alias"] = item.Item1 })).ToArray();
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);

            int placed = form.ApplyWallTool([(10, 10), (14, 10), (14, 14)], team: 1);
            Assert.True(placed >= 8, "應沿折線放置多件城牆：" + GetField<System.Windows.Forms.ToolStripStatusLabel>(form, "_status").Text);
            var session = GetField<AgainstRomeMapEditor.Modules.Placement.PlacementEditSession>(form, "_placementSession");
            Assert.Equal(placed, session.Count);
            Assert.All(Enumerable.Range(0, session.Count), index => Assert.Contains(session[index].Type.NameDef, catalog.Select(item => item.NameDef)));

            Invoke(form, "Undo");
            Assert.Equal(0, session.Count);
            Invoke(form, "Redo");
            Assert.Equal(placed, session.Count);
        });
    }
}
