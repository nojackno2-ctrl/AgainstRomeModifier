using AgainstRomeMapEditor.Rendering;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class DynamicAtlasPackerTests
{
    [Fact]
    public void MaxRects_packs_heterogeneous_rectangles_with_high_efficiency()
    {
        // 建立 512x512 尺寸圖集，置入不同長寬比的建築與精靈
        var packer = new DynamicAtlasPacker(maxSize: 512, gutter: 1);

        (int W, int H)[] sprites = [
            (64, 64), (128, 64), (32, 96), (200, 150),
            (48, 48), (64, 32), (80, 80), (128, 128),
            (16, 32), (32, 32), (64, 128), (96, 64)
        ];

        int successCount = 0;
        foreach (var (w, h) in sprites)
        {
            if (packer.TryInsert(w, h, out var placedRect, out var uv, MaxRectsHeuristic.BestShortSideFit))
            {
                successCount++;
                Assert.Equal(w, placedRect.Width);
                Assert.Equal(h, placedRect.Height);
                Assert.True(placedRect.X >= 1);
                Assert.True(placedRect.Y >= 1);
                Assert.True(uv.U1 > uv.U0);
                Assert.True(uv.V1 > uv.V0);
            }
        }

        Assert.Equal(sprites.Length, successCount);
        double efficiency = packer.CalculateEfficiency(packer.UsedWidth, packer.UsedHeight);
        Assert.True(efficiency > 60.0, $"Efficiency should exceed 60%, got {efficiency:F2}%");
    }

    [Fact]
    public void Gutter_margins_are_strictly_respected_and_isolated()
    {
        var packer = new DynamicAtlasPacker(maxSize: 64, gutter: 2);

        Assert.True(packer.TryInsert(10, 10, out var rectA, out _));
        Assert.True(packer.TryInsert(10, 10, out var rectB, out _));

        // 檢查兩矩形之間保有至少 4 像素 (2 * gutter) 的安全間隔，杜絕貼圖濾波溢色
        Assert.Equal(2, rectA.X);
        Assert.Equal(2, rectA.Y);
        Assert.True(rectB.X >= rectA.Right + 4 || rectB.Y >= rectA.Bottom + 4);
    }

    [Fact]
    public void Transaction_rollback_reverts_state_completely_on_failure()
    {
        var packer = new DynamicAtlasPacker(maxSize: 64, gutter: 1);

        // 放置一個常駐精靈 (20x20)
        Assert.True(packer.TryInsert(20, 20, out _, out _));
        int initialPlaced = packer.PlacedCount;
        int initialFree = packer.FreeRectanglesCount;
        int initialWidth = packer.UsedWidth;
        int initialHeight = packer.UsedHeight;
        double initialEfficiency = packer.CalculateEfficiency(64, 64);

        // 開始事務：嘗試放置一組過大的動畫
        packer.BeginTransaction();
        Assert.True(packer.TryInsert(20, 20, out _, out _));
        Assert.True(packer.TryInsert(20, 20, out _, out _));
        // 40x40 加上 gutter 恰好能放入剩餘的 42x42；43x43 才確實放不下。
        Assert.False(packer.TryInsert(43, 43, out _, out _));

        // 執行回滾
        packer.RollbackTransaction();

        // 狀態必須 100% 恢復至事務開始前
        Assert.Equal(initialPlaced, packer.PlacedCount);
        Assert.Equal(initialFree, packer.FreeRectanglesCount);
        Assert.Equal(initialWidth, packer.UsedWidth);
        Assert.Equal(initialHeight, packer.UsedHeight);
        Assert.Equal(initialEfficiency, packer.CalculateEfficiency(64, 64));

        // 後續配置也必須與未曾開始事務的 packer 一致。
        var control = new DynamicAtlasPacker(maxSize: 64, gutter: 1);
        Assert.True(control.TryInsert(20, 20, out _, out _));
        foreach (var (width, height) in new[] { (40, 40), (20, 20), (20, 20), (1, 1) })
        {
            Assert.Equal(control.TryInsert(width, height, out var expectedRect, out var expectedUv),
                packer.TryInsert(width, height, out var actualRect, out var actualUv));
            Assert.Equal(expectedRect, actualRect);
            Assert.Equal(expectedUv, actualUv);
        }
    }

    [Fact]
    public void TryInsertBatch_operates_atomically_for_animation_frames()
    {
        var packer = new DynamicAtlasPacker(maxSize: 128, gutter: 1);

        // 8 格合法動畫序列
        var validAnimation = Enumerable.Range(0, 8).Select(_ => (24, 24)).ToArray();
        List<AtlasRect> placedRects = [];
        List<AtlasUv> uvs = [];

        Assert.True(packer.TryInsertBatch(validAnimation, placedRects, uvs));
        Assert.Equal(8, placedRects.Count);
        Assert.Equal(8, uvs.Count);

        // 包含一格過大尺寸的損壞動畫序列
        var badAnimation = new (int, int)[] { (16, 16), (200, 200), (16, 16) };
        Assert.False(packer.TryInsertBatch(badAnimation, placedRects, uvs));
        Assert.Empty(placedRects); // 整套全部取消，不留下任何殘餘碎片
        Assert.Equal(8, packer.PlacedCount); // 依然保持原先 8 格
    }

    [Fact]
    public void NextPowerOfTwo_computes_correct_texture_boundaries()
    {
        Assert.Equal(1, DynamicAtlasPacker.NextPowerOfTwo(0));
        Assert.Equal(1, DynamicAtlasPacker.NextPowerOfTwo(1));
        Assert.Equal(2, DynamicAtlasPacker.NextPowerOfTwo(2));
        Assert.Equal(4, DynamicAtlasPacker.NextPowerOfTwo(3));
        Assert.Equal(256, DynamicAtlasPacker.NextPowerOfTwo(150));
        Assert.Equal(4096, DynamicAtlasPacker.NextPowerOfTwo(3000));
        Assert.Equal(4096, DynamicAtlasPacker.NextPowerOfTwo(4096));
    }
}
