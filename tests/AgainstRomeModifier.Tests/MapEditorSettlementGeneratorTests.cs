using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

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

            // Small real-alias fixture, independent of the generator and game assets.
            var catalog = ScriptObjectAliases.Parse("""
                [ObjDefName]
                GER_HAU00 = BauGerHau00_Haupthaus
                GER_LAG00 = BauGerLag00_Lagerhaus
                GER_WOH00 = BauGerWoh00_Wohnhaus
                GER_BAU00 = BauGerBau00_Bauernhof
                GER_WAF00 = BauGerWaf00_Waffenschmiede
                GER_STA00 = BauGerSta00_Pferdestall
                GER_SCHRE00 = BauGerSchre00_Schreinerei
                ALL_EBE00 = FigTieEbe00_Wildschwein
                """).Select(alias => new SdlObjectType(alias.NameDef, -1, alias.Category, alias.Tribe, 0,
                    new Dictionary<string, string> { ["alias"] = alias.Alias })).ToList();
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
            Assert.True(generatedCount > 14, $"Expected 14 buildings plus wildlife, but got {generatedCount}");
            Assert.Equal(14, Enumerable.Range(0, generatedCount).Count(i => form.PlacementSession[i].Type.Category == SdlObjectCategory.Building));
            foreach (var alias in catalog.Where(t => t.Category == SdlObjectCategory.Building))
                Assert.Equal(2, Enumerable.Range(0, generatedCount).Count(i => form.PlacementSession[i].Type.NameDef == alias.NameDef));

            Assert.All(Enumerable.Range(0, generatedCount).Select(i => form.PlacementSession[i]).Where(p => p.Type.IsAnimal),
                animal => { Assert.Equal(-1, animal.Team); Assert.Equal(0, animal.UnitCount); });

            // 3. Single Undo -> all placements undone in 1 step
            Invoke(form, "Undo");
            Assert.Equal(0, form.PlacementSession.Count);

            // 4. Redo -> restored
            Invoke(form, "Redo");
            Assert.Equal(generatedCount, form.PlacementSession.Count);
        });
    }
}
