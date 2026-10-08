using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Settlement_generator_dialog_applies_with_single_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "SettlementGen", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Setup mock catalog for Germanic buildings and wildlife
            var catalog = new List<SdlObjectType>();
            string[] requiredTypes = ["BauGerHau00", "BauGerLag00", "BauGerWoh00", "BauGerKas00", "BauGerSch00", "BauGerTur00", "FigHir00", "FigSch00"];
            foreach (var typeName in requiredTypes)
            {
                catalog.Add(new SdlObjectType(typeName, 1, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = typeName }));
            }
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);

            int initialPlaced = form.PlacementSession.Count;
            Assert.Equal(0, initialPlaced);

            // 1. Cancel dialog -> no changes
            form.SettlementGeneratorDialogRunner = _ => DialogResult.Cancel;
            form.RunSettlementGenerator();
            Assert.Equal(0, form.PlacementSession.Count);

            // 2. Mock DialogRunner to configure 2 players, Germanic, seed 42
            form.SettlementGeneratorDialogRunner = dialog =>
            {
                dialog.PlayerCount = 2;
                dialog.SelectedTribe = SettlementTribe.Germanic;
                dialog.Seed = 42;
                dialog.IncludeNature = false;
                return DialogResult.OK;
            };

            form.RunSettlementGenerator();

            int generatedCount = form.PlacementSession.Count;
            Assert.True(generatedCount >= 12, $"Expected at least 12 placed objects for 2 players, but got {generatedCount}");

            // 3. Single Undo -> all placements undone in 1 step
            Invoke(form, "Undo");
            Assert.Equal(0, form.PlacementSession.Count);

            // 4. Redo -> restored
            Invoke(form, "Redo");
            Assert.Equal(generatedCount, form.PlacementSession.Count);
        });
    }
}
