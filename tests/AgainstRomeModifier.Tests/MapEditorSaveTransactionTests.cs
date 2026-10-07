using System.IO.Compression;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 驗證 MapEditorForm.TrySaveMap 的不可分割性（Atomicity）與回滾保證：
/// 1. 前置驗證（ScenarioSavePreflight）拒絕無效事件目標時，在發生任何檔案寫入前即中止，磁碟完全不變且保留髒狀態與編輯階段。
/// 2. 地形、屬性與場景寫入後若遭遇缺失或損壞的 BCI，交易將所有修改、新增檔案與刪除快取完整回滾，
///    並在表單記憶體中保留編輯過的地形與事件，修正 BCI 後重試儲存可順利成功。
/// </summary>
public sealed partial class MapEditorSaveTransactionTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmSaveTx_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Invalid_event_target_rejection_before_any_file_writes_preserves_map_bytes_and_dirty_session()
    {
        string map = CreateFixture("ENDL_005");
        var initialFiles = SnapshotDirectory(map);

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Preflight", "無盡模式"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });

            var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            Guid nonExistentTarget = Guid.NewGuid();
            var invalidEvent = new ScenarioEvent("InvalidTargetEvent", 5)
            {
                Actions = [new(ScenarioActionKind.Message, "Targeted")],
                Conditions = [new(ScenarioConditionKind.ObjectExists, nonExistentTarget)]
            };
            events.Add(invalidEvent);
            Invoke(form, "RefreshEventList", 0);
            Invoke(form, "UpdateEditorState");

            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True((bool)Invoke(form, "EventsDirty")!);

            bool saved = form.TrySaveMap(showSuccess: false, out Exception? error);
            Assert.False(saved);
            Assert.NotNull(error);
            Assert.IsType<InvalidDataException>(error);
            Assert.Contains("目標物件已刪除", error.Message);

            // 表單記憶體狀態未被破壞或清除，保留髒狀態與新增的事件
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True((bool)Invoke(form, "EventsDirty")!);
            Assert.Single(events);
            Assert.Equal("InvalidTargetEvent", events[0].Name);
        });

        // 磁碟未發生任何寫入：所有既有檔案 byte 逐位完全一致，且未產生新檔案
        var currentFiles = SnapshotDirectory(map);
        Assert.Equal(initialFiles.Keys.OrderBy(k => k), currentFiles.Keys.OrderBy(k => k));
        foreach (var (path, bytes) in initialFiles)
        {
            Assert.Equal(bytes, currentFiles[path]);
        }
        Assert.False(File.Exists(Path.Combine(map, ScenarioDocument.FileName)));
    }

    [Fact]
    public void Missing_bci_failure_after_writes_rolls_back_all_files_and_preserves_memory_for_retry()
    {
        string map = CreateFixture("ENDL_006");
        Assert.False(File.Exists(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile)));

        byte[] originalBoden = File.ReadAllBytes(Path.Combine(map, "boden.bmp"));
        byte[] originalEmboss = File.ReadAllBytes(Path.Combine(map, "emboss.bmp"));
        byte[] originalBriefing = File.ReadAllBytes(Path.Combine(map, "TEXT", "US", "briefing.put"));
        byte[] originalBodenIni = File.ReadAllBytes(Path.Combine(map, "boden.ini"));
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
        {
            Assert.True(File.Exists(Path.Combine(map, cache)));
        }

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_006", map, true, "MissingBci", "無盡模式"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });

            // 1. 地形高度變更
            Type modeType = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(modeType, "Height"));
            for (int pass = 0; pass < 3; pass++)
            {
                for (int x = 28; x <= 36; x++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, 40, "", ""));
                Invoke(form, "CommitStroke");
            }

            // 2. 地圖屬性（任務標題）變更
            var titleBox = GetField<TextBox>(form, "_title");
            titleBox.Text = "Updated Title";

            // 3. 有效事件新增
            var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            events.Add(new ScenarioEvent("SaveRetryTimer", 5)
            {
                Actions = [new(ScenarioActionKind.Message, "Injected")]
            });
            Invoke(form, "RefreshEventList", 0);
            Invoke(form, "UpdateEditorState");

            var session = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(session.HeightsDirty);
            Assert.True((bool)Invoke(form, "EventsDirty")!);

            // 第一次儲存：地形／屬性／場景已寫出，但在注入腳本時發現缺 ak_level.bci，觸發交易回滾
            bool saved = form.TrySaveMap(showSuccess: false, out Exception? error);
            Assert.False(saved);
            Assert.NotNull(error);
            Assert.IsType<FileNotFoundException>(error);
            Assert.Contains("ak_level.bci", error.Message);

            // 磁碟回滾驗證：全部檔案還原、新增的場景檔被刪除、被刪除的高度的快取被復原
            Assert.Equal(originalBoden, File.ReadAllBytes(Path.Combine(map, "boden.bmp")));
            Assert.Equal(originalEmboss, File.ReadAllBytes(Path.Combine(map, "emboss.bmp")));
            Assert.Equal(originalBriefing, File.ReadAllBytes(Path.Combine(map, "TEXT", "US", "briefing.put")));
            Assert.Equal(originalBodenIni, File.ReadAllBytes(Path.Combine(map, "boden.ini")));
            foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
            {
                Assert.True(File.Exists(Path.Combine(map, cache)), $"{cache} 應在回滾後被復原");
                Assert.Equal("stale", File.ReadAllText(Path.Combine(map, cache)));
            }
            Assert.False(File.Exists(Path.Combine(map, ScenarioDocument.FileName)), "arm_scenario.json 應在回滾後被刪除");

            // 表單記憶體保留編輯狀態（高度與事件皆為 dirty）
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(session.HeightsDirty);
            Assert.True((bool)Invoke(form, "EventsDirty")!);
            Assert.Single(events);
            Assert.Equal("SaveRetryTimer", events[0].Name);
            Assert.Equal("Updated Title", titleBox.Text);

            // 補齊有效 BCI
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            byte[] validBci = ScenarioEventsTests.Fixture().Serialize();
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), validBci);

            // 重試儲存：應順利成功
            bool retrySaved = form.TrySaveMap(showSuccess: false, out Exception? retryError);
            Assert.True(retrySaved);
            Assert.Null(retryError);

            // 儲存成功後表單髒狀態清除
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(session.HeightsDirty);
            Assert.False((bool)Invoke(form, "EventsDirty")!);

            // 磁碟狀態驗證：地形確實寫入、快取被刪除失效、場景與腳本注入成功
            byte[] savedBodenBytes = File.ReadAllBytes(Path.Combine(map, "boden.bmp"));
            Assert.NotEqual(originalBoden, savedBodenBytes);
            TerrainLayer savedBoden = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"))!;
            TerrainLayer origBodenLayer = DecodeOriginal(originalBoden);
            int center = (40 * 4 + 2) * 257 + 32 * 4 + 2;
            Assert.True(savedBoden.Green[center] > origBodenLayer.Green[center] + 10);
            Assert.Equal(origBodenLayer.Green[0], savedBoden.Green[0]);

            foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
            {
                Assert.False(File.Exists(Path.Combine(map, cache)), $"{cache} 應在儲存成功後被刪除失效");
            }

            Assert.True(File.Exists(Path.Combine(map, ScenarioDocument.FileName)));
            var loadedScenario = ScenarioDocument.Load(map);
            Assert.Equal("SaveRetryTimer", Assert.Single(loadedScenario.Events).Name);

            Assert.True(File.Exists(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
            byte[] injectedBci = File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile));
            Assert.NotEqual(validBci, injectedBci);

            string savedBriefing = File.ReadAllText(Path.Combine(map, "TEXT", "US", "briefing.put"));
            Assert.Contains("var:briefing_titel_1 =\"Updated Title\";", savedBriefing);
        });
    }

    [Fact]
    public void Corrupt_bci_failure_after_writes_rolls_back_all_files_and_preserves_memory_for_retry()
    {
        string map = CreateFixture("ENDL_007");
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        byte[] corruptBci = [0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6, 7, 8];
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), corruptBci);

        byte[] originalBoden = File.ReadAllBytes(Path.Combine(map, "boden.bmp"));
        byte[] originalEmboss = File.ReadAllBytes(Path.Combine(map, "emboss.bmp"));
        byte[] originalBriefing = File.ReadAllBytes(Path.Combine(map, "TEXT", "US", "briefing.put"));
        byte[] originalBodenIni = File.ReadAllBytes(Path.Combine(map, "boden.ini"));
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
        {
            Assert.True(File.Exists(Path.Combine(map, cache)));
        }

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_007", map, true, "BadBci", "無盡模式"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });

            // 1. 地形高度變更
            Type modeType = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(modeType, "Height"));
            for (int pass = 0; pass < 3; pass++)
            {
                for (int x = 28; x <= 36; x++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, 40, "", ""));
                Invoke(form, "CommitStroke");
            }

            // 2. 地圖屬性變更
            var titleBox = GetField<TextBox>(form, "_title");
            titleBox.Text = "Bad Bci Title";

            // 3. 有效事件新增
            var events = (List<ScenarioEvent>)typeof(MapEditorForm).GetField("_events", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            events.Add(new ScenarioEvent("CorruptBciRetryTimer", 3)
            {
                Actions = [new(ScenarioActionKind.Message, "Recovered")]
            });
            Invoke(form, "RefreshEventList", 0);
            Invoke(form, "UpdateEditorState");

            var session = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(session.HeightsDirty);
            Assert.True((bool)Invoke(form, "EventsDirty")!);

            // 第一次儲存：寫出地形／屬性／場景後，解析損壞 BCI 時拋出 InvalidDataException，觸發回滾
            bool saved = form.TrySaveMap(showSuccess: false, out Exception? error);
            Assert.False(saved);
            Assert.NotNull(error);
            Assert.IsType<InvalidDataException>(error);

            // 磁碟回滾驗證
            Assert.Equal(originalBoden, File.ReadAllBytes(Path.Combine(map, "boden.bmp")));
            Assert.Equal(originalEmboss, File.ReadAllBytes(Path.Combine(map, "emboss.bmp")));
            Assert.Equal(originalBriefing, File.ReadAllBytes(Path.Combine(map, "TEXT", "US", "briefing.put")));
            Assert.Equal(originalBodenIni, File.ReadAllBytes(Path.Combine(map, "boden.ini")));
            Assert.Equal(corruptBci, File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile)));
            Assert.False(File.Exists(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
            foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
            {
                Assert.True(File.Exists(Path.Combine(map, cache)));
                Assert.Equal("stale", File.ReadAllText(Path.Combine(map, cache)));
            }
            Assert.False(File.Exists(Path.Combine(map, ScenarioDocument.FileName)));

            // 表單記憶體保留編輯狀態
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(session.HeightsDirty);
            Assert.True((bool)Invoke(form, "EventsDirty")!);
            Assert.Single(events);
            Assert.Equal("CorruptBciRetryTimer", events[0].Name);
            Assert.Equal("Bad Bci Title", titleBox.Text);

            // 修復 BCI：寫入合法 BCI
            byte[] validBci = ScenarioEventsTests.Fixture().Serialize();
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), validBci);

            // 重試儲存
            bool retrySaved = form.TrySaveMap(showSuccess: false, out Exception? retryError);
            Assert.True(retrySaved);
            Assert.Null(retryError);

            // 表單髒狀態清除
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(session.HeightsDirty);
            Assert.False((bool)Invoke(form, "EventsDirty")!);

            // 磁碟驗證
            byte[] savedBodenBytes = File.ReadAllBytes(Path.Combine(map, "boden.bmp"));
            Assert.NotEqual(originalBoden, savedBodenBytes);
            foreach (string cache in TerrainLayerFiles.HeightDependentCaches)
            {
                Assert.False(File.Exists(Path.Combine(map, cache)));
            }
            Assert.True(File.Exists(Path.Combine(map, ScenarioDocument.FileName)));
            Assert.Equal("CorruptBciRetryTimer", Assert.Single(ScenarioDocument.Load(map).Events).Name);
            Assert.True(File.Exists(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
            Assert.Equal(validBci, File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
            Assert.Contains("var:briefing_titel_1 =\"Bad Bci Title\";", File.ReadAllText(Path.Combine(map, "TEXT", "US", "briefing.put")));
        });
    }

    private static void RunInSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try { action(); }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(60)), "STA 測試執行逾時（可能彈出模態對話框或死鎖）。");
        if (failure is not null)
        {
            ExceptionDispatchInfo.Capture(failure).Throw();
        }
    }

    private static Dictionary<string, byte[]> SnapshotDirectory(string directory)
    {
        var snapshot = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        if (!Directory.Exists(directory)) return snapshot;
        foreach (string file in Directory.EnumerateFiles(directory, "*", SearchOption.AllDirectories))
        {
            string relative = Path.GetRelativePath(directory, file);
            snapshot[relative] = File.ReadAllBytes(file);
        }
        return snapshot;
    }

    private static object? Invoke(MapEditorForm form, string name, params object[] args)
    {
        MethodInfo method = typeof(MapEditorForm).GetMethods(BindingFlags.Instance | BindingFlags.NonPublic)
            .Single(item => item.Name == name && item.GetParameters().Length == args.Length);
        try { return method.Invoke(form, args); }
        catch (TargetInvocationException ex) when (ex.InnerException is not null) { throw ex.InnerException; }
    }

    private static T GetField<T>(MapEditorForm form, string name)
        => (T)typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;

    private static T GetProperty<T>(MapEditorForm form, string name)
        => (T)typeof(MapEditorForm).GetProperty(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;

    private static TerrainLayer DecodeOriginal(byte[] bytes)
    {
        string path = Path.Combine(Path.GetTempPath(), "ArmBoden_" + Guid.NewGuid().ToString("N") + ".bmp");
        File.WriteAllBytes(path, bytes);
        try { return TerrainLayerFiles.Read(path)!; } finally { File.Delete(path); }
    }

    private string CreateFixture(string mapName = "ENDL_005")
    {
        string map = Path.Combine(_root, "MAPS", mapName);
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
        string floortexDat = Path.Combine(_root, "floortex.dat");
        if (!File.Exists(floortexDat))
        {
            using ZipArchive zip = ZipFile.Open(floortexDat, ZipArchiveMode.Create);
            foreach ((string name, byte shade) in new[] { ("4BB___51", (byte)110), ("4BB___52", (byte)120) })
            {
                using Stream stream = zip.CreateEntry($"SYSTEM/DATA/FLOORTEXTURE/{name}.bmp").Open();
                stream.Write(Encode(128, Enumerable.Repeat(shade, 128 * 128).ToArray()));
            }
        }
        return map;
    }

    private static byte[] Grid(int size, Func<int, int, double> value)
    {
        var result = new byte[size * size];
        for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) result[y * size + x] = (byte)Math.Clamp((int)Math.Round(value(x, y)), 0, 255);
        return result;
    }

    private static byte[] Encode(int size, byte[] values)
    {
        var blank = new TerrainLayer(size, size, new int[size * size], values.Select(value => (byte)(value ^ 1)).ToArray());
        return TerrainLayerFiles.EncodeWithGreen(blank, values);
    }

    private static void WriteGray(string path, int size, byte[] values) => File.WriteAllBytes(path, Encode(size, values));

    public void Dispose()
    {
        if (Directory.Exists(_root))
        {
            try { Directory.Delete(_root, true); }
            catch { /* best effort cleanup */ }
        }
    }
}
