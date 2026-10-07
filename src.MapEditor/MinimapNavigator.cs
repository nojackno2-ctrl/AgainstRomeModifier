using System.Numerics;

namespace AgainstRomeMapEditor;

/// <summary>
/// Age of Empires II 風格小地圖導航之純幾何計算邏輯。
/// 支援 PictureBoxSizeMode.Zoom 縮放長寬比、黑邊（letterboxing）與可見視角/相機位置換算。
/// </summary>
public static class MinimapNavigator
{
    /// <summary>
    /// 計算 PictureBox 在 SizeMode = Zoom 時，影像在控制項客戶區中的實際繪製矩形。
    /// 若控制項或影像尺寸無效，回傳 Rectangle.Empty。
    /// </summary>
    public static Rectangle CalculateZoomedImageBounds(Size clientSize, Size imageSize)
    {
        if (clientSize.Width <= 0 || clientSize.Height <= 0 || imageSize.Width <= 0 || imageSize.Height <= 0)
            return Rectangle.Empty;

        float scale = Math.Min(clientSize.Width / (float)imageSize.Width, clientSize.Height / (float)imageSize.Height);
        int width = Math.Max(1, (int)MathF.Round(imageSize.Width * scale));
        int height = Math.Max(1, (int)MathF.Round(imageSize.Height * scale));
        int x = (clientSize.Width - width) / 2;
        int y = (clientSize.Height - height) / 2;

        return new Rectangle(x, y, width, height);
    }

    /// <summary>
    /// 將小地圖 PictureBox 上的滑鼠點轉換為地圖 tile 座標（浮點數）。
    /// 若點位於影像矩形外部（黑邊區域），則回傳 false。
    /// </summary>
    /// <param name="point">PictureBox 客戶區座標。</param>
    /// <param name="imageBounds">Zoom 模式下的實際影像矩形。</param>
    /// <param name="mapDimension">地圖格數（例如 64）。</param>
    /// <param name="tilePosition">輸出的 tile 座標 (tileX, tileY)。</param>
    /// <returns>若點在影像範圍內回傳 true，否則 false。</returns>
    public static bool TryPointToTile(Point point, Rectangle imageBounds, int mapDimension, out Vector2 tilePosition)
    {
        tilePosition = Vector2.Zero;
        if (mapDimension <= 0 || imageBounds.Width <= 0 || imageBounds.Height <= 0)
            return false;

        if (!imageBounds.Contains(point))
            return false;

        float u = (point.X - imageBounds.X) / (float)imageBounds.Width;
        float v = (point.Y - imageBounds.Y) / (float)imageBounds.Height;

        // 依 minimap.bmp 與 boden.bmp / MapCanvasControl 的座標系：
        // 影像左上角為 (0,0)，X 向右對應 tileX (0 -> mapDimension)，Y 向下對應 tileY/WorldZ (0 -> mapDimension)。
        float tileX = Math.Clamp(u * mapDimension, 0f, (float)mapDimension);
        float tileY = Math.Clamp(v * mapDimension, 0f, (float)mapDimension);

        tilePosition = new Vector2(tileX, tileY);
        return true;
    }

    /// <summary>
    /// 將地圖 tile 座標轉換為小地圖影像矩形內的客戶區點位（PointF）。
    /// </summary>
    public static PointF TileToPoint(Vector2 tilePosition, Rectangle imageBounds, int mapDimension)
    {
        if (mapDimension <= 0 || imageBounds.Width <= 0 || imageBounds.Height <= 0)
            return new PointF(imageBounds.X, imageBounds.Y);

        float u = tilePosition.X / mapDimension;
        float v = tilePosition.Y / mapDimension;

        return new PointF(
            imageBounds.X + u * imageBounds.Width,
            imageBounds.Y + v * imageBounds.Height);
    }

    /// <summary>
    /// 依據 2D 畫布之可見 SceneBounds 與視圖尺寸，計算 2D 畫布在小地圖上的可見區域矩形。
    /// </summary>
    public static RectangleF CalculateCanvasVisibleMinimapRect(
        Rectangle sceneBounds,
        Size canvasClientSize,
        Rectangle minimapImageBounds,
        int mapDimension)
    {
        if (mapDimension <= 0 || sceneBounds.Width <= 0 || sceneBounds.Height <= 0 || minimapImageBounds.Width <= 0 || minimapImageBounds.Height <= 0)
            return RectangleF.Empty;

        // 畫布的可視視窗為 (0, 0, canvasClientSize.Width, canvasClientSize.Height)
        // 算出該視窗相對於 sceneBounds 的交集比例
        float left = Math.Max(0, -sceneBounds.Left) / (float)sceneBounds.Width;
        float top = Math.Max(0, -sceneBounds.Top) / (float)sceneBounds.Height;
        float right = Math.Min(sceneBounds.Width, canvasClientSize.Width - sceneBounds.Left) / (float)sceneBounds.Width;
        float bottom = Math.Min(sceneBounds.Height, canvasClientSize.Height - sceneBounds.Top) / (float)sceneBounds.Height;

        if (right <= left || bottom <= top)
            return RectangleF.Empty;

        float u0 = Math.Clamp(left, 0f, 1f);
        float v0 = Math.Clamp(top, 0f, 1f);
        float u1 = Math.Clamp(right, 0f, 1f);
        float v1 = Math.Clamp(bottom, 0f, 1f);

        return new RectangleF(
            minimapImageBounds.X + u0 * minimapImageBounds.Width,
            minimapImageBounds.Y + v0 * minimapImageBounds.Height,
            Math.Max(2f, (u1 - u0) * minimapImageBounds.Width),
            Math.Max(2f, (v1 - v0) * minimapImageBounds.Height));
    }
}
