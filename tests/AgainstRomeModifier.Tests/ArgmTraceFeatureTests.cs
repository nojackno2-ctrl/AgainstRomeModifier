using System.Text;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class ArgmTraceFeatureTests
{
    private const uint KnownAnalyzedTimeDateStamp = 0x404D1710;

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
    public void Apply_installs_dll_ini_and_marker_and_is_detected()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        var feature = NewFeature();
        Assert.False(feature.IsInstalled(dir.Path));

        feature.Apply(dir.Path, enabled: true);

        Assert.True(File.Exists(dir.File("version.dll")));
        Assert.True(File.Exists(dir.File("argm_trace.ini")));
        Assert.True(File.Exists(dir.File(".against-rome-modifier-argmtrace.json")));
        Assert.True(feature.IsInstalled(dir.Path));

        // The embedded version.dll is the real 32-bit build, so it must be a
        // valid PE that starts with "MZ".
        byte[] dll = File.ReadAllBytes(dir.File("version.dll"));
        Assert.True(dll.Length > 0 && dll[0] == (byte)'M' && dll[1] == (byte)'Z');
    }

    [Fact]
    public void Apply_on_known_build_unlocks_hooks_in_ini()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);

        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        Assert.Contains("expectedTimeDateStamp=404D1710", ini);
    }

    [Fact]
    public void Apply_on_unknown_build_keeps_hooks_locked_in_ini()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), 0x11223344);

        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        Assert.Contains("expectedTimeDateStamp=0", ini);
        Assert.DoesNotContain("expectedTimeDateStamp=404D1710", ini);
    }

    [Fact]
    public void Apply_without_game_exe_keeps_hooks_locked()
    {
        using var dir = new TempDir();
        // No Against_Rome.exe present -> TimeDateStamp unreadable -> locked.
        NewFeature().Apply(dir.Path, enabled: true);

        string ini = File.ReadAllText(dir.File("argm_trace.ini"), Encoding.UTF8);
        Assert.Contains("expectedTimeDateStamp=0", ini);
    }

    [Fact]
    public void Remove_deletes_managed_files_but_keeps_capture_log()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        var feature = NewFeature();
        feature.Apply(dir.Path, enabled: true);

        // Simulate a capture the user would want to keep.
        File.WriteAllText(dir.File("argm_trace.log"), "captured events");

        feature.Apply(dir.Path, enabled: false);

        Assert.False(File.Exists(dir.File("version.dll")));
        Assert.False(File.Exists(dir.File("argm_trace.ini")));
        Assert.False(File.Exists(dir.File(".against-rome-modifier-argmtrace.json")));
        Assert.False(feature.IsInstalled(dir.Path));
        Assert.True(File.Exists(dir.File("argm_trace.log")));
    }

    [Fact]
    public void Apply_aborts_when_unmanaged_version_dll_present()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        // A version.dll from some other program, with no modifier marker.
        File.WriteAllBytes(dir.File("version.dll"), new byte[] { 1, 2, 3, 4 });

        Assert.Throws<IOException>(() => NewFeature().Apply(dir.Path, enabled: true));

        // The foreign file must be left untouched.
        Assert.Equal(new byte[] { 1, 2, 3, 4 }, File.ReadAllBytes(dir.File("version.dll")));
    }

    [Fact]
    public void Remove_preserves_user_modified_managed_dll()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        var feature = NewFeature();
        feature.Apply(dir.Path, enabled: true);

        // User tampered with the managed DLL; hash no longer matches the manifest.
        File.WriteAllBytes(dir.File("version.dll"), new byte[] { 9, 9, 9 });

        feature.Apply(dir.Path, enabled: false);

        // The modified file is kept, not deleted.
        Assert.True(File.Exists(dir.File("version.dll")));
    }

    [Fact]
    public void Reapply_is_idempotent()
    {
        using var dir = new TempDir();
        WriteFakeExe(dir.File("Against_Rome.exe"), KnownAnalyzedTimeDateStamp);
        var feature = NewFeature();

        feature.Apply(dir.Path, enabled: true);
        byte[] firstDll = File.ReadAllBytes(dir.File("version.dll"));
        feature.Apply(dir.Path, enabled: true);
        byte[] secondDll = File.ReadAllBytes(dir.File("version.dll"));

        Assert.True(feature.IsInstalled(dir.Path));
        Assert.Equal(firstDll, secondDll);
    }
}
