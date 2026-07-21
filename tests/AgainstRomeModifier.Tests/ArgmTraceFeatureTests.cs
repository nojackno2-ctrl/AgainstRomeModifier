using System.Text;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class ArgmTraceFeatureTests
{
    private const uint KnownAnalyzedTimeDateStamp = 0x404D1710;
    private const string Dll = "winmm.dll";
    private const string Marker = ".against-rome-modifier-argmtrace.json";

    private sealed class NullLogger : ILogger
    {
        public void Log(string message) { }
    }

    private sealed class TempDir : IDisposable
    {
        public string Path { get; } = System.IO.Path.Combine(
            System.IO.Path.GetTempPath(), "argmtrace_test_" + Guid.NewGuid().ToString("N"));
        public TempDir() => Directory.CreateDirectory(Path);
        public string File(string name) => System.IO.Path.Combine(Path, name);
        public void Dispose()
        {
            try { if (Directory.Exists(Path)) Directory.Delete(Path, true); }
            catch (IOException) { }
        }
    }

    // Minimal PE just large enough for ArgmTraceFeature to read
    // FileHeader.TimeDateStamp (DOS e_lfanew at 0x3C, "PE\0\0", stamp at PE+8).
    private static void WriteFakeExe(string path, uint timeDateStamp)
    {
        byte[] buf = new byte[0x100];
        buf[0] = (byte)'M';
        buf[1] = (byte)'Z';
        const uint peOffset = 0x80;
        BitConverter.GetBytes(peOffset).CopyTo(buf, 0x3C);
        buf[peOffset] = (byte)'P';
        buf[peOffset + 1] = (byte)'E';
        BitConverter.GetBytes(timeDateStamp).CopyTo(buf, (int)peOffset + 8);
        File.WriteAllBytes(path, buf);
    }

    private static ArgmTraceFeature NewFeature() => new(new NullLogger());

    [Fact]
    public void Apply_installs_winmm_ini_and_marker_and_is_detected()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        var feature = NewFeature();
        Assert.False(feature.IsInstalled(dir.Path));

        feature.Apply(dir.Path, enabled: true);

        Assert.True(File.Exists(dir.File(Dll)));
        Assert.True(File.Exists(dir.File("argm_trace.ini")));
        Assert.True(File.Exists(dir.File(Marker)));
        Assert.True(feature.IsInstalled(dir.Path));

        // The embedded winmm.dll is the real 32-bit build: a valid PE ("MZ").
        byte[] dll = File.ReadAllBytes(dir.File(Dll));
        Assert.True(dll.Length > 0 && dll[0] == (byte)'M' && dll[1] == (byte)'Z');
    }

    [Fact]
    public void Apply_deploys_in_log_only_mode_by_default()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        // Safe default: proxy loads and logs, but installs no hooks until the
        // user flips enableHooks=1 after confirming the game is stable.
        Assert.Contains("enableHooks=0", ini);
    }

    [Fact]
    public void Apply_on_known_build_records_unlock_stamp_in_ini()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        Assert.Contains("expectedTimeDateStamp=404D1710", ini);
    }

    [Fact]
    public void Apply_on_unknown_build_keeps_stamp_zero_in_ini()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), 0x11223344);

        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        Assert.Contains("expectedTimeDateStamp=0", ini);
        Assert.DoesNotContain("expectedTimeDateStamp=404D1710", ini);
    }

    [Fact]
    public void Apply_migrates_away_a_legacy_version_dll()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        // Reconstruct the superseded proxy's on-disk state: a version.dll plus a
        // marker recording it as our managed DLL (as the old ArgmTraceFeature did).
        // No winmm.dll yet.
        byte[] legacy = { 1, 2, 3, 4 };
        File.WriteAllBytes(dir.File("version.dll"), legacy);
        string legacyHash = Convert.ToHexString(
            System.Security.Cryptography.SHA256.HashData(legacy)).ToLowerInvariant();
        // The superseded build's marker had NO DllName field — exactly the
        // format this migration must still recognize as a legacy version.dll.
        File.WriteAllText(dir.File(Marker),
            "{\"DllSha256\":\"" + legacyHash +
            "\",\"IniSha256\":\"\",\"ExpectedTimeDateStamp\":0}");

        NewFeature().Apply(dir.Path, enabled: true);

        // Legacy version.dll must be gone; winmm.dll is the managed proxy now.
        Assert.False(File.Exists(dir.File("version.dll")));
        Assert.True(File.Exists(dir.File(Dll)));
    }

    [Fact]
    public void Remove_deletes_managed_files_but_keeps_capture_log()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        var feature = NewFeature();
        feature.Apply(dir.Path, enabled: true);

        File.WriteAllText(dir.File("argm_trace.log"), "captured events");

        feature.Apply(dir.Path, enabled: false);

        Assert.False(File.Exists(dir.File(Dll)));
        Assert.False(File.Exists(dir.File("argm_trace.ini")));
        Assert.False(File.Exists(dir.File(Marker)));
        Assert.False(feature.IsInstalled(dir.Path));
        Assert.True(File.Exists(dir.File("argm_trace.log")));
    }

    [Fact]
    public void Apply_aborts_when_unmanaged_winmm_present()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        File.WriteAllBytes(dir.File(Dll), new byte[] { 1, 2, 3, 4 });

        Assert.Throws<IOException>(() => NewFeature().Apply(dir.Path, enabled: true));
        Assert.Equal(new byte[] { 1, 2, 3, 4 }, File.ReadAllBytes(dir.File(Dll)));
    }

    [Fact]
    public void Reapply_is_idempotent()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        var feature = NewFeature();

        feature.Apply(dir.Path, enabled: true);
        byte[] first = File.ReadAllBytes(dir.File(Dll));
        feature.Apply(dir.Path, enabled: true);
        byte[] second = File.ReadAllBytes(dir.File(Dll));

        Assert.True(feature.IsInstalled(dir.Path));
        Assert.Equal(first, second);
    }
}
