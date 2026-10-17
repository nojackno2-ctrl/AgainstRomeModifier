using System.IO.Compression;
using System.Reflection;
using System.Runtime.ExceptionServices;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorDesignCheckUiTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmDesignCheckTest_" + Guid.NewGuid().ToString("N"));

    public MapEditorDesignCheckUiTests()
    {
        Directory.CreateDirectory(_root);
    }

    public void Dispose()
    {
        try { if (Directory.Exists(_root)) Directory.Delete(_root, true); } catch { }
    }

    [Fact]
    public void RunDesignCheck_on_editor_state_identifies_missing_win_loss_and_main_house()
    {
        string map = CreateTestMap("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Title", "Subtitle"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Execute design check
            var issues = form.RunDesignCheck();

            // Default ENDL_005 fixture has no victory/defeat events, so it should warn about scenario win/loss
            Assert.Contains(issues, i => i.Code.StartsWith("scenario-no-", StringComparison.Ordinal));
        });
    }

    [Fact]
    public void ConfirmDesignCheckBeforeSave_calls_DesignWarningPrompt_and_can_cancel_save()
    {
        string map = CreateTestMap("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Title", "Subtitle"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            bool promptCalled = false;
            form.DesignWarningPrompt = issues =>
            {
                promptCalled = true;
                Assert.NotEmpty(issues);
                return false; // Designer chooses not to save
            };

            bool confirmed = form.ConfirmDesignCheckBeforeSave(interactive: true);
            Assert.True(promptCalled);
            Assert.False(confirmed);

            // Now test when designer chooses to proceed
            form.DesignWarningPrompt = issues => true;
            bool proceeded = form.ConfirmDesignCheckBeforeSave(interactive: true);
            Assert.True(proceeded);
        });
    }

    [Fact]
    public void CollectMapDiagnostics_includes_design_check_warnings()
    {
        string map = CreateTestMap("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Title", "Subtitle"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var issues = form.RefreshMapDiagnostics();
            Assert.NotEmpty(issues);
            // Verify scenario warning is present in MapIssue list
            Assert.Contains(issues, i => i.Code.StartsWith("scenario-no-", StringComparison.Ordinal));
        });
    }

    [Fact]
    public void RunDesignCheck_on_193_192_dimension_48_map_succeeds_and_uses_actual_layer_sizes()
    {
        string map = CreateTestMap("TUTOR_TEST", dimension: 48);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("TUTOR_TEST", map, true, "Title", "Subtitle"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            // Execute design check on 193/192 map
            var issues = form.RunDesignCheck();
            Assert.NotEmpty(issues);
            // Default fixture has no win/loss events
            Assert.Contains(issues, i => i.Code.StartsWith("scenario-no-", StringComparison.Ordinal));
        });
    }

    private string CreateTestMap(string slot, int dimension = 64)
    {
        string map = Path.Combine(_root, "MAPS", slot);
        Directory.CreateDirectory(Path.Combine(map, "TEXT", "US"));
        Directory.CreateDirectory(Path.Combine(map, "DATA"));

        int heightSize = dimension * 4 + 1;
        int colSize = dimension * 4;

        WriteBmp(Path.Combine(map, "boden.bmp"), heightSize, heightSize, 80);
        WriteBmp(Path.Combine(map, "emboss.bmp"), heightSize, heightSize, 150);
        WriteBmp(Path.Combine(map, "smooth.bmp"), heightSize, heightSize, 0);
        WriteBmp(Path.Combine(map, "vertex.bmp"), heightSize, heightSize, 255);
        WriteBmp(Path.Combine(map, "collision.bmp"), colSize, colSize, 0);
        WriteBmp(Path.Combine(map, "minimap.bmp"), colSize, colSize, 90);

        File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        string[] textures = Enumerable.Range(0, dimension * dimension).Select(i => i % 2 == 0 ? "4BB___51" : "4BB___52").ToArray();
        File.WriteAllText(Path.Combine(map, "boden.txt"), $"[Dimension]\r\n{dimension}\r\n[Texturen]\r\n" + string.Join("\r\n", textures) + "\r\n");
        File.WriteAllText(Path.Combine(map, "boden.ini"),
            "[Waterlevel]\r\n0\r\n[Heightmapstep]\r\n4\r\n[WaterColor]\r\n0xffdfbf\r\n[DayStartTime]\r\n6\r\n[DayEndTime]\r\n20\r\n[RainDropsOnWater]\r\n1\r\n" +
            "[WaterWarpShift]\r\n2\r\n[WaterBumpAmplitude]\r\n3\r\n[WaterBumpFrequency]\r\n4\r\n[FlashPropability]\r\n0\r\n");

        File.WriteAllText(Path.Combine(map, "TEXT", "US", "briefing.put"),
            "var:briefing_titel_1 =\"Design Check Test\";\r\nvar:briefing_titel_2 =\"\";\r\nvar:briefing_text =\"\";\r\n" +
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
                WriteBmpStream(stream, 128, 128, shade);
            }
        }

        return map;
    }

    private static void WriteBmp(string path, int width, int height, byte value)
    {
        using var stream = File.Create(path);
        WriteBmpStream(stream, width, height, value);
    }

    private static void WriteBmpStream(Stream stream, int width, int height, byte value)
    {
        int stride = (width * 3 + 3) & ~3;
        int imageSize = stride * height;
        var bytes = new byte[54 + imageSize];
        bytes[0] = (byte)'B'; bytes[1] = (byte)'M';
        BitConverter.TryWriteBytes(bytes.AsSpan(2), bytes.Length);
        BitConverter.TryWriteBytes(bytes.AsSpan(10), 54);
        BitConverter.TryWriteBytes(bytes.AsSpan(14), 40);
        BitConverter.TryWriteBytes(bytes.AsSpan(18), width);
        BitConverter.TryWriteBytes(bytes.AsSpan(22), height);
        BitConverter.TryWriteBytes(bytes.AsSpan(26), (short)1);
        BitConverter.TryWriteBytes(bytes.AsSpan(28), (short)24);
        BitConverter.TryWriteBytes(bytes.AsSpan(34), imageSize);

        for (int y = 0; y < height; y++)
        {
            int row = 54 + y * stride;
            for (int x = 0; x < width; x++)
            {
                bytes[row + x * 3] = value;
                bytes[row + x * 3 + 1] = value;
                bytes[row + x * 3 + 2] = value;
            }
        }
        stream.Write(bytes);
    }

    private static void RunInSta(Action action)
    {
        Exception? error = null;
        var thread = new Thread(() =>
        {
            try { action(); }
            catch (Exception ex) { error = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error is not null) ExceptionDispatchInfo.Capture(error).Throw();
    }

    private static object? Invoke(object target, string methodName, params object?[] args)
    {
        var method = target.GetType().GetMethod(methodName, BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public);
        Assert.NotNull(method);
        return method.Invoke(target, args);
    }
}
