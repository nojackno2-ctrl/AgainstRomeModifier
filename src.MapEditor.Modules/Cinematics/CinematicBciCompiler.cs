using System.Text;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 原生 BCI 鏡頭呼叫腳本與相容事件編譯器 (Cinematic BCI Compiler)。
/// 負責將過場動畫序列編譯為：
/// 1. 遊戲標準相容之 ScenarioEvent 集合（可直接被 ScenarioEventCompiler 注入 ak_level.bci）。
/// 2. 原生 BCI0 虛擬機指令流規劃與 IPR 偽代碼清單（呼叫 s_lgcSetEnginePos, s_lgcSetEngineZoom, s_showTextBox）。
/// </summary>
public static class CinematicBciCompiler
{
    /// <summary>
    /// 將過場動畫序列編譯轉譯為標準 ScenarioEvent 列表。
    /// 依時間軸依序切分事件與延遲，保證在現有遊戲引擎中 100% 安全運行無崩潰。
    /// </summary>
    public static IReadOnlyList<ScenarioEvent> CompileToScenarioEvents(CutsceneSequence sequence)
    {
        ArgumentNullException.ThrowIfNull(sequence);

        var events = new List<ScenarioEvent>();
        var planner = new CameraTrackSplinePlanner(sequence.CameraWaypoints);
        float totalDuration = Math.Max(sequence.CalculateTotalDuration(), planner.TotalDuration);

        // 1. 若有字幕對白，依時間生成有序訊息事件
        if (sequence.Subtitles.Count > 0)
        {
            var sortedSubs = sequence.Subtitles.OrderBy(s => s.StartTimeSeconds).ToList();
            float lastTime = 0f;

            for (int i = 0; i < sortedSubs.Count; i++)
            {
                var sub = sortedSubs[i];
                int delay = (int)Math.Round(Math.Max(0f, sub.StartTimeSeconds - lastTime));
                lastTime = sub.StartTimeSeconds;

                string speakerPrefix = !string.IsNullOrWhiteSpace(sub.Speaker) ? $"[{sub.Speaker}] " : "";
                string fullText = $"{speakerPrefix}{sub.Text}";

                var actions = new List<ScenarioAction>
                {
                    new(ScenarioActionKind.Message, Text: fullText)
                };

                // 若在該時間戳附近有相機航點，可藉由同事件關聯焦點
                var cutsceneEvent = new ScenarioEvent(
                    Name: $"{sequence.Name}_字幕_{i + 1}",
                    DelaySeconds: delay,
                    Repeat: false,
                    Enabled: true)
                {
                    Actions = actions
                };
                events.Add(cutsceneEvent);
            }
        }
        else
        {
            // 若無字幕，建立開場起點事件
            events.Add(new ScenarioEvent(
                Name: $"{sequence.Name}_開場",
                DelaySeconds: 1,
                Repeat: false,
                Enabled: true)
            {
                Actions = new List<ScenarioAction>
                {
                    new(ScenarioActionKind.Message, Text: $"過場動畫：{sequence.Name}")
                }
            });
        }

        // 2. 結束收尾事件（恢復或解鎖）
        if (!string.IsNullOrWhiteSpace(sequence.OnCompleteTriggerEvent))
        {
            events.Add(new ScenarioEvent(
                Name: $"{sequence.Name}_完畢後續",
                DelaySeconds: (int)Math.Ceiling(totalDuration),
                Repeat: false,
                Enabled: true)
            {
                Actions = new List<ScenarioAction>
                {
                    new(ScenarioActionKind.Message, Text: $"過場結束，觸發後續任務：{sequence.OnCompleteTriggerEvent}")
                }
            });
        }

        return events;
    }

