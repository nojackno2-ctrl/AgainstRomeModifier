using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.AI;

/// <summary>
/// 戰役任務編譯結果，包含降階後的合法 ScenarioEvent 清單與診斷資訊。
/// </summary>
public sealed record CampaignCompilationResult(
    bool Success,
    IReadOnlyList<ScenarioEvent> CompiledEvents,
    IReadOnlyList<string> Diagnostics);

/// <summary>
/// 將高階戰役目標（波次進攻、定時增援、基地防守、刺殺/摧毀）
/// 編譯降階為原版合法 BCI 執行序與 ScenarioEvent 之編譯管線。
/// </summary>
public static class CampaignMissionCompiler
{
    /// <summary>
    /// 編譯戰役企劃為原版合法的 ScenarioEvent 清單。
    /// </summary>
    public static CampaignCompilationResult Compile(
        CampaignMissionPlan plan,
        IReadOnlyList<WaypointPath>? waypointPaths,
        IReadOnlyCollection<string> knownAliases,
        ScenarioDocument? existingScenario = null)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(knownAliases);

        var diagnostics = new List<string>();
        var compiledEvents = new List<ScenarioEvent>();

        // 1. 企劃層級自我驗證
        try
        {
            plan.Validate();
        }
        catch (Exception ex)
        {
            diagnostics.Add($"[Error] 企劃總綱驗證失敗：{ex.Message}");
            return new CampaignCompilationResult(false, compiledEvents, diagnostics);
        }

        // 2. 驗證所有部隊別名是否合法
        foreach (var wave in plan.Waves)
        {
            foreach (var squad in wave.Squads)
            {
                if (!knownAliases.Contains(squad.Alias, StringComparer.OrdinalIgnoreCase))
                {
                    diagnostics.Add($"[Error] 波次 {wave.WaveIndex} 的部隊別名未知：{squad.Alias}");
                }
            }
        }

        foreach (var reinf in plan.Reinforcements)
        {
            foreach (var squad in reinf.Squads)
            {
                if (!knownAliases.Contains(squad.Alias, StringComparer.OrdinalIgnoreCase))
                {
                    diagnostics.Add($"[Error] 增援「{reinf.Name}」的部隊別名未知：{squad.Alias}");
                }
            }
        }

        // 3. 驗證目標物件持久 ID 是否存在於場景中
        var knownSpawnIds = existingScenario?.Spawns.Select(s => s.Id).ToHashSet() ?? new HashSet<Guid>();
        foreach (var obj in plan.Objectives)
        {
            if (obj.TargetId.HasValue && obj.TargetId.Value != Guid.Empty && existingScenario is not null)
            {
                if (!knownSpawnIds.Contains(obj.TargetId.Value))
                {
                    diagnostics.Add($"[Error] 目標「{obj.Title}」指定的目標物件 ID ({obj.TargetId.Value}) 不存在於場景放置中。");
                }
            }
        }

        if (diagnostics.Any(d => d.StartsWith("[Error]", StringComparison.Ordinal)))
        {
            return new CampaignCompilationResult(false, compiledEvents, diagnostics);
        }

        // 4. 編譯開局任務簡報與初始化事件
        if (!string.IsNullOrWhiteSpace(plan.Briefing))
        {
            var briefingEvent = new ScenarioEvent(
                Name: $"MissionBriefing_{SanitizeName(plan.Title)}",
                DelaySeconds: 2,
                Repeat: false,
                Enabled: true)
            {
                Actions =
                [
                    new ScenarioAction(ScenarioActionKind.Message, Text: plan.Briefing)
                ]
            };
            compiledEvents.Add(briefingEvent);
        }

