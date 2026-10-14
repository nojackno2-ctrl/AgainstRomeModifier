using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Saved_and_duplicate_family_diagnostics_report_no_construction_site_warning_while_invalid_intents_are_rejected()
    {
        string map = CreateFixture("ENDL_012");
        const int count = 10;
        SetupSyntheticFamilyWorldStore(map, count);

        // Previous Scenario: 預建物件與 DataSlot 綁定至 slot 0 (root UID 100)
        Guid sourceGuid = Guid.NewGuid();
        var initialScenario = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: 0, Y: 20f, Prebuilt: true) { Id = sourceGuid }],
            DataSlots = [new ScenarioDataSlot(0, 100u) { SpawnId = sourceGuid }],
            Events = []
        };
        using (var rollback = new FileRollbackScope()) { initialScenario.Save(map, rollback); rollback.Commit(); }

        RunInSta(() =>
        {
            var buildingType = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });

            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "SavedFamilyDiag", "Test"));
            _ = form.Handle;

            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { buildingType });
            Invoke(form, "LoadSelectedMap");

            // 1. 驗證來源已儲存群組（closed family，無獨立單物件範本）無 construction-site 假警告
            var initialDiagnostics = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(initialDiagnostics, d => d.Code == "construction-site");

            // 2. 觸發 DuplicateSelectedPlacedObjects 建立 pending duplicate
            _ = form.PlacedList.Handle;
            Assert.Single(form.PlacedList.Items);
            form.PlacedList.Items[0].Selected = true;
            form.DuplicateSelectedPlacedObjects();

            Assert.Equal(2, form.PlacementSession.Count);
            Guid duplicateGuid = form.PlacementSession[1].ScenarioId;
            Assert.Equal(sourceGuid, form.PlacementSession[1].CopySourceScenarioId);

            // pending duplicate 具有有效來源 binding 與符合契約的平移 intent，不應有 construction-site 警告
            var pendingDiagnostics = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(pendingDiagnostics, d => d.Code == "construction-site");

            // 3. 測試不合契約的無效／過期／失配參數仍不當作有效（保持警告或拒絕）

            // 3a. Mismatched team：修改副本隊伍為不支援／不一致隊伍 (1 != 0)
            var originalPlaced = form.PlacementSession.Capture();
            var mismatchedTeam = originalPlaced[1] with { Team = 1 };
            form.PlacementSession.Replace(1, mismatchedTeam);
            var mismatchedTeamDiag = form.RefreshMapDiagnostics();
            Assert.Contains(mismatchedTeamDiag, d => d.Code == "construction-site" && d.ObjectId == duplicateGuid);

            // 3b. Mismatched angle：修改副本角度為不一致 (90 != 0)
            var mismatchedAngle = originalPlaced[1] with { Angle = 90f };
            form.PlacementSession.Replace(1, mismatchedAngle);
            var mismatchedAngleDiag = form.RefreshMapDiagnostics();
            Assert.Contains(mismatchedAngleDiag, d => d.Code == "construction-site" && d.ObjectId == duplicateGuid);

            // 3c. Stale / invalid CopySourceScenarioId：指向不存在的來源 Guid
            var staleSource = originalPlaced[1] with { CopySourceScenarioId = Guid.NewGuid() };
            form.PlacementSession.Replace(1, staleSource);
            var staleSourceDiag = form.RefreshMapDiagnostics();
            Assert.Contains(staleSourceDiag, d => d.Code == "construction-site" && d.ObjectId == duplicateGuid);

            // A valid slot/UID does not authorize a different alias.
            var wrongAlias = originalPlaced[1] with
            {
                Type = buildingType with { TemplateFields = new Dictionary<string, string> { ["alias"] = "OtherTest" } }
            };
            form.PlacementSession.Replace(1, wrongAlias);
            var wrongAliasDiag = form.RefreshMapDiagnostics();
            Assert.Contains(wrongAliasDiag, d => d.Code == "construction-site" && d.ObjectId == duplicateGuid);

            // 3d. Non-prebuilt category / figure：非建築物件
            var figureType = new SdlObjectType("FigGerTest", 1, SdlObjectCategory.Figure, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });
            var nonBuilding = originalPlaced[1] with { Type = figureType };
            form.PlacementSession.Replace(1, nonBuilding);
            var nonBuildingDiag = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(nonBuildingDiag, d => d.Code == "construction-site"); // 人物非建築，不發 construction-site

            // 恢復有效副本狀態
            form.PlacementSession.Replace(1, originalPlaced[1]);
            var restoredDiag = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(restoredDiag, d => d.Code == "construction-site");

            // 4. 成功儲存後（duplicate 成為持久 DATA 物件與 Scenario binding），重讀無 construction-site
            Assert.True(form.TrySaveMap(showSuccess: false, out Exception? error), error?.ToString());
            Assert.Null(error);

            var savedDiagnostics = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(savedDiagnostics, d => d.Code == "construction-site");

            // 5. 重新載入地圖表單驗證持久狀態無假警告
            using var reloadedForm = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "SavedFamilyDiagReloaded", "Test"));
            _ = reloadedForm.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloadedForm, new[] { buildingType });
            Invoke(reloadedForm, "LoadSelectedMap");

            Assert.Equal(2, reloadedForm.PlacementSession.Count);
            var reloadedDiagnostics = reloadedForm.RefreshMapDiagnostics();
            Assert.DoesNotContain(reloadedDiagnostics, d => d.Code == "construction-site");
        });
    }
}

