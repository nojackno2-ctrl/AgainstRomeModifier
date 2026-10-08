namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 過場動畫演員指令類型。
/// </summary>
public enum CutsceneUnitOrderKind
{
    /// <summary>下令前往座標點 (對應原生 s_conMoveTo)。</summary>
    MoveTo,
    /// <summary>下令攻擊指定位置/目標 (對應原生 s_attacksObj / s_conCombat)。</summary>
    AttackTarget,
    /// <summary>原地下令警戒/防守。</summary>
    HoldGround,
    /// <summary>切換隊形 (對應原生 s_getFormDef / 陣形幾何)。</summary>
    ChangeFormation,
    /// <summary>播放特定動作動畫。</summary>
    PlayAnimation
}

/// <summary>
/// 過場特效/事件標記類型。
/// </summary>
public enum CutsceneFxEventKind
{
    /// <summary>寬銀幕黑邊開啟 (Cinema Letterbox)。</summary>
    LetterboxOn,
    /// <summary>寬銀幕黑邊關閉。</summary>
    LetterboxOff,
    /// <summary>黑幕淡入 (Fade In)。</summary>
    FadeIn,
    /// <summary>黑幕淡出 (Fade Out)。</summary>
    FadeOut,
    /// <summary>切換背景音樂或播放環境音。</summary>
    PlayAudio,
    /// <summary>觸發原生戰役腳本事件 (ScenarioEvent Trigger)。</summary>
    TriggerScenarioEvent
}

/// <summary>
/// 對白與字幕關鍵影格 (Subtitle Keyframe)。
/// </summary>
public sealed record SubtitleKeyframe
{
    /// <summary>字幕出現時間（秒）。</summary>
    public float StartTimeSeconds { get; init; }

    /// <summary>字幕持續時間（秒）。</summary>
    public float DurationSeconds { get; init; } = 4.0f;

    /// <summary>說話者姓名或稱號（例如「日耳曼首領 阿爾米紐斯」、「旁白」）。</summary>
    public string Speaker { get; init; } = string.Empty;

    /// <summary>字幕對白內容本文。</summary>
    public string Text { get; init; } = string.Empty;

    /// <summary>選用語音樣本代碼（例如 "voice_ger_intro01"）。</summary>
    public string? VoiceSampleAlias { get; init; }

    /// <summary>字幕結束時間點（秒）。</summary>
    public float EndTimeSeconds => StartTimeSeconds + DurationSeconds;

    public SubtitleKeyframe() { }

    public SubtitleKeyframe(float startTime, float duration, string speaker, string text, string? voiceSample = null)
    {
        StartTimeSeconds = Math.Max(0f, startTime);
        DurationSeconds = Math.Max(0.5f, duration);
        Speaker = speaker ?? string.Empty;
        Text = text ?? string.Empty;
        VoiceSampleAlias = voiceSample;
    }
}

/// <summary>
/// 部隊演員指令關鍵影格 (Unit Order Keyframe)。
/// </summary>
public sealed record UnitOrderKeyframe
{
    /// <summary>觸發指令時間戳（秒）。</summary>
    public float TimeSeconds { get; init; }

    /// <summary>受令單位之 GUID 標識（若對應地圖放置物件）。</summary>
    public Guid UnitId { get; init; } = Guid.Empty;

    /// <summary>受令單位之別名或生成名稱（例如 "GER_CHIEF", "ROM_PATROL_01"）。</summary>
    public string UnitAlias { get; init; } = string.Empty;

    /// <summary>指令動作種類。</summary>
    public CutsceneUnitOrderKind Order { get; init; } = CutsceneUnitOrderKind.MoveTo;

    /// <summary>目標世界座標 X。</summary>
    public float TargetX { get; init; } = 8192f;

    /// <summary>目標世界座標 Z。</summary>
    public float TargetZ { get; init; } = 8192f;

    /// <summary>隊形編號 (0..8)。</summary>
    public int FormationIndex { get; init; }

    public UnitOrderKeyframe() { }

