using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 遊戲內驗收用：以真正的 MapEditorForm 與存檔交易，把指定案例寫入已安裝遊戲的測試地圖。
/// 僅在同時設定 ARM_GAME_PATH、ARM_INGAME_MAP（例如 ENDL_005）與 ARM_INGAME_SCENARIO 時執行；
/// 會修改該自製地圖，執行前須先備份。案例：
/// victory＝3 秒訊息＋第一支 team 0 部隊進入其東方矩形後訊息與勝利；
/// defeat＝3 秒訊息＋45 秒後訊息與失敗（用於存讀檔後事件是否延續）。
/// </summary>
public sealed class InGameAcceptanceScenarioTests
{
    [Fact]
    public void Write_in_game_acceptance_scenario()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        string? mapId = Environment.GetEnvironmentVariable("ARM_INGAME_MAP");
        string? scenario = Environment.GetEnvironmentVariable("ARM_INGAME_SCENARIO");
        if (string.IsNullOrWhiteSpace(game) || string.IsNullOrWhiteSpace(mapId) || string.IsNullOrWhiteSpace(scenario)) return;
        GameMapInfo map = new GameMapCatalog().Require(game, mapId);
        Assert.True(map.IsCustom, "只允許寫入自製地圖。");
        string report = "";
        RunInSta(() =>
        {
            using var form = new MapEditorForm(game, map);
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var unit = form.PlacementSession.Capture().First(item => item.Type.Category == SdlObjectCategory.Figure && item.Team == 0);
            var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            events.RemoveAll(item => item.Name.StartsWith("ARM ", StringComparison.Ordinal));
            events.Add(new("ARM start", 3) { Actions = [new(ScenarioActionKind.Message, scenario == "victory"
                ? "ARM test: move the soldiers east into the marked area."
                : "ARM test: defeat in 45 seconds. Save and load now.")] });
            if (scenario == "victory")
            {
                int x1 = (int)unit.WorldX + 1000, x2 = x1 + 1000, z1 = (int)unit.WorldZ - 600, z2 = (int)unit.WorldZ + 600;
                events.Add(new("ARM area win", 1)
                {
                    Conditions = [new(ScenarioConditionKind.ObjectInArea, unit.ScenarioId, x1, z1, x2, z2)],
                    Actions = [new(ScenarioActionKind.Message, "ARM test: area reached - victory."), new(ScenarioActionKind.Victory)]
                });
                report = $"unit {unit.Type.NameDef} id={unit.ScenarioId} at ({unit.WorldX},{unit.WorldZ}); area X {x1}-{x2}, Z {z1}-{z2}";
            }
            else if (scenario == "defeat")
            {
                events.Add(new("ARM timed defeat", 45) { Actions = [new(ScenarioActionKind.Message, "ARM test: timer elapsed - defeat."), new(ScenarioActionKind.Defeat)] });
                report = "timed defeat at 45 s";
            }
            else throw new ArgumentException("未知案例：" + scenario);
            Invoke(form, "RefreshEventList", 0);
            Invoke(form, "UpdateEditorState");
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
        });
        File.WriteAllText(Path.Combine(Path.GetTempPath(), "ArmInGameScenario.txt"), $"{DateTime.Now:O} {mapId} {scenario}: {report}");
    }

    private static object? Invoke(MapEditorForm form, string name, params object[] args)
    {
        MethodInfo method = typeof(MapEditorForm).GetMethods(BindingFlags.Instance | BindingFlags.NonPublic)
            .Single(item => item.Name == name && item.GetParameters().Length == args.Length);
        try { return method.Invoke(form, args); }
        catch (TargetInvocationException ex) when (ex.InnerException is not null) { throw ex.InnerException; }
    }

    private static void RunInSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() => { try { action(); } catch (Exception ex) { failure = ex; } });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromMinutes(3)), "STA 逾時（可能彈出模態對話框）。");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }
}
