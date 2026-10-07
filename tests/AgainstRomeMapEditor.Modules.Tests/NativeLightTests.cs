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
}
