namespace AgainstRomeMapEditor.Modules.Diff;

using AgainstRomeModifier.Scripting;

/// <summary>
/// 差異狀態列舉。
/// </summary>
public enum DiffStatus
{
    /// <summary>無變更。</summary>
    Unchanged,

    /// <summary>新增項目（當前版本有，歷史版本無）。</summary>
    Added,

    /// <summary>刪除項目（歷史版本有，當前版本無）。</summary>
    Deleted,

    /// <summary>修改項目（屬性或參數改變）。</summary>
    Modified,

    /// <summary>移動項目（坐標位移）。</summary>
    Moved
}

/// <summary>
/// 局部還原可選圖層旗標。
/// </summary>
[Flags]
public enum RollbackLayerFlags
{
    None = 0,

    /// <summary>地形高度起伏圖層 (boden.bmp)。</summary>
    Height = 1 << 0,

    /// <summary>地表材質圖塊圖層 (boden.txt)。</summary>
    Textures = 1 << 1,

    /// <summary>通行與碰撞遮罩圖層 (collision.bmp)。</summary>
    Collision = 1 << 2,

    /// <summary>場景物件與部隊實體圖層 (DATA/objects.dat 與 ScenarioSpawns)。</summary>
    Objects = 1 << 3,

    /// <summary>全部圖層。</summary>
    All = Height | Textures | Collision | Objects
}

/// <summary>
/// 地圖版本比對快照資料結構（包含地形、材質、碰撞、物件與劇本事件）。
/// </summary>
public sealed record MapVersionSnapshot(
    string VersionLabel,
    int TileDimension,
    int HeightDimension,
    IReadOnlyList<byte>? Heights,
    float HeightStep = 4.0f,
    IReadOnlyList<string>? Textures = null,
    int CollisionDimension = 0,
    IReadOnlyList<byte>? Collision = null,
    IReadOnlyList<DiffObjectItem>? Objects = null,
    IReadOnlyList<ScenarioEvent>? Events = null
);

/// <summary>
/// 統一物件表示項目，兼顧 ScenarioSpawn 與 LevelWorldObject。
/// </summary>
public sealed record DiffObjectItem(
    Guid Id,
    int Slot,
    uint Uid,
    string Alias,
    int TypeId,
    int Team,
    float X,
    float Y,
    float Z,
    float Rotation = 0f,
    int UnitCount = 0,
    bool Prebuilt = false
)
{
    /// <summary>
    /// 計算與另一物件的水平平面距離。
    /// </summary>
    public float DistanceTo(DiffObjectItem other)
    {
        ArgumentNullException.ThrowIfNull(other);
        float dx = X - other.X;
        float dz = Z - other.Z;
        return MathF.Sqrt(dx * dx + dz * dz);
    }
}

/// <summary>
/// 單一高度頂點的取樣前後變更（完全公開，可無縫轉換為 TerrainSampleChange）。
/// </summary>
public readonly record struct DiffTerrainSampleChange(int Index, byte Before, byte After)
{
    internal TerrainSampleChange ToSampleChange() => new(Index, Before, After);
}

/// <summary>
/// 單一高度頂點的變更記錄。
/// </summary>
public sealed record HeightVertexChange(
    int VertexX,
    int VertexY,
    byte OldHeight,
    byte NewHeight,
    float Delta
);

/// <summary>
/// 地形高度圖層比對報告。
/// </summary>
public sealed record HeightDiffRecord(
    int Dimension,
    float[] DeltaGrid,
    int ChangedVertexCount,
    float MaxRaise,
    float MaxLower,
    float VolumeDelta,
    IReadOnlyList<HeightVertexChange> SampleChanges
);

/// <summary>
/// 單一地表格位材質變更。
/// </summary>
public sealed record TileTextureChange(
    int TileX,
    int TileY,
    string OldTexture,
    string NewTexture
);

/// <summary>
/// 地表材質圖層比對報告。
/// </summary>
public sealed record TextureDiffRecord(
    int Dimension,
    int ChangedTileCount,
    IReadOnlyList<TileTextureChange> ChangedTiles,
    IReadOnlyDictionary<(string From, string To), int> Transitions
);

/// <summary>
/// 單一碰撞格位變更。
/// </summary>
public sealed record CollisionTileChange(
    int TileX,
    int TileY,
    byte OldValue,
    byte NewValue
);

/// <summary>
/// 碰撞通行圖層比對報告。
/// </summary>
public sealed record CollisionDiffRecord(
    int Dimension,
    int NewlyBlockedCount,
    int NewlyClearedCount,
    IReadOnlyList<CollisionTileChange> Changes
);

/// <summary>
/// 單一物件修改詳情。
/// </summary>
public sealed record ObjectModification(
    DiffObjectItem Baseline,
    DiffObjectItem Current,
    float DistanceMoved,
    float RotationDelta,
    IReadOnlyList<string> ChangedAttributes
);

