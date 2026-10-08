namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 運鏡試飛預覽器播放狀態。
/// </summary>
public enum CameraPreviewState
{
    /// <summary>停止狀態。</summary>
    Stopped,
    /// <summary>正在播放試飛。</summary>
    Playing,
    /// <summary>暫停播放。</summary>
    Paused
}

/// <summary>
/// 3D 視圖即時運鏡預覽器 (Camera Flight Previewer)。
/// 純邏輯控制器（無直接 GPU/WinForms 耦合），負責驅動相機飛行時間軸，
/// 觸發每影格相機姿態插值、OSD 字幕事件與過場演出。
/// </summary>
public sealed class CameraFlightPreviewer
{
    private CameraTrackSplinePlanner _splinePlanner = new();
    private CutsceneSequence? _activeSequence;
    private CameraPose? _savedOriginalCameraPose;
    private SubtitleKeyframe? _currentActiveSubtitle;
    private readonly HashSet<int> _triggeredFxIndices = new();

    /// <summary>當前播放狀態。</summary>
    public CameraPreviewState State { get; private set; } = CameraPreviewState.Stopped;

    /// <summary>當前飛行進度時間（秒）。</summary>
    public float CurrentTime { get; private set; }

    /// <summary>當前過場總長度（秒）。</summary>
    public float TotalDuration => _splinePlanner.TotalDuration;

    /// <summary>播放速度倍率 (0.1x ~ 5.0x，預設 1.0x)。</summary>
    public float PlaybackSpeed { get; set; } = 1.0f;

    /// <summary>播放完畢時是否自動還原相機視角至試飛前之位置。</summary>
    public bool AutoRestoreCameraOnStop { get; set; } = true;

    /// <summary>試飛啟動前之原始相機姿態備份。</summary>
    public CameraPose? SavedOriginalCameraPose => _savedOriginalCameraPose;

    /// <summary>當前正在播放的過場動畫序列。</summary>
    public CutsceneSequence? ActiveSequence => _activeSequence;

    /// <summary>當前正在顯示的字幕（若無則為 null）。</summary>
    public SubtitleKeyframe? CurrentActiveSubtitle => _currentActiveSubtitle;

    /// <summary>相機每影格即時姿態更新事件。</summary>
    public event EventHandler<CameraPose>? CameraPoseUpdated;

    /// <summary>字幕對白顯示/切換/消失事件。</summary>
    public event EventHandler<SubtitleKeyframe?>? ActiveSubtitleChanged;

    /// <summary>過場特效與事件觸發。</summary>
    public event EventHandler<CutsceneFxKeyframe>? FxEventTriggered;

    /// <summary>試飛播放結束事件。</summary>
    public event EventHandler? PlaybackFinished;

    /// <summary>
    /// 以指定過場序列啟動相機運鏡試飛。
    /// </summary>
    /// <param name="sequence">過場動畫序列。</param>
    /// <param name="currentCameraPose">當前 3D 視圖相機之姿態（用於試飛結束時原樣還原）。</param>
    public void Play(CutsceneSequence sequence, CameraPose? currentCameraPose = null)
    {
        ArgumentNullException.ThrowIfNull(sequence);

        _activeSequence = sequence;
        _splinePlanner = new CameraTrackSplinePlanner(sequence.CameraWaypoints);
        _savedOriginalCameraPose = currentCameraPose;
        _triggeredFxIndices.Clear();
        CurrentTime = 0f;
        State = CameraPreviewState.Playing;

        // 即時評估第 0 幀
        EvaluateFrame(0f);
    }

    /// <summary>
    /// 暫停試飛。
    /// </summary>
    public void Pause()
    {
        if (State == CameraPreviewState.Playing)
        {
            State = CameraPreviewState.Paused;
        }
    }

    /// <summary>
    /// 繼續播放。
    /// </summary>
    public void Resume()
    {
        if (State == CameraPreviewState.Paused)
        {
            State = CameraPreviewState.Playing;
        }
    }

    /// <summary>
    /// 停止試飛，並可選擇還原相機姿態。
    /// </summary>
    public void Stop(bool restoreCamera = true)
    {
        State = CameraPreviewState.Stopped;
        CurrentTime = 0f;

        if (_currentActiveSubtitle is not null)
        {
            _currentActiveSubtitle = null;
            ActiveSubtitleChanged?.Invoke(this, null);
        }

        if (restoreCamera && AutoRestoreCameraOnStop && _savedOriginalCameraPose is { } original)
        {
            CameraPoseUpdated?.Invoke(this, original);
        }
    }

    /// <summary>
    /// 時間軸任意跳轉 (Timeline Scrubbing)。
    /// </summary>
    public void ScrubTo(float timeSeconds)
    {
        if (_activeSequence is null) return;

        float clamped = Math.Clamp(timeSeconds, 0f, TotalDuration);
        CurrentTime = clamped;
        EvaluateFrame(clamped);
    }

    /// <summary>
    /// 每幀推進時鐘並更新相機與各軌道。
    /// </summary>
    /// <param name="deltaTimeSeconds">距離上一幀經過的真實時間（秒）。</param>
    public void Update(float deltaTimeSeconds)
    {
        if (State != CameraPreviewState.Playing || _activeSequence is null) return;
        if (deltaTimeSeconds <= 0f) return;

        float advance = deltaTimeSeconds * Math.Max(0.01f, PlaybackSpeed);
        CurrentTime += advance;

        if (CurrentTime >= TotalDuration)
        {
            CurrentTime = TotalDuration;
            EvaluateFrame(CurrentTime);
            State = CameraPreviewState.Stopped;
            PlaybackFinished?.Invoke(this, EventArgs.Empty);

            if (_activeSequence.RestoreCameraOnComplete && AutoRestoreCameraOnStop && _savedOriginalCameraPose is { } original)
            {
                CameraPoseUpdated?.Invoke(this, original);
            }
            return;
        }

        EvaluateFrame(CurrentTime);
    }

    private void EvaluateFrame(float time)
    {
        // 1. 樣條姿態評估
        CameraPose pose = _splinePlanner.Evaluate(time);
        CameraPoseUpdated?.Invoke(this, pose);

        if (_activeSequence is null) return;

        // 2. 字幕軌道比對
        SubtitleKeyframe? matchedSub = null;
        foreach (var sub in _activeSequence.Subtitles)
        {
            if (time >= sub.StartTimeSeconds && time <= sub.EndTimeSeconds)
            {
                matchedSub = sub;
                break;
            }
        }

        if (!ReferenceEquals(_currentActiveSubtitle, matchedSub))
        {
            _currentActiveSubtitle = matchedSub;
            ActiveSubtitleChanged?.Invoke(this, matchedSub);
        }

        // 3. 特效與事件軌道觸發
        for (int i = 0; i < _activeSequence.FxEvents.Count; i++)
        {
            var fx = _activeSequence.FxEvents[i];
            if (time >= fx.TimeSeconds && _triggeredFxIndices.Add(i))
            {
                FxEventTriggered?.Invoke(this, fx);
            }
        }
    }
}
