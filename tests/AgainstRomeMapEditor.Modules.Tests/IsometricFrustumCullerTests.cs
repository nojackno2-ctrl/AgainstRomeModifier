using System.Numerics;
using AgainstRomeMapEditor.Rendering;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class IsometricFrustumCullerTests
{
    [Fact]
    public void Tall_tower_with_ground_anchor_offscreen_remains_visible_due_to_height()
    {
        // 模擬正交等角投影視角：相機俯視看著 (10, 0, 10)，視窗 800x600
        Matrix4x4 view = Matrix4x4.CreateLookAt(new Vector3(10f, 20f, 30f), new Vector3(10f, 0f, 10f), Vector3.UnitY);
        Matrix4x4 proj = Matrix4x4.CreateOrthographic(40f, 30f, 0.1f, 100f);
        Matrix4x4 viewProj = view * proj;
        Vector2 viewport = new(800f, 600f);

        // 羅馬高塔：其地面錨點在下方邊緣外 (WorldZ=26)，但高度有 14 單位
        var tallTower = new IsometricSortItem(1, 10f, 0f, 26f, IsometricFootprint.RomanTower);

        // 扁平單位：與高塔在同一地面位置，但高度只有 0
        var flatUnit = new IsometricSortItem(2, 10f, 0f, 26f, IsometricFootprint.Point);

        // 檢驗三維包圍盒投影：高塔因為塔身向上延伸，必須判定為可見 (避免 Pop-in)！
        bool towerVisible = IsometricFrustumCuller.IsVisible(tallTower, viewProj, viewport, screenPadding: 0f);
        bool unitVisible = IsometricFrustumCuller.IsVisible(flatUnit, viewProj, viewport, screenPadding: 0f);

        // 高塔因包含頂點高度而進入螢幕視野
        Assert.True(towerVisible);
        // 若單位無高度且在視野下方外側，則應被剔除或至少高塔比扁平單位包圍盒更大
        var towerBox = IsometricFrustumCuller.ProjectAabbToScreen(WorldAabb.FromSortItem(tallTower), viewProj, viewport);
        var unitBox = IsometricFrustumCuller.ProjectAabbToScreen(WorldAabb.FromSortItem(flatUnit), viewProj, viewport);

        Assert.True(towerBox.MinY < unitBox.MinY, "Tall tower top should reach higher up in screen coordinates");
    }

    [Fact]
    public void Completely_distant_objects_are_properly_culled()
    {
        Matrix4x4 view = Matrix4x4.CreateLookAt(new Vector3(10f, 20f, 30f), new Vector3(10f, 0f, 10f), Vector3.UnitY);
        Matrix4x4 proj = Matrix4x4.CreateOrthographic(40f, 30f, 0.1f, 100f);
        Matrix4x4 viewProj = view * proj;
        Vector2 viewport = new(800f, 600f);

        var distantObject = new IsometricSortItem(999, 500f, 0f, 500f, IsometricFootprint.Unit);
        Assert.False(IsometricFrustumCuller.IsVisible(distantObject, viewProj, viewport, screenPadding: 32f));
    }

    [Fact]
    public void CullItems_zero_allocation_writes_exact_visible_indices()
    {
        Matrix4x4 view = Matrix4x4.CreateLookAt(new Vector3(10f, 20f, 30f), new Vector3(10f, 0f, 10f), Vector3.UnitY);
        Matrix4x4 proj = Matrix4x4.CreateOrthographic(40f, 30f, 0.1f, 100f);
        Matrix4x4 viewProj = view * proj;
        Vector2 viewport = new(800f, 600f);

        IsometricSortItem[] items = [
            new(1, 10f, 0f, 10f, IsometricFootprint.Unit),   // 中心可見
            new(2, 500f, 0f, 500f, IsometricFootprint.Unit), // 遠方不可見
            new(3, 11f, 0f, 11f, IsometricFootprint.Unit),   // 中心可見
            new(4, -500f, 0f, -500f, IsometricFootprint.Unit) // 遠方不可見
        ];

        Span<int> visibleIndices = stackalloc int[items.Length];
        int count = IsometricFrustumCuller.CullItems(items, viewProj, viewport, visibleIndices);

        Assert.Equal(2, count);
        Assert.Equal(0, visibleIndices[0]);
        Assert.Equal(2, visibleIndices[1]);
    }

    [Fact]
    public void Chunk_grid_hierarchical_culling_matches_direct_cull()
    {
        Matrix4x4 view = Matrix4x4.CreateLookAt(new Vector3(10f, 20f, 30f), new Vector3(10f, 0f, 10f), Vector3.UnitY);
        Matrix4x4 proj = Matrix4x4.CreateOrthographic(40f, 30f, 0.1f, 100f);
        Matrix4x4 viewProj = view * proj;
        Vector2 viewport = new(800f, 600f);

        IsometricSortItem[] items = [
            new(1, 10f, 0f, 10f, IsometricFootprint.Unit),
            new(2, 12f, 0f, 12f, IsometricFootprint.Unit),
            new(3, 200f, 0f, 200f, IsometricFootprint.Unit),
            new(4, 250f, 0f, 250f, IsometricFootprint.Unit),
        ];

        var chunks = IsometricFrustumCuller.BuildChunkGrid(items);
        Span<int> chunkResults = stackalloc int[items.Length];
        int count = IsometricFrustumCuller.CullWithChunks(items, chunks.Values, viewProj, viewport, chunkResults);

        Assert.Equal(2, count);
        Assert.Contains(0, chunkResults[..count].ToArray());
        Assert.Contains(1, chunkResults[..count].ToArray());
    }
}
