using System.Drawing;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Minimap_zoom_bounds_letterbox_horizontal_and_vertical()
    {
        // 寬長比小於影像（水平裁切／上下黑邊）：例如 client 是 200x400，image 是 100x100
        Size clientTall = new(200, 400);
        Size imageSquare = new(100, 100);
        Rectangle boundsTall = MinimapNavigator.CalculateZoomedImageBounds(clientTall, imageSquare);
        Assert.Equal(new Rectangle(0, 100, 200, 200), boundsTall);

        // 寬長比大於影像（左右黑邊）：例如 client 是 400x200，image 是 100x100
        Size clientWide = new(400, 200);
        Rectangle boundsWide = MinimapNavigator.CalculateZoomedImageBounds(clientWide, imageSquare);
        Assert.Equal(new Rectangle(100, 0, 200, 200), boundsWide);

        // 非 1:1 的影像長寬比：例如 client 是 300x300，image 是 200x100（2:1）
        Size imageRect = new(200, 100);
        Rectangle boundsRect = MinimapNavigator.CalculateZoomedImageBounds(new Size(300, 300), imageRect);
        Assert.Equal(new Rectangle(0, 75, 300, 150), boundsRect);

        // 零尺寸與無效尺寸防護
        Assert.Equal(Rectangle.Empty, MinimapNavigator.CalculateZoomedImageBounds(Size.Empty, imageSquare));
        Assert.Equal(Rectangle.Empty, MinimapNavigator.CalculateZoomedImageBounds(clientTall, Size.Empty));
    }

    [Fact]
    public void Minimap_point_to_tile_outside_clicks_are_ignored()
    {
        Rectangle imageBounds = new(50, 50, 200, 200);
        const int dimension = 64;

        // 影像左方黑邊
        Assert.False(MinimapNavigator.TryPointToTile(new Point(20, 100), imageBounds, dimension, out _));
        // 影像上方黑邊
        Assert.False(MinimapNavigator.TryPointToTile(new Point(100, 10), imageBounds, dimension, out _));
        // 影像右方黑邊
        Assert.False(MinimapNavigator.TryPointToTile(new Point(260, 100), imageBounds, dimension, out _));
        // 影像下方黑邊
        Assert.False(MinimapNavigator.TryPointToTile(new Point(100, 260), imageBounds, dimension, out _));
        // 負座標
        Assert.False(MinimapNavigator.TryPointToTile(new Point(-5, -5), imageBounds, dimension, out _));
    }

    [Fact]
    public void Minimap_coordinates_round_trip_accurately()
    {
        Rectangle imageBounds = new(20, 30, 200, 200);
        const int dimension = 64;

        // 測試左上角、中心、右下角等多個 tile 座標
        Vector2[] testTiles =
        [
            new Vector2(0f, 0f),
            new Vector2(32f, 32f),
            new Vector2(16.5f, 48.25f),
            new Vector2(64f, 64f)
        ];

        foreach (Vector2 tile in testTiles)
        {
            PointF pt = MinimapNavigator.TileToPoint(tile, imageBounds, dimension);
            // 只要不是剛好在邊界外（例如 64,64 位於 imageBounds.Right, imageBounds.Bottom，Contains 是左閉右開）
            if (tile.X < dimension && tile.Y < dimension)
            {
                Point intPt = new((int)MathF.Round(pt.X), (int)MathF.Round(pt.Y));
                bool success = MinimapNavigator.TryPointToTile(intPt, imageBounds, dimension, out Vector2 backTile);
                Assert.True(success);
                Assert.Equal(tile.X, backTile.X, 0.5f);
                Assert.Equal(tile.Y, backTile.Y, 0.5f);
            }
        }
    }

    [Fact]
    public void Minimap_canvas_visible_rect_computation_handles_zoom_and_pan()
    {
        Rectangle minimapImage = new(0, 0, 200, 200);
        const int dimension = 64;
        Size canvasClient = new(400, 400);

        // 場景置中且無縮放（sceneBounds 正好等於 canvasClient）
        Rectangle sceneCentered = new(0, 0, 400, 400);
        RectangleF visibleAll = MinimapNavigator.CalculateCanvasVisibleMinimapRect(sceneCentered, canvasClient, minimapImage, dimension);
        Assert.Equal(0f, visibleAll.X, 0.01f);
        Assert.Equal(0f, visibleAll.Y, 0.01f);
        Assert.Equal(200f, visibleAll.Width, 0.01f);
        Assert.Equal(200f, visibleAll.Height, 0.01f);

        // 放大 2 倍且向右下方平移（可視區域只包含場景左上角）
        Rectangle sceneZoomed = new(0, 0, 800, 800);
        RectangleF visibleTopLeft = MinimapNavigator.CalculateCanvasVisibleMinimapRect(sceneZoomed, canvasClient, minimapImage, dimension);
        Assert.Equal(0f, visibleTopLeft.X, 0.01f);
        Assert.Equal(0f, visibleTopLeft.Y, 0.01f);
        Assert.Equal(100f, visibleTopLeft.Width, 0.01f);
        Assert.Equal(100f, visibleTopLeft.Height, 0.01f);
    }

    [Fact]
    public void Minimap_click_moves_3d_camera_target_in_sta()
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "MinimapNav", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual;
            form.Location = new Point(-30000, -30000);
            form.Show();
            Application.DoEvents();

            Invoke(form, "SetActiveView", true);
            Application.DoEvents();

            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady)
            {
                if (Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED") == "1")
                {
                    Assert.Fail($"ARM_OPENGL_REQUIRED=1 但 3D 視圖初始化失敗: {view.LastFailureReason}");
                }
                return;
            }

            var overview = GetField<PictureBox>(form, "_overview");
            Assert.NotNull(overview.Image);

            // 確保 overview 控制項有尺寸
            overview.Size = new Size(180, 180);
            Application.DoEvents();

            Rectangle imageBounds = MinimapNavigator.CalculateZoomedImageBounds(overview.ClientSize, overview.Image.Size);
            Assert.True(imageBounds.Width > 0 && imageBounds.Height > 0);

            // 點擊影像中約 (u=0.75, v=0.25) 的位置，對應 64x64 地圖的 tile (48, 16)
            int clickX = imageBounds.X + (int)(imageBounds.Width * 0.75f);
            int clickY = imageBounds.Y + (int)(imageBounds.Height * 0.25f);
            var mouseArgs = new MouseEventArgs(MouseButtons.Left, 1, clickX, clickY, 0);

            // 觸發 overview 的導航方法
            Invoke(form, "NavigateMinimap", mouseArgs);
            Application.DoEvents();

            Vector3 target = view.CameraTarget;
            Assert.Equal(48f, target.X, 1.5f);
            Assert.Equal(16f, target.Z, 1.5f);
        });
    }
}
