using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

public sealed class CharacterizationTests
{
    [Fact]
    public void Custom_unit_layers_do_not_duplicate_the_four_experimental_modifiers()
    {
        double[] fallback = { 100, 20, 10, 10, 4, 1000, 5, 2000, 500 };
        double[] custom = { 150, 30, 15, 15, 99, 1200, 9, 9999, 9999 };

        double[] merged = BackupManager.MergeUnitStatsLayers(
            fallback, custom, supportsSpellRadius: true);

        Assert.Equal(150, merged[0]);
        Assert.Equal(4, merged[4]);
        Assert.Equal(2000, merged[7]);
        Assert.Equal(500, merged[8]);
    }

    private static PatchProfile AllEnabled() => new()
    {
        FocusLoss = true,
        FastCiviProduction = true,
        InfiniteMorale = true,
        FreeProduction = true,
        FreeUpgrade = true,
        NoSpellCost = true,
        MaxPopulation = true,
        Balance = true,
        RangedRange3x = true,
        UnitMovementSpeed2x = true,
        SpellEntireMap = false,
        SpellRange3x = true,
        HousingCapacity20x = true,
        StorageCapacity10x = true,
        HqHp10x = true,
        FastBuildUpgradeRepair = true,
        FoodHealing10x = true,
        VillageBuildRange = true,
        NoSpellAltar = true,
        GameSpeed = 2,
        ToEnglish = true,
        DgVoodoo = true,
        EndlessAiModules = new(StringComparer.OrdinalIgnoreCase)
        {
            ["M1"] = true, ["M2"] = true, ["M3"] = true,
            ["M4"] = true, ["M5"] = true, ["M6"] = true,
        },
    };

    [Fact]
    public void Hybrid_fixture_starts_with_all_endless_ai_modules_in_original_state()
    {
        using var fixture = BackupZipGameFixture.Create();
        var orchestrator = new EndlessAiOrchestrator();

        foreach (EndlessAiModule module in orchestrator.UserModules)
        {
            Assert.Equal(PatchState.Original, orchestrator.DetectModule(fixture.RootPath, module));
        }
        Assert.Equal(PatchState.Original, orchestrator.DetectModule(fixture.RootPath, orchestrator.R0));
    }

