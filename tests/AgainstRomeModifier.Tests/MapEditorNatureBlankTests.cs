using System.Buffers.Binary;
using System.IO.Compression;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 驗證空白地形（Blank Terrain）在存在待存自然物件種植（Pending Planting）時的行為：
/// 1. 清除已存的可移除地景（Removable Landscape）與待存新增物件（Pending Additions）。
/// 2. 保留連結物件（Linked Objects）與非地景物件（建築）。
/// 3. Undo/Redo 能精確還原新增物件的順序與旋轉角度（Rotation）。
/// 4. 儲存失敗時（Failed Save）透過 UI 保持髒狀態（Dirty State）、歷史紀錄不變，且磁碟 bytes 經 rollback 回滾完全不變。
/// 僅使用隔離的合成臨時地圖 fixture，不存取遊戲安裝目錄。
/// </summary>
public sealed class MapEditorNatureBlankTests
{
    [Fact]
    public void Blank_terrain_clears_stored_removable_and_pending_additions_while_preserving_linked_and_building_objects()
    {
        Run((form, map) =>
        {
            // 設置已儲存物件：
            // Slot 0: 可移除地景（LanGerLau00, Linked=false）
            // Slot 1: 連結地景（LanGerLau00, Linked=true）-> 必須保留
            // Slot 2: 可移除地景（LanGerEic00, Linked=false）
            // Slot 3: 建築物（BauRomHau00, Linked=false）-> 非地景，必須保留
            const float position = 10.5f * SdlSceneCatalog.WorldUnitsPerMapPixel * 4;
            Set(form, "_levelObjects", new LevelWorldObject[]
            {
                new(0, 1, 8, 1, position, 0, position, 0.5f, false),
                new(1, 1, 8, 2, position + 50, 0, position + 50, 1.2f, true),
                new(2, 2, 8, 3, position + 100, 0, position + 100, 2.1f, false),
                new(3, 3, 8, 4, position + 150, 0, position + 150, 0.0f, false),
            });

            // 種植待存物件（Pending Planting）
            Paint(form, 10, 10);
            Paint(form, 14, 10);
            Invoke(form, "CommitStroke");

            var pendingAdditions = Additions(form).Cast<NatureAddition>().ToArray();
            Assert.NotEmpty(pendingAdditions);
            Assert.True(pendingAdditions.Length >= 2);

            var session = Get<NatureEditSession>(form, "_natureSession");
            Assert.True(session.IsDirty);
            Assert.Empty(session.RemovedSlots);

            // 執行空白地形（不跳確認對話框）
            form.ApplyBlankTerrain(confirm: false);

            // 驗證待存新增物件已被完全清除
            Assert.Empty(Additions(form));

            // 驗證可移除地景槽位已標記移除（0 與 2）
            Assert.Contains(0, session.RemovedSlots);
            Assert.Contains(2, session.RemovedSlots);

            // 驗證連結地景物件（Slot 1）與建築物件（Slot 3）未被標記移除
            Assert.DoesNotContain(1, session.RemovedSlots);
            Assert.DoesNotContain(3, session.RemovedSlots);

            Assert.True(session.IsDirty);
            Assert.True(Button(form, "_undoButton").Enabled);
        });
    }

