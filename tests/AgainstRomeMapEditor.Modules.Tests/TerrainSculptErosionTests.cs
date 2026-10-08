using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainSculptErosionTests
{
    private const int TestSize = 33; // 33x33 heightmap

    private static TerrainHeightEditSession CreateSession(byte defaultHeight = 100)
    {
        byte[] heights = Enumerable.Repeat(defaultHeight, TestSize * TestSize).ToArray();
        return new TerrainHeightEditSession(TestSize, heights, null, 0, null);
    }

    [Fact]
    public void WorkBuffer_handles_bilinear_and_gradients_correctly()
    {
        var buffer = new TerrainHeightWorkBuffer(4);
        // 設定一傾斜面：z = x * 10
        for (int y = 0; y < 4; y++)
        for (int x = 0; x < 4; x++)
        {
            buffer[x, y] = x * 10f;
        }

        // 雙線性插值在 (1.5, 1.5) 處應為 15.0
        float sample = buffer.SampleBilinear(1.5f, 1.5f);
        Assert.Equal(15.0f, sample, 0.01f);

        // 梯度 gx 應約為 10，gy 應為 0
        (float gx, float gy) = buffer.CalculateGradient(1.5f, 1.5f);
        Assert.Equal(10.0f, gx, 0.1f);
        Assert.Equal(0.0f, gy, 0.1f);
    }

    [Fact]
    public void WorkBuffer_extracts_only_modified_changes()
    {
        byte[] baseline = new byte[16];
        var buffer = new TerrainHeightWorkBuffer(4, baseline);

        buffer[1, 1] = 50.4f; // 四捨五入為 50
        buffer[2, 2] = 0.2f;  // 四捨五入為 0，與 baseline 相同

        var changes = buffer.ExtractChanges(baseline);
        Assert.Single(changes);
        Assert.Equal(1 * 4 + 1, changes[0].Index);
        Assert.Equal(0, changes[0].Before);
        Assert.Equal(50, changes[0].After);
    }

    [Fact]
    public void SculptFilter_Elevate_and_Depress_modify_heights_within_brush()
    {
        var buffer = new TerrainHeightWorkBuffer(TestSize);
        float cx = 16f, cy = 16f, radius = 8f;

        TerrainSculptFilter.Elevate(buffer, cx, cy, radius, 30f, TerrainFalloffType.Smoothstep);

        // 中心點升高最多
        Assert.True(buffer[(int)cx, (int)cy] > 25f);

        // 筆刷外邊界保持為 0
        Assert.Equal(0f, buffer[0, 0]);
        Assert.Equal(0f, buffer[2, 2]);

        // 下壓測試
        TerrainSculptFilter.Depress(buffer, cx, cy, radius, 15f, TerrainFalloffType.Smoothstep);
        Assert.True(buffer[(int)cx, (int)cy] < 30f && buffer[(int)cx, (int)cy] > 0f);
    }

    [Theory]
    [InlineData(1.0f)]
    [InlineData(2.0f)]
    [InlineData(4.0f)]
    public void SculptFilter_Terrace_creates_stepped_flat_plateaus(float edgeSharpness)
    {
        var buffer = new TerrainHeightWorkBuffer(TestSize);
        // 建立連續斜坡 0 ~ 80
        for (int y = 0; y < TestSize; y++)
        for (int x = 0; x < TestSize; x++)
        {
            buffer[x, y] = x * 2.5f;
        }

        // 套用全圖台階化，階梯間距 20
        TerrainSculptFilter.Terrace(buffer, -1f, -1f, -1f, stepInterval: 20f, flatness: 0.95f, edgeSharpness: edgeSharpness);

        // 驗證階梯中間區域斜率趨近於 0（形成平頂台地）
        // 例如原始高度 28~32 區間應被拉平收斂至該階中央
        float diff1 = MathF.Abs(buffer[11, 0] - buffer[12, 0]);
        float originalDiff = 2.5f;
        Assert.True(diff1 < originalDiff * 0.5f, "階面應被平坦化");
        Assert.True(buffer[9, 0] - buffer[8, 0] > originalDiff, "階緣應比原始斜坡陡峭");
        Assert.Equal(30f, buffer[12, 0]);
        for (int x = 1; x < TestSize; x++)
            Assert.True(buffer[x, 0] >= buffer[x - 1, 0], "階梯高度應保持單調");
    }

    [Fact]
    public void SculptFilter_SharpenRidge_enhances_convex_peaks()
    {
        var buffer = new TerrainHeightWorkBuffer(TestSize);
        // 建立一山脊：沿 y 軸中心 (x=16) 為脊頂高 100，向兩側線性遞減
        for (int y = 0; y < TestSize; y++)
        for (int x = 0; x < TestSize; x++)
        {
            buffer[x, y] = Math.Max(0f, 100f - MathF.Abs(x - 16f) * 6f);
        }

        float ridgeBefore = buffer[16, 16];
        TerrainSculptFilter.SharpenRidge(buffer, 16f, 16f, 10f, gain: 0.6f, ridgeOnly: true, iterations: 2);
        float ridgeAfter = buffer[16, 16];

        // 山脊頂部為凸起 (Laplacian < 0)，銳化後應向上拉升增強
        Assert.True(ridgeAfter > ridgeBefore, "凸出山脊頂部經尖銳化應增高");
    }

    [Fact]
    public void HydraulicErosion_carves_valleys_and_deposits_sediment()
    {
        var buffer = new TerrainHeightWorkBuffer(TestSize);
        // 建立一孤峰 (錐體)
        float cx = 16f, cy = 16f;
        for (int y = 0; y < TestSize; y++)
        for (int x = 0; x < TestSize; x++)
        {
            float dist = MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            buffer[x, y] = MathF.Max(10f, 120f - dist * 8f);
        }

        float initialSummit = buffer[(int)cx, (int)cy];
        float initialBase = buffer[4, 4];

        var simParams = new HydraulicErosionParams
        {
            DropletCount = 3000,
            MaxLifetime = 25,
            Inertia = 0.15f,
            ErosionRate = 0.4f,
            DepositionRate = 0.3f,
            RandomSeed = 1234
        };

        HydraulicErosionSimulator.Simulate(buffer, simParams, cx, cy, 12f);

        // 陡峭山腰或山峰受水流侵蝕，部分高度下降
        float summitAfter = buffer[(int)cx, (int)cy];
        Assert.True(summitAfter <= initialSummit);

        // 部分泥沙沉積至山麓
        bool hasChanges = false;
        for (int y = 0; y < TestSize; y++)
        for (int x = 0; x < TestSize; x++)
        {
            float initialH = MathF.Max(10f, 120f - MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy)) * 8f);
            if (MathF.Abs(buffer[x, y] - initialH) > 0.05f)
            {
                hasChanges = true;
                break;
            }
        }
        Assert.True(hasChanges, "水力侵蝕應產生實質地形重塑變更");
    }

    [Fact]
    public void ThermalErosion_relaxes_vertical_cliff_to_talus_angle_with_mass_conservation()
    {
        var buffer = new TerrainHeightWorkBuffer(TestSize);
        // 建立一垂直斷崖：x <= 15 為高台地 (150)，x >= 16 為低窪 (50)
        for (int y = 0; y < TestSize; y++)
        for (int x = 0; x < TestSize; x++)
        {
            buffer[x, y] = x <= 15 ? 150f : 50f;
        }

        float totalMassBefore = buffer.RawData.Sum();
        float[] screeMap = new float[TestSize * TestSize];

        var thermalParams = new ThermalErosionParams
        {
            TalusAngleDegrees = 35.0f,
            ErosionRate = 0.4f,
            Iterations = 10,
            GridSpacing = 1.0f
        };

        ThermalErosionSimulator.Simulate(buffer, thermalParams, screeAccumulationMap: screeMap);

        // 懸崖頂部邊緣 (x=15) 應崩塌下降
        Assert.True(buffer[15, 16] < 150f, "過陡岩壁應崩塌降低");

        // 懸崖底部坡腳 (x=16) 應堆積碎石升高
        Assert.True(buffer[16, 16] > 50f, "坡腳應堆積碎石升高");

        // 碎石堆積圖應記錄正向沉積
        Assert.True(screeMap[16 * TestSize + 16] > 0f, "Scree 堆積圖應記錄崩落碎石");

        // 質量守恆檢驗（邊界無出界時總高度質量守恆）
        float totalMassAfter = buffer.RawData.Sum();
        Assert.Equal(totalMassBefore, totalMassAfter, 0.5f);
    }

    [Fact]
    public void TerrainSculptPipeline_integrates_seamlessly_with_session_and_undo_redo()
    {
        var session = CreateSession(defaultHeight: 80);

        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);

        // 透過管線執行隆起雕刻
        var changes = TerrainSculptPipeline.Elevate(session, 16f, 16f, 6f, 25f);
        Assert.NotEmpty(changes);
        Assert.True(session.HeightsDirty);
        Assert.True(session.CanUndo);

        session.CommitStroke();
        Assert.False(session.CanRedo);

        byte summitHeight = session.Heights[16 * TestSize + 16];
        Assert.True(summitHeight > 80);

        // 執行熱力滑坡侵蝕
        TerrainSculptPipeline.ThermalErosion(session, new ThermalErosionParams { Iterations = 3 }, 16f, 16f, 8f);
        session.CommitStroke();

        // 撤銷熱力侵蝕
        session.Undo();
        Assert.Equal(summitHeight, session.Heights[16 * TestSize + 16]);

        // 撤銷隆起雕刻，還原回基準面 80
        session.Undo();
        Assert.Equal((byte)80, session.Heights[16 * TestSize + 16]);
        Assert.False(session.HeightsDirty);

        // 重做隆起雕刻
        session.Redo();
        Assert.Equal(summitHeight, session.Heights[16 * TestSize + 16]);
        Assert.True(session.HeightsDirty);
    }
}