    [Fact]
    public void T1_all_enabled_round_trip_is_detected()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, AllEnabled(), fixture.Backup, rollback);
            rollback.Commit();
        }

        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.FocusLoss);
        Assert.True(detected.FastCiviProduction);
        Assert.True(detected.InfiniteMorale);
        Assert.True(detected.FreeProduction);
        Assert.True(detected.FreeUpgrade);
        Assert.True(detected.NoSpellCost);
        Assert.True(detected.MaxPopulation);
        Assert.True(detected.Balance);
        Assert.True(detected.RangedRange3x);
        Assert.True(detected.UnitMovementSpeed2x);
        Assert.False(detected.SpellEntireMap);
        Assert.True(detected.SpellRange3x);
        Assert.True(detected.HousingCapacity20x);
        Assert.True(detected.StorageCapacity10x);
        Assert.True(detected.HqHp10x);
        Assert.True(detected.FastBuildUpgradeRepair);
        Assert.True(detected.FoodHealing10x);
        Assert.True(detected.VillageBuildRange);
        Assert.True(detected.NoSpellAltar);
        Assert.True(detected.ToEnglish);
        Assert.True(detected.DgVoodoo);
        Assert.Equal(2, detected.GameSpeed);
        foreach (string id in AllEnabled().EndlessAiModules.Keys)
        {
            Assert.True(detected.GetEndlessAiModule(id), id);
        }
    }

    [Fact]
    public void T2_restore_all_returns_original_fixture_bytes()
    {
        using var fixture = BackupZipGameFixture.Create();
        Dictionary<string, byte[]> original = Snapshot(fixture.RootPath);
        var engine = new PatchEngine(new NullLogger());
        Apply(engine, fixture);

        using (var rollback = new FileRollbackScope())
        {
            engine.RestoreOriginalFiles(fixture.RootPath, fixture.Backup, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in original)
        {
            Assert.True(File.Exists(Path.Combine(fixture.RootPath, path)), path);
            byte[] actual = File.ReadAllBytes(Path.Combine(fixture.RootPath, path));
            bool equal = bytes.SequenceEqual(actual);
            if (!equal && IsPfil(bytes) && IsPfil(actual))
            {
                equal = GameLZSS.DecompressPfil(bytes).SequenceEqual(GameLZSS.DecompressPfil(actual));
            }
            Assert.True(equal, path);
        }
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "D3D8.dll")));
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "DDraw.dll")));
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "dgVoodooCpl.exe")));
    }

    [Fact]
    public void T3_category_restore_keeps_the_other_category_enabled()
    {
        using var statsFixture = BackupZipGameFixture.Create();
        var statsEngine = new PatchEngine(new NullLogger());
        Apply(statsEngine, statsFixture);
        using (var rollback = new FileRollbackScope())
        {
            statsEngine.RestoreStatsOnly(statsFixture.RootPath, statsFixture.Backup, rollback);
            rollback.Commit();
        }
        PatchProfile afterStats = statsEngine.DetectCurrentPatchState(statsFixture.RootPath, statsFixture.Backup);
        Assert.False(afterStats.FastCiviProduction);
        Assert.False(afterStats.FreeProduction);
        Assert.False(afterStats.Balance);
        Assert.False(afterStats.RangedRange3x);
        Assert.False(afterStats.UnitMovementSpeed2x);
        Assert.False(afterStats.SpellEntireMap);
        Assert.False(afterStats.SpellRange3x);
        Assert.False(afterStats.MaxPopulation);
        Assert.False(afterStats.FoodHealing10x);
        Assert.True(afterStats.FocusLoss);
        Assert.True(afterStats.VillageBuildRange);
        Assert.Equal(2, afterStats.GameSpeed);
 
        using var compatFixture = BackupZipGameFixture.Create();
        var compatEngine = new PatchEngine(new NullLogger());
        Apply(compatEngine, compatFixture);
        using (var rollback = new FileRollbackScope())
        {
            compatEngine.RestoreCompatOnly(compatFixture.RootPath, compatFixture.Backup, rollback);
            rollback.Commit();
        }
        PatchProfile afterCompat = compatEngine.DetectCurrentPatchState(compatFixture.RootPath, compatFixture.Backup);
        Assert.False(afterCompat.FocusLoss);
        Assert.False(afterCompat.VillageBuildRange);
        Assert.Equal(1, afterCompat.GameSpeed);
        Assert.True(afterCompat.FastCiviProduction);
        Assert.True(afterCompat.FreeProduction);
        Assert.True(afterCompat.Balance);
        Assert.True(afterCompat.RangedRange3x);
        Assert.True(afterCompat.UnitMovementSpeed2x);
        Assert.False(afterCompat.SpellEntireMap);
        Assert.True(afterCompat.SpellRange3x);
        Assert.True(afterCompat.MaxPopulation);
        Assert.True(afterCompat.FoodHealing10x);
    }

    [Fact]
    public void T4_repeated_apply_is_byte_idempotent()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        Apply(engine, fixture);
        Dictionary<string, byte[]> first = Snapshot(fixture.RootPath);
        Apply(engine, fixture);
        Dictionary<string, byte[]> second = Snapshot(fixture.RootPath);

        Assert.Equal(first.Keys.OrderBy(x => x), second.Keys.OrderBy(x => x));
        foreach (string path in first.Keys) Assert.Equal(first[path], second[path]);
    }



    private static void Apply(PatchEngine engine, BackupZipGameFixture fixture)
    {
        using var rollback = new FileRollbackScope();
        engine.ApplyPatches(fixture.RootPath, AllEnabled(), fixture.Backup, rollback);
        rollback.Commit();
    }

    private static Dictionary<string, byte[]> Snapshot(string root) =>
        Directory.GetFiles(root, "*", SearchOption.AllDirectories)
            .ToDictionary(path => Path.GetRelativePath(root, path), File.ReadAllBytes, StringComparer.OrdinalIgnoreCase);

    private static bool IsPfil(byte[] bytes) =>
        bytes.Length >= 4 && bytes[0] == (byte)'P' && bytes[1] == (byte)'F' && bytes[2] == (byte)'I' && bytes[3] == (byte)'L';
}
