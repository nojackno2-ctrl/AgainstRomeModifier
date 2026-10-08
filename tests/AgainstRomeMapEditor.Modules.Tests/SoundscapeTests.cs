using AgainstRomeMapEditor.Modules.Soundscape;
using SoundscapeZoneFlags = AgainstRomeMapEditor.Modules.Soundscape.SoundscapeZoneAttributes;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class SoundscapeTests
{
    #region 1. Catalog 與空間衰減幾何測試

    [Fact]
    public void Catalog_HasDefaultNativePresets()
    {
        var catalog = SoundscapeZoneCatalog.Shared;
        Assert.NotNull(catalog.Resolve("Amb_Forest_Dense_Day"));
        Assert.NotNull(catalog.Resolve("Amb_River_Gentle"));
        Assert.NotNull(catalog.Resolve("Amb_Waterfall_Roar"));
        Assert.NotNull(catalog.Resolve("Amb_Coast_Waves"));
        Assert.NotNull(catalog.Resolve("Snd_Campfire_Crackling"));
        Assert.NotNull(catalog.Resolve("Snd_Blacksmith_Anvil"));
    }

    [Fact]
    public void AttenuationCurve_EvaluatesExpectedThresholds()
    {
        // 距離小於內半徑：音量 1.0
        Assert.Equal(1.0f, SoundscapeZoneCatalog.CalculateAttenuation(50f, 100f, 500f, AttenuationCurveKind.Linear));
        Assert.Equal(1.0f, SoundscapeZoneCatalog.CalculateAttenuation(100f, 100f, 500f, AttenuationCurveKind.SmoothStep));

        // 距離大於外半徑：音量 0.0
        Assert.Equal(0.0f, SoundscapeZoneCatalog.CalculateAttenuation(550f, 100f, 500f, AttenuationCurveKind.Linear));
        Assert.Equal(0.0f, SoundscapeZoneCatalog.CalculateAttenuation(500f, 100f, 500f, AttenuationCurveKind.Logarithmic));

        // 中點衰減值 (t = 0.5)
        float midLinear = SoundscapeZoneCatalog.CalculateAttenuation(300f, 100f, 500f, AttenuationCurveKind.Linear);
        Assert.Equal(0.5f, midLinear, 3);

        float midSmooth = SoundscapeZoneCatalog.CalculateAttenuation(300f, 100f, 500f, AttenuationCurveKind.SmoothStep);
        Assert.Equal(0.5f, midSmooth, 3); // 3*(0.5)^2 - 2*(0.5)^3 = 0.5 -> 1 - 0.5 = 0.5

        float midExp = SoundscapeZoneCatalog.CalculateAttenuation(300f, 100f, 500f, AttenuationCurveKind.Exponential);
        Assert.Equal(0.25f, midExp, 3); // (1 - 0.5)^2 = 0.25
    }

    [Fact]
    public void DistanceCalculation_CircleAndRectangle()
    {
        var circle = new SoundscapeZone
        {
            ShapeType = SoundscapeShapeType.Circle,
            CenterWorldX = 1000f,
            CenterWorldZ = 1000f,
            ParamA = 200f // 半徑 200
        };

        // 圓心內部
        Assert.Equal(0f, SoundscapeZoneCatalog.CalculateDistanceToZone(1000f, 1000f, circle));
        Assert.Equal(0f, SoundscapeZoneCatalog.CalculateDistanceToZone(1150f, 1000f, circle));

        // 圓外 100 單位 (1000 + 200 + 100 = 1300)
        Assert.Equal(100f, SoundscapeZoneCatalog.CalculateDistanceToZone(1300f, 1000f, circle), 2);

        var rect = new SoundscapeZone
        {
            ShapeType = SoundscapeShapeType.OrientedRectangle,
            CenterWorldX = 0f,
            CenterWorldZ = 0f,
            ParamA = 100f, // halfW = 100
            ParamB = 50f,  // halfH = 50
            ParamAngleDeg = 0f
        };

        // 矩形內部
        Assert.Equal(0f, SoundscapeZoneCatalog.CalculateDistanceToZone(50f, 20f, rect));
        // 矩形右側 (X=150, Z=0) -> 距離 50
        Assert.Equal(50f, SoundscapeZoneCatalog.CalculateDistanceToZone(150f, 0f, rect), 2);
    }

    [Fact]
    public void DistanceCalculation_ConvexPolygon()
    {
        var polyZone = new SoundscapeZone
        {
            ShapeType = SoundscapeShapeType.ConvexPolygon,
            PolygonVertices = [
                new SoundscapeVertex(0f, 0f),
                new SoundscapeVertex(200f, 0f),
                new SoundscapeVertex(200f, 200f),
                new SoundscapeVertex(0f, 200f)
            ]
        };

        // 多邊形內部
        Assert.Equal(0f, SoundscapeZoneCatalog.CalculateDistanceToZone(100f, 100f, polyZone));
        // 多邊形外部
        Assert.Equal(50f, SoundscapeZoneCatalog.CalculateDistanceToZone(250f, 100f, polyZone), 2);
    }

    #endregion

    #region 2. 規劃器與智慧伴生生成測試

    [Fact]
    public void Planner_CreatesValidZones()
    {
        var planner = new SoundscapeZonePlanner();
        var pointZone = planner.CreatePointZone("Campfire", "Snd_Campfire_Crackling", 500f, 10f, 600f, 80f, 300f);
        Assert.Equal(SoundscapeShapeType.Point, pointZone.ShapeType);
        Assert.Equal(AudioCategory.PointEmitter, pointZone.Category);
        Assert.Equal(500f, pointZone.CenterWorldX);

        var rectZone = planner.CreateRectangleZone("Valley", "Amb_River_Gentle", 1000f, 0f, 1000f, 300f, 80f, angleDeg: 45f);
        Assert.Equal(SoundscapeShapeType.OrientedRectangle, rectZone.ShapeType);
        Assert.Equal(45f, rectZone.ParamAngleDeg);

        List<SoundscapeVertex> triangle = [
            new(0f, 0f), new(100f, 0f), new(0f, 100f)
        ];
        var polyZone = planner.CreateConvexPolygonZone("Grove", "Amb_Forest_Light", triangle);
        Assert.Equal(SoundscapeShapeType.ConvexPolygon, polyZone.ShapeType);
        Assert.Equal(3, polyZone.PolygonVertices.Count);
    }

    [Fact]
    public void Planner_ComputesConvexHullCorrectly()
    {
        List<SoundscapeVertex> points = [
            new(0f, 0f),
            new(10f, 0f),
            new(10f, 10f),
            new(0f, 10f),
            new(5f, 5f) // 內部點
        ];

        var hull = SoundscapeZonePlanner.ComputeConvexHull(points);
        Assert.Equal(4, hull.Count); // 內部點應被排除，剩下外圍 4 角
    }

    [Fact]
    public void Planner_GeneratesHydrologySoundscapes()
    {
        var planner = new SoundscapeZonePlanner();

        // 模擬 16x16 地圖，中央有一小水潭（4 格水）
        int dim = 16;
        var heights = new byte[17 * 17];
        Array.Fill(heights, (byte)150); // 全高地

        // (5,5) 至 (6,6) 4 格低於水位 120
        heights[5 * 17 + 5] = 80;
        heights[5 * 17 + 6] = 80;
        heights[6 * 17 + 5] = 80;
        heights[6 * 17 + 6] = 80;

        var context = new SoundscapeZonePlanner.WaterCompanionContext(
            Dimension: dim,
            TileWorldSize: 64.0f,
            Heights: heights,
            VertexSize: 17,
            HeightMapStep: 4.0f,
            WaterLevel: 120,
            RiverTileIndices: null);

        var zones = planner.GenerateHydrologySoundscapes(context);
        Assert.NotEmpty(zones);
        Assert.Contains(zones, z => z.SoundDefId == "Amb_Pond_Marsh");
    }

    [Fact]
    public void Planner_GeneratesForestSoundscapes()
    {
        var planner = new SoundscapeZonePlanner();

        // 在同一個 512x512 空間箱 (X: 100~300, Z: 100~300) 散佈 10 棵樹木
        var trees = new List<(float X, float Z)>();
        for (int i = 0; i < 10; i++)
        {
            trees.Add((100f + i * 20f, 100f + (i % 3) * 30f));
        }

        var zones = planner.GenerateForestSoundscapes(trees, clusterBinWorldSize: 512f, denseClusterThreshold: 8);
        Assert.NotEmpty(zones);
        Assert.Contains(zones, z => z.SoundDefId == "Amb_Forest_Dense_Day");
        Assert.Contains(zones, z => z.SoundDefId == "Amb_Forest_Birds");
    }

    [Fact]
    public void Planner_GeneratesSettlementSoundscapes()
    {
        var planner = new SoundscapeZonePlanner();
        var buildings = new List<(string TypeName, float X, float Y, float Z)>
        {
            ("BauGerSchmied01", 1200f, 10f, 1400f),
            ("BauGerFeuerstelle", 1100f, 10f, 1350f)
        };

        var zones = planner.GenerateSettlementSoundscapes(buildings);
        Assert.Equal(2, zones.Count);
        Assert.Contains(zones, z => z.SoundDefId == "Snd_Blacksmith_Anvil");
        Assert.Contains(zones, z => z.SoundDefId == "Snd_Campfire_Crackling");
    }

    #endregion

    #region 3. 混音評估器與立體聲方位測試

    [Fact]
    public void MixEvaluator_EvaluatesVolumeAndStereoPanning()
    {
        var evaluator = new SoundscapeMixEvaluator();

        // 放置一個在聆聽者正右方的點音源
        // 聆聽者朝向北 (0 度)，位於 (1000, 0, 1000)
        // 音源位於 (1300, 0, 1000) -> 距離 300，相對角度 +90 度 (右方)
        var rightZone = new SoundscapeZone
        {
            ShapeType = SoundscapeShapeType.Point,
            CenterWorldX = 1300f,
            CenterWorldZ = 1000f,
            InnerRadius = 100f,
            OuterRadius = 600f,
            BaseVolume = 1.0f,
            AttenuationCurve = AttenuationCurveKind.Linear,
            Category = AudioCategory.PointEmitter
        };

        var mix = evaluator.EvaluateMix(
            listenerX: 1000f,
            listenerY: 0f,
            listenerZ: 1000f,
            listenerYawDeg: 0f,
            zones: [rightZone]);

        Assert.Equal(1, mix.ActiveAudibleCount);
        Assert.False(mix.ChannelBudgetExceeded);
        Assert.True(mix.TotalPerceivedVolume > 0.0f);

        var ch = mix.Channels[0];
        // 右方音源的右聲道增益應明顯大於左聲道
        Assert.True(ch.PanningRight > ch.PanningLeft);
        Assert.True(mix.MasterRightVolume > mix.MasterLeftVolume);
    }

    [Fact]
    public void MixEvaluator_HandlesChannelBudgetOverflow()
    {
        var evaluator = new SoundscapeMixEvaluator();

        // 建立 30 個距離極近的音源
        var zones = new List<SoundscapeZone>();
        for (int i = 0; i < 30; i++)
        {
            zones.Add(new SoundscapeZone
            {
                ShapeType = SoundscapeShapeType.Point,
                CenterWorldX = 1000f + i,
                CenterWorldZ = 1000f,
                InnerRadius = 200f,
                OuterRadius = 800f,
                BaseVolume = 1.0f,
                Priority = (ushort)(100 + i) // 優先級遞增
            });
        }

        var mix = evaluator.EvaluateMix(
            listenerX: 1000f, listenerY: 0f, listenerZ: 1000f,
            listenerYawDeg: 0f,
            zones: zones,
            maxHardwareChannels: 16);

        Assert.True(mix.ChannelBudgetExceeded);
        Assert.Equal(16, mix.ActiveAudibleCount);
        // 應保留最高優先級的通道
        Assert.True(mix.Channels.All(c => c.Priority >= 114));
    }

    [Fact]
    public void MixEvaluator_BuildsGizmoAndHeatmap()
    {
        var evaluator = new SoundscapeMixEvaluator();
        var zone = new SoundscapeZone
        {
            ShapeType = SoundscapeShapeType.Circle,
            CenterWorldX = 500f,
            CenterWorldZ = 500f,
            ParamA = 100f,
            InnerRadius = 50f,
            OuterRadius = 300f,
            Category = AudioCategory.Hydrology
        };

        var gizmos = evaluator.BuildGizmos([zone], selectedZoneId: zone.Id);
        Assert.Single(gizmos);
        Assert.True(gizmos[0].IsSelected);
        Assert.NotEmpty(gizmos[0].CoreBoundary);
        Assert.NotEmpty(gizmos[0].FalloffBoundary);

        var heatmap = evaluator.GenerateAcousticHeatmap([zone], 0f, 0f, 1000f, 1000f, sampleGridResolution: 16);
        Assert.Equal(16, heatmap.GetLength(0));
        Assert.Equal(16, heatmap.GetLength(1));
        // 中心處強度應大於 0
        Assert.True(heatmap[8, 8] > 0.0f);
    }

    #endregion

    #region 4. 二進位儲存與合約讀寫測試

    [Fact]
    public void BinaryStorage_RoundTripPreservesAllFields()
    {
        var originalSnapshot = new SoundscapeSnapshot(
            GlobalAmbienceId: "Amb_Plains_Wind",
            GlobalAmbienceVolume: 0.75f,
            Zones: [
                new SoundscapeZone
                {
                    Name = "Sacred Grove",
                    SoundDefId = "Amb_Forest_Dense_Day",
                    ShapeType = SoundscapeShapeType.ConvexPolygon,
                    AttenuationCurve = AttenuationCurveKind.SmoothStep,
                    Category = AudioCategory.Vegetation,
                    BaseVolume = 0.85f,
                    InnerRadius = 150f,
                    OuterRadius = 650f,
                    CenterWorldX = 2500f,
                    CenterWorldY = 30f,
                    CenterWorldZ = 2800f,
                    Flags = SoundscapeZoneFlags.IsActive | SoundscapeZoneFlags.IsLooping,
                    Priority = 180,
                    PolygonVertices = [
                        new(2400f, 2700f),
                        new(2600f, 2700f),
                        new(2600f, 2900f),
                        new(2400f, 2900f)
                    ]
                },
                new SoundscapeZone
                {
                    Name = "Blacksmith Forge",
                    SoundDefId = "Snd_Blacksmith_Anvil",
                    ShapeType = SoundscapeShapeType.Point,
                    AttenuationCurve = AttenuationCurveKind.Exponential,
                    Category = AudioCategory.Settlement,
                    BaseVolume = 0.9f,
                    InnerRadius = 80f,
                    OuterRadius = 400f,
                    CenterWorldX = 1200f,
                    CenterWorldY = 15f,
                    CenterWorldZ = 1300f,
                    Flags = SoundscapeZoneFlags.IsActive
                }
            ]);

        byte[] binary = SoundscapeBinaryStorage.SerializeToBinary(originalSnapshot);
        Assert.True(binary.Length > 64);

        var restored = SoundscapeBinaryStorage.DeserializeFromBinary(binary);
        Assert.Equal(originalSnapshot.GlobalAmbienceId, restored.GlobalAmbienceId);
        Assert.Equal(originalSnapshot.GlobalAmbienceVolume, restored.GlobalAmbienceVolume, 3);
        Assert.Equal(2, restored.Zones.Count);

        var z0 = restored.Zones[0];
        Assert.Equal("Sacred Grove", z0.Name);
        Assert.Equal("Amb_Forest_Dense_Day", z0.SoundDefId);
        Assert.Equal(SoundscapeShapeType.ConvexPolygon, z0.ShapeType);
        Assert.Equal(4, z0.PolygonVertices.Count);
        Assert.Equal(2400f, z0.PolygonVertices[0].X, 2);

        var z1 = restored.Zones[1];
        Assert.Equal("Blacksmith Forge", z1.Name);
        Assert.Equal(SoundscapeShapeType.Point, z1.ShapeType);
    }

    [Fact]
    public void BinaryStorage_JsonAndScriptExport()
    {
        var snapshot = new SoundscapeSnapshot(
            "Amb_Plains_Wind", 0.5f,
            [
                new SoundscapeZone
                {
                    Name = "River Loop",
                    SoundDefId = "Amb_River_Gentle",
                    ShapeType = SoundscapeShapeType.Circle,
                    CenterWorldX = 1000f,
                    CenterWorldZ = 1000f,
                    ParamA = 150f
                }
            ]);

        string json = SoundscapeBinaryStorage.ExportToJson(snapshot);
        Assert.Contains("River Loop", json);
        var fromJson = SoundscapeBinaryStorage.ImportFromJson(json);
        Assert.Single(fromJson.Zones);

        string script = SoundscapeBinaryStorage.ExportToScript(snapshot);
        Assert.Contains("InitMapSoundscapes", script);
        Assert.Contains("Amb_River_Gentle", script);
    }

    #endregion

    #region 5. EditorModule 契約與 Undo/Redo 測試

    [Fact]
    public void EditorModule_HandlesTransactionsAndHistory()
    {
        var module = new SoundscapeEditorModule();
        Assert.False(module.IsDirty);
        Assert.Equal("SoundscapeZones", module.ModuleId);

        var zone = new SoundscapeZone
        {
            Name = "Initial Zone",
            CenterWorldX = 500f,
            CenterWorldZ = 500f
        };

        module.AddZone(zone);
        Assert.True(module.IsDirty);
        Assert.Single(module.Zones);

        // 接受變更 -> 清除 Dirty
        module.AcceptChanges();
        Assert.False(module.IsDirty);

        // 再加一個區域
        module.AddZone(new SoundscapeZone { Name = "Second Zone" });
        Assert.True(module.IsDirty);
        Assert.Equal(2, module.Zones.Count);

        // Undo -> 恢復 1 個區域
        Assert.True(module.CanUndo);
        Assert.True(module.Undo());
        Assert.Single(module.Zones);

        // Redo -> 恢復 2 個區域
        Assert.True(module.CanRedo);
        Assert.True(module.Redo());
        Assert.Equal(2, module.Zones.Count);

        // Reset -> 恢復到最近一次 Accepted baseline (1 個區域)
        module.Reset();
        Assert.Single(module.Zones);
        Assert.False(module.IsDirty);
    }

    #endregion
}
