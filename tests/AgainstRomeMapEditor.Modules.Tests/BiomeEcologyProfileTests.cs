using System.Runtime.CompilerServices;
using AgainstRomeModifier.Maps;
using AgainstRomeMapEditor.Modules.Nature;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class BiomeEcologyProfileTests
{
    private static LevelObjectTemplate CreateMockTemplate()
        => (LevelObjectTemplate)RuntimeHelpers.GetUninitializedObject(typeof(LevelObjectTemplate));

    [Theory]
    [InlineData(BiomeType.GermanicForest, "Ger")]
    [InlineData(BiomeType.DanubeBroadleaf, "Ger")]
    [InlineData(BiomeType.ItalianHills, "Ita")]
    [InlineData(BiomeType.BritishMarsh, "Kel")]
    public void Presets_define_valid_species_covering_all_ecological_layers(BiomeType biomeType, string expectedRegion)
    {
        BiomeEcologyProfile profile = BiomeEcologyProfile.GetPreset(biomeType);

        Assert.Equal(biomeType, profile.Biome);
        Assert.Equal(expectedRegion, profile.RegionCode);
        Assert.NotEmpty(profile.DisplayNameZh);
        Assert.NotEmpty(profile.DisplayNameEn);

        // 驗證生態層閾值合規
        Assert.True(profile.CanopyThreshold > profile.UnderstoryThreshold);
        Assert.True(profile.UnderstoryThreshold > profile.GroundFloraThreshold);
        Assert.True(profile.GroundFloraThreshold > 0f);
        Assert.True(profile.CliffSlopeThreshold > 15f);

        // 驗證五大生態垂直層級皆有配置物種
        var layers = profile.Species.Select(s => s.Layer).ToHashSet();
        Assert.Contains(VegetationLayer.Canopy, layers);
        Assert.Contains(VegetationLayer.Understory, layers);
        Assert.Contains(VegetationLayer.GroundFlora, layers);
        Assert.Contains(VegetationLayer.Riparian, layers);
        Assert.Contains(VegetationLayer.Rock, layers);

        foreach (var species in profile.Species)
        {
            Assert.StartsWith("Lan", species.NamePrefix);
            Assert.True(species.Weight > 0);
            Assert.True(species.FootprintRadiusTiles > 0.1f);
            Assert.True(species.MaxSlope > 0f);
        }
    }

    [Fact]
    public void ResolveTemplates_maps_exact_prefixes_correctly()
    {
        var profile = BiomeEcologyProfile.GermanicForest;
        var templateNad = CreateMockTemplate();
        var templateBus = CreateMockTemplate();

        var templates = new Dictionary<int, LevelObjectTemplate>
        {
            [101] = templateNad,
            [102] = templateBus,
        };
        var objdefNames = new Dictionary<int, string>
        {
            [101] = "LanGerNad00_Tanne_gross",
            [102] = "LanGerBus01_Hasel",
        };

        ResolvedEcologyRoster roster = profile.ResolveTemplates(templates, objdefNames);

        Assert.True(roster.HasLayer(VegetationLayer.Canopy));
        Assert.True(roster.HasLayer(VegetationLayer.Understory));

        var canopy = roster.GetLayerSpecies(VegetationLayer.Canopy);
        Assert.Contains(canopy, s => s.Name == "LanGerNad00_Tanne_gross");

        var understory = roster.GetLayerSpecies(VegetationLayer.Understory);
        Assert.Contains(understory, s => s.Name == "LanGerBus01_Hasel");
    }

    [Fact]
    public void ResolveTemplates_falls_back_to_regional_and_global_categories()
    {
        var profile = BiomeEcologyProfile.ItalianHills; // Region "Ita"
        var templateItaOther = CreateMockTemplate();
        var templateGlobal = CreateMockTemplate();

        // 模擬該地圖僅有一般的 LanIta 樹木與其他地區的草叢
        var templates = new Dictionary<int, LevelObjectTemplate>
        {
            [201] = templateItaOther,
            [202] = templateGlobal,
        };
        var objdefNames = new Dictionary<int, string>
        {
            [201] = "LanItaBau99", // 義大利一般樹木 (tree)
            [202] = "LanGerGra01", // 日耳曼草 (grass)
        };

        ResolvedEcologyRoster roster = profile.ResolveTemplates(templates, objdefNames);

        // 應該透過地域或全局 fallback 匹配到 Canopy 與 GroundFlora
        Assert.True(roster.HasLayer(VegetationLayer.Canopy));
        Assert.True(roster.HasLayer(VegetationLayer.GroundFlora));

        Assert.True(roster.TryPick(VegetationLayer.Canopy, new Random(42), out var pickedCanopy));
        Assert.Equal("LanItaBau99", pickedCanopy.Name);

        Assert.True(roster.TryPick(VegetationLayer.GroundFlora, new Random(42), out var pickedFlora));
        Assert.Equal("LanGerGra01", pickedFlora.Name);
    }

    [Fact]
    public void ResolvedEcologyRoster_TryPick_respects_weights_distribution()
    {
        var t1 = CreateMockTemplate();
        var t2 = CreateMockTemplate();

        var speciesList = new List<ResolvedSpecies>
        {
            new(t1, "Rare", VegetationLayer.Canopy, Weight: 10.0, FootprintRadiusTiles: 1f, MinElevation: 0, MaxElevation: 255, MaxSlope: 30f),
            new(t2, "Common", VegetationLayer.Canopy, Weight: 90.0, FootprintRadiusTiles: 1f, MinElevation: 0, MaxElevation: 255, MaxSlope: 30f),
        };

        var roster = new ResolvedEcologyRoster(speciesList);
        var random = new Random(12345);

        int commonCount = 0;
        const int iterations = 1000;
        for (int i = 0; i < iterations; i++)
        {
            Assert.True(roster.TryPick(VegetationLayer.Canopy, random, out var picked));
            if (picked.Name == "Common") commonCount++;
        }

        // 90% 權重的物種應在約 900 次左右（容差 800 ~ 950）
        Assert.InRange(commonCount, 820, 960);
    }
}
