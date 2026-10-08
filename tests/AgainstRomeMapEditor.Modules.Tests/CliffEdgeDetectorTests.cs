using AgainstRomeMapEditor;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class CliffEdgeDetectorTests
{
    [Fact]
    public void DetectFromTileHeights_identifies_north_facing_cliff_line()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];

        // y = 0..3: 高度 100（高台）
        // y = 4..7: 高度 20（低谷，高度差 80）
        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                heights[y * dimension + x] = y <= 3 ? 100f : 20f;
            }
        }

        CliffDetectionResult result = CliffEdgeDetector.DetectFromTileHeights(dimension, heights, minSlopeDegrees: 45f, minElevationDelta: 15f);

        // y = 3 的圖塊向南（y = 4）大幅下落，其下坡方向為 South
        // y = 4 的圖塊向北（y = 3）大幅上升，其壁面朝南（Facing South）
        Assert.NotEmpty(result.CliffCells);
        var ridge = result.CliffCells.Where(c => c.Y == 3).ToArray();
        Assert.Equal(dimension, ridge.Length);
        Assert.All(ridge, cell => Assert.Equal(CliffFacing.South, cell.Facing));
    }

    [Fact]
    public void DetectFromTileHeights_identifies_outer_convex_corner()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];

        // 建立西南高台：x <= 3 && y >= 4 高度 100，其餘高度 20
        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                heights[y * dimension + x] = (x <= 3 && y >= 4) ? 100f : 20f;
            }
        }

        CliffDetectionResult result = CliffEdgeDetector.DetectFromTileHeights(dimension, heights, minSlopeDegrees: 45f, minElevationDelta: 15f);

        // (3, 4) 是高台之東北頂角，向北（y=3）下落且向東（x=4）下落，應判定為東北外凸角
        CliffCell? corner = result.GetCell(3, 4);
        Assert.NotNull(corner);
        Assert.Equal(CliffFacing.NorthEastOuter, corner.Facing);
    }

    [Fact]
    public void DetectFromTileHeights_identifies_inner_concave_corner()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];

        // 建立峽谷凹槽：全域高度 100，只有西南 (x <= 3 && y >= 4) 為凹槽低地 20
        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                heights[y * dimension + x] = (x <= 3 && y >= 4) ? 20f : 100f;
            }
        }

        CliffDetectionResult result = CliffEdgeDetector.DetectFromTileHeights(dimension, heights, minSlopeDegrees: 45f, minElevationDelta: 15f);

        // 低谷角落 (3, 4) 的北面（y=3）為高壁，東面（x=4）為高壁，應判定為東北內凹角
        CliffCell? corner = result.GetCell(3, 4);
        Assert.NotNull(corner);
        Assert.Equal(CliffFacing.NorthEastInner, corner.Facing);
    }

    [Fact]
    public void Flat_terrain_detects_no_cliffs()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];
        Array.Fill(heights, 50f);

        CliffDetectionResult result = CliffEdgeDetector.DetectFromTileHeights(dimension, heights);
        Assert.Empty(result.CliffCells);
        Assert.Empty(result.Contours);
    }

    [Fact]
    public void DetectFromVertexHeights_detects_intra_tile_steep_slope()
    {
        // 頂點尺寸 9 (2x2 tiles, step = 4)
        const int vertexSize = 9;
        const int tileDim = 2;
        var vertexHeights = new byte[vertexSize * vertexSize];

        // 讓 (0, 0) 圖塊內部頂點產生劇烈垂直高低差（0 -> 40，delta = 40 > 15）
        for (int vy = 0; vy <= 4; vy++)
        {
            for (int vx = 0; vx <= 4; vx++)
            {
                vertexHeights[vy * vertexSize + vx] = (byte)(vx * 10); // 0, 10, 20, 30, 40
            }
        }

        CliffDetectionResult result = CliffEdgeDetector.DetectFromVertexHeights(
            vertexSize,
            vertexHeights,
            tileDimension: tileDim,
            heightmapStep: 4,
            minSlopeDegrees: 45f,
            minElevationDelta: 15f);

        Assert.True(result.IsCliff(0, 0), "圖塊內部垂直陡坡應被正確偵測。");
    }

    [Fact]
    public void Chained_contours_form_coherent_paths()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];

        // 建立單一階梯 (x=4 處落差)
        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                heights[y * dimension + x] = x < 4 ? 80f : 20f;
            }
        }

        CliffDetectionResult result = CliffEdgeDetector.DetectFromTileHeights(dimension, heights);
        Assert.NotEmpty(result.Contours);

        // 應包含串接起來的連續邊界路徑
        CliffContourPath contour = result.Contours[0];
        Assert.True(contour.Cells.Count >= dimension);
    }
}
