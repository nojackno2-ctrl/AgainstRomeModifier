using System.Text.Json;
using AgainstRomeModifier.Cli;

namespace AgainstRomeModifier.Tests;

public sealed class CliTests
{
    [Fact]
    public void IsCliInvocation_identifies_cli_args_properly()
    {
        Assert.True(CliRunner.IsCliInvocation(["--cli"]));
        Assert.True(CliRunner.IsCliInvocation(["--cli", "--game", "C:\\Game"]));
        Assert.True(CliRunner.IsCliInvocation(["--help"]));
        Assert.True(CliRunner.IsCliInvocation(["-h"]));
        Assert.True(CliRunner.IsCliInvocation(["--json"]));
        Assert.True(CliRunner.IsCliInvocation(["status"]));
        Assert.True(CliRunner.IsCliInvocation(["apply", "--all"]));
        Assert.True(CliRunner.IsCliInvocation(["restore", "--all"]));
        Assert.True(CliRunner.IsCliInvocation(["features"]));
        Assert.True(CliRunner.IsCliInvocation(["detect-game"]));
        Assert.True(CliRunner.IsCliInvocation(["--game", "C:\\Game", "status"]));
        Assert.True(CliRunner.IsCliInvocation(["stats", "list"]));
        Assert.True(CliRunner.IsCliInvocation(["profile", "schema"]));

        // GUI launches:
        Assert.False(CliRunner.IsCliInvocation([]));
        Assert.False(CliRunner.IsCliInvocation(["--game", "C:\\Game"]));
        Assert.False(CliRunner.IsCliInvocation(["--game", "C:\\Game", "--map", "ENDL_005"]));
    }

    [Fact]
    public async Task Help_command_renders_text_and_json()
    {
        using var stdoutText = new StringWriter();
        using var stderrText = new StringWriter();
        int exitText = await CliRunner.RunAsync(["help"], stdoutText, stderrText);
        Assert.Equal(0, exitText);
        string textOutput = stdoutText.ToString();
        Assert.Contains("Against Rome Modifier CLI", textOutput);
        Assert.Contains("status", textOutput);
        Assert.Contains("apply", textOutput);
        Assert.Contains("restore", textOutput);

        using var stdoutJson = new StringWriter();
        using var stderrJson = new StringWriter();
        int exitJson = await CliRunner.RunAsync(["help", "--json"], stdoutJson, stderrJson);
        Assert.Equal(0, exitJson);
        string jsonOutput = stdoutJson.ToString();
        using var doc = JsonDocument.Parse(jsonOutput);
        Assert.True(doc.RootElement.GetProperty("success").GetBoolean());
        Assert.Equal("help", doc.RootElement.GetProperty("command").GetString());
        var data = doc.RootElement.GetProperty("data");
        Assert.True(data.GetProperty("commands").GetArrayLength() > 5);
    }

    [Fact]
    public async Task Features_command_lists_all_features_and_supports_filtering()
    {
        using var stdoutAll = new StringWriter();
        int exitAll = await CliRunner.RunAsync(["features", "--json"], stdoutAll);
        Assert.Equal(0, exitAll);
        using var docAll = JsonDocument.Parse(stdoutAll.ToString());
        Assert.True(docAll.RootElement.GetProperty("success").GetBoolean());
        var allFeatures = docAll.RootElement.GetProperty("data");
        Assert.True(allFeatures.GetArrayLength() > 20);

        // Filter by category
        using var stdoutStats = new StringWriter();
        int exitStats = await CliRunner.RunAsync(["features", "--category", "Stats", "--json"], stdoutStats);
        Assert.Equal(0, exitStats);
        using var docStats = JsonDocument.Parse(stdoutStats.ToString());
        var statsFeatures = docStats.RootElement.GetProperty("data");
        foreach (var item in statsFeatures.EnumerateArray())
        {
            Assert.Equal("Stats", item.GetProperty("category").GetString());
        }
    }

