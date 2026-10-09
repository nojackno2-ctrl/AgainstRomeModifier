using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 實驗性預覽／編輯器資料，尚未接入遊戲 BCI；姿態和預覽距離的原生對應未驗證。
/// 樣條相機軌跡規劃器 (Camera Track Spline Planner)。
/// 基於向心 Catmull-Rom (Centripetal Catmull-Rom, α=0.5) 與弧長/時間參數化，
/// 負責將離散航點平滑插值為連續的 3D 鏡頭飛行軌跡。
/// </summary>
public sealed class CameraTrackSplinePlanner
{
    private readonly List<CameraWaypoint> _waypoints = new();
    private readonly List<float> _cumulativeTimes = new();
    private readonly List<float> _unwrappedYaws = new();

    /// <summary>所有航點列表（唯讀副本）。</summary>
    public IReadOnlyList<CameraWaypoint> Waypoints => _waypoints;

    /// <summary>整段運鏡總持續時間（秒）。</summary>
    public float TotalDuration => _cumulativeTimes.Count > 0 ? _cumulativeTimes[^1] : 0f;

    /// <summary>航點數量。</summary>
    public int Count => _waypoints.Count;

    /// <summary>
    /// 建構空白樣條規劃器。
    /// </summary>
    public CameraTrackSplinePlanner() { }

    /// <summary>
    /// 以指定航點集合初始化樣條規劃器。
    /// </summary>
    public CameraTrackSplinePlanner(IEnumerable<CameraWaypoint> waypoints)
    {
        ArgumentNullException.ThrowIfNull(waypoints);
        SetWaypoints(waypoints);
    }

    /// <summary>
    /// 設定並重建航點樣條。
    /// </summary>
    public void SetWaypoints(IEnumerable<CameraWaypoint> waypoints)
    {
        ArgumentNullException.ThrowIfNull(waypoints);
        _waypoints.Clear();
        _waypoints.AddRange(waypoints);
        RebuildTimeline();
    }

    /// <summary>
    /// 新增單一航點至軌跡末端。
    /// </summary>
    public void AddWaypoint(CameraWaypoint waypoint)
    {
        ArgumentNullException.ThrowIfNull(waypoint);
        _waypoints.Add(waypoint);
        RebuildTimeline();
    }

    /// <summary>
    /// 清空所有航點。
    /// </summary>
    public void Clear()
    {
        _waypoints.Clear();
        _cumulativeTimes.Clear();
        _unwrappedYaws.Clear();
    }

    private void RebuildTimeline()
    {
        _cumulativeTimes.Clear();
        _unwrappedYaws.Clear();

        if (_waypoints.Count == 0) return;

        // 1. 累積時間戳記
        float total = 0f;
        for (int i = 0; i < _waypoints.Count; i++)
        {
            if (i > 0)
            {
                total += Math.Max(0.001f, _waypoints[i].DurationFromPrevious);
            }
            _cumulativeTimes.Add(total);
        }

        // 2. 角度連續展開 (Angle Unwrapping)，避免 350° 轉向 10° 出現逆時針 340° 瘋轉
        float lastYaw = _waypoints[0].YawDegrees;
        _unwrappedYaws.Add(lastYaw);
        for (int i = 1; i < _waypoints.Count; i++)
        {
            float currentYaw = _waypoints[i].YawDegrees;
            float diff = currentYaw - (lastYaw % 360f);
            diff = ((diff + 180f) % 360f + 360f) % 360f - 180f; // 最短有向角差 (-180..180)
            lastYaw += diff;
            _unwrappedYaws.Add(lastYaw);
        }
    }

    /// <summary>
    /// 評估指定時間戳 (秒) 的相機姿態與焦點座標。
    /// </summary>
    public CameraPose Evaluate(float timeSeconds)
    {
        if (_waypoints.Count == 0)
        {
            return new CameraPose(new Vector3(8192f, 0f, 8192f), 30f, 45f, 82f, timeSeconds);
        }

        if (_waypoints.Count == 1 || timeSeconds <= 0f)
        {
            var first = _waypoints[0];
            return new CameraPose(first.Position, first.PitchDegrees, NormalizeAngle(first.YawDegrees), first.Zoom, 0f);
        }

        if (timeSeconds >= TotalDuration)
        {
            var last = _waypoints[^1];
            return new CameraPose(last.Position, last.PitchDegrees, NormalizeAngle(last.YawDegrees), last.Zoom, TotalDuration);
        }

        // 二分搜尋尋找當前時間所在的航點區間 [i, i+1]
        int segmentIndex = 0;
        for (int i = 0; i < _cumulativeTimes.Count - 1; i++)
        {
            if (timeSeconds >= _cumulativeTimes[i] && timeSeconds <= _cumulativeTimes[i + 1])
            {
                segmentIndex = i;
                break;
            }
        }

        float tStart = _cumulativeTimes[segmentIndex];
        float tEnd = _cumulativeTimes[segmentIndex + 1];
        float segmentDuration = tEnd - tStart;
        float rawNormalizedT = segmentDuration > 0.0001f ? (timeSeconds - tStart) / segmentDuration : 0f;
        rawNormalizedT = Math.Clamp(rawNormalizedT, 0f, 1f);

        // 套用段落加減速曲線 (Easing)
        float easedT = ApplyEasing(rawNormalizedT, _waypoints[segmentIndex + 1].Easing);

        // 取得樣條控制點 P0, P1, P2, P3
        var (p0, p1, p2, p3) = GetSplineSegmentPositions(segmentIndex);
        Vector3 position = EvaluateCentripetalCatmullRom(p0, p1, p2, p3, easedT);

        // 角度與縮放插值
        float pitch = MathF.Min(89f, MathF.Max(10f, MathUtilLerp(_waypoints[segmentIndex].PitchDegrees, _waypoints[segmentIndex + 1].PitchDegrees, easedT)));
        float yawUnwrapped = MathUtilLerp(_unwrappedYaws[segmentIndex], _unwrappedYaws[segmentIndex + 1], easedT);
        float zoom = Math.Max(1f, MathUtilLerp(_waypoints[segmentIndex].Zoom, _waypoints[segmentIndex + 1].Zoom, easedT));

        return new CameraPose(position, pitch, NormalizeAngle(yawUnwrapped), zoom, timeSeconds);
    }