/// <summary>
/// 場景物件圖層比對報告。
/// </summary>
public sealed record ObjectDiffRecord(
    IReadOnlyList<DiffObjectItem> Added,
    IReadOnlyList<DiffObjectItem> Deleted,
    IReadOnlyList<ObjectModification> Modified
)
{
    /// <summary>
    /// 物件總變更數。
    /// </summary>
    public int TotalChanges => Added.Count + Deleted.Count + Modified.Count;
}

/// <summary>
/// 單一劇本事件變更詳情。
/// </summary>
public sealed record EventModification(
    string EventName,
    ScenarioEvent Baseline,
    ScenarioEvent Current,
    IReadOnlyList<string> DetailChanges
);

/// <summary>
/// 劇本腳本事件比對報告。
/// </summary>
public sealed record EventDiffRecord(
    IReadOnlyList<ScenarioEvent> Added,
    IReadOnlyList<ScenarioEvent> Deleted,
    IReadOnlyList<EventModification> Modified
)
{
    /// <summary>
    /// 事件總變更數。
    /// </summary>
    public int TotalChanges => Added.Count + Deleted.Count + Modified.Count;
}

/// <summary>
/// 完整地圖版本差異報告。
/// </summary>
public sealed record MapDiffReport(
    DateTimeOffset ComparedAt,
    string BaselineLabel,
    string CurrentLabel,
    HeightDiffRecord Height,
    TextureDiffRecord Textures,
    CollisionDiffRecord Collision,
    ObjectDiffRecord Objects,
    EventDiffRecord Events
)
{
    /// <summary>
    /// 是否存在任何圖層差異。
    /// </summary>
    public bool HasDifferences =>
        Height.ChangedVertexCount > 0 ||
        Textures.ChangedTileCount > 0 ||
        Collision.NewlyBlockedCount > 0 ||
        Collision.NewlyClearedCount > 0 ||
        Objects.TotalChanges > 0 ||
        Events.TotalChanges > 0;

    /// <summary>
    /// 產出繁體中文摘要報告。
    /// </summary>
    public string GenerateSummaryText()
    {
        var sb = new System.Text.StringBuilder();
        sb.AppendLine($"【地圖版本比對報告】");
        sb.AppendLine($"基準版本 (Baseline): {BaselineLabel}");
        sb.AppendLine($"當前版本 (Current): {CurrentLabel}");
        sb.AppendLine($"比對時間: {ComparedAt:yyyy-MM-dd HH:mm:ss}");
        sb.AppendLine($"總體狀態: {(HasDifferences ? "檢測到差異變更" : "兩版本完全一致")}");
        sb.AppendLine();

        sb.AppendLine($"--- 1. 地形起伏 (Heightmap) ---");
        sb.AppendLine($"  變更頂點數: {Height.ChangedVertexCount:N0} 個");
        sb.AppendLine($"  最大高度抬升: +{Height.MaxRaise:F1} (世界單位)");
        sb.AppendLine($"  最大深度下陷: -{Height.MaxLower:F1} (世界單位)");
        sb.AppendLine($"  淨體積變化: {(Height.VolumeDelta >= 0 ? "+" : "")}{Height.VolumeDelta:F1}");
        sb.AppendLine();

        sb.AppendLine($"--- 2. 地表材質 (Floor Materials) ---");
        sb.AppendLine($"  變更格位數: {Textures.ChangedTileCount:N0} 格");
        if (Textures.Transitions.Count > 0)
        {
            sb.AppendLine("  主要材質轉移:");
            foreach (var kvp in Textures.Transitions.OrderByDescending(t => t.Value).Take(5))
            {
                sb.AppendLine($"    {kvp.Key.From} -> {kvp.Key.To}: {kvp.Value} 格");
            }
        }
        sb.AppendLine();

        sb.AppendLine($"--- 3. 通行碰撞 (Collision) ---");
        sb.AppendLine($"  新增阻擋格: {Collision.NewlyBlockedCount:N0} 格");
        sb.AppendLine($"  新增通行格: {Collision.NewlyClearedCount:N0} 格");
        sb.AppendLine();

        sb.AppendLine($"--- 4. 場景物件 (Level Objects) ---");
        sb.AppendLine($"  新增物件: {Objects.Added.Count:N0} 個");
        sb.AppendLine($"  刪除物件: {Objects.Deleted.Count:N0} 個");
        sb.AppendLine($"  修改/移動物件: {Objects.Modified.Count:N0} 個");
        sb.AppendLine();

        sb.AppendLine($"--- 5. 劇本事件 (Scenario Events) ---");
        sb.AppendLine($"  新增事件: {Events.Added.Count:N0} 個");
        sb.AppendLine($"  刪除事件: {Events.Deleted.Count:N0} 個");
        sb.AppendLine($"  修改事件: {Events.Modified.Count:N0} 個");

        return sb.ToString();
    }
}