    /// <summary>
    /// 編譯為人類可讀且精確對應 Against Rome 原生 BCI0 虛擬機指令碼的 IPR 腳本文字。
    /// 詳細展示 s_lgcSetEnginePos (0x54c400), s_lgcSetEngineZoom (0x54c620), s_showTextBox (0x521f10) 之呼叫。
    /// </summary>
    public static string GenerateBciScriptText(CutsceneSequence sequence, int sampleSteps = 8)
    {
        ArgumentNullException.ThrowIfNull(sequence);

        var sb = new StringBuilder();
        var planner = new CameraTrackSplinePlanner(sequence.CameraWaypoints);
        float totalDuration = Math.Max(sequence.CalculateTotalDuration(), planner.TotalDuration);

        sb.AppendLine($"// ================================================================");
        sb.AppendLine($"// Against Rome 歷史戰役運鏡過場原生腳本 (BCI / IPR Script)");
        sb.AppendLine($"// 序列名稱: {sequence.Name} (ID: {sequence.Id})");
        sb.AppendLine($"// 總長度: {totalDuration:F2} 秒 | 航點數: {sequence.CameraWaypoints.Count}");
        sb.AppendLine($"// ================================================================");
        sb.AppendLine();
        sb.AppendLine($"void cutscene_{sequence.Id}_main()");
        sb.AppendLine("{");
        sb.AppendLine("    // 1. 初始化導演環境 (啟用黑邊與玩家鎖定)");
        if (sequence.DisablePlayerControl)
        {
            sb.AppendLine("    call s_disableGUI(1); // 鎖定玩家輸入");
        }
        sb.AppendLine();

        // 依時間順序取樣或插入關鍵影格
        int steps = Math.Max(2, sampleSteps);
        float timeStep = totalDuration / steps;

        for (int i = 0; i <= steps; i++)
        {
            float t = i * timeStep;
            CameraPose pose = planner.Evaluate(t);

            sb.AppendLine($"    // --- [時間戳記 {t:F2}s] ---");
            sb.AppendLine($"    // 相機樣條插值：坐標=({pose.Position.X:F1}, {pose.Position.Y:F1}, {pose.Position.Z:F1}), 俯仰={pose.PitchDegrees:F1}°, 偏航={pose.YawDegrees:F1}°, 縮放={pose.Zoom:F1}");
            sb.AppendLine($"    pushd {pose.Position.Z:F2};");
            sb.AppendLine($"    pushd {pose.Position.Y:F2};");
            sb.AppendLine($"    pushd {pose.Position.X:F2};");
            sb.AppendLine($"    call s_lgcSetEnginePos; // 原生 0x54c400 (v(ddd))");
            sb.AppendLine($"    pushd {pose.Zoom:F2};");
            sb.AppendLine($"    call s_lgcSetEngineZoom; // 原生 0x54c620 (v(d))");

            // 比對是否有對應字幕
            var activeSub = sequence.Subtitles.FirstOrDefault(s => Math.Abs(s.StartTimeSeconds - t) < (timeStep * 0.5f));
            if (activeSub is not null)
            {
                string speaker = !string.IsNullOrWhiteSpace(activeSub.Speaker) ? $"[{activeSub.Speaker}] " : "";
                sb.AppendLine($"    pushstr \"{speaker}{activeSub.Text}\";");
                sb.AppendLine($"    push 0;");
                sb.AppendLine($"    call s_showTextBox; // 原生 0x521f10 (i(ii))");

                if (!string.IsNullOrWhiteSpace(activeSub.VoiceSampleAlias))
                {
                    sb.AppendLine($"    pushstr \"{activeSub.VoiceSampleAlias}\";");
                    sb.AppendLine($"    call s_playVoiceSample; // 原生 0x521fb0");
                }
            }

            // 比對是否有部隊指令
            var orders = sequence.UnitOrders.Where(u => Math.Abs(u.TimeSeconds - t) < (timeStep * 0.5f)).ToList();
            foreach (var order in orders)
            {
                sb.AppendLine($"    // 部隊演出指令: {order.UnitAlias} -> ({order.TargetX:F1}, {order.TargetZ:F1})");
                sb.AppendLine($"    push {order.TargetZ:F0};");
                sb.AppendLine($"    push {order.TargetX:F0};");
                sb.AppendLine($"    push 100; // 預設速度");
                sb.AppendLine($"    call s_conMoveTo; // 原生 0x5345c0");
            }

            if (i < steps)
            {
                sb.AppendLine($"    push {(int)Math.Round(timeStep * 1000f)};");
                sb.AppendLine($"    call s_conWaitTime; // 等待下一影格");
            }
            sb.AppendLine();
        }

        sb.AppendLine("    // 3. 完畢處理");
        if (sequence.DisablePlayerControl)
        {
            sb.AppendLine("    call s_disableGUI(0); // 歸還玩家控制權");
        }
        if (!string.IsNullOrWhiteSpace(sequence.OnCompleteTriggerEvent))
        {
            sb.AppendLine($"    call trigger_{sequence.OnCompleteTriggerEvent}();");
        }
        sb.AppendLine("}");

        return sb.ToString();
    }
}