    /// <summary>
    /// 取樣整條軌跡為連續點列，供 3D 視圖繪製軌跡輔助線 (Spline Trajectory Gizmo)。
    /// </summary>
    public IReadOnlyList<CameraPose> SamplePath(int sampleCount)
    {
        if (sampleCount < 2 || _waypoints.Count < 2)
        {
            return _waypoints.Select(w => new CameraPose(w.Position, w.PitchDegrees, NormalizeAngle(w.YawDegrees), w.Zoom, 0f)).ToList();
        }

        var results = new List<CameraPose>(sampleCount);
        float step = TotalDuration / (sampleCount - 1);
        for (int i = 0; i < sampleCount; i++)
        {
            float t = i * step;
            results.Add(Evaluate(t));
        }
        return results;
    }

    private (Vector3 p0, Vector3 p1, Vector3 p2, Vector3 p3) GetSplineSegmentPositions(int segmentIndex)
    {
        int count = _waypoints.Count;
        Vector3 p1 = _waypoints[segmentIndex].Position;
        Vector3 p2 = _waypoints[segmentIndex + 1].Position;

        // 端點鏡像虛擬延伸 (Clamped Ghost Points)
        Vector3 p0 = segmentIndex > 0 ? _waypoints[segmentIndex - 1].Position : p1 + (p1 - p2);
        Vector3 p3 = segmentIndex + 2 < count ? _waypoints[segmentIndex + 2].Position : p2 + (p2 - p1);

        return (p0, p1, p2, p3);
    }

    /// <summary>
    /// 向心 Catmull-Rom 樣條評估 (Centripetal Catmull-Rom Spline, α = 0.5)。
    /// 依據 Barry-Goldman 金字塔算法遞迴求值，數學上保證不自交、無尖點過衝。
    /// </summary>
    public static Vector3 EvaluateCentripetalCatmullRom(Vector3 p0, Vector3 p1, Vector3 p2, Vector3 p3, float t)
    {
        const float alpha = 0.5f;

        float GetKnot(float tPrev, Vector3 a, Vector3 b)
        {
            float dist = Vector3.Distance(a, b);
            return tPrev + MathF.Pow(Math.Max(1e-4f, dist), alpha);
        }

        float t0 = 0f;
        float t1 = GetKnot(t0, p0, p1);
        float t2 = GetKnot(t1, p1, p2);
        float t3 = GetKnot(t2, p2, p3);

        // 映射 t 從 [0, 1] 至 [t1, t2]
        float actualT = t1 + t * (t2 - t1);

        Vector3 a1 = (t1 - actualT) / (t1 - t0) * p0 + (actualT - t0) / (t1 - t0) * p1;
        Vector3 a2 = (t2 - actualT) / (t2 - t1) * p1 + (actualT - t1) / (t2 - t1) * p2;
        Vector3 a3 = (t3 - actualT) / (t3 - t2) * p2 + (actualT - t2) / (t3 - t2) * p3;

        Vector3 b1 = (t2 - actualT) / (t2 - t0) * a1 + (actualT - t0) / (t2 - t0) * a2;
        Vector3 b2 = (t3 - actualT) / (t3 - t1) * a2 + (actualT - t1) / (t3 - t1) * a3;

        Vector3 c = (t2 - actualT) / (t2 - t1) * b1 + (actualT - t1) / (t2 - t1) * b2;
        return c;
    }

    /// <summary>
    /// 套用過渡曲線。
    /// </summary>
    public static float ApplyEasing(float t, CameraEasingType easing)
    {
        t = Math.Clamp(t, 0f, 1f);
        return easing switch
        {
            CameraEasingType.Linear => t,
            CameraEasingType.SmoothStep => t * t * (3f - 2f * t),
            CameraEasingType.EaseInQuad => t * t,
            CameraEasingType.EaseOutQuad => 1f - (1f - t) * (1f - t),
            CameraEasingType.EaseInOutCubic => t < 0.5f ? 4f * t * t * t : 1f - MathF.Pow(-2f * t + 2f, 3f) / 2f,
            _ => t
        };
    }

    private static float MathUtilLerp(float a, float b, float t) => a + (b - a) * t;

    private static float NormalizeAngle(float degrees)
    {
        float angle = degrees % 360f;
        if (angle < 0f) angle += 360f;
        return angle;
    }
}
