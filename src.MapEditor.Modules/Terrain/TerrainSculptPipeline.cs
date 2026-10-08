namespace AgainstRomeMapEditor;

/// <summary>
/// 地形雕刻與侵蝕演算法管線（Terrain Sculpt &amp; Erosion Pipeline）。
/// 提供將高精度浮點工作緩衝區（TerrainHeightWorkBuffer）、水力/熱力侵蝕及幾何濾鏡
/// 整合至 TerrainHeightEditSession 且完全相容單步 Undo/Redo 的高階管線介面。
/// </summary>
internal static class TerrainSculptPipeline
{
    /// <summary>
    /// 對 TerrainHeightEditSession 執行自訂雕刻或物理運算，並自動追蹤差異提交至待處理筆畫中。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> Execute(
        TerrainHeightEditSession session,
        Action<TerrainHeightWorkBuffer> process)
    {
        ArgumentNullException.ThrowIfNull(session);
        ArgumentNullException.ThrowIfNull(process);

        var buffer = new TerrainHeightWorkBuffer(session.VertexSize, session.Heights);
        process(buffer);

        var changes = buffer.ExtractChanges(session.Heights);
        if (changes.Count > 0)
        {
            session.ApplySampleChanges(changes);
        }

        return changes;
    }

    /// <summary>
    /// 套用幾何隆起濾鏡。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> Elevate(
        TerrainHeightEditSession session,
        float cx, float cy, float radius,
        float strength,
        TerrainFalloffType falloff = TerrainFalloffType.Smoothstep,
        float maxHeight = 255f)
    {
        return Execute(session, buffer =>
            TerrainSculptFilter.Elevate(buffer, cx, cy, radius, strength, falloff, maxHeight));
    }

    /// <summary>
    /// 套用幾何下壓濾鏡。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> Depress(
        TerrainHeightEditSession session,
        float cx, float cy, float radius,
        float strength,
        TerrainFalloffType falloff = TerrainFalloffType.Smoothstep,
        float minHeight = 0f)
    {
        return Execute(session, buffer =>
            TerrainSculptFilter.Depress(buffer, cx, cy, radius, strength, falloff, minHeight));
    }

    /// <summary>
    /// 套用階梯化/高原平頂化濾鏡（Terrace Filter）。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> Terrace(
        TerrainHeightEditSession session,
        float cx, float cy, float radius,
        float stepInterval,
        float flatness = 0.85f,
        float edgeSharpness = 1.5f,
        float strength = 1.0f,
        float baseOffset = 0f)
    {
        return Execute(session, buffer =>
            TerrainSculptFilter.Terrace(buffer, cx, cy, radius, stepInterval, flatness, edgeSharpness, strength, baseOffset));
    }

    /// <summary>
    /// 套用山脊尖銳化濾鏡（Ridge Sharpening Filter）。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> SharpenRidge(
        TerrainHeightEditSession session,
        float cx, float cy, float radius,
        float gain = 0.5f,
        bool ridgeOnly = true,
        int iterations = 1)
    {
        return Execute(session, buffer =>
            TerrainSculptFilter.SharpenRidge(buffer, cx, cy, radius, gain, ridgeOnly, iterations));
    }

    /// <summary>
    /// 套用粒子級水力侵蝕模擬（Hydraulic Erosion）。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> HydraulicErosion(
        TerrainHeightEditSession session,
        HydraulicErosionParams? parameters = null,
        float? brushX = null, float? brushY = null, float? brushRadius = null)
    {
        return Execute(session, buffer =>
            HydraulicErosionSimulator.Simulate(buffer, parameters, brushX, brushY, brushRadius));
    }

    /// <summary>
    /// 套用熱力滑坡侵蝕模擬（Thermal / Talus Erosion）。
    /// </summary>
    public static IReadOnlyList<TerrainSampleChange> ThermalErosion(
        TerrainHeightEditSession session,
        ThermalErosionParams? parameters = null,
        float? brushX = null, float? brushY = null, float? brushRadius = null,
        float[]? screeAccumulationMap = null)
    {
        return Execute(session, buffer =>
            ThermalErosionSimulator.Simulate(buffer, parameters, brushX, brushY, brushRadius, screeAccumulationMap));
    }
}
