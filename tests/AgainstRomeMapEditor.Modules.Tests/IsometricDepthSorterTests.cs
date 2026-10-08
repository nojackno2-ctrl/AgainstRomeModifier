using AgainstRomeMapEditor.Rendering;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class IsometricDepthSorterTests
{
    [Fact]
    public void Layer_ordering_always_dominates_regardless_of_coordinates()
    {
        // 即便陰影位於畫面最南方 (WorldX=100, WorldZ=100)，實體物件位於北方 (WorldX=0, WorldZ=0)，
        // 陰影圖層 (GroundShadow) 永遠優先於實體物件 (StandardObject) 繪製
        var shadow = new IsometricSortItem(1, 100f, 0f, 100f, IsometricFootprint.Point, IsometricRenderLayer.GroundShadow);
        var unit = new IsometricSortItem(2, 0f, 0f, 0f, IsometricFootprint.Unit, IsometricRenderLayer.StandardObject);
        var canopy = new IsometricSortItem(3, 0f, 10f, 0f, IsometricFootprint.Point, IsometricRenderLayer.OverheadCanopy);

        Assert.True(IsometricDepthSorter.ComputeKey(shadow) < IsometricDepthSorter.ComputeKey(unit));
        Assert.True(IsometricDepthSorter.ComputeKey(unit) < IsometricDepthSorter.ComputeKey(canopy));

        IsometricSortItem[] items = [canopy, shadow, unit];
        IsometricDepthSorter.Sort(items);

        Assert.Equal(IsometricRenderLayer.GroundShadow, items[0].Layer);
        Assert.Equal(IsometricRenderLayer.StandardObject, items[1].Layer);
        Assert.Equal(IsometricRenderLayer.OverheadCanopy, items[2].Layer);
    }

    [Fact]
    public void High_altitude_unit_behind_tree_is_drawn_before_foreground_tree()
    {
        // 解決高低差假性靠近痛點：
        // 山頂部隊 (WorldX=5, WorldZ=5, WorldY=40，位處地圖北方深處)
        // 山腳前景大樹 (WorldX=12, WorldZ=12, WorldY=0，位處南方前景)
        // 在 2.5D 畫家演算法中，山頂部隊必須先畫 (Depth 小)，前景大樹後畫 (Depth 大)，
        // 確保前景大樹能正確遮擋背景山頂部隊，而不被相機 3D View-Z 誤判反轉！
        var mountainSoldier = new IsometricSortItem(101, 5f, 40f, 5f, IsometricFootprint.Unit);
        var foregroundTree = new IsometricSortItem(102, 12f, 0f, 12f, new IsometricFootprint(1.5f, 1.5f, 8f));

        Assert.True(IsometricDepthSorter.ShouldDrawBefore(mountainSoldier, foregroundTree));
        Assert.True(IsometricDepthSorter.ComputeKey(mountainSoldier) < IsometricDepthSorter.ComputeKey(foregroundTree));

        IsometricSortItem[] scene = [foregroundTree, mountainSoldier];
        IsometricDepthSorter.Sort(scene);

        Assert.Equal(101, scene[0].ItemId); // 山頂部隊先畫
        Assert.Equal(102, scene[1].ItemId); // 前景大樹後畫
    }

    [Fact]
    public void Multi_tile_large_building_and_surrounding_units_sort_correctly()
    {
        // 羅馬高塔：佔地 2x2 格，起始坐標 (10, 10)，最南端足跡延伸至 (12, 12)
        var tower = new IsometricSortItem(200, 10f, 0f, 10f, IsometricFootprint.RomanTower);

        // 小兵 A 站在高塔北方屋後 (WorldX=9, WorldZ=9) -> 高塔前方遮擋小兵 A，故小兵 A 應先畫
        var soldierRear = new IsometricSortItem(201, 9f, 0f, 9f, IsometricFootprint.Unit);

        // 小兵 B 站在高塔南方門前 (WorldX=13, WorldZ=13) -> 小兵 B 遮擋高塔基底，故小兵 B 應後畫
        var soldierFront = new IsometricSortItem(202, 13f, 0f, 13f, IsometricFootprint.Unit);

        Assert.True(IsometricDepthSorter.ComputeKey(soldierRear) < IsometricDepthSorter.ComputeKey(tower));
        Assert.True(IsometricDepthSorter.ComputeKey(tower) < IsometricDepthSorter.ComputeKey(soldierFront));

        IsometricSortItem[] scene = [soldierFront, tower, soldierRear];
        IsometricDepthSorter.Sort(scene);

        Assert.Equal(201, scene[0].ItemId); // 屋後小兵
        Assert.Equal(200, scene[1].ItemId); // 高塔本體
        Assert.Equal(202, scene[2].ItemId); // 門前小兵
    }

    [Fact]
    public void Deterministic_tie_breaker_prevents_flickering_between_coplanar_items()
    {
        // 兩個同位置、同深度的物件，排序必須具有 100% 確定性 (Deterministic)，絕不可因隨機浮點擾動而幀間閃爍
        var itemA = new IsometricSortItem(10, 5f, 0f, 5f, IsometricFootprint.Point, subPriority: 0);
        var itemB = new IsometricSortItem(20, 5f, 0f, 5f, IsometricFootprint.Point, subPriority: 0);

        var keyA = IsometricDepthSorter.ComputeKey(itemA);
        var keyB = IsometricDepthSorter.ComputeKey(itemB);

        Assert.NotEqual(keyA.Value, keyB.Value);

        IsometricSortItem[] list1 = [itemA, itemB];
        IsometricSortItem[] list2 = [itemB, itemA];

        IsometricDepthSorter.Sort(list1);
        IsometricDepthSorter.Sort(list2);

        Assert.Equal(list1[0].ItemId, list2[0].ItemId);
        Assert.Equal(list1[1].ItemId, list2[1].ItemId);
    }

    [Fact]
    public void SortIndices_produces_identical_sequence_without_modifying_source()
    {
        IsometricSortItem[] original = [
            new(1, 20f, 0f, 20f, IsometricFootprint.Unit),
            new(2, 5f, 0f, 5f, IsometricFootprint.Unit),
            new(3, 15f, 0f, 15f, IsometricFootprint.Unit),
            new(4, 0f, 0f, 0f, IsometricFootprint.Unit),
        ];

        Span<int> indices = stackalloc int[original.Length];
        IsometricDepthSorter.SortIndices(original, indices);

        Assert.Equal(3, indices[0]); // ID 4 (0,0)
        Assert.Equal(1, indices[1]); // ID 2 (5,5)
        Assert.Equal(2, indices[2]); // ID 3 (15,15)
        Assert.Equal(0, indices[3]); // ID 1 (20,20)
    }
}
