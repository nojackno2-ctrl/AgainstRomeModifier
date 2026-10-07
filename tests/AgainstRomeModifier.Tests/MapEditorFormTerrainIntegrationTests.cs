using System.IO.Compression;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 以真正的 MapEditorForm 走完「載入 → 地形高度／通行區域繪製 → 儲存」，驗證 UI 路徑實際寫出的檔案。
/// 地圖為合成 fixture（不讀取遊戲安裝目錄）；表單不顯示，因此不建立 OpenGL context。
/// </summary>
public sealed class MapEditorFormTerrainIntegrationTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmFormTerrain_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Height_and_passability_tools_save_layers_and_invalidate_caches_through_the_form()
    {
        string map = CreateFixture();
        byte[] originalBoden = File.ReadAllBytes(Path.Combine(map, "boden.bmp"));
        byte[] originalVertex = File.ReadAllBytes(Path.Combine(map, "vertex.bmp"));
        Exception? failure = null;
        bool saved = false;
        var thread = new Thread(() =>
        {
            try
            {
                var info = new GameMapInfo("ENDL_005", map, true, "Height Test", "無盡模式");
                using var form = new MapEditorForm(_root, info);
                _ = form.Handle;
                Invoke(form, "LoadSelectedMap");
                Type modeType = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                Invoke(form, "SetEditMode", Enum.Parse(modeType, "Height"));
                for (int pass = 0; pass < 3; pass++)
                {
                    for (int x = 28; x <= 36; x++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, 40, "", ""));
                    Invoke(form, "CommitStroke");
                }
                Invoke(form, "SetEditMode", Enum.Parse(modeType, "Collision"));
                for (int y = 10; y <= 14; y++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(50, y, "", ""));
                Invoke(form, "CommitStroke");
                saved = (bool)Invoke(form, "SaveMap", false)!;
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromMinutes(2)), "表單整合測試逾時（可能跳出了錯誤對話框）。");
        Assert.Null(failure);
        Assert.True(saved);

        TerrainLayer boden = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"))!;
        TerrainLayer original = DecodeOriginal(originalBoden);
        int center = (40 * 4 + 2) * 257 + 32 * 4 + 2;
        Assert.True(boden.Green[center] > original.Green[center] + 10, $"中心高度應被升高：{original.Green[center]} → {boden.Green[center]}");
        Assert.Equal(original.Green[0], boden.Green[0]); // 遠處未修改
        Assert.True(File.ReadAllBytes(Path.Combine(map, "emboss.bmp")).Length > 54);
        TerrainLayer collision = TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"))!;
        Assert.Equal(255, collision.Green[(12 * 4 + 2) * 256 + 50 * 4 + 2]);
        Assert.Equal(0, collision.Green[(40 * 4) * 256 + 10 * 4]);
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches) Assert.False(File.Exists(Path.Combine(map, cache)), cache + " 應被刪除以讓遊戲重建");
        Assert.True(File.Exists(Path.Combine(map, "DATA", "way.dat")));
        Assert.Equal(originalVertex, File.ReadAllBytes(Path.Combine(map, "vertex.bmp")));
        Assert.NotEqual(originalBoden, File.ReadAllBytes(Path.Combine(map, "boden.bmp")));
    }

    [Fact]
    public void Blank_terrain_flattens_repaints_clears_and_resets_auxiliary_layers_on_save()
    {
        string map = CreateFixture();
        WriteGray(Path.Combine(map, "collision.bmp"), 256, Grid(256, (x, y) => x is >= 40 and < 60 ? 255 : 0));
        WriteGray(Path.Combine(map, "smooth.bmp"), 257, Enumerable.Repeat((byte)3, 257 * 257).ToArray());
        WriteGray(Path.Combine(map, "vertex.bmp"), 257, Enumerable.Repeat((byte)180, 257 * 257).ToArray());
        File.WriteAllText(Path.Combine(map, "boden.txt"), "[Dimension]\r\n64\r\n[Texturen]\r\n" +
            string.Join("\r\n", Enumerable.Range(0, 64 * 64).Select(index => index % 64 < 20 ? "4BC___51" : "4BB___51")) + "\r\n");
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Update))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/4BC___51.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)170, 128 * 128).ToArray()));
        Exception? failure = null;
        bool saved = false, dirtyBeforeSave = false;
        var thread = new Thread(() =>
        {
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Blank", "無盡模式"), startWithBlankTerrain: true);
                _ = form.Handle;
                Invoke(form, "LoadSelectedMap");
                form.ApplyBlankTerrain(confirm: false);
                dirtyBeforeSave = (bool)typeof(MapEditorForm).GetProperty("IsDirty", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
                saved = (bool)Invoke(form, "SaveMap", false)!;
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromMinutes(2)), "表單整合測試逾時（可能跳出了錯誤對話框）。");
        Assert.Null(failure);
        Assert.True(dirtyBeforeSave);
        Assert.True(saved);

        // Waterlevel 120 ÷ Heightmapstep 4 = 30；空白地形高 30 + 20 = 50。
        Assert.All(TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"))!.Green, value => Assert.Equal(50, value));
        Assert.All(TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"))!.Green, value => Assert.Equal(0, value));
        Assert.All(TerrainLayerFiles.Read(Path.Combine(map, "vertex.bmp"))!.Green, value => Assert.Equal(255, value));
        Assert.All(TerrainLayerFiles.Read(Path.Combine(map, "smooth.bmp"))!.Green, value => Assert.Equal(0, value));
        byte[] emboss = TerrainLayerFiles.Read(Path.Combine(map, "emboss.bmp"))!.Green;
        Assert.Single(emboss.Distinct()); // 平地光照一致（擬合截距約 150）
        Assert.InRange(emboss[0], 140, 160);
        string[] textures = File.ReadAllLines(Path.Combine(map, "boden.txt")).SkipWhile(line => line != "[Texturen]").Skip(1).Where(line => line.Length > 0).ToArray();
        Assert.Equal(64 * 64, textures.Length);
        Assert.All(textures, texture => Assert.StartsWith("4BB", texture)); // 最常見的基礎材質（草地 B）鋪滿
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches) Assert.False(File.Exists(Path.Combine(map, cache)));
        Assert.True(File.Exists(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"))); // 聚落保留
    }

    [Fact]
    public void Inspector_tabs_are_not_covered_by_the_toolbars()
    {
        string map = CreateFixture();
        Exception? failure = null;
        int tabsTop = 0, toolbarsBottom = 0;
        var thread = new Thread(() =>
        {
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Layout", "無盡模式")) { Size = new System.Drawing.Size(1440, 900) };
                _ = form.Handle;
                form.PerformLayout();
                var tabs = (System.Windows.Forms.TabControl)typeof(MapEditorForm).GetField("_inspectorTabs", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
                for (System.Windows.Forms.Control? control = tabs; control is not null && control != form; control = control.Parent) tabsTop += control.Top;
                toolbarsBottom = form.Controls.OfType<System.Windows.Forms.ToolStrip>().Where(strip => strip.Dock == System.Windows.Forms.DockStyle.Top).Max(strip => strip.Bottom);
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromMinutes(1)));
        Assert.Null(failure);
        Assert.True(tabsTop >= toolbarsBottom, $"分頁標籤（top={tabsTop}）被工具列（bottom={toolbarsBottom}）遮住");
    }

    [Fact]
    public void Stroke_path_fills_every_tile_between_sparse_mouse_samples()
    {
        Assert.Equal(new[] { (1, 0), (2, 0), (3, 0) }, TerrainStrokePath.Between(0, 0, 3, 0));
        Assert.Equal(new[] { (1, 1), (2, 2) }, TerrainStrokePath.Between(0, 0, 2, 2));
        Assert.Empty(TerrainStrokePath.Between(5, 5, 5, 5));
        var steep = TerrainStrokePath.Between(10, 10, 12, 4).ToArray();
        Assert.Equal((12, 4), steep[^1]);
        Assert.All(steep.Zip(steep.Skip(1)), pair => Assert.True(Math.Abs(pair.First.Item1 - pair.Second.Item1) <= 1 && Math.Abs(pair.First.Item2 - pair.Second.Item2) <= 1));
    }

    [Fact]
    public void Event_only_form_save_preserves_building_slots_and_removal_restores_original_script()
    {
        string map = CreateFixture(); Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        string script = Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile);
        byte[] original = ScenarioEventsTests.Fixture().Serialize(); File.WriteAllBytes(script, original);
        var baseline = new ScenarioDocument { Spawns = [new("HOUSE", 4000, 5000, 0, Prebuilt: true)], DataSlots = [new(42, 123)] };
        using (var rollback = new FileRollbackScope()) { baseline.Save(map, rollback); rollback.Commit(); }
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Events", "無盡模式"));
                _ = form.Handle; Invoke(form, "LoadSelectedMap");
                var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
                events.Add(new ScenarioEvent(new string('T', 100), 2) { Actions = [new(ScenarioActionKind.Message, "Ready")] });
                Invoke(form, "RefreshEventList", 0); Invoke(form, "UpdateEditorState");
                Invoke(form, "DuplicateEvent");
                Assert.Equal(2, events.Count);
                Assert.Equal(100, events[1].Name.Length);
                Assert.Equal(events[0].DelaySeconds, events[1].DelaySeconds);
                Assert.Equal(events[0].Enabled, events[1].Enabled);
                Assert.Equal(events[0].Repeat, events[1].Repeat);
                Assert.NotSame(events[0].Actions, events[1].Actions);
                events[1].Actions.Add(new(ScenarioActionKind.Message, "Copy only"));
                Assert.Single(events[0].Actions);
                Assert.True((bool)typeof(MapEditorForm).GetProperty("IsDirty", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!);
                Assert.True((bool)Invoke(form, "SaveMap", false)!);
                Assert.False((bool)typeof(MapEditorForm).GetProperty("IsDirty", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!);
                ScenarioDocument saved = ScenarioDocument.Load(map);
                Assert.Equal(baseline.Spawns, saved.Spawns); Assert.Equal(baseline.DataSlots, saved.DataSlots); Assert.Equal(2, saved.Events.Count);
                Assert.Single(saved.Events[0].Actions); Assert.Equal(2, saved.Events[1].Actions.Count);
                Assert.NotEqual(original, File.ReadAllBytes(script));
                while (events.Count < 256) events.Add(events[0]);
                Invoke(form, "RefreshEventList", 0);
                foreach (string name in new[] { "_eventAdd", "_eventCopy" })
                    Assert.False(((System.Windows.Forms.Button)typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!).Enabled);
                Assert.True(((System.Windows.Forms.Button)typeof(MapEditorForm).GetField("_eventEdit", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!).Enabled);
                Invoke(form, "DuplicateEvent"); Assert.Equal(256, events.Count);
                events.Clear(); Assert.True((bool)Invoke(form, "SaveMap", false)!);
                Assert.Equal(original, File.ReadAllBytes(script)); Assert.Empty(ScenarioDocument.Load(map).Events);
                Assert.Equal(baseline.DataSlots, ScenarioDocument.Load(map).DataSlots);
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(60))); Assert.Null(failure);
    }

    [Fact]
    public void Original_map_disables_event_commands_and_event_dialogs_construct()
    {
        string map = CreateFixture(); Exception? failure = null;
        var thread = new Thread(() =>
        {
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_000", map, false, "Original", "無盡模式"));
                _ = form.Handle; Invoke(form, "LoadSelectedMap");
                var tabs = (System.Windows.Forms.TabControl)typeof(MapEditorForm).GetField("_inspectorTabs", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
                Assert.Single(tabs.TabPages[4].Controls); Assert.Single(tabs.TabPages[5].Controls);
                foreach (string name in new[] { "_eventAdd", "_eventEdit", "_eventCopy", "_eventDelete" })
                    Assert.False(((System.Windows.Forms.Button)typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!).Enabled);
                using var dialog = new ScenarioEventDialog(new("Event") { Actions = [new(ScenarioActionKind.Message, "Ready")] }, ["GER_INF01"], false);
                using var action = new ScenarioActionDialog(new(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01"), ["GER_INF01"], true);
                _ = dialog.Handle; _ = action.Handle;
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(60))); Assert.Null(failure);
    }

    private static object? Invoke(MapEditorForm form, string name, params object[] args)
    {
        MethodInfo method = typeof(MapEditorForm).GetMethods(BindingFlags.Instance | BindingFlags.NonPublic)
            .Single(item => item.Name == name && item.GetParameters().Length == args.Length);
        try { return method.Invoke(form, args); }
        catch (TargetInvocationException ex) when (ex.InnerException is not null) { throw ex.InnerException; }
    }

    private static TerrainLayer DecodeOriginal(byte[] bytes)
    {
        string path = Path.Combine(Path.GetTempPath(), "ArmBoden_" + Guid.NewGuid().ToString("N") + ".bmp");
        File.WriteAllBytes(path, bytes);
        try { return TerrainLayerFiles.Read(path)!; } finally { File.Delete(path); }
    }

    private string CreateFixture()
    {
        string map = Path.Combine(_root, "MAPS", "ENDL_005");
        Directory.CreateDirectory(Path.Combine(map, "TEXT", "US"));
        Directory.CreateDirectory(Path.Combine(map, "DATA"));
        byte[] heights = Grid(257, (x, y) => 80 + 20 * Math.Sin(x / 20.0) * Math.Cos(y / 25.0));
        byte[] emboss = new byte[heights.Length];
        for (int y = 0; y < 257; y++)
        for (int x = 0; x < 257; x++)
        {
            int gx = heights[y * 257 + Math.Min(256, x + 1)] - heights[y * 257 + Math.Max(0, x - 1)];
            int gy = heights[Math.Min(256, y + 1) * 257 + x] - heights[Math.Max(0, y - 1) * 257 + x];
            emboss[y * 257 + x] = (byte)Math.Clamp(150 + 3 * gx - 2 * gy, 0, 255);
        }
        WriteGray(Path.Combine(map, "boden.bmp"), 257, heights);
        WriteGray(Path.Combine(map, "emboss.bmp"), 257, emboss);
        WriteGray(Path.Combine(map, "smooth.bmp"), 257, new byte[257 * 257]);
        WriteGray(Path.Combine(map, "vertex.bmp"), 257, Enumerable.Repeat((byte)255, 257 * 257).ToArray());
        WriteGray(Path.Combine(map, "collision.bmp"), 256, new byte[256 * 256]);
        WriteGray(Path.Combine(map, "minimap.bmp"), 256, Enumerable.Repeat((byte)90, 256 * 256).ToArray());
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches) File.WriteAllText(Path.Combine(map, cache), "stale");
        File.WriteAllBytes(Path.Combine(map, "DATA", "way.dat"), new byte[] { 1, 2 });
        File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        string[] textures = Enumerable.Range(0, 64 * 64).Select(index => index % 7 == 0 ? "4BB___52" : "4BB___51").ToArray();
        File.WriteAllText(Path.Combine(map, "boden.txt"), "[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", textures) + "\r\n");
        File.WriteAllText(Path.Combine(map, "boden.ini"),
            "[Waterlevel]\r\n120\r\n[Heightmapstep]\r\n4\r\n[WaterColor]\r\n0xffdfbf\r\n[DayStartTime]\r\n6\r\n[DayEndTime]\r\n20\r\n[RainDropsOnWater]\r\n1\r\n" +
            "[WaterWarpShift]\r\n2\r\n[WaterBumpAmplitude]\r\n3\r\n[WaterBumpFrequency]\r\n4\r\n[FlashPropability]\r\n0\r\n");
        File.WriteAllText(Path.Combine(map, "TEXT", "US", "briefing.put"),
            "var:briefing_titel_1 =\"Height Test\";\r\nvar:briefing_titel_2 =\"\";\r\nvar:briefing_text =\"\";\r\n" +
            string.Concat(Enumerable.Range(0, 8).Select(index => $"var:briefing_text_teamname{index} =\"T{index}\";\r\n")));
        File.WriteAllText(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"),
            "[settlement]\r\nrefpos=10000,320,6000\r\n[object0000]\r\nnamedef=BauRomHau00_Haupthaus\r\ndef=1676\r\npos=0.00,0.00,0.00\r\nteam=3\r\nangle=0.00\r\n");
        using (ZipArchive zip = ZipFile.Open(Path.Combine(_root, "floortex.dat"), ZipArchiveMode.Create))
            foreach ((string name, byte shade) in new[] { ("4BB___51", (byte)110), ("4BB___52", (byte)120) })
                using (Stream stream = zip.CreateEntry($"SYSTEM/DATA/FLOORTEXTURE/{name}.bmp").Open())
                    stream.Write(Encode(128, Enumerable.Repeat(shade, 128 * 128).ToArray()));
        return map;
    }

    private static byte[] Grid(int size, Func<int, int, double> value)
    {
        var result = new byte[size * size];
        for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) result[y * size + x] = (byte)Math.Clamp((int)Math.Round(value(x, y)), 0, 255);
        return result;
    }

    // 以「全部像素都改變」的方式呼叫 EncodeWithGreen，產生 R=G=B 的 24-bit 灰階 BMP。
    private static byte[] Encode(int size, byte[] values)
    {
        var blank = new TerrainLayer(size, size, new int[size * size], values.Select(value => (byte)(value ^ 1)).ToArray());
        return TerrainLayerFiles.EncodeWithGreen(blank, values);
    }

    private static void WriteGray(string path, int size, byte[] values) => File.WriteAllBytes(path, Encode(size, values));

    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
