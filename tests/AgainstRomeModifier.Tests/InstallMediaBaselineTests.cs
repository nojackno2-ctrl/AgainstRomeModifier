using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

internal sealed class RequiresInstallMediaFactAttribute : FactAttribute
{
    public RequiresInstallMediaFactAttribute()
    {
        if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("ARM_INSTALL_MEDIA_BASELINE")))
            Skip = "Requires opt-in CRC-validated installation-media baseline; proprietary files are not in Git.";
    }
}

internal sealed class RequiresInstallMediaTheoryAttribute : TheoryAttribute
{
    public RequiresInstallMediaTheoryAttribute()
    {
        if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("ARM_INSTALL_MEDIA_BASELINE")))
            Skip = "Requires opt-in CRC-validated installation-media baseline; proprietary files are not in Git.";
    }
}

public sealed class InstallMediaBaselineTests
{
    [RequiresInstallMediaFact]
    public void Actual_media_patch_engine_applies_detects_and_restores_real_files()
    {
        using var fixture = MediaFixture.Create();
        var backup = new BackupManager(new NullLogger());
        string[] paths = Directory.GetFiles(fixture.Root, "*", SearchOption.AllDirectories);
        var original = paths.ToDictionary(path => path, File.ReadAllBytes);
        foreach (var (path, bytes) in original)
            backup.SetBackupFile(Path.GetRelativePath(fixture.Root, path).Replace('\\', '/'), bytes);
        var engine = new PatchEngine(new NullLogger());
        var profile = new PatchProfile
        {
            Balance = true, MaxPopulation = true, RangedRange3x = true,
            SpellDamage5x = true, SpellHealing10x = true, HousingCapacity20x = true,
            StorageCapacity10x = true, HqHp10x = true, FastBuildUpgradeRepair = true,
            FoodHealing10x = true, VillageBuildRange = true, NoSpellAltar = true,
            VillageGarrisonQuota3x = true, GameSpeed = 2,
            EndlessAiModules = new(StringComparer.OrdinalIgnoreCase)
            { ["M1"] = true, ["Core"] = true, ["M5"] = true, ["M6"] = true }
        };
        // Balance detection represents changed unit values, not a persisted checkbox.
        // Request an explicit HP difference even if the built-in preset matches this CD.
        const string customUnit = "FigRomInf00_Lanze_Schild";
        double[] customStats = backup.GetDefaultBalancedStats(customUnit);
        customStats[0] = backup.GetOriginalStats(customUnit)[0] + 1;
        profile.CustomUnitStats = new(StringComparer.OrdinalIgnoreCase) { [customUnit] = customStats };
        using (var rollback = new FileRollbackScope())
        { engine.ApplyPatches(fixture.Root, profile, backup, rollback); rollback.Commit(); }
        var detected = engine.DetectCurrentPatchState(fixture.Root, backup);
        foreach (var key in new[] { FeatureKeys.EndlessAiCore, FeatureKeys.EndlessAiM1,
            FeatureKeys.EndlessAiM5, FeatureKeys.EndlessAiM6 }) Assert.True(detected.Get(key), key.Id);
        Assert.True(detected.Balance);
        Assert.True(detected.MaxPopulation);
        Assert.True(detected.FoodHealing10x);
        Assert.True(detected.VillageGarrisonQuota3x);
        using (var rollback = new FileRollbackScope())
        { engine.RestoreOriginalFiles(fixture.Root, backup, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in original)
        {
            byte[] actual = File.ReadAllBytes(path);
            if (bytes.AsSpan().StartsWith("PFIL"u8))
                Assert.Equal(GameLZSS.DecompressPfil(bytes), GameLZSS.DecompressPfil(actual));
            else Assert.Equal(bytes, actual);
        }
        fixture.AssertSourceUnchanged();
    }

    [RequiresInstallMediaFact]
    public void Actual_media_exe_and_definition_baselines_pass_product_original_validators()
    {
        using var fixture = MediaFixture.Create();
        Assert.True(OriginalFileValidator.IsExeOriginal(File.ReadAllBytes(Path.Combine(fixture.Root, "Against_Rome.exe"))));
        Assert.True(OriginalFileValidator.IsObjdefOriginal(File.ReadAllBytes(Path.Combine(fixture.Root, "SYSTEM/DATA_MP/DEFAULTS/objdef.dau"))));
        Assert.True(OriginalFileValidator.IsPartgeoOriginal(File.ReadAllBytes(Path.Combine(fixture.Root, "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau"))));
    }

    [RequiresInstallMediaTheory]
    [InlineData("M1")]
    [InlineData("Core")]
    [InlineData("M5")]
    [InlineData("M6")]
    [InlineData("VillageGarrisonQuota3x")]
    public void Actual_media_modules_detect_apply_reload_and_restore_original_payload(string moduleId)
    {
        using var fixture = MediaFixture.Create();
        var orchestrator = new EndlessAiOrchestrator();
        var module = Assert.Single(orchestrator.UserModules, candidate => candidate.Id == moduleId);
        string[] paths = module.Patches.SelectMany(patch => EndlessAiOrchestrator.ResolvePaths(fixture.Root, patch.TargetPattern))
            .Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        Assert.NotEmpty(paths);
        var original = paths.ToDictionary(path => path, path => GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        Assert.Equal(PatchState.Original, orchestrator.DetectModule(fixture.Root, module));
        Assert.True(orchestrator.ApplyModule(fixture.Root, module, true));
        // Applying is buffered; the source and test files still contain their original payload.
        Assert.All(paths, path => Assert.Equal(original[path], GameLZSS.DecompressPfil(File.ReadAllBytes(path))));
        using (var rollback = new FileRollbackScope()) { orchestrator.SaveAll(fixture.Root, rollback); rollback.Commit(); }
        var loaded = new EndlessAiOrchestrator();
        var reloadedModule = Assert.Single(loaded.UserModules, candidate => candidate.Id == moduleId);
        Assert.Equal(PatchState.Ultimate, loaded.DetectModule(fixture.Root, reloadedModule));
        Assert.True(loaded.ApplyModule(fixture.Root, reloadedModule, false));
        using (var rollback = new FileRollbackScope()) { loaded.SaveAll(fixture.Root, rollback); rollback.Commit(); }
        Assert.Equal(PatchState.Original, new EndlessAiOrchestrator().DetectModule(fixture.Root, module));
        Assert.All(paths, path => Assert.Equal(original[path], GameLZSS.DecompressPfil(File.ReadAllBytes(path))));
        fixture.AssertSourceUnchanged();
    }

    private sealed class MediaFixture : IDisposable
    {
        public string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmMediaBaseline_" + Guid.NewGuid().ToString("N"));
        private readonly Dictionary<string, string> _sourceHashes = new(StringComparer.OrdinalIgnoreCase);

        public static MediaFixture Create()
        {
            string source = Path.GetFullPath(Environment.GetEnvironmentVariable("ARM_INSTALL_MEDIA_BASELINE")!);
            using var manifest = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(source, "report.json")));
            Assert.Equal(new[] { "b48bcd4675bb20ae782c35480c40ffc53e3bb663414e2aa7a70517851d285484",
                "d66b47df8d62bc0ea2ee5af3309eafb6a828871ec085e9ef2e831b8c4b2e99d2" },
                manifest.RootElement.GetProperty("iso_sha256").EnumerateArray().Select(value => value.GetString()).ToArray());
            var fixture = new MediaFixture();
            try
            {
                foreach (var file in manifest.RootElement.GetProperty("files").EnumerateArray())
                {
                    string relative = file.GetProperty("path").GetString()!;
                    Assert.StartsWith("%MAINDIR%\\", relative);
                    relative = relative[10..].Replace('\\', Path.DirectorySeparatorChar);
                    string path = Path.GetFullPath(Path.Combine(source, "files", relative));
                    string destination = Path.GetFullPath(Path.Combine(fixture.Root, relative));
                    Assert.StartsWith(source + Path.DirectorySeparatorChar, path);
                    Assert.StartsWith(fixture.Root + Path.DirectorySeparatorChar, destination);
                    byte[] bytes = File.ReadAllBytes(path);
                    string hash = Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant();
                    Assert.Equal(file.GetProperty("sha256").GetString(), hash);
                    fixture._sourceHashes.Add(path, hash);
                    Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
                    File.WriteAllBytes(destination, bytes);
                }
                Assert.True(fixture._sourceHashes.Count >= 100, "Media fixture must contain the real script/config baseline.");
                return fixture;
            }
            catch { fixture.Dispose(); throw; }
        }

        public void AssertSourceUnchanged()
        {
            foreach (var (path, hash) in _sourceHashes)
                Assert.Equal(hash, Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant());
        }

        public void Dispose()
        {
            AssertSourceUnchanged();
            if (Directory.Exists(Root)) Directory.Delete(Root, true);
        }
    }
}
