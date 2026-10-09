using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 樣條插值過渡曲線類型。
/// </summary>
public enum CameraEasingType
{
    /// <summary>等速線性過渡。</summary>
    Linear,
    /// <summary>平滑 S 型過渡 (3t^2 - 2t^3)。</summary>
    SmoothStep,
    /// <summary>慢入二次方 (t^2)。</summary>
    EaseInQuad,
    /// <summary>慢出二次方 (1 - (1-t)^2)。</summary>
    EaseOutQuad,
    /// <summary>慢入慢出三次方 (前半 4t^3, 後半 1 - (-2t+2)^3 / 2)。</summary>
    EaseInOutCubic
}

/// <summary>
/// 相機路徑點 (Camera Waypoint)，定義特定時刻相機在世界空間中的焦點位置與姿態。
/// </summary>
public sealed record CameraWaypoint
{
    /// <summary>
    /// 世界空間焦點座標 (X, Y, Z)。
    /// 註：Against Rome 中地圖範圍為 0..16384 (64 tiles * 256)。
    /// </summary>
    public Vector3 Position { get; init; } = new(8192f, 0f, 8192f);

    /// <summary>相機俯仰角 (Pitch)，以度為單位 (原遊戲預設約 30° 等角俯角，範圍建議 15°..85°)。</summary>
    public float PitchDegrees { get; init; } = 30f;

    /// <summary>相機偏航角 (Yaw)，以度為單位 (原遊戲預設 45° 等角方向，0°..360°)。</summary>
    public float YawDegrees { get; init; } = 45f;

    /// <summary>相機焦距 / 距離 (Zoom / Distance，預設 82f，僅用於預覽；不等同原生 0..9 縮放)。</summary>
    public float Zoom { get; init; } = 82f;

    /// <summary>從前一個航點飛行到達此航點所需持續時間（秒）。第一個航點通常為 0。</summary>
    public float DurationFromPrevious { get; init; } = 3.0f;

    /// <summary>到達此航點之加減速曲線風格。</summary>
    public CameraEasingType Easing { get; init; } = CameraEasingType.SmoothStep;

    /// <summary>航點備註標籤（例如「雪峰遠景」、「大本營俯衝定格」）。</summary>
    public string? Label { get; init; }

    /// <summary>
    /// 建構預設航點。
    /// </summary>
    public CameraWaypoint() { }

    /// <summary>
    /// 建構指定座標與姿態之航點。
    /// </summary>
    public CameraWaypoint(Vector3 position, float pitchDegrees = 30f, float yawDegrees = 45f, float zoom = 82f, float duration = 3.0f, CameraEasingType easing = CameraEasingType.SmoothStep, string? label = null)
    {
        Position = position;
        PitchDegrees = pitchDegrees;
        YawDegrees = yawDegrees;
        Zoom = zoom;
        DurationFromPrevious = Math.Max(0f, duration);
        Easing = easing;
        Label = label;
    }
}

/// <summary>
/// 相機在特定時間戳的即時姿態與幾何數據。
/// </summary>
public readonly record struct CameraPose(
    Vector3 Position,
    float PitchDegrees,
    float YawDegrees,
    float Zoom,
    float TimeSeconds);