    [Fact]
    public void Undo_after_blank_terrain_restores_exact_pending_additions_order_and_rotations()
    {
        Run((form, map) =>
        {
            const float position = 10.5f * SdlSceneCatalog.WorldUnitsPerMapPixel * 4;
            Set(form, "_levelObjects", new LevelWorldObject[]
            {
                new(0, 1, 8, 1, position, 0, position, 0.5f, false),
                new(1, 1, 8, 2, position, 0, position, 1.0f, true),
            });

            // 先種植多個待存物件
            Paint(form, 8, 8);
            Paint(form, 12, 8);
            Paint(form, 16, 8);
            Invoke(form, "CommitStroke");

            var originalAdditions = Additions(form).Cast<NatureAddition>().ToArray();
            Assert.NotEmpty(originalAdditions);
            float[] originalRotations = originalAdditions.Select(a => a.Rotation).ToArray();
            string[] originalNames = originalAdditions.Select(a => a.Name).ToArray();
            float[] originalXs = originalAdditions.Select(a => a.X).ToArray();
            float[] originalZs = originalAdditions.Select(a => a.Z).ToArray();

            // 套用空白地形
            form.ApplyBlankTerrain(confirm: false);
            var session = Get<NatureEditSession>(form, "_natureSession");
            Assert.Empty(session.Additions);
            Assert.Equal(new[] { 0 }, session.RemovedSlots);

            // 執行 Undo
            Invoke(form, "Undo");

            // 驗證已移除槽位復原為空
            Assert.Empty(session.RemovedSlots);

            // 驗證待存新增物件完整還原，且順序、旋轉與座標精確一致
            var restoredAdditions = Additions(form).Cast<NatureAddition>().ToArray();
            Assert.Equal(originalAdditions.Length, restoredAdditions.Length);

            for (int i = 0; i < originalAdditions.Length; i++)
            {
                Assert.Equal(originalNames[i], restoredAdditions[i].Name);
                Assert.Equal(originalRotations[i], restoredAdditions[i].Rotation);
                Assert.Equal(originalXs[i], restoredAdditions[i].X);
                Assert.Equal(originalZs[i], restoredAdditions[i].Z);
            }

            // 驗證 Redo 與再次 Undo
            Invoke(form, "Redo");
            Assert.Empty(session.Additions);
            Assert.Equal(new[] { 0 }, session.RemovedSlots);

            Invoke(form, "Undo");
            var secondRestored = Additions(form).Cast<NatureAddition>().ToArray();
            Assert.Equal(originalAdditions.Length, secondRestored.Length);
            for (int i = 0; i < originalAdditions.Length; i++)
            {
                Assert.Equal(originalRotations[i], secondRestored[i].Rotation);
            }
        });
    }

    [Fact]
    public void Failed_save_via_ui_keeps_dirty_state_history_and_leaves_bytes_unchanged()
    {
        Run((form, map) =>
        {
            // 在地圖上種植自然物件
            Paint(form, 10, 10);
            Invoke(form, "CommitStroke");

            var session = Get<NatureEditSession>(form, "_natureSession");
            Assert.True(session.IsDirty);
            Assert.True(session.CanUndo);
            int pendingAdditionsCount = session.Additions.Count;
            Assert.True(pendingAdditionsCount > 0);

            // 記錄儲存嘗試前所有檔案的 bytes
            string objectsPath = Path.Combine(map, "DATA", "objects.dat");
            string bodenPath = Path.Combine(map, "boden.bmp");
            string briefingPath = Path.Combine(map, "TEXT", "US", "briefing.put");
            string iniPath = Path.Combine(map, "boden.ini");

            byte[] originalObjectsBytes = File.ReadAllBytes(objectsPath);
            byte[] originalBodenBytes = File.ReadAllBytes(bodenPath);
            byte[] originalBriefingBytes = File.ReadAllBytes(briefingPath);
            byte[] originalIniBytes = File.ReadAllBytes(iniPath);

            // 鎖定 objects.dat 檔案使其無法寫入，引發 SaveMap 中的 IOException
            // 以宿主的交易接口取得錯誤，不顯示模態視窗。
            bool saveResult;
            using (var lockStream = new FileStream(objectsPath, FileMode.Open, FileAccess.Read, FileShare.None))
            {
                saveResult = form.TrySaveMap(false, out Exception? error);
                Assert.NotNull(error);
            }

            // 驗證儲存回傳失敗
            Assert.False(saveResult);

            // 驗證表單與 session 依然維持 Dirty 狀態與歷史紀錄
            Assert.True(session.IsDirty);
            Assert.True(session.CanUndo);
            Assert.Equal(pendingAdditionsCount, session.Additions.Count);

            // 驗證所有檔案在 rollback 機制作用下 bytes 完全不變
            Assert.Equal(originalObjectsBytes, File.ReadAllBytes(objectsPath));
            Assert.Equal(originalBodenBytes, File.ReadAllBytes(bodenPath));
            Assert.Equal(originalBriefingBytes, File.ReadAllBytes(briefingPath));
            Assert.Equal(originalIniBytes, File.ReadAllBytes(iniPath));

            // 驗證儲存失敗後 Undo 仍然可以正常運作
            Invoke(form, "Undo");
            Assert.Empty(session.Additions);
            Assert.False(session.IsDirty);
        });
    }

