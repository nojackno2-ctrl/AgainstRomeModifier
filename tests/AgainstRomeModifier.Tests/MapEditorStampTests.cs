using System.IO.Compression;
using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Stamp_view_reports_backtracking_tiles_and_resets_on_mouse_release(bool use3D)
    {
        string map = CreateFixture();
        AddStampTextures("weg1");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            Invoke(form, "SelectSampledTexture", "weg1");
            Control view = use3D ? GetField<Map3DViewControl>(form, "_view3d") : GetField<MapCanvasControl>(form, "_canvas");
            Assert.NotNull(view);
            view.Size = new Size(640, 480);
            var reported = new List<(int X, int Y)>();
            if (view is Map3DViewControl gl) { Assert.True(gl.ContinuousPaint); gl.TexturePainted += (_, e) => reported.Add((e.X, e.Y)); }
            else { var canvas = (MapCanvasControl)view; Assert.True(canvas.ContinuousPaint); canvas.TexturePainted += (_, e) => reported.Add((e.X, e.Y)); }
            object? Call(string method, params object[] args) => view.GetType().GetMethod(method, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(view, args);
            var points = new List<(Point Pixel, int X, int Y)>();
            for (int y = 40; y < 440 && points.Count < 2; y += 40)
            for (int x = 40; x < 600 && points.Count < 2; x += 40)
            {
                object[] args = [new Point(x, y), 0, 0];
                if (!(bool)Call("TryGetTile", args)!) continue;
                int tileX = (int)args[1], tileY = (int)args[2];
                if (points.Any(p => p.X == tileX && p.Y == tileY)) continue;
                points.Add((new Point(x, y), tileX, tileY));
            }
            Assert.Equal(2, points.Count);
            Point a = points[0].Pixel, b = points[1].Pixel;
            Call("OnMouseDown", new MouseEventArgs(MouseButtons.Left, 1, a.X, a.Y, 0));
            Call("OnMouseMove", new MouseEventArgs(MouseButtons.Left, 0, b.X, b.Y, 0));
            Call("OnMouseMove", new MouseEventArgs(MouseButtons.Left, 0, a.X, a.Y, 0));
            Call("OnMouseMove", new MouseEventArgs(MouseButtons.Left, 0, a.X, a.Y, 0));
            Assert.Equal(new[] { (points[0].X, points[0].Y), (points[1].X, points[1].Y), (points[0].X, points[0].Y) }, reported);
            Call("OnMouseUp", new MouseEventArgs(MouseButtons.Left, 1, a.X, a.Y, 0));
            Call("OnMouseDown", new MouseEventArgs(MouseButtons.Left, 1, a.X, a.Y, 0));
            Assert.Equal(4, reported.Count);
            Call("OnMouseUp", new MouseEventArgs(MouseButtons.Left, 1, a.X, a.Y, 0));
            GetField<CheckBox>(form, "_stampMode").Checked = false;
            if (view is Map3DViewControl glAfter) Assert.False(glAfter.ContinuousPaint);
            else Assert.False(((MapCanvasControl)view).ContinuousPaint);
        });
    }

    [Fact]
    public void Stamp_drag_fills_skipped_tiles_and_separate_strokes_undo_independently()
    {
        string map = CreateFixture();
        AddStampTextures("weg1");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            Invoke(form, "SelectSampledTexture", "weg1");
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string[] baseline = document.Textures.ToArray();
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(11, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(11, 10, "", ""));
            Invoke(form, "CommitStroke");
            for (int x = 5; x <= 11; x++) Assert.Equal("weg1", document.GetTexture(x, 6));
            for (int y = 6; y <= 10; y++) Assert.Equal("weg1", document.GetTexture(11, y));
            string[] firstStroke = document.Textures.ToArray();
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(30, 30, "", ""));
            Invoke(form, "CommitStroke");
            Assert.Equal(firstStroke[20 * 64 + 20], document.GetTexture(20, 20));
            Invoke(form, "Undo");
            Assert.Equal(firstStroke, document.Textures);
            Invoke(form, "Undo");
            Assert.Equal(baseline, document.Textures);
            Invoke(form, "Redo");
            Assert.Equal(firstStroke, document.Textures);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(firstStroke, GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures);
        });
    }

    [Fact]
    public void Stamp_palette_filters_regional_sets_but_search_and_override_show_all()
    {
        string map = CreateFixture();
        AddStampTextures("weg1", "L2B02T5A", "L3B02T5A", "L12B02T5A");
        var document = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt"));
        document.SetTexture(0, 0, "L2B02T5A");
        document.Save();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            var palette = GetField<ListBox>(form, "_palette");
            string[] Keys() => palette.Items.Cast<object>().Select(item => (string)item.GetType().GetProperty("Key")!.GetValue(item)!).ToArray();
            Assert.Contains("tile:weg1", Keys());
            Assert.Contains("tile:L2B02T5A", Keys());
            Assert.DoesNotContain("tile:L3B02T5A", Keys());
            Assert.DoesNotContain("tile:L12B02T5A", Keys());
            GetField<TextBox>(form, "_paletteSearch").Text = "L3";
            Assert.Contains("tile:L3B02T5A", Keys());
            GetField<TextBox>(form, "_paletteSearch").Text = "";
            GetField<CheckBox>(form, "_stampOtherRegions").Checked = true;
            Assert.Contains("tile:L3B02T5A", Keys());
            Assert.Contains("tile:L12B02T5A", Keys());
        });
    }

    private void AddStampTextures(params string[] names)
    {
        using ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update);
        foreach (string name in names)
        {
            using Stream stream = zip.CreateEntry($"SYSTEM/DATA/FLOORTEXTURE/{name}.bmp").Open();
            stream.Write(Encode(128, Enumerable.Repeat((byte)60, 128 * 128).ToArray()));
        }
    }

    [Fact]
    public void Stamp_brush_change_and_outside_map_reset_drag_origin()
    {
        string map = CreateFixture();
        AddStampTextures("weg1", "weg2");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            Invoke(form, "SelectSampledTexture", "weg1");
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string between = document.GetTexture(10, 10);
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 5, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(8, 8, "", ""));
            for (int i = 5; i <= 8; i++) Assert.Equal("weg1", document.GetTexture(i, i));
            Invoke(form, "SelectSampledTexture", "weg2");
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(15, 15, "", ""));
            Assert.Equal(between, document.GetTexture(10, 10));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(-1, 15, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(25, 15, "", ""));
            Assert.NotEqual("weg2", document.GetTexture(20, 15));
            Invoke(form, "CommitStroke");
            Invoke(form, "Undo");
            Assert.Equal("weg1", document.GetTexture(5, 5));
            Assert.NotEqual("weg2", document.GetTexture(15, 15));
            Invoke(form, "Undo");
            Assert.NotEqual("weg1", document.GetTexture(5, 5));
        });
    }

    [Fact]
    public void Stamp_palette_without_regional_evidence_keeps_all_sets_and_mixed_map_keeps_both()
    {
        string map = CreateFixture();
        AddStampTextures("L2B02T5A", "L3B02T5A", "L12B02T5A");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            var palette = GetField<ListBox>(form, "_palette");
            string[] Keys() => palette.Items.Cast<object>().Select(item => (string)item.GetType().GetProperty("Key")!.GetValue(item)!).ToArray();
            Assert.Contains("tile:L2B02T5A", Keys());
            Assert.Contains("tile:L3B02T5A", Keys());
            Assert.Contains("tile:L12B02T5A", Keys());
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            document.SetTexture(0, 0, "L2B02T5A");
            document.SetTexture(1, 0, "L12B02T5A");
            Invoke(form, "LoadPalette", "");
            Assert.Contains("tile:L2B02T5A", Keys());
            Assert.Contains("tile:L12B02T5A", Keys());
            Assert.DoesNotContain("tile:L3B02T5A", Keys());
        });
    }

    [Fact]
    public void Tile_stamp_places_an_original_road_tile_that_saves_reloads_and_samples()
    {
        string map = CreateFixture();
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/weg1.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)60, 128 * 128).ToArray()));
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Stamp", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(0, MapEditorForm.StampCategory("weg1"));
            Assert.Equal(1, MapEditorForm.StampCategory("FLUSS3"));
            Assert.Equal(5, MapEditorForm.StampCategory("L2B02T5A"));
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            var palette = GetField<ListBox>(form, "_palette");
            int road = palette.Items.Cast<object>().ToList().FindIndex(item => item.ToString()!.Contains("weg1") || item.GetType().GetProperty("Key")!.GetValue(item)!.Equals("tile:weg1"));
            Assert.True(road >= 0);
            Assert.Equal(0, road); // 道路類排在最前
            palette.SelectedIndex = road;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "CommitStroke");
            Assert.Equal("weg1", GetField<BodenTexturesDocument>(form, "_texturesDocument").GetTexture(5, 6));
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());

            palette.SelectedIndex = palette.Items.Count - 1;
            Invoke(form, "SelectSampledTexture", "weg1");
            Assert.Equal("weg1", GetField<string>(form, "_stampTexture"));
        });
        RunInSta(() =>
        {
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reopen", "Test"));
            _ = reopened.Handle;
            Invoke(reopened, "LoadSelectedMap");
            Assert.Equal("weg1", GetField<BodenTexturesDocument>(reopened, "_texturesDocument").GetTexture(5, 6));
        });
    }

    [Fact]
    public void Auto_road_stroke_plans_straights_and_turns_in_views_and_undoes_as_one_step()
    {
        string map = CreateFixture();
        AddStampTextures("H_WEG1", "V_WEG1", "weg1");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "AutoRoad", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            GetField<CheckBox>(form, "_autoRoad").Checked = true;
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string[] baseline = document.Textures.ToArray();

            // 繪製 L 型路徑：(5, 6) -> (6, 6) -> (7, 6) [轉角] -> (7, 7) -> (7, 8)
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(6, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(7, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(7, 7, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(7, 8, "", ""));

            Invoke(form, "CommitStroke");

            // 驗證材質自動挑選：東西向 H_WEG1、轉角 weg1、南北向 V_WEG1
            Assert.Equal("H_WEG1", document.GetTexture(5, 6));
            Assert.Equal("H_WEG1", document.GetTexture(6, 6));
            Assert.Equal("weg1", document.GetTexture(7, 6));
            Assert.Equal("V_WEG1", document.GetTexture(7, 7));
            Assert.Equal("V_WEG1", document.GetTexture(7, 8));

            string[] roadState = document.Textures.ToArray();

            // 單步 Undo 還原
            Invoke(form, "Undo");
            Assert.Equal(baseline, document.Textures);

            // Redo 恢復
            Invoke(form, "Redo");
            Assert.Equal(roadState, document.Textures);

            // 儲存與重載驗證
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(roadState, GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures);
        });
    }

    [Fact]
    public void Auto_road_escape_cancels_pending_stroke()
    {
        string map = CreateFixture();
        AddStampTextures("H_WEG1", "V_WEG1", "weg1");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "AutoRoadEscape", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            GetField<CheckBox>(form, "_autoRoad").Checked = true;
            var document = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            string[] baseline = document.Textures.ToArray();

            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(6, 6, "", ""));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(7, 6, "", ""));

            // 按下 Escape 鍵取消未提交的筆畫
            object[] args = [new Message(), Keys.Escape];
            bool handled = (bool)typeof(MapEditorForm)
                .GetMethod("ProcessCmdKey", BindingFlags.Instance | BindingFlags.NonPublic)!
                .Invoke(form, args)!;

            Assert.True(handled);
            Assert.Equal(baseline, document.Textures);

            // 放開滑鼠觸發 CommitStroke，不應殘留任何道路
            Invoke(form, "CommitStroke");
            Assert.Equal(baseline, document.Textures);
        });
    }

    [Fact]
    public void Stamp_palette_filters_by_nature_region_when_map_has_no_L_tiles()
    {
        string map = CreateFixture();
        AddStampTextures("weg1", "L2B02T5A", "L3B02T5A", "L4B02T5A");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "StampNatureRegion", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // 模擬地圖上有日耳曼地景物件（LanGerNad18 -> "Ger" -> 關聯 L2 系列）
            const float pos = 100f;
            SetField(form, "_levelObjects", new LevelWorldObject[]
            {
                new(0, 101, 8, 1, pos, 0, pos, 0, false)
            });
            SetField(form, "_objdefNames", new Dictionary<int, string>
            {
                [101] = "LanGerNad18"
            });

            GetField<CheckBox>(form, "_stampMode").Checked = true;
            Invoke(form, "LoadPalette", "");

            var palette = GetField<ListBox>(form, "_palette");
            string[] Keys() => palette.Items.Cast<object>().Select(item => (string)item.GetType().GetProperty("Key")!.GetValue(item)!).ToArray();

            // 非 L 系列（道路）依然保留
            Assert.Contains("tile:weg1", Keys());
            // 日耳曼 L2 系列被自動包含
            Assert.Contains("tile:L2B02T5A", Keys());
            // 匈人 L3 與不列顛 L4 系列被過濾
            Assert.DoesNotContain("tile:L3B02T5A", Keys());
            Assert.DoesNotContain("tile:L4B02T5A", Keys());

            // 勾選顯示其他地區圖塊後，全部顯示
            GetField<CheckBox>(form, "_stampOtherRegions").Checked = true;
            Assert.Contains("tile:L3B02T5A", Keys());
            Assert.Contains("tile:L4B02T5A", Keys());
        });
    }

    private static void SetField(MapEditorForm form, string name, object? value)
        => typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, value);
}
