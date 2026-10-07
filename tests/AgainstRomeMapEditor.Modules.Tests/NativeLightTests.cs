using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeLightTests
{
    [Fact]
    public void NativeLightDefinition_CreateDefault_ReturnsExpectedValues()
    {
        var def = NativeLightDefinition.CreateDefault(5);
        Assert.Equal(5, def.Index);
        Assert.True(def.IsActive);
        Assert.Equal(Vector3.One, def.Color);
        Assert.Equal(500.0f, def.Radius);
        Assert.Equal(250000.0f, def.RadiusSquared);
        Assert.Equal(1.0f / 250000.0f, def.InvRadiusSquared, precision: 6);
        Assert.Equal(0, def.FlickerType);
        Assert.Equal(0.0f, def.FlickerParam);
        Assert.Equal(0, def.SpecialFx);
        Assert.Equal("DefaultLight", def.Name);
    }

    [Fact]
    public void NativeLightCatalog_Parse_ParsesIniTableAndSkipsComments()
    {
        const string ini = """
            ; Comment line
            [LightDefault]
            ;idx ,activ,  red,  grn,  blu,      rad, type,     typep,spefx,-------------name-------------
               0,    1,  1.00,  1.00,  1.00,   500.00,    0,      0.00,    0,DefaultLight
               1,    1,  1.00,  0.60,  0.20,   300.00,    1,      0.15,    0,TorchFire
               2,    0,  0.50,  0.80,  1.00,   450.00,    2,      1.00,    1,ColdMagic
            # Another comment
            """;

        var catalog = NativeLightCatalog.Parse(ini);
        Assert.Equal(3, catalog.Count);

        Assert.True(catalog.TryGetDefinition(1, out var torch));
        Assert.Equal(1, torch.Index);
        Assert.True(torch.IsActive);
        Assert.Equal(new Vector3(1.00f, 0.60f, 0.20f), torch.Color);
        Assert.Equal(300.0f, torch.Radius);
        Assert.Equal(1, torch.FlickerType);
        Assert.Equal(0.15f, torch.FlickerParam, precision: 3);
        Assert.Equal("TorchFire", torch.Name);

        Assert.True(catalog.TryGetDefinition(2, out var magic));
        Assert.False(magic.IsActive);
        Assert.Equal(new Vector3(0.50f, 0.80f, 1.00f), magic.Color);
        Assert.Equal(450.0f, magic.Radius);
        Assert.Equal(2, magic.FlickerType);
        Assert.Equal(1, magic.SpecialFx);
    }

    [Fact]
    public void NativeLightCatalog_GetDefinition_ReturnsFallbackWhenMissing()
    {
        var catalog = new NativeLightCatalog();
        var def = catalog.GetDefinition(42);

        Assert.NotNull(def);
        Assert.Equal(42, def.Index);
        Assert.Equal(500.0f, def.Radius);
        Assert.Equal(Vector3.One, def.Color);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_RespectsRadiusCutoff()
    {
        var ambient = new Vector3(0.2f, 0.3f, 0.4f);
        var def = new NativeLightDefinition
        {
            Index = 1,
            IsActive = true,
            Color = new Vector3(1.0f, 0.8f, 0.5f),
            Radius = 100.0f
        };
        var light = new NativeLightInstance(new Vector3(1000, 50, 1000), def);

        // 距離 light 超過 100：如位於 (1000, 50, 1101)，距離 101
        var targetPos = new Vector3(1000, 50, 1101);
        var lit = NativeLightCalculator.Evaluate(ambient, targetPos, new[] { light });

        Assert.Equal(ambient, lit);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_AtCenter_GivesMaxOfAmbientAndLightColor()
    {
        var ambient = new Vector3(0.2f, 0.6f, 0.1f);
        var def = new NativeLightDefinition
        {
            Index = 1,
            IsActive = true,
            Color = new Vector3(0.9f, 0.4f, 0.8f),
            Radius = 200.0f
        };
        var light = new NativeLightInstance(new Vector3(500, 0, 500), def);

        var lit = NativeLightCalculator.Evaluate(ambient, new Vector3(500, 0, 500), new[] { light });
        // Max(ambient, candidate) at center where falloff = 1.0:
        // R = max(0.2, 0.9) = 0.9
        // G = max(0.6, 0.4) = 0.6
        // B = max(0.1, 0.8) = 0.8
        Assert.Equal(new Vector3(0.9f, 0.6f, 0.8f), lit);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_ClampsToOne()
    {
        var ambient = new Vector3(0.5f, 0.5f, 0.5f);
        var def = new NativeLightDefinition
        {
            Index = 1,
            IsActive = true,
            Color = new Vector3(1.5f, 1.2f, 0.8f),
            Radius = 100.0f
        };
        var light = new NativeLightInstance(new Vector3(0, 0, 0), def);

        var lit = NativeLightCalculator.Evaluate(ambient, new Vector3(0, 0, 0), new[] { light });
        Assert.Equal(1.0f, lit.X);
        Assert.Equal(1.0f, lit.Y);
        Assert.Equal(0.8f, lit.Z);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_QuadraticFalloff_MatchesFormula()
    {
        var ambient = Vector3.Zero;
        var def = new NativeLightDefinition
        {
            Index = 1,
            IsActive = true,
            Color = new Vector3(1.0f, 1.0f, 1.0f),
            Radius = 100.0f
        };
        var light = new NativeLightInstance(new Vector3(0, 0, 0), def);

        // 距離 d = 60，d² = 3600，R² = 10000。
        // falloff = 1 - 3600/10000 = 0.64
        var target = new Vector3(60, 0, 0);
        var lit = NativeLightCalculator.Evaluate(ambient, target, new[] { light });

        Assert.Equal(0.64f, lit.X, precision: 4);
        Assert.Equal(0.64f, lit.Y, precision: 4);
        Assert.Equal(0.64f, lit.Z, precision: 4);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_MultipleLights_PerChannelMax()
    {
        var ambient = new Vector3(0.1f, 0.1f, 0.1f);
        var light1 = new NativeLightInstance(
            new Vector3(0, 0, 0),
            new NativeLightDefinition { Index = 1, IsActive = true, Color = new Vector3(0.8f, 0.2f, 0.0f), Radius = 100.0f });

        var light2 = new NativeLightInstance(
            new Vector3(0, 0, 0),
            new NativeLightDefinition { Index = 2, IsActive = true, Color = new Vector3(0.0f, 0.7f, 0.9f), Radius = 100.0f });

        // 中心點受兩個光源照射：
        // light1: (0.8, 0.2, 0.0)
        // light2: (0.0, 0.7, 0.9)
        // 合成後每色道取 max: (0.8, 0.7, 0.9)
        var lit = NativeLightCalculator.Evaluate(ambient, new Vector3(0, 0, 0), new[] { light1, light2 });
        Assert.Equal(new Vector3(0.8f, 0.7f, 0.9f), lit);
    }

    [Fact]
    public void NativeLightCalculator_Evaluate_SkipsInactiveLights()
    {
        var ambient = new Vector3(0.2f, 0.2f, 0.2f);
        var inactiveLight = new NativeLightInstance(
            new Vector3(0, 0, 0),
            new NativeLightDefinition { Index = 1, IsActive = false, Color = Vector3.One, Radius = 500.0f },
            isActive: false);

        var lit = NativeLightCalculator.Evaluate(ambient, new Vector3(0, 0, 0), new[] { inactiveLight });
        Assert.Equal(ambient, lit);
    }

    [Theory]
    [InlineData(96, 48, 192f, 0f)]       // 右前門柱火把
    [InlineData(-128, 64, 0f, 256f)]     // 左前門柱火把
    [InlineData(128, -64, 0f, -256f)]    // 右後側火把
    [InlineData(-128, -64, -256f, 0f)]   // 左後側火把
    public void IsometricAptOffsetToWorldDelta_TransformsAccurately(int screenX, int screenY, float expectedWorldX, float expectedWorldZ)
    {
        var worldDelta = NativeLightCalculator.IsometricAptOffsetToWorldDelta(screenX, screenY, heightOffset: 80f);
        Assert.Equal(expectedWorldX, worldDelta.X);
        Assert.Equal(80f, worldDelta.Y);
        Assert.Equal(expectedWorldZ, worldDelta.Z);
    }

    [Fact]
    public void NativeLightInstance_ThrowsOnNullDefinition()
    {
        Assert.Throws<ArgumentNullException>(() => new NativeLightInstance(Vector3.Zero, null!));
    }

    [Fact]
    public void NativeLightCatalog_ParseRealExcerpt_MatchesGameDefinitions()
    {
        // 來自遊戲真實 lightdef.dau / [LightDefault] 之代表性片段（涵蓋主屋火光 Kohleschale、鐵匠 Schmiedenglut、火堆 Feuerstelle、火把 Flamme_Fackel）
        const string excerpt = """
            [LightDefault]
            ;idx ,activ,  red,  grn,  blu,      rad, type,     typep,spefx,-------------name-------------
                0,    1, 0.33, 0.93, 1.00,    10.00,    0,    100.00,    1,                     Testlicht
                1,    1, 1.48, 1.22, 0.00,   350.00,    1,      0.05,    0,                   Kohleschale
                2,    1, 1.50, 1.09, 0.00,   350.00,    1,      0.05,    0,                 Schmiedenglut
                7,    1, 1.33, 0.61, 0.33,   300.00,    1,      0.15,    0,                LD_Feuerstelle
               18,    1, 1.60, 1.33, 0.00,   150.00,    1,      0.05,    0,              LD_Flamme_Fackel
            """;

        var catalog = NativeLightCatalog.Parse(excerpt);
        Assert.Equal(5, catalog.Count);

        // 主屋火盆 (Kohleschale, idx 1)
        Assert.True(catalog.TryGetDefinition(1, out var kohle));
        Assert.Equal(1, kohle.Index);
        Assert.True(kohle.IsActive);
        Assert.Equal(new Vector3(1.48f, 1.22f, 0.00f), kohle.Color);
        Assert.Equal(350.0f, kohle.Radius);
        Assert.Equal(1, kohle.FlickerType);
        Assert.Equal(0.05f, kohle.FlickerParam, precision: 3);
        Assert.Equal("Kohleschale", kohle.Name);

        // 火堆 (LD_Feuerstelle, idx 7)
        Assert.True(catalog.TryGetDefinition(7, out var feuer));
        Assert.Equal(7, feuer.Index);
        Assert.Equal(new Vector3(1.33f, 0.61f, 0.33f), feuer.Color);
        Assert.Equal(300.0f, feuer.Radius);
        Assert.Equal(1, feuer.FlickerType);
        Assert.Equal(0.15f, feuer.FlickerParam, precision: 3);
        Assert.Equal("LD_Feuerstelle", feuer.Name);

        // 火把 (LD_Flamme_Fackel, idx 18)
        Assert.True(catalog.TryGetDefinition(18, out var fackel));
        Assert.Equal(18, fackel.Index);
        Assert.Equal(new Vector3(1.60f, 1.33f, 0.00f), fackel.Color);
        Assert.Equal(150.0f, fackel.Radius);
        Assert.Equal(1, fackel.FlickerType);
        Assert.Equal(0.05f, fackel.FlickerParam, precision: 3);
        Assert.Equal("LD_Flamme_Fackel", fackel.Name);
    }

    [Fact]
    public void NativeLightCatalog_ParseBytes_SupportsPfilCompression()
    {
        const string excerpt = "[LightDefault]\n   1, 1, 1.48, 1.22, 0.00, 350.00, 1, 0.05, 0, Kohleschale\n";
        byte[] rawBytes = AgainstRomeModifier.Maps.MapTextEncoding.Game.GetBytes(excerpt);

        // 測試純文字 bytes
        var catFromPlain = NativeLightCatalog.Parse(rawBytes);
        Assert.True(catFromPlain.TryGetDefinition(1, out var plainDef));
        Assert.Equal("Kohleschale", plainDef.Name);

        // 建立假 PFIL 標頭測試壓縮容器
        byte[] header = new byte[64];
        header[0] = (byte)'P'; header[1] = (byte)'F'; header[2] = (byte)'I'; header[3] = (byte)'L';
        byte[] pfilBytes = AgainstRomeModifier.GameLZSS.CompressPfil(rawBytes, header);
        var catFromPfil = NativeLightCatalog.Parse(pfilBytes);
        Assert.True(catFromPfil.TryGetDefinition(1, out var pfilDef));
        Assert.Equal("Kohleschale", pfilDef.Name);
        Assert.Equal(350.0f, pfilDef.Radius);
    }

    [Fact]
    public void NativeAptLightPoints_ExtractLightPoints_CorrectlyCalculatesDeltasAndWorldPositions()
    {
        // 建立符合 APAT v3 規格之合成最小檔案結構：
        // header: 28 words
        // row offsets & widths: 62 words
        // first anchor: 2 words
        // anchor: 2 ints (569, 405)
        // extraA, extraB: 2 ints (4, 0)
        // groups: 1 int (0)
        // extraA points: 4 pairs of ints
        var stream = new MemoryStream();
        using (var writer = new BinaryWriter(stream))
        {
            uint[] header = new uint[28];
            header[0] = 0x54415041; // "APAT"
            header[1] = 3;          // v3
            header[3] = 112;        // tableOffset
            header[6] = 64;
            header[7] = 31;
            header[9] = 1;          // groupVariants
            header[13] = 8;
            header[20] = 1;         // colors
            header[21] = 1;         // variants
            header[23] = 64;        // width
            header[24] = 31;        // height
            header[27] = 8;         // pdat bytes
            foreach (uint w in header) writer.Write(w);

            // rowOffsets (31 words) + rowWidths (31 words)
            for (int i = 0; i < 62; i++) writer.Write(0u);

            // First anchor pair (2 words)
            writer.Write(0u); writer.Write(0u);

            // AnchorX, AnchorY (569, 405)
            writer.Write(569);
            writer.Write(405);

            // extraA = 4, extraB = 0
            writer.Write(4u);
            writer.Write(0u);

            // groups = 0
            writer.Write(0u);

            // 4 light points (與日耳曼主屋 gerhau00.apt extraA 相同之螢幕坐標)
            writer.Write(665); writer.Write(453); // delta = (96, 48)
            writer.Write(441); writer.Write(469); // delta = (-128, 64)
            writer.Write(697); writer.Write(341); // delta = (128, -64)
            writer.Write(441); writer.Write(341); // delta = (-128, -64)
        }

        byte[] aptBytes = stream.ToArray();

        var points = NativeAptLightPoints.ExtractLightPoints(aptBytes);
        Assert.Equal(4, points.Count);
        Assert.Equal(96, points[0].DeltaAnchorX);
        Assert.Equal(48, points[0].DeltaAnchorY);
        Assert.Equal(-128, points[1].DeltaAnchorX);
        Assert.Equal(64, points[1].DeltaAnchorY);

        // 測試計算世界空間坐標（建築物位於 10624, 10112，地面 0，高度偏移 80）
        var worldPositions = NativeAptLightPoints.GetBuildingWorldLightPositions(
            aptBytes,
            buildingWorldX: 10624f,
            buildingWorldZ: 10112f,
            groundY: 0f,
            aptHeightOffset: 80f);

        Assert.Equal(4, worldPositions.Count);
        // L0: 10624 + (96 + 2*48) = 10624 + 192 = 10816, Z = 10112 + (2*48 - 96) = 10112
        Assert.Equal(new Vector3(10816f, 80f, 10112f), worldPositions[0]);
        // L1: 10624 + (-128 + 2*64) = 10624 + 0 = 10624, Z = 10112 + (2*64 - (-128)) = 10112 + 256 = 10368
        Assert.Equal(new Vector3(10624f, 80f, 10368f), worldPositions[1]);
        // L2: 10624 + (128 - 128) = 10624, Z = 10112 + (-128 - 128) = 10112 - 256 = 9856
        Assert.Equal(new Vector3(10624f, 80f, 9856f), worldPositions[2]);
        // L3: 10624 + (-128 - 128) = 10624 - 256 = 10368, Z = 10112 + (-128 - (-128)) = 10112
        Assert.Equal(new Vector3(10368f, 80f, 10112f), worldPositions[3]);
    }
}
