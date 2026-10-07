using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class MaterialSuitabilityTests
{
    [Fact]
    public void Materials_are_ranked_by_how_well_they_blend_into_the_current_map()
    {
        // B1 在地圖上；B2 與 B1 有過渡；B4 只能經由 B2 過渡；B3 沒有任何過渡。
        string[] shapes = ["10", "20", "30", "40", "60", "70", "80", "90"];
        var names = new[] { "4B1___50", "4B2___50", "4B3___50", "4B4___50" }
            .Concat(shapes.Select(shape => "4U12__" + shape))
            .Concat(shapes.Select(shape => "4U24__" + shape));
        var catalog = new FloorMaterialCatalog(names);
        string[] map = ["B1"];
        Assert.Equal(0, catalog.MapSuitability("B1", map));
        Assert.Equal(1, catalog.MapSuitability("B2", map));
        Assert.Equal(2, catalog.MapSuitability("B4", map));
        Assert.Equal(3, catalog.MapSuitability("B3", map));
        Assert.True(catalog.HasTransition("B2", "B1"));
        Assert.False(catalog.HasTransition("B1", "B4"));
    }
}