    public UnitOrderKeyframe(float time, string unitAlias, CutsceneUnitOrderKind order, float targetX, float targetZ, Guid unitId = default, int formation = 0)
    {
        TimeSeconds = Math.Max(0f, time);
        UnitAlias = unitAlias ?? string.Empty;
        Order = order;
        TargetX = targetX;
        TargetZ = targetZ;
        UnitId = unitId;
        FormationIndex = formation;
    }
}

/// <summary>
/// 畫面導演特效與環境事件關鍵影格 (Cutscene FX Keyframe)。
/// </summary>
public sealed record CutsceneFxKeyframe
{
    /// <summary>觸發時間（秒）。</summary>
    public float TimeSeconds { get; init; }

    /// <summary>事件類型。</summary>
    public CutsceneFxEventKind Kind { get; init; }

    /// <summary>附帶參數（音效檔名、事件名稱、淡入時間參數等）。</summary>
    public string Parameter { get; init; } = string.Empty;

    public CutsceneFxKeyframe() { }

    public CutsceneFxKeyframe(float time, CutsceneFxEventKind kind, string parameter = "")
    {
        TimeSeconds = Math.Max(0f, time);
        Kind = kind;
        Parameter = parameter ?? string.Empty;
    }
}

/// <summary>
/// 完整歷史戰役過場動畫序列 (Cutscene Sequence)。
/// 將多軌道（相機軌、字幕軌、演員指令軌、畫面特效軌）整合為單一時間軸。
/// </summary>
public sealed record CutsceneSequence
{
    /// <summary>唯一序列識別碼。</summary>
    public string Id { get; init; } = Guid.NewGuid().ToString("N");

    /// <summary>序列名稱（例如「條頓堡森林伏擊開場運鏡」）。</summary>
    public string Name { get; init; } = "未命名過場序列";

    /// <summary>相機路徑點軌道。</summary>
    public List<CameraWaypoint> CameraWaypoints { get; init; } = new();

    /// <summary>字幕對白關鍵影格軌道。</summary>
    public List<SubtitleKeyframe> Subtitles { get; init; } = new();

    /// <summary>部隊調度指令軌道。</summary>
    public List<UnitOrderKeyframe> UnitOrders { get; init; } = new();

    /// <summary>電影氛圍與事件軌道。</summary>
    public List<CutsceneFxKeyframe> FxEvents { get; init; } = new();

    /// <summary>開場是否自動切換為寬銀幕黑邊遮罩。</summary>
    public bool AutoLetterbox { get; init; } = true;

    /// <summary>運鏡期間是否暫時停用玩家滑鼠與鍵盤操控。</summary>
    public bool DisablePlayerControl { get; init; } = true;

    /// <summary>運鏡播放完畢後，是否將鏡頭平滑歸位回玩家初始大本營。</summary>
    public bool RestoreCameraOnComplete { get; init; } = true;

    /// <summary>播放完畢時觸發之後續戰役事件名稱（可選）。</summary>
    public string? OnCompleteTriggerEvent { get; init; }

    /// <summary>
    /// 計算本序列之總長度（取相機軌累計時長與各關鍵影格結束時間之最大值）。
    /// </summary>
    public float CalculateTotalDuration()
    {
        float cameraDuration = 0f;
        for (int i = 1; i < CameraWaypoints.Count; i++)
        {
            cameraDuration += Math.Max(0.001f, CameraWaypoints[i].DurationFromPrevious);
        }

        float maxSubtitle = Subtitles.Count > 0 ? Subtitles.Max(s => s.EndTimeSeconds) : 0f;
        float maxUnitOrder = UnitOrders.Count > 0 ? UnitOrders.Max(u => u.TimeSeconds) : 0f;
        float maxFx = FxEvents.Count > 0 ? FxEvents.Max(e => e.TimeSeconds) : 0f;

        return Math.Max(cameraDuration, Math.Max(maxSubtitle, Math.Max(maxUnitOrder, maxFx)));
    }
}