        // 5. 編譯定時增援（TimerReinforcement）
        foreach (var reinf in plan.Reinforcements)
        {
            var actions = new List<ScenarioAction>();
            if (!string.IsNullOrWhiteSpace(reinf.NotificationText))
            {
                actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: reinf.NotificationText));
            }

            foreach (var squad in reinf.Squads)
            {
                actions.Add(new ScenarioAction(
                    ScenarioActionKind.SpawnUnit,
                    Alias: squad.Alias,
                    Team: squad.Team,
                    Count: squad.Count,
                    X: reinf.SpawnX,
                    Z: reinf.SpawnZ));
            }

            var reinfEvent = new ScenarioEvent(
                Name: $"Reinf_{SanitizeName(reinf.Name)}",
                DelaySeconds: reinf.TriggerDelaySeconds,
                Repeat: false,
                Enabled: true)
            {
                Actions = actions
            };
            compiledEvents.Add(reinfEvent);
        }

        // 6. 編譯進攻波次（WaveAttack）
        foreach (var wave in plan.Waves.OrderBy(w => w.WaveIndex))
        {
            var actions = new List<ScenarioAction>();
            if (!string.IsNullOrWhiteSpace(wave.Announcement))
            {
                actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: wave.Announcement));
            }

            foreach (var squad in wave.Squads)
            {
                actions.Add(new ScenarioAction(
                    ScenarioActionKind.SpawnUnit,
                    Alias: squad.Alias,
                    Team: squad.Team,
                    Count: squad.Count,
                    X: wave.SpawnX,
                    Z: wave.SpawnZ));
            }

            var waveEvent = new ScenarioEvent(
                Name: $"Wave_{wave.WaveIndex:D2}",
                DelaySeconds: wave.TriggerDelaySeconds,
                Repeat: false,
                Enabled: true)
            {
                Actions = actions
            };
            compiledEvents.Add(waveEvent);
        }

        // 7. 編譯勝敗目標（Objectives）
        foreach (var obj in plan.Objectives)
        {
            switch (obj.Type)
            {
                case CampaignObjectiveType.DefendTarget:
                    // 守護目標被擊毀 -> 宣告任務失敗（Defeat）
                    var defendFailEvent = new ScenarioEvent(
                        Name: $"DefendFail_{SanitizeName(obj.Title)}",
                        DelaySeconds: 1,
                        Repeat: false,
                        Enabled: true)
                    {
                        Conditions =
                        [
                            new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, TargetId: obj.TargetId!.Value)
                        ],
                        Actions =
                        [
                            new ScenarioAction(ScenarioActionKind.Message, Text: "Defense failed: the protected target was destroyed!"),
                            new ScenarioAction(ScenarioActionKind.Defeat)
                        ]
                    };
                    compiledEvents.Add(defendFailEvent);
                    break;

                case CampaignObjectiveType.DestroyTarget:
                    // 目標被摧毀 -> 宣告勝利（Victory）
                    var destroyWinEvent = new ScenarioEvent(
                        Name: $"DestroyWin_{SanitizeName(obj.Title)}",
                        DelaySeconds: 1,
                        Repeat: false,
                        Enabled: true)
                    {
                        Conditions =
                        [
                            new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, TargetId: obj.TargetId!.Value)
                        ],
                        Actions =
                        [
                            new ScenarioAction(ScenarioActionKind.Message, Text: "Objective achieved: the target was destroyed!"),
                            new ScenarioAction(ScenarioActionKind.Victory)
                        ]
                    };
                    compiledEvents.Add(destroyWinEvent);
                    break;

                case CampaignObjectiveType.SurviveTime:
                    // 時間到達 -> 宣告勝利
                    var surviveWinEvent = new ScenarioEvent(
                        Name: $"SurviveWin_{SanitizeName(obj.Title)}",
                        DelaySeconds: obj.RequiredSeconds,
                        Repeat: false,
                        Enabled: true)
                    {
                        Actions =
                        [
                            new ScenarioAction(ScenarioActionKind.Message, Text: $"Defense successful: survived for {obj.RequiredSeconds} seconds!"),
                            new ScenarioAction(ScenarioActionKind.Victory)
                        ]
                    };
                    compiledEvents.Add(surviveWinEvent);
                    break;

                case CampaignObjectiveType.ReachArea:
                    // 目標進入區域 -> 宣告勝利
                    var reachWinEvent = new ScenarioEvent(
                        Name: $"ReachWin_{SanitizeName(obj.Title)}",
                        DelaySeconds: 1,
                        Repeat: false,
                        Enabled: true)
                    {
                        Conditions =
                        [
                            new ScenarioCondition(
                                ScenarioConditionKind.ObjectInArea,
                                TargetId: obj.TargetId!.Value,
                                MinX: obj.TargetMinX,
                                MinZ: obj.TargetMinZ,
                                MaxX: obj.TargetMaxX,
                                MaxZ: obj.TargetMaxZ)
                        ],
                        Actions =
                        [
                            new ScenarioAction(ScenarioActionKind.Message, Text: "Mission complete: the target unit reached the designated area!"),
                            new ScenarioAction(ScenarioActionKind.Victory)
                        ]
                    };
                    compiledEvents.Add(reachWinEvent);
                    break;

                case CampaignObjectiveType.WaveSurvival:
                    // 抵擋全部波次：設定在最後一波之後一段緩衝時間勝利
                    int lastWaveDelay = plan.Waves.Count > 0 ? plan.Waves.Max(w => w.TriggerDelaySeconds) : 0;
                    int surviveBuffer = obj.RequiredSeconds > 0 ? obj.RequiredSeconds : 120;
                    var waveSurvivalEvent = new ScenarioEvent(
                        Name: $"WaveSurvivalWin_{SanitizeName(obj.Title)}",
                        DelaySeconds: lastWaveDelay + surviveBuffer,
                        Repeat: false,
                        Enabled: true)
                    {
                        Actions =
                        [
                            new ScenarioAction(ScenarioActionKind.Message, Text: "Victory! All enemy attack waves have been repelled!"),
                            new ScenarioAction(ScenarioActionKind.Victory)
                        ]
                    };
                    compiledEvents.Add(waveSurvivalEvent);
                    break;
            }
        }

        // 8. 最終降階校驗（使用 ScenarioEventValidator）
        try
        {
            ScenarioEventValidator.Validate(compiledEvents, knownAliases);
            if (existingScenario is not null)
            {
                ScenarioEventValidator.ValidateConditions(compiledEvents, existingScenario);
            }
        }
        catch (Exception ex)
        {
            diagnostics.Add($"[Error] 編譯後之 ScenarioEvent 驗證失敗：{ex.Message}");
            return new CampaignCompilationResult(false, compiledEvents, diagnostics);
        }

        diagnostics.Add($"[Info] 成功編譯 {compiledEvents.Count} 個戰役執行序事件。");
        return new CampaignCompilationResult(true, compiledEvents, diagnostics);
    }

    private static string SanitizeName(string raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return "Item";
        var chars = raw.Select(c => char.IsLetterOrDigit(c) ? c : '_').ToArray();
        string clean = new string(chars).Trim('_');
        return clean.Length > 30 ? clean[..30] : clean;
    }
}