    [Fact]
    public async Task DetectGame_command_validates_path()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliDetectTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]); // Dummy PE header

            using var stdout = new StringWriter();
            int exit = await CliRunner.RunAsync(["detect-game", "--game", tempDir, "--json"], stdout);
            Assert.Equal(0, exit);

            using var doc = JsonDocument.Parse(stdout.ToString());
            Assert.True(doc.RootElement.GetProperty("success").GetBoolean());
            var data = doc.RootElement.GetProperty("data");
            Assert.True(data.GetProperty("isValid").GetBoolean());
            Assert.True(data.GetProperty("hasExe").GetBoolean());
            Assert.Equal(Path.GetFullPath(tempDir), data.GetProperty("detectedPath").GetString());
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Status_command_reports_error_when_game_dir_invalid()
    {
        string nonExistent = Path.Combine(Path.GetTempPath(), "NonExistent_" + Guid.NewGuid().ToString("N"));
        using var stdout = new StringWriter();
        using var stderr = new StringWriter();
        int exit = await CliRunner.RunAsync(["status", "--game", nonExistent, "--json"], stdout, stderr);
        Assert.NotEqual(0, exit);

        using var doc = JsonDocument.Parse(stdout.ToString());
        Assert.False(doc.RootElement.GetProperty("success").GetBoolean());
        Assert.NotEmpty(doc.RootElement.GetProperty("error").GetString()!);
    }

    [Fact]
    public async Task Apply_command_rejects_unknown_features_with_exit_code_2()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliApplyTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);

            using var stdout = new StringWriter();
            using var stderr = new StringWriter();
            int exit = await CliRunner.RunAsync(
                ["apply", "--game", tempDir, "--enable", "NonExistentFeature123", "--dry-run", "--json"],
                stdout, stderr);

            Assert.Equal(2, exit);
            using var doc = JsonDocument.Parse(stdout.ToString());
            Assert.False(doc.RootElement.GetProperty("success").GetBoolean());
            Assert.Contains("NonExistentFeature123", doc.RootElement.GetProperty("error").GetString()!);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Apply_command_rejects_invalid_speed_or_quota_multiplier()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliSpeedTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);

            using var stdout = new StringWriter();
            using var stderr = new StringWriter();
            int exit = await CliRunner.RunAsync(
                ["apply", "--game", tempDir, "--game-speed", "99", "--dry-run", "--json"],
                stdout, stderr);

            Assert.Equal(2, exit);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Profile_schema_returns_expected_metadata()
    {
        using var stdout = new StringWriter();
        int exit = await CliRunner.RunAsync(["profile", "schema", "--json"], stdout);
        Assert.Equal(0, exit);

        using var doc = JsonDocument.Parse(stdout.ToString());
        Assert.True(doc.RootElement.GetProperty("success").GetBoolean());
        var data = doc.RootElement.GetProperty("data");
        Assert.Equal("AgainstRome.PatchProfile", data.GetProperty("type").GetString());
        Assert.True(data.GetProperty("availableFeatures").GetArrayLength() > 20);
    }

    [Fact]
    public async Task Unknown_command_returns_exit_code_2_with_json_error()
    {
        using var stdout = new StringWriter();
        using var stderr = new StringWriter();
        int exit = await CliRunner.RunAsync(["totally-unknown-verb", "--json"], stdout, stderr);
        Assert.Equal(2, exit);

        using var doc = JsonDocument.Parse(stdout.ToString());
        Assert.False(doc.RootElement.GetProperty("success").GetBoolean());
        Assert.Contains("totally-unknown-verb", doc.RootElement.GetProperty("error").GetString()!);
    }

    [Fact]
    public async Task Missing_required_argument_returns_exit_code_2()
    {
        using var stdout = new StringWriter();
        using var stderr = new StringWriter();
        int exit = await CliRunner.RunAsync(["apply", "--game"], stdout, stderr);
        Assert.Equal(2, exit);
        Assert.Contains("--game 缺少路徑參數", stderr.ToString());
    }

    [Fact]
    public async Task Maps_command_lists_maps_from_game_directory()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliMapsTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);
            string mapDir = Path.Combine(tempDir, "MAPS", "ENDL_000");
            Directory.CreateDirectory(mapDir);
            File.WriteAllBytes(Path.Combine(mapDir, "boden.txt"), [1, 2, 3]);
            File.WriteAllBytes(Path.Combine(mapDir, "minimap.bmp"), [4, 5, 6]);

            using var stdout = new StringWriter();
            int exit = await CliRunner.RunAsync(["maps", "list", "--game", tempDir, "--json"], stdout);
            Assert.Equal(0, exit);

            using var doc = JsonDocument.Parse(stdout.ToString());
            Assert.True(doc.RootElement.GetProperty("success").GetBoolean());
            var maps = doc.RootElement.GetProperty("data");
            Assert.True(maps.GetArrayLength() >= 1);
            Assert.Equal("ENDL_000", maps[0].GetProperty("id").GetString());
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Saves_command_lists_saves_from_game_directory()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliSavesTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);
            string saveDir = Path.Combine(tempDir, "SAVE", "ESAVE_000");
            Directory.CreateDirectory(saveDir);
            File.WriteAllText(Path.Combine(saveDir, "save.ini"), "titel = Test Save\r\nlevel = ENDL_000\r\n");

            using var stdout = new StringWriter();
            int exit = await CliRunner.RunAsync(["saves", "list", "--game", tempDir, "--json"], stdout);
            Assert.Equal(0, exit);

            using var doc = JsonDocument.Parse(stdout.ToString());
            Assert.True(doc.RootElement.GetProperty("success").GetBoolean());
            var data = doc.RootElement.GetProperty("data");
            var saves = data.GetProperty("saves");
            Assert.True(saves.GetArrayLength() >= 1);
            Assert.Equal("ESAVE_000", saves[0].GetProperty("folder").GetString());
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Stats_command_supports_list_and_get()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliStatsTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);

            using var stdoutList = new StringWriter();
            int exitList = await CliRunner.RunAsync(["stats", "list", "--game", tempDir, "--json"], stdoutList);
            Assert.Equal(0, exitList);

            using var docList = JsonDocument.Parse(stdoutList.ToString());
            Assert.True(docList.RootElement.GetProperty("success").GetBoolean());
            var statsArray = docList.RootElement.GetProperty("data");
            Assert.True(statsArray.GetArrayLength() > 10);

            using var stdoutGet = new StringWriter();
            int exitGet = await CliRunner.RunAsync(["stats", "get", "FigGerInf01_Schwert", "--game", tempDir, "--json"], stdoutGet);
            Assert.Equal(0, exitGet);

            using var docGet = JsonDocument.Parse(stdoutGet.ToString());
            Assert.True(docGet.RootElement.GetProperty("success").GetBoolean());
            var unit = docGet.RootElement.GetProperty("data");
            Assert.Equal("FigGerInf01_Schwert", unit.GetProperty("key").GetString());

            // Test case-insensitive partial match
            using var stdoutPartial = new StringWriter();
            int exitPartial = await CliRunner.RunAsync(["stats", "get", "gerinf01", "--game", tempDir, "--json"], stdoutPartial);
            Assert.Equal(0, exitPartial);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public async Task Stats_export_writes_file_successfully()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ARM_CliExportTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            File.WriteAllBytes(Path.Combine(tempDir, "Against_Rome.exe"), [0x4D, 0x5A]);
            string outFile = Path.Combine(tempDir, "exported_stats.ini");

            using var stdout = new StringWriter();
            int exit = await CliRunner.RunAsync(
                ["stats", "export", "--game", tempDir, "--format", "ini", "--output", outFile, "--json"],
                stdout);
            Assert.Equal(0, exit);
            Assert.True(File.Exists(outFile));
            string content = File.ReadAllText(outFile);
            Assert.Contains("Against Rome Modifier", content);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }
}