    [Fact]
    public void Failed_save_on_non_custom_map_rejects_without_modal_and_preserves_dirty_state()
    {
        Run((form, map) =>
        {
            // 先在自製地圖上種植自然物件
            Paint(form, 10, 10);
            Invoke(form, "CommitStroke");

            var session = Get<NatureEditSession>(form, "_natureSession");
            Assert.True(session.IsDirty);
            Assert.True(session.CanUndo);

            // 隨後模擬地圖狀態為非自製地圖（例如官方戰役地圖）
            var nonCustomInfo = new GameMapInfo("ENDL_000", map, false, "Official", "Campaign");
            Set(form, "_selected", nonCustomInfo);

            // 呼叫 SaveMap：非自製地圖立即回傳 false，不彈出任何 modal
            bool saveResult = form.TrySaveMap(false, out Exception? error);
            Assert.Null(error);
            Assert.False(saveResult);

            // 驗證髒狀態與歷史依然保留
            Assert.True(session.IsDirty);
            Assert.True(session.CanUndo);
            Assert.NotEmpty(session.Additions);

            // 驗證可繼續 Undo
            Invoke(form, "Undo");
            Assert.Empty(session.Additions);
        });
    }

    private static void Run(Action<MapEditorForm, string> action)
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmAgyNatureQA_" + Guid.NewGuid().ToString("N"));
        string map = Path.Combine(root, "MAPS", "ENDL_005");
        CreateSyntheticMapFixture(root, map);

        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try
            {
                var info = new GameMapInfo("ENDL_005", map, true, "NatureBlank", "Test");
                using var form = new MapEditorForm(root, info);
                _ = form.Handle;

                string bodenTxt = Path.Combine(map, "boden.txt");
                Set(form, "_texturesDocument", BodenTexturesDocument.Load(bodenTxt));
                Set(form, "_terrainLayers", new TerrainHeightEditSession(257, new byte[257 * 257], new byte[257 * 257], 256, new byte[256 * 256]));
                Set(form, "_bodenLayer", TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp")));

                var objdefNames = new Dictionary<int, string>
                {
                    [1] = "LanGerLau00",
                    [2] = "LanGerEic00",
                    [3] = "BauRomHau00"
                };
                Set(form, "_objdefNames", objdefNames);
                Set(form, "_natureCatalogTask", Task.FromResult<IReadOnlyDictionary<int, LevelObjectTemplate>>(new Dictionary<int, LevelObjectTemplate>()));

                var template = (LevelObjectTemplate)Activator.CreateInstance(typeof(LevelObjectTemplate),
                    BindingFlags.Instance | BindingFlags.NonPublic, null,
                    new object[] { 1, new byte[79], new uint[18], Array.Empty<byte[]>(), new byte[17], new byte[17] }, null)!;

                Type itemType = typeof(MapEditorForm).GetNestedType("NatureTypeItem", BindingFlags.NonPublic)!;
                Get<ListBox>(form, "_natureTypes").Items.Add(Activator.CreateInstance(itemType, template, "LanGerLau00")!);
                Get<ListBox>(form, "_natureTypes").SelectedIndex = 0;
                Get<MapCanvasControl>(form, "_canvas").BrushSize = 1;

                Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                Invoke(form, "SetEditMode", Enum.Parse(mode, "Nature"));

                action(form, map);
            }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();

        Assert.True(thread.Join(TimeSpan.FromSeconds(30)), "MapEditorNatureBlankTests timed out.");
        try
        {
            Assert.Null(failure);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                try { Directory.Delete(root, true); } catch { }
            }
        }
    }

    private static void CreateSyntheticMapFixture(string root, string map)
    {
        Directory.CreateDirectory(Path.Combine(map, "TEXT", "US"));
        Directory.CreateDirectory(Path.Combine(map, "DATA"));

        // boden.txt
        string[] textures = Enumerable.Repeat("4BB___51", 64 * 64).ToArray();
        File.WriteAllText(Path.Combine(map, "boden.txt"), "[Dimension]\r\n64\r\n[Texturen]\r\n" + string.Join("\r\n", textures) + "\r\n");

        // boden.ini
        File.WriteAllText(Path.Combine(map, "boden.ini"),
            "[Waterlevel]\r\n120\r\n[Heightmapstep]\r\n4\r\n[WaterColor]\r\n0xffdfbf\r\n[DayStartTime]\r\n6\r\n[DayEndTime]\r\n20\r\n[RainDropsOnWater]\r\n1\r\n" +
            "[WaterWarpShift]\r\n2\r\n[WaterBumpAmplitude]\r\n3\r\n[WaterBumpFrequency]\r\n4\r\n[FlashPropability]\r\n0\r\n");

        // briefing.put
        File.WriteAllText(Path.Combine(map, "TEXT", "US", "briefing.put"),
            "var:briefing_titel_1 =\"Nature Blank Test\";\r\nvar:briefing_titel_2 =\"\";\r\nvar:briefing_text =\"\";\r\n" +
            string.Concat(Enumerable.Range(0, 8).Select(index => $"var:briefing_text_teamname{index} =\"T{index}\";\r\n")));

        // SDL
        File.WriteAllText(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"),
            "[settlement]\r\nrefpos=10000,320,6000\r\n[object0000]\r\nnamedef=BauRomHau00_Haupthaus\r\ndef=1676\r\npos=0.00,0.00,0.00\r\nteam=3\r\nangle=0.00\r\n");

        // BMP layers
        WriteGray(Path.Combine(map, "boden.bmp"), 257, new byte[257 * 257]);
        WriteGray(Path.Combine(map, "emboss.bmp"), 257, new byte[257 * 257]);
        WriteGray(Path.Combine(map, "smooth.bmp"), 257, new byte[257 * 257]);
        WriteGray(Path.Combine(map, "vertex.bmp"), 257, Enumerable.Repeat((byte)255, 257 * 257).ToArray());
        WriteGray(Path.Combine(map, "collision.bmp"), 256, new byte[256 * 256]);
        WriteGray(Path.Combine(map, "minimap.bmp"), 256, Enumerable.Repeat((byte)90, 256 * 256).ToArray());

        // DATA files
        const int count = 8;
        byte[] objects = new byte[16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        BinaryPrimitives.WriteInt32LittleEndian(objects, 1);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), count);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);
        File.WriteAllBytes(Path.Combine(map, "DATA", "objects.dat"), objects);

        byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
        BinaryPrimitives.WriteInt32LittleEndian(data, 1);
        BinaryPrimitives.WriteInt32LittleEndian(data.AsSpan(4), count);
        File.WriteAllBytes(Path.Combine(map, "DATA", "objdata.dat"), data);

        byte[] positions = new byte[8 + 16 * LevelObjectStore.PositionSize];
        BinaryPrimitives.WriteInt32LittleEndian(positions, 1);
        BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), 16);
        File.WriteAllBytes(Path.Combine(map, "DATA", "position.dat"), positions);

        // floortex.dat in root
        using (ZipArchive zip = ZipFile.Open(Path.Combine(root, "floortex.dat"), ZipArchiveMode.Create))
        using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/4BB___51.bmp").Open())
            stream.Write(Encode(128, Enumerable.Repeat((byte)110, 128 * 128).ToArray()));
    }

    private static byte[] Encode(int size, byte[] values)
    {
        var blank = new TerrainLayer(size, size, new int[size * size], values.Select(value => (byte)(value ^ 1)).ToArray());
        return TerrainLayerFiles.EncodeWithGreen(blank, values);
    }

    private static void WriteGray(string path, int size, byte[] values) => File.WriteAllBytes(path, Encode(size, values));

    private static System.Collections.IList Additions(MapEditorForm form)
        => (System.Collections.IList)Get<NatureEditSession>(form, "_natureSession").Additions;

    private static ToolStripButton Button(MapEditorForm form, string name) => Get<ToolStripButton>(form, name);

    private static void Paint(MapEditorForm form, int x, int y)
        => Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, y, "", ""));

    private static T Get<T>(object target, string name)
        => (T)target.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(target)!;

    private static void Set(object target, string name, object? value)
        => target.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(target, value);

    private static object? Invoke(object target, string name, params object[] args)
    {
        MethodInfo method = target.GetType().GetMethods(BindingFlags.Instance | BindingFlags.NonPublic)
            .Single(item => item.Name == name && item.GetParameters().Length == args.Length);
        try { return method.Invoke(target, args); }
        catch (TargetInvocationException ex) when (ex.InnerException is not null) { throw ex.InnerException; }
    }
}
